
use crate::config::Config;
use crate::packages::PackagesDb;
use crate::error::{Result, Error};

use packages::{request_handler};
use packages::requests::{AllPackagesReq, AllPackages};
use packages::error::Result as ApiResult;
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


	// now spawn the server
	
	let mut server = Server::new(("0.0.0.0", cfg.port), cfg.con_key.clone()).await
		.map_err(|e| Error::other("server failed", e))?;

	server.register_data(pack_db);
	server.register_request(all_packages);

	println!("start server on 0.0.0.0:{:?}", cfg.port);

	server.run().await
		.map_err(|e| Error::other("server failed", e))
}

request_handler!(
	 async fn all_packages(req: AllPackagesReq, packages: PackagesDb) -> ApiResult<AllPackages> {
	 	todo!()
	 }
);