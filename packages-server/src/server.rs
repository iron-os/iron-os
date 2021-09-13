
use crate::config::Config;
use crate::packages::PackagesDb;
use crate::files::Files;
use crate::error::{Result, Error};

use packages::{request_handler};
use packages::requests::{
	PackageInfoReq, PackageInfo, ImageInfoReq, ImageInfo, GetFileReq, GetFile,
	SetFileReq, SetFile, SetPackageInfoReq, SetPackageInfo
};
use packages::stream::packet::PacketError;
use packages::error::{Result as ApiResult, Error as ApiError};
use packages::server::Server;

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

	// now spawn the server
	
	let mut server = Server::new(("0.0.0.0", cfg.port), cfg.con_key.clone()).await
		.map_err(|e| Error::other("server failed", e))?;

	println!("start server on 0.0.0.0:{:?}", cfg.port);

	server.register_data(pack_db);
	server.register_data(files);
	server.register_data(cfg);
	// server.register_request(all_packages);
	server.register_request(package_info);
	server.register_request(image_info);
	server.register_request(get_file);
	server.register_request(set_file);
	server.register_request(set_package_info);

	server.run().await
		.map_err(|e| Error::other("server failed", e))
}

// request_handler!(
// 	 async fn all_packages(req: AllPackagesReq, packages: PackagesDb) -> ApiResult<AllPackages> {
// 	 	todo!("all packages")
// 	 }
// );

request_handler!(
	async fn package_info(req: PackageInfoReq, packages: PackagesDb) -> ApiResult<PackageInfo> {
		Ok(PackageInfo {
			package: packages.get_package(&req.channel, &req.name).await
		})
	}
);

request_handler!(
	async fn image_info(req: ImageInfoReq, packages: PackagesDb) -> ApiResult<ImageInfo> {
		Ok(ImageInfo {
			image: packages.get_image(&req.channel).await
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

// todo some party could upload an old version
request_handler!(
	async fn set_file(req: SetFileReq, files: Files, cfg: Config) -> ApiResult<SetFile> {
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
		cfg: Config,
		packages: PackagesDb
	) -> ApiResult<SetPackageInfo> {
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

