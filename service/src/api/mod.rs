
use crate::Bootloader;
use crate::ui::{ApiSender};
use crate::util::io_other;
use crate::packages::Packages;
use crate::display::{Display, State};

use std::io;

use api::server::Server;
use api::requests::{
	ui::{OpenPageReq, OpenPage},
	system::{
		SystemInfoReq, SystemInfo, ShortPackage,
		InstallOnReq, InstallOn as ApiInstallOn
	},
	device::{
		SetDisplayStateReq, SetDisplayState,
		DisksReq, Disk as ApiDisk, Disks as ApiDisks
	},
	packages::{
		ListPackagesReq, ListPackages, AddPackage, AddPackageReq
	}
};
use api::request_handler;
use api::error::{Result as ApiResult, Error as ApiError};

use tokio::fs;
use tokio::task::JoinHandle;

use bootloader_api::{VersionInfoReq, Disks, InstallOn};

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
	server.register_request(disks);
	server.register_request(install_on);
	server.register_request(list_packages);
	server.register_request(add_package);

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
		eprintln!("opening page {}", req.url);
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
		bootloader: Bootloader,
		packages: Packages
	) -> ApiResult<SystemInfo> {

		let version_info = bootloader.request(&VersionInfoReq).await
			.map_err(ApiError::io)?;

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

// setPowerState

// listPackages
// addpackage
// removePackage

request_handler!(
	async fn list_packages(
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
	async fn add_package(
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