
mod device_info;

use crate::Bootloader;
use crate::ui::{ApiSender};
use crate::util::io_other;
use crate::packages::Packages;
use crate::display::{Display, State};

use std::io;

use api::server::Server;
use api::requests::{
	ui::OpenPageReq,
	system::{
		SystemInfoReq, SystemInfo, ShortPackage,
		InstallOnReq
	},
	device::{
		SetDisplayStateReq, SetDisplayBrightnessReq,
		DisksReq, Disk as ApiDisk, Disks as ApiDisks
	},
	packages::{
		ListPackagesReq, ListPackages, AddPackage, AddPackageReq
	},
	device::{DeviceInfoReq, DeviceInfo}
};
use api::{request_handler, Action};
use api::error::{Result as ApiResult, Error as ApiError};

use tokio::fs;
use tokio::task::JoinHandle;

pub async fn start(
	client: Bootloader,
	ui_api: ApiSender,
	display: Display,
	packages: Packages
) -> io::Result<JoinHandle<()>> {

	// since there is only one instance of service running this is fine
	let _ = fs::remove_file("/data/service-api").await;

	let mut server = Server::new("/data/service-api").await
		.map_err(io_other)?;
	server.register_data(ui_api);
	server.register_data(client);
	server.register_data(display);
	server.register_data(packages);
	server.register_request(open_page);
	server.register_request(system_info);
	server.register_request(set_display_state);
	server.register_request(set_display_brightness);
	server.register_request(disks);
	server.register_request(install_on);
	server.register_request(list_packages);
	server.register_request(add_package);
	server.register_request(device_info_req);

	Ok(tokio::spawn(async move {
		server.run().await
			.expect("could not run api server")
	}))
}

request_handler!(
	async fn open_page<Action>(
		req: OpenPageReq,
		ui_api: ApiSender
	) -> ApiResult<()> {
		eprintln!("opening page {}", req.url);
		ui_api.open_page(req.url);
		Ok(())
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
	async fn system_info<Action>(
		_req: SystemInfoReq,
		bootloader: Bootloader,
		packages: Packages
	) -> ApiResult<SystemInfo> {

		let version_info = bootloader.version_info().await
			.map_err(ApiError::internal)?;

		let cfg = packages.config().await;

		let packages_list: Vec<_> = packages.packages().await
			.into_iter()
			.map(|pack| {
				ShortPackage {
					name: pack.name,
					version: pack.version_str,
					path: pack.path
				}
			})
			.collect();

		Ok(SystemInfo {
			version: version_info.version_str,
			board: version_info.board,
			product: version_info.product,
			installed: version_info.installed,
			channel: cfg.channel,
			packages: packages_list
		})
	}
);

/*
#[derive(Debug, Serialize, Deserialize)]
pub struct SetDisplayStateReq {
	pub on: bool
}
*/
request_handler!(
	async fn set_display_state<Action>(
		req: SetDisplayStateReq,
		display: Display
	) -> ApiResult<()> {
		let SetDisplayStateReq { on } = req;
		display.set_state(match on {
			true => State::On,
			false => State::Off
		}).await
			.ok_or_else(|| ApiError::internal("could not set display state"))
	}
);

request_handler!(
	async fn set_display_brightness<Action>(
		req: SetDisplayBrightnessReq,
		display: Display
	) -> ApiResult<()> {
		display.set_brightness(req.brightness).await
			.ok_or_else(|| ApiError::internal("could not set display state"))
	}
);

request_handler!(
	async fn disks<Action>(
		_req: DisksReq,
		bootloader: Bootloader
	) -> ApiResult<ApiDisks> {
		let disks_list = bootloader.disks().await
			.map_err(ApiError::internal)?;

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
	async fn device_info_req<Action>(
		_req: DeviceInfoReq,
		bootloader: Bootloader
	) -> ApiResult<DeviceInfo> {
		device_info::read(bootloader).await
			.map_err(ApiError::internal_display)
	}
);

request_handler!(
	async fn install_on<Action>(
		req: InstallOnReq,
		bootloader: Bootloader
	) -> ApiResult<()> {
		bootloader.install_on(req.disk).await.map_err(ApiError::internal)
	}
);

// setPowerState

// listPackages
// addpackage
// removePackage

request_handler!(
	async fn list_packages<Action>(
		_req: ListPackagesReq,
		packages: Packages
	) -> ApiResult<ListPackages> {

		let cfg = packages.config().await;
		let list = packages.packages().await;

		Ok(ListPackages {
			packages: list,
			sources: cfg.sources,
			channel: cfg.channel,
			on_run: cfg.on_run
		})
	}
);

request_handler!(
	async fn add_package<Action>(
		req: AddPackageReq,
		packages: Packages
	) -> ApiResult<AddPackage> {
		let mut packages = packages.clone();
		let pack = packages.add_package(req.name).await;

		Ok(AddPackage { package: pack })
	}
);

/*
Get JournalLogs

journalctl -n 400 --output-fields=_SYSTEMD_UNIT,MESSAGE,CODE_FILE,CODE_LINE,CODE_FUNC,_EXE -r -o json > test.txt
*/