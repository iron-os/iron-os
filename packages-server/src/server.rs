
use crate::config::Config;
use crate::packages::PackagesDb;
use crate::auth::AuthDb;
use crate::files::Files;
use crate::error::{Result, Error};

use packages::{request_handler};
use packages::requests::{
	PackageInfoReq, PackageInfo, GetFileReq, GetFile,
	SetFileReq, SetFile, SetPackageInfoReq, SetPackageInfo,
	NewAuthKeyReq, NewAuthKey, NewAuthKeyKind, AuthenticationReq, Authentication
};
use packages::stream::packet::PacketError;
use packages::error::{Result as ApiResult, Error as ApiError};
use packages::server::{Server, Session, Configurator, Config as ServerConfig};
use packages::auth::AuthKey;

pub async fn serve() -> Result<()> {

	let cfg = match Config::read().await {
		Ok(cfg) => cfg,
		Err(e) => {
			eprintln!("reading configuration failed");
			eprintln!("to create a configuration use the command `create`");
			return Err(e)
		}
	};

	if cfg.sign_key.is_none() {
		eprintln!("please define the signature public key `sign-key`");
		return Ok(())
	}

	let pack_db = match PackagesDb::read().await {
		Ok(p) => p,
		Err(e) => {
			eprintln!("reading packages db failed");
			eprintln!("to create the packages db file use the command `create`");
			return Err(e)
		}
	};

	let files = Files::read(&cfg).await?;

	let auth_db = match AuthDb::read().await {
		Ok(a) => a,
		Err(e) => {
			eprintln!("reading auth db failed");
			eprintln!("to create the auth db file use the command `create`");
			return Err(e)
		}
	};

	// now spawn the server
	
	let mut server = Server::new(
		("0.0.0.0", cfg.port),
		cfg.con_key.clone()
	).await.map_err(|e| Error::other("server failed", e))?;

	println!("start server on 0.0.0.0:{:?}", cfg.port);

	server.register_data(pack_db);
	server.register_data(files);
	server.register_data(cfg);
	server.register_data(auth_db);
	// server.register_request(all_packages);
	server.register_request(package_info);
	server.register_request(get_file);
	server.register_request(set_file);
	server.register_request(set_package_info);
	server.register_request(new_auth_key);
	server.register_request(auth_req);

	server.run().await
		.map_err(|e| Error::other("server failed", e))
}

request_handler!(
	async fn package_info(
		req: PackageInfoReq,
		packages: PackagesDb
	) -> ApiResult<PackageInfo> {
		Ok(PackageInfo {
			package: packages.get_package(
				&req.arch,
				&req.channel,
				&req.name
			).await
		})
	}
);

request_handler!(
	async fn get_file(req: GetFileReq, files: Files) -> ApiResult<GetFile> {
		let file = files.get(&req.hash).await;
		match file {
			Some(file) => GetFile::new(req, file).await,
			None => Ok(GetFile::empty())
		}
	}
);

#[derive(Debug, Clone, Hash)]
struct Challenge(AuthKey);

// Administration stuff
// to create a new user we need to get a random value signed by the signature
// key
request_handler!(
	async fn new_auth_key(
		req: NewAuthKeyReq,
		session: Session,
		cfg: Config,
		auth_db: AuthDb
	) -> ApiResult<NewAuthKey> {
		if let Some(sign) = req.sign {
			let chall = session.get::<Challenge>();
			let valid = match chall {
				Some(chal) => {
					let sign_key = cfg.sign_key.as_ref().unwrap();
					sign_key.verify(chal.0.as_ref(), &sign)
				},
				None => false
			};
			if !valid {
				return Err(ApiError::Stream(
					PacketError::Body("Signature incorrect".into()).into()
				))
			}

			let key = AuthKey::new();
			session.set(key.clone());
			auth_db.insert(key.clone()).await;
			return Ok(NewAuthKey {
				kind: NewAuthKeyKind::NewKey,
				key
			})
		}

		// create a challenge
		let key = AuthKey::new();
		let chall = Challenge(key.clone());
		session.set(chall);

		Ok(NewAuthKey {
			kind: NewAuthKeyKind::Challenge,
			key
		})
	}
);


request_handler!(
	async fn auth_req(
		req: AuthenticationReq,
		session: Session,
		auth_db: AuthDb
	) -> ApiResult<Authentication> {
		let valid = auth_db.contains(&req.key).await;
		if valid {
			session.set(req.key.clone());
			// need to increase the body limit
			let conf = session.get::<Configurator<ServerConfig>>().unwrap();
			let mut cfg = conf.read();
			// the limit should be 200mb
			cfg.body_limit = 200_000_000;
			conf.update(cfg);
		}

		Ok(Authentication { valid })
	}
);

async fn valid_auth(sess: &Session, auth_db: &AuthDb) -> ApiResult<()> {
	let valid = match sess.get::<AuthKey>() {
		Some(k) => auth_db.contains(&k).await,
		None => false
	};

	if valid {
		Ok(())
	} else {
		Err(ApiError::Stream(
			PacketError::Body("Not logged in".into()).into()
		))
	}
}

// todo some party could upload an old version
request_handler!(
	async fn set_file(
		req: SetFileReq,
		files: Files,
		session: Session,
		auth_db: AuthDb,
		cfg: Config
	) -> ApiResult<SetFile> {
		valid_auth(session, auth_db).await?;

		// generate hash of file
		let hash = req.hash();
		// validate signature
		let sign_key = cfg.sign_key.as_ref().unwrap();
		let signature = req.signature();
		if !sign_key.verify(hash.as_slice(), signature) {
			return Err(ApiError::Stream(
				PacketError::Body("Signature incorrect".into()).into()
			))
		}

		// now write to disk
		let file = req.file();

		files.set(&hash, file).await
			.map_err(ApiError::io)?;

		Ok(SetFile)
	}
);

// todo some party could upload an old version
request_handler!(
	async fn set_package_info(
		req: SetPackageInfoReq,
		files: Files,
		session: Session,
		auth_db: AuthDb,
		cfg: Config,
		packages: PackagesDb
	) -> ApiResult<SetPackageInfo> {
		valid_auth(session, auth_db).await?;

		// check that we have a file with that version
		let hash = &req.package.version;
		if !files.exists(hash).await {
			return Err(ApiError::Stream(
				PacketError::Body("version does not exists".into()).into()
			))
		}

		// validate that the signature is correct
		let sign_key = cfg.sign_key.as_ref().unwrap();
		if !sign_key.verify(hash.as_slice(), &req.package.signature) {
			return Err(ApiError::Stream(
				PacketError::Body("Signature incorrect".into()).into()
			))
		}

		// now set it
		packages.set_package(req.channel, req.package).await;

		Ok(SetPackageInfo)
	}
);

