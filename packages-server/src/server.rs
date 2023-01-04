use crate::config::Config;
use crate::packages::{PackagesDb, PackageEntry};
use crate::auth::AuthDb;
use crate::files::Files;
use crate::error::{Result, Error};

use stream_api::{request_handler, raw_request_handler};
use packages::requests::{
	PackageInfoReq, PackageInfo,
	SetPackageInfoReq,
	GetFileReq, GetFile,
	GetFilePartReq, GetFilePart,
	SetFileReq,
	AuthenticateReaderReq, AuthKey,
	AuthenticateWriter1Req, AuthenticateWriter1, Challenge,
	AuthenticateWriter2Req,
	NewAuthKeyReaderReq,
	ChangeWhitelistReq
};
use packages::packages::Channel;
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

	if !cfg.has_sign_key() {
		eprintln!("please define the signature public key `sign-key`");
		return Ok(())
	}

	let pack_db = match PackagesDb::read().await {
		Ok(p) => p,
		Err(e) => {
			eprintln!("reading packages db failed");
			eprintln!(
				"to create the packages db file use the command `create`"
			);
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
	server.register_request(set_package_info);
	server.register_request(get_file);
	server.register_request(get_file_part);
	server.register_request(set_file);
	server.register_request(authenticate_reader);
	server.register_request(authenticate_writer1);
	server.register_request(authenticate_writer2);
	server.register_request(new_auth_key_reader);
	server.register_request(change_whitelist);

	server.run().await
		.map_err(|e| Error::other("server failed", e))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthReader(Channel);

#[allow(dead_code)]
fn valid_reader_auth(sess: &Session) -> ApiResult<Channel> {
	match sess.get::<AuthReader>() {
		Some(c) => Ok(c.0),
		None => Err(ApiError::NotAuthenticated)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthWriterChallenge(Channel, Challenge);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthWriter(Channel);

fn valid_writer_auth(sess: &Session) -> ApiResult<Channel> {
	match sess.get::<AuthWriter>() {
		Some(a) => Ok(a.0),
		None => Err(ApiError::NotAuthenticated)
	}
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

// todo some party could upload an old version
request_handler!(
	async fn set_package_info<Action>(
		req: SetPackageInfoReq,
		files: Files,
		session: Session,
		cfg: Config,
		packages: PackagesDb
	) -> ApiResult<()> {
		let channel = valid_writer_auth(session)?;

		// check that we have a file with that version
		let hash = &req.package.version;
		if !files.exists(hash).await {
			return Err(ApiError::Request(format!("version does not exists")))
		}

		// validate that the signature is correct
		let sign_pub_key = cfg.sign_pub_key_by_channel(channel)
			.ok_or_else(|| ApiError::NoSignKeyForChannel(channel))?;

		if !sign_pub_key.verify(hash, &req.package.signature) {
			return Err(ApiError::SignatureIncorrect)
		}

		// now set it
		packages.push_package(channel, PackageEntry {
			package: req.package,
			whitelist: req.whitelist
		}).await;

		Ok(())
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

raw_request_handler!(
	async fn get_file_part<Action, EncryptedBytes>(
		req: GetFilePartReq,
		files: Files
	) -> ApiResult<GetFilePart> {
		let file = files.get(&req.hash).await
			.ok_or(ApiError::FileNotFound)?;

		GetFilePart::from_file(file, req.start, req.len).await
	}
);

// todo some party could upload an old version
raw_request_handler!(
	async fn set_file<Action, EncryptedBytes>(
		req: SetFileReq,
		files: Files,
		session: Session,
		cfg: Config
	) -> ApiResult<()> {
		let channel = valid_writer_auth(session)?;

		// generate hash of file
		let hash = req.hash();
		// validate signature
		let sign_pub_key = cfg.sign_pub_key_by_channel(channel)
			.ok_or_else(|| ApiError::NoSignKeyForChannel(channel))?;
		if !sign_pub_key.verify(&hash, req.signature()) {
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

// Administration stuff
// to authenticate as a reader your have to call NewAuthKeyReaderReq
request_handler!(
	async fn authenticate_reader<Action>(
		req: AuthenticateReaderReq,
		session: Session,
		auth_db: AuthDb
	) -> ApiResult<()> {
		let channel = match auth_db.get(&req.key).await {
			Some(c) => c,
			None => return Err(ApiError::AuthKeyUnknown)
		};

		session.set(AuthReader(channel));

		Ok(())

		// if let Some(sign) = req.sign {
		// 	let chall = session.get::<Challenge>();
		// 	let valid = match chall {
		// 		Some(chal) => {
		// 			let sign_key = cfg.sign_key.as_ref().unwrap();
		// 			sign_key.verify(chal.0.as_ref(), &sign)
		// 		},
		// 		None => false
		// 	};

		// 	if !valid {
		// 		return Err(ApiError::Request("Signature incorrect".into()))
		// 	}

		// 	let key = AuthKey::new();
		// 	session.set(key.clone());
		// 	auth_db.insert(key.clone()).await;
		// 	return Ok(NewAuthKey {
		// 		kind: NewAuthKeyKind::NewKey,
		// 		key
		// 	})
		// }

		// // create a challenge
		// let key = AuthKey::new();
		// let chall = Challenge(key.clone());
		// session.set(chall);

		// Ok(NewAuthKey {
		// 	kind: NewAuthKeyKind::Challenge,
		// 	key
		// })
	}
);

request_handler!(
	async fn authenticate_writer1<Action>(
		req: AuthenticateWriter1Req,
		session: Session
	) -> ApiResult<AuthenticateWriter1> {
		let challenge = Challenge::new();
		session.set(AuthWriterChallenge(req.channel, challenge.clone()));

		Ok(AuthenticateWriter1 { challenge })
	}
);

request_handler!(
	async fn authenticate_writer2<Action>(
		req: AuthenticateWriter2Req,
		session: Session,
		cfg: Config
	) -> ApiResult<()> {
		let AuthWriterChallenge(channel, challenge) =
			session.take::<AuthWriterChallenge>().ok_or_else(|| {
				ApiError::NotAuthenticated
			})?;

		let sign_pub_key = cfg.sign_pub_key_by_channel(channel)
			.ok_or_else(|| ApiError::NoSignKeyForChannel(channel))?;

		if !sign_pub_key.verify(challenge, &req.signature) {
			return Err(ApiError::SignatureIncorrect)
		}

		session.set(AuthWriter(channel));

		// need to increase the body limit (since we are a writer)
		let conf = session.get::<Configurator<ServerConfig>>().unwrap();
		let mut cfg = conf.read();
		// the limit should be 200mb
		cfg.body_limit = 200_000_000;
		conf.update(cfg);

		Ok(())
	}
);


request_handler!(
	async fn new_auth_key_reader<Action>(
		_req: NewAuthKeyReaderReq,
		session: Session,
		auth_db: AuthDb
	) -> ApiResult<AuthKey> {
		let channel = valid_writer_auth(session)?;

		// you are a valid writer
		// so let's create a new AuthKey to read data
		let key = AuthKey::new();
		auth_db.insert(key.clone(), channel).await;
		session.set(AuthReader(channel));

		Ok(key)
	}
);

request_handler!(
	async fn change_whitelist<Action>(
		req: ChangeWhitelistReq,
		session: Session,
		packages: PackagesDb
	) -> ApiResult<()> {
		let channel = valid_writer_auth(session)?;

		let changed = packages.change_whitelist(
			&channel,
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