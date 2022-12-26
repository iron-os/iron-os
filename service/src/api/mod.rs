mod device_info;
mod network_manager;

use network_manager::{NetworkManager, DeviceKind, ApSecurityFlag};

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
		ListPackagesReq, ListPackages, AddPackage, AddPackageReq, UpdateReq
	},
	device::{DeviceInfoReq, DeviceInfo, SetPowerStateReq, PowerState},
	network::{
		AccessPointsReq, AccessPoints, AccessPoint,
		ConnectionsReq, Connections, Connection, ConnectionKind, ConnectionWifi
	}
};
use api::{request_handler, Action};
use api::error::{Result as ApiResult, Error as ApiError};

use tokio::fs;
use tokio::task::{JoinHandle, spawn_blocking};

fn api_error_dbus(e: dbus::Error) -> ApiError {
	ApiError::Internal(e.to_string())
}

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
	server.register_request(set_power_state);
	server.register_request(list_packages);
	server.register_request(add_package);
	server.register_request(update_req);
	server.register_request(device_info_req);
	server.register_request(access_points_req);
	server.register_request(connections);

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
			device_id: version_info.device_id.clone(),
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

request_handler!(
	async fn set_power_state<Action>(
		req: SetPowerStateReq,
		bootloader: Bootloader
	) -> ApiResult<()> {
		let r = match req.state {
			PowerState::Shutdown => bootloader.shutdown().await,
			PowerState::Restart => bootloader.restart().await
		};
		r.map_err(ApiError::internal)
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
		let package = packages.add_package(req.name).await;

		Ok(AddPackage { package })
	}
);

request_handler!(
	async fn update_req<Action>(
		_req: UpdateReq,
		packages: Packages
	) -> ApiResult<()> {
		Ok(packages.update().await)
	}
);

/*
Get JournalLogs

journalctl -n 400 --output-fields=_SYSTEMD_UNIT,MESSAGE,CODE_FILE,CODE_LINE,CODE_FUNC,_EXE -r -o json > test.txt
*/

fn access_points_sync() -> ApiResult<AccessPoints> {
	let nm = NetworkManager::connect().map_err(api_error_dbus)?;

	// lets get the first device that is a wifi device
	let device = nm.devices().map_err(api_error_dbus)?
		.into_iter()
		.find_map(|device| {
			// check the kind
			match device.kind() {
				Ok(DeviceKind::Wifi) => {},
				Ok(_) => return None,
				Err(e) => {
					eprintln!("could not get device kind {:?}", e);
					return None
				}
			}

			let inf = match device.interface() {
				Ok(s) => s,
				Err(_) => return None
			};

			Some((inf, device))
		});
	let (device_name, device) = match device {
		Some(d) => d,
		None => return Ok(AccessPoints { device: String::new(), list: vec![] })
	};

	let access_points = device.access_points().map_err(api_error_dbus)?;

	let list = access_points.into_iter()
		.filter_map(|ap| {
			let has_mgmt_psk =
				ap.wpa_flags()
					.map(|f| f.matches(ApSecurityFlag::KeyMgmtPsk))
					.unwrap_or(false) ||
				ap.rsn_flags()
					.map(|f| f.matches(ApSecurityFlag::KeyMgmtPsk))
					.unwrap_or(false);

			if !has_mgmt_psk {
				return None
			}

			let (ssid, strength) = match (ap.ssid(), ap.strength()) {
				(Ok(s), Ok(st)) => (s, st),
				_ => return None
			};

			Some(AccessPoint { ssid, strength })
		})
		.collect();

	Ok(AccessPoints {
		device: device_name,
		list
	})
}

request_handler!(
	async fn access_points_req<Action>(
		_req: AccessPointsReq,
	) -> ApiResult<AccessPoints> {
		spawn_blocking(access_points_sync).await
			.unwrap()
	}
);


fn connections_sync() -> ApiResult<Connections> {
	let nm = NetworkManager::connect().map_err(api_error_dbus)?;

	let cons = nm.connections().map_err(api_error_dbus)?;

	let list = cons.into_iter()
		.filter_map(|con| {
			let setts = con.get_settings().ok()?;

			let con_setts = setts.get("connection")?;

			let ty = con_setts.get_str("type")?;
			let id = con_setts.get_str("id")?.to_string();
			let uuid = con_setts.get_str("uuid")?.to_string();

			let kind = match ty {
				"802-11-wireless" => {
					let interface_name = con_setts.get_str("interface-name")?
						.to_string();
					let wifi_setts = setts.get("802-11-wireless")?;
					let ssid = wifi_setts.get_string_from_bytes("ssid")?;
					let mode = wifi_setts.get_str("mode")?;
					if mode != "infrastructure" {
						eprintln!("unknown wifi mode {:?}", mode);
						return None
					}

					let wifi_sec_setts = setts.get("802-11-wireless-security")?;
					let key_mgmt = wifi_sec_setts.get_str("key-mgmt")?;
					if key_mgmt != "wpa-psk" {
						eprintln!("unknown key-mgmt {:?}", key_mgmt);
						return None
					}

					let wifi = ConnectionWifi { interface_name, ssid };
					ConnectionKind::Wifi(wifi)
				},
				"802-3-ethernet" => return None,
				"gsm" => {
					eprintln!("gsm settings {:#?}", setts);
					return None
				},
				ty => {
					eprintln!("connection type unknown {:?}", ty);
					return None
				}
			};

			Some(Connection { id, uuid, kind })
		})
		.collect();

	Ok(Connections { list })
}

request_handler!(
	async fn connections<Action>(
		_req: ConnectionsReq,
	) -> ApiResult<Connections> {
		spawn_blocking(connections_sync).await
			.unwrap()
	}
);