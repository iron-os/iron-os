
use crate::Bootloader;
use crate::ui::{ApiSender};
use crate::util::io_other;
use crate::packages::Packages;
use crate::display::{Display, State};

use std::io;

use packages::packages::Channel;

use api::server::Server;
use api::requests::{
	open_page::{OpenPageReq, OpenPage},
	system_info::{SystemInfoReq, SystemInfo, Package, Channel as ApiChannel},
	set_display_state::{SetDisplayStateReq, SetDisplayState},
	disks::{DisksReq, Disk as ApiDisk, Disks as ApiDisks},
	install_on::{InstallOnReq, InstallOn as ApiInstallOn}
};
use api::request_handler;
use api::error::{Result as ApiResult, Error as ApiError};

use tokio::fs;
use tokio::task::JoinHandle;

use bootloader_api::{VersionInfoReq, Disks, InstallOn};

pub async fn start(
	client: Bootloader,
	ui_api: ApiSender,
	display: Display
) -> io::Result<JoinHandle<()>> {

	// since there is only one instance of service running this is fine
	let _ = fs::remove_file("/data/service-api").await;

	let mut server = Server::new("/data/service-api").await
		.map_err(io_other)?;
	server.register_data(ui_api);
	server.register_data(client);
	server.register_data(display);
	server.register_request(open_page);
	server.register_request(system_info);
	server.register_request(set_display_state);
	server.register_request(disks);
	server.register_request(install_on);

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

fn convert_channel(channel: Channel) -> ApiChannel {
	match channel {
		Channel::Debug => ApiChannel::Debug,
		Channel::Alpha => ApiChannel::Alpha,
		Channel::Beta => ApiChannel::Beta,
		Channel::Release => ApiChannel::Release
	}
}

request_handler!(
	async fn system_info(
		_req: SystemInfoReq,
		bootloader: Bootloader
	) -> ApiResult<SystemInfo> {

		let version_info = bootloader.request(&VersionInfoReq).await
			.map_err(ApiError::io)?;

		let packages = Packages::load().await
			.map_err(ApiError::io_other)?;

		let packages_list: Vec<_> = packages.list.into_iter()
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
			channel: convert_channel(packages.cfg.channel),
			packages: packages_list
		})
	}
);

/*
#[derive(Debug, Serialize, Deserialize)]
pub struct SetDisplayStateReq {
	// 0-1
	pub brightness: f32,
	pub on: bool
}
*/
request_handler!(
	async fn set_display_state(
		req: SetDisplayStateReq,
		display: Display
	) -> ApiResult<SetDisplayState> {
		let SetDisplayStateReq { brightness: _, on } = req;
		display.set_state(match on {
			true => State::On,
			false => State::Off
		})
			.map(|_| SetDisplayState)
			.ok_or_else(|| {
				ApiError::io_other("could not set display state")
			})
	}
);

request_handler!(
	async fn disks(
		_req: DisksReq,
		bootloader: Bootloader
	) -> ApiResult<ApiDisks> {
		let disks_list = bootloader.request(&Disks).await
			.map_err(ApiError::io)?;

		Ok(ApiDisks(
			disks_list.into_iter()
				.filter(|disk| !disk.active)
				.map(|disk| ApiDisk {
					name: disk.name,
					initialized: disk.initialized,
					size: disk.size
				})
				.collect()
		))
	}
);

request_handler!(
	async fn install_on(
		req: InstallOnReq,
		bootloader: Bootloader
	) -> ApiResult<ApiInstallOn> {
		bootloader.request(&InstallOn {
			name: req.name
		}).await
			.map(|_| ApiInstallOn)
			.map_err(ApiError::io)
	}
);