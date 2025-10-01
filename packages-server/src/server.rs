use crate::auth::AuthDb;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::files::Files;
use crate::packages::{PackageEntry, PackagesDb};

use sentry::ClientInitGuard;

use packages::error::{Error as ApiError, Result as ApiResult};
use packages::packages::Channel;
use packages::requests::{
	AuthKey, AuthenticateReaderReq, AuthenticateWriter1,
	AuthenticateWriter1Req, AuthenticateWriter2Req, Challenge,
	ChangeWhitelistReq, EmptyJson, GetFile, GetFilePart, GetFilePartReq,
	GetFileReq, NewAuthKeyReader, NewAuthKeyReaderReq, PackageInfo,
	PackageInfoReq, SetFileReq, SetPackageInfoReq,
};
use packages::server::{Config as ServerConfig, Configurator, Server, Session};
use stream_api::api;

use tracing::error;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

pub async fn serve(tracing: &str, path: &str) -> Result<()> {
	let cfg = match Config::read(path).await {
		Ok(cfg) => cfg,
		Err(e) => {
			eprintln!(
				"reading configuration failed\nto create a configuration \
				use the command `create`"
			);
			return Err(e);
		}
	};

	if !cfg.has_sign_key() {
		eprintln!("please define the signature public key `sign-key`");
		return Ok(());
	}

	let _sentry_guard = enable_tracing(tracing, cfg.sentry_url.as_deref());

	let pack_db =
		match PackagesDb::read(&cfg).await {
			Ok(p) => p,
			Err(e) => {
				error!("reading packages db failed\nto create the packages db file \
				use the command `create`");
				return Err(e);
			}
		};

	let files = Files::read(&cfg).await?;

	let auth_db =
		match AuthDb::read(&cfg).await {
			Ok(a) => a,
			Err(e) => {
				error!("reading auth db failed\nto create the auth db file use the \
				command `create`");
				return Err(e);
			}
		};

	// now spawn the server

	let mut server = Server::new(("0.0.0.0", cfg.port), cfg.con_key.clone())
		.await
		.map_err(|e| Error::other("server failed", e))?;

	println!("start server on 0.0.0.0:{:?}", cfg.port);

	server.register_data(pack_db);
	server.register_data(files);
	server.register_data(cfg);
	server.register_data(auth_db);
	// server.register_request(all_packages);

	register_requests(&mut server);

	server
		.run()
		.await
		.map_err(|e| Error::other("server failed", e))
}

fn enable_tracing(
	tracing: &str,
	sentry_url: Option<&str>,
) -> Option<ClientInitGuard> {
	// setup tracing
	let tracing_reg = tracing_subscriber::registry()
		.with(EnvFilter::from(tracing))
		.with(fmt::layer());

	let enable_sentry = !cfg!(debug_assertions) && sentry_url.is_some();

	let (guard, sentry_layer) = if enable_sentry {
		let guard = sentry::init((
			sentry_url.clone().unwrap(),
			sentry::ClientOptions {
				release: sentry::release_name!(),
				..Default::default()
			},
		));

		(Some(guard), Some(sentry_tracing::layer()))
	} else {
		(None, None)
	};

	tracing_reg.with(sentry_layer).init();

	if enable_sentry {
		tracing::info!("using sentry");
	}

	guard
}

pub fn register_requests<L>(server: &mut Server<L>) {
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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthReader(Channel);

#[allow(dead_code)]
fn valid_reader_auth(sess: &Session) -> ApiResult<Channel> {
	match sess.get::<AuthReader>() {
		Some(c) => Ok(c.0),
		None => Err(ApiError::NotAuthenticated),
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthWriterChallenge(Channel, Challenge);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AuthWriter(Channel);

fn valid_writer_auth(sess: &Session) -> ApiResult<Channel> {
	match sess.get::<AuthWriter>() {
		Some(a) => Ok(a.0),
		None => Err(ApiError::NotAuthenticated),
	}
}

#[api(PackageInfoReq)]
async fn package_info(
	req: PackageInfoReq,
	packages: &PackagesDb,
) -> ApiResult<PackageInfo> {
	Ok(PackageInfo {
		package: packages.get_package(&req).await.map(|e| e.package),
	})
}

// todo some party could upload an old version
#[api(SetPackageInfoReq)]
async fn set_package_info(
	req: SetPackageInfoReq,
	files: &Files,
	session: &Session,
	cfg: &Config,
	packages: &PackagesDb,
) -> ApiResult<EmptyJson> {
	let channel = valid_writer_auth(session)?;

	// check that we have a file with that version
	let hash = &req.package.version;
	if !files.exists(hash).await {
		return Err(ApiError::Request(format!("version does not exists")));
	}

	// validate that the signature is correct
	let sign_pub_key = cfg
		.sign_pub_key_by_channel(channel)
		.ok_or_else(|| ApiError::NoSignKeyForChannel(channel))?;

	if !sign_pub_key.verify(hash, &req.package.signature) {
		return Err(ApiError::SignatureIncorrect);
	}

	// now set it
	packages
		.push_package(
			channel,
			PackageEntry {
				package: req.package,
				requirements: req.requirements,
				whitelist: req.whitelist,
				auto_whitelist_limit: req.auto_whitelist_limit,
			},
		)
		.await;

	Ok(EmptyJson)
}

#[api(GetFileReq<B>)]
async fn get_file(req: GetFileReq<B>, files: &Files) -> ApiResult<GetFile<B>> {
	let file = files.get(&req.hash).await;
	match file {
		Some(file) => GetFile::from_file(file).await,
		None => Ok(GetFile::empty()),
	}
}

#[api(GetFilePartReq<B>)]
async fn get_file_part(
	req: GetFilePartReq<B>,
	files: &Files,
) -> ApiResult<GetFilePart<B>> {
	let file = files.get(&req.hash).await.ok_or(ApiError::FileNotFound)?;

	GetFilePart::from_file(file, req.start, req.len).await
}

// todo some party could upload an old version
#[api(SetFileReq<B>)]
async fn set_file(
	req: SetFileReq<B>,
	files: &Files,
	session: &Session,
	cfg: &Config,
) -> ApiResult<EmptyJson> {
	let channel = valid_writer_auth(session)?;

	// generate hash of file
	let hash = req.hash();
	// validate signature
	let sign_pub_key = cfg
		.sign_pub_key_by_channel(channel)
		.ok_or_else(|| ApiError::NoSignKeyForChannel(channel))?;
	if !sign_pub_key.verify(&hash, req.signature()) {
		return Err(ApiError::SignatureIncorrect);
	}

	// now write to disk
	let file = req.file();

	files.set(&hash, file).await.map_err(|e| {
		ApiError::Internal(format!("could not write file {}", e))
	})?;

	Ok(EmptyJson)
}

// Administration stuff
// to authenticate as a reader your have to call NewAuthKeyReaderReq
#[api(AuthenticateReaderReq)]
async fn authenticate_reader(
	req: AuthenticateReaderReq,
	session: &Session,
	auth_db: &AuthDb,
) -> ApiResult<EmptyJson> {
	let channel = match auth_db.get(&req.key).await {
		Some(c) => c,
		None => return Err(ApiError::AuthKeyUnknown),
	};

	session.set(AuthReader(channel));

	Ok(EmptyJson)
}

#[api(AuthenticateWriter1Req)]
async fn authenticate_writer1(
	req: AuthenticateWriter1Req,
	session: &Session,
) -> ApiResult<AuthenticateWriter1> {
	let challenge = Challenge::new();
	session.set(AuthWriterChallenge(req.channel, challenge.clone()));

	Ok(AuthenticateWriter1 { challenge })
}

#[api(AuthenticateWriter2Req)]
async fn authenticate_writer2(
	req: AuthenticateWriter2Req,
	session: &Session,
	cfg: &Config,
) -> ApiResult<EmptyJson> {
	let AuthWriterChallenge(channel, challenge) = session
		.take::<AuthWriterChallenge>()
		.ok_or_else(|| ApiError::NotAuthenticated)?;

	let sign_pub_key = cfg
		.sign_pub_key_by_channel(channel)
		.ok_or_else(|| ApiError::NoSignKeyForChannel(channel))?;

	if !sign_pub_key.verify(challenge, &req.signature) {
		return Err(ApiError::SignatureIncorrect);
	}

	session.set(AuthWriter(channel));

	// need to increase the body limit (since we are a writer)
	if let Some(conf) = session.get::<Configurator<ServerConfig>>() {
		// this should always be available instead while testing
		tracing::warn!("no conf");

		let mut cfg = conf.read();
		// the limit should be 200mb
		cfg.body_limit = 200_000_000;
		conf.update(cfg);
	}

	Ok(EmptyJson)
}

#[api(NewAuthKeyReaderReq)]
async fn new_auth_key_reader(
	session: &Session,
	auth_db: &AuthDb,
) -> ApiResult<NewAuthKeyReader> {
	let channel = valid_writer_auth(session)?;

	// you are a valid writer
	// so let's create a new AuthKey to read data
	let key = AuthKey::new();
	auth_db.insert(key.clone(), channel).await;
	session.set(AuthReader(channel));

	Ok(NewAuthKeyReader(key))
}

#[api(ChangeWhitelistReq)]
async fn change_whitelist(
	req: ChangeWhitelistReq,
	session: &Session,
	packages: &PackagesDb,
) -> ApiResult<EmptyJson> {
	let channel = valid_writer_auth(session)?;

	let changed = packages
		.change_whitelist(
			&channel,
			&req.arch,
			&req.name,
			&req.version,
			&req.change,
		)
		.await;

	if changed {
		Ok(EmptyJson)
	} else {
		Err(ApiError::VersionNotFound)
	}
}
