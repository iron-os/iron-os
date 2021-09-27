
use crate::Bootloader;
use crate::ui::{ApiSender};
use crate::util::io_other;
use crate::packages::Packages;

use std::io;

use api::server::Server;
use api::requests::{OpenPageReq, OpenPage, SystemInfoReq, SystemInfo, Package};
use api::request_handler;
use api::error::{Result as ApiResult, Error as ApiError};

use tokio::fs;
use tokio::task::JoinHandle;

use bootloader_api::VersionInfoReq;

pub async fn start(
	client: Bootloader,
	ui_api: ApiSender
) -> io::Result<JoinHandle<()>> {

	// since there is only one instance of service running this is fine
	let _ = fs::remove_file("/data/service-api").await;

	let mut server = Server::new("/data/service-api").await
		.map_err(io_other)?;
	server.register_data(ui_api);
	server.register_data(client);
	server.register_request(open_page);

	Ok(tokio::spawn(async move {
		server.run().await
			.expect("could not run api server")
	}))
}

request_handler!(
	async fn open_page(
		req: OpenPageReq,
		ui_api: ApiSender
	) -> ApiResult<OpenPage> {
		ui_api.open_page(req.url);
		Ok(OpenPage)
	}
);

/*
pub struct SystemInfo {
    pub version: String,
    pub packages: Vec<Package>,
    pub installed: bool,
}
*/

request_handler!(
	async fn system_info(
		_req: SystemInfoReq,
		bootloader: Bootloader
	) -> ApiResult<SystemInfo> {

		let version_info = bootloader.request(&VersionInfoReq).await
			.map_err(ApiError::io)?;

		let packages = Packages::load().await
			.map_err(ApiError::io_other)?;

		let packages: Vec<_> = packages.list.into_iter()
			.map(|pack| {
				let p = pack.package();
				Package {
					name: p.name.clone(),
					version: p.version_str.clone(),
					path: format!("/data/packages/{}/{}", p.name, pack.current())
				}
			})
			.collect();

		Ok(SystemInfo {
			version: version_info.version_str,
			installed: version_info.installed,
			packages
		})
	}
);