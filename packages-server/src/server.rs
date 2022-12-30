use crate::config::Config;
use crate::packages::{PackagesDb, PackageEntry};
use crate::auth::AuthDb;
use crate::files::Files;
use crate::error::{Result, Error};

use stream_api::{request_handler, raw_request_handler};
use packages::requests::{
	PackageInfoReq, PackageInfo, GetFileReq, GetFile,
	SetFileReq, SetPackageInfoReq,
	NewAuthKeyReq, NewAuthKey, NewAuthKeyKind,
	AuthenticationReq, AuthKey,
	ChangeWhitelistReq
};
use packages::action::Action;
use packages::error::{Result as ApiResult, Error as ApiError};
use packages::server::{
	Server, Session, Configurator, Config as ServerConfig, EncryptedBytes
	};

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
	server.register_request(change_whitelist);

	server.run().await
		.map_err(|e| Error::other("server failed", e))
}

request_handler!(
	async fn package_info<Action>(
		req: PackageInfoReq,
		packages: PackagesDb
	) -> ApiResult<PackageInfo> {
		Ok(PackageInfo {
			package: packages.get_package(
				&req.arch,
				&req.channel,
				&req.name,
				&req.device_id
			).await.map(|e| e.package)
		})
	}
);

raw_request_handler!(
	async fn get_file<Action, EncryptedBytes>(
		req: GetFileReq,
		files: Files
	) -> ApiResult<GetFile> {
		let file = files.get(&req.hash).await;
		match file {
			Some(file) => GetFile::from_file(file).await,
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
	async fn new_auth_key<Action>(
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
				return Err(ApiError::Request("Signature incorrect".into()))
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
	async fn auth_req<Action>(
		req: AuthenticationReq,
		session: Session,
		auth_db: AuthDb
	) -> ApiResult<()> {
		let valid = auth_db.contains(&req.key).await;
		if !valid {
			return Err(ApiError::AuthKeyUnknown)
		}

		session.set(req.key.clone());
		// need to increase the body limit
		let conf = session.get::<Configurator<ServerConfig>>().unwrap();
		let mut cfg = conf.read();
		// the limit should be 200mb
		cfg.body_limit = 200_000_000;
		conf.update(cfg);

		Ok(())
	}
);

async fn valid_auth(sess: &Session, auth_db: &AuthDb) -> ApiResult<()> {
	match sess.get::<AuthKey>() {
		Some(k) => if auth_db.contains(&k).await {
			Ok(())
		} else {
			Err(ApiError::NotAuthenticated)
		},
		None => Err(ApiError::NotAuthenticated)
	}
}

// todo some party could upload an old version
raw_request_handler!(
	async fn set_file<Action, EncryptedBytes>(
		req: SetFileReq,
		files: Files,
		session: Session,
		auth_db: AuthDb,
		cfg: Config
	) -> ApiResult<()> {
		valid_auth(session, auth_db).await?;

		// generate hash of file
		let hash = req.hash();
		// validate signature
		let sign_key = cfg.sign_key.as_ref().unwrap();
		let signature = req.signature();
		if !sign_key.verify(&hash, signature) {
			return Err(ApiError::SignatureIncorrect)
		}

		// now write to disk
		let file = req.file();

		files.set(&hash, file).await
			.map_err(|e| ApiError::Internal(
				format!("could not write file {}", e)
			))?;

		Ok(())
	}
);

// todo some party could upload an old version
request_handler!(
	async fn set_package_info<Action>(
		req: SetPackageInfoReq,
		files: Files,
		session: Session,
		auth_db: AuthDb,
		cfg: Config,
		packages: PackagesDb
	) -> ApiResult<()> {
		valid_auth(session, auth_db).await?;

		// check that we have a file with that version
		let hash = &req.package.version;
		if !files.exists(hash).await {
			return Err(ApiError::Request(format!("version does not exists")))
		}

		// validate that the signature is correct
		let sign_key = cfg.sign_key.as_ref().unwrap();
		if !sign_key.verify(hash, &req.package.signature) {
			return Err(ApiError::SignatureIncorrect)
		}

		// now set it
		packages.push_package(req.channel, PackageEntry {
			package: req.package,
			whitelist: req.whitelist
		}).await;

		Ok(())
	}
);

request_handler!(
	async fn change_whitelist<Action>(
		req: ChangeWhitelistReq,
		session: Session,
		auth_db: AuthDb,
		packages: PackagesDb
	) -> ApiResult<()> {
		valid_auth(session, auth_db).await?;

		let changed = packages.change_whitelist(
			&req.channel,
			&req.arch,
			&req.name,
			&req.version,
			req.whitelist
		).await;

		if changed {
			Ok(())
		} else {
			Err(ApiError::VersionNotFound)
		}
	}
);