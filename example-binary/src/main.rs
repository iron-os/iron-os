use tokio::fs;
use tokio::time::{sleep, Duration};

use clap::Parser;

use service_api::client::Client;
use service_api::requests::network::{AddConnectionKind, AddConnectionWifi};

#[derive(Debug, Parser)]
struct Args {
	#[clap(subcommand)]
	subcmd: Option<SubCommand>,
}

#[derive(Debug, Parser)]
enum SubCommand {
	/// List all disks that could be used to install on
	ListDisks(ListDisks),
	/// Install on a specific disk
	Install(Install),
	/// View Device Info
	DeviceInfo(DeviceInfo),
	/// View System Info
	SystemInfo(SystemInfo),
	/// List all packages
	ListPackages(ListPackages),
	/// UpdateNow
	UpdateNow(UpdateNow),
	/// List Access Points
	ListAccessPoints(ListAccessPoints),
	/// List Connections
	ListConnections(ListConnections),
	/// Connecte to Wifi
	ConnectWifi(ConnectWifi),
	/// Take a Screenshot
	Screenshot,
}

#[derive(Debug, Parser)]
struct ListDisks {}

#[derive(Debug, Parser)]
struct Install {
	disk: String,
}

#[derive(Debug, Parser)]
struct DeviceInfo {}

#[derive(Debug, Parser)]
struct SystemInfo {}

#[derive(Debug, Parser)]
struct ListPackages {}

#[derive(Debug, Parser)]
struct UpdateNow {}

#[derive(Debug, Parser)]
struct ListAccessPoints {}

#[derive(Debug, Parser)]
struct ListConnections {}

/// Connect to a wifi which was listed in list access point
///
/// should be a wpa-psk access point
#[derive(Debug, Parser)]
struct ConnectWifi {
	pub interface_name: String,
	pub ssid: String,
	pub password: String,
}

#[tokio::main]
async fn main() {
	let args = Args::parse();

	let client = Client::connect("/data/service-api")
		.await
		.expect("could not connect to the service api");

	match args.subcmd {
		Some(SubCommand::ListDisks(_)) => {
			let disks = client.disks().await.expect("failed to list disks");

			println!("{:>8} | {:>4} | {:>8}", "Name", "Init", "Size");
			for disk in disks {
				println!(
					"{:>8} | {:>4} | {:>8.0}gb",
					disk.name,
					disk.initialized,
					disk.size / 1000_000_000
				);
			}
		}
		Some(SubCommand::Install(i)) => {
			println!("Installing... to {}", i.disk);
			client.install_on(i.disk).await.expect("failed to install");
			println!("Installation completed");
		}
		Some(SubCommand::DeviceInfo(_)) => {
			println!("Display device info");
			let device_info = client
				.device_info()
				.await
				.expect("failed to get device info");
			println!("{:#?}", device_info);
		}
		Some(SubCommand::SystemInfo(_)) => {
			println!("Display system info");
			let system_info = client
				.system_info()
				.await
				.expect("failed to get system info");
			println!("{:#?}", system_info);
		}
		Some(SubCommand::ListPackages(_)) => {
			println!("Display packages");
			let packages = client
				.list_packages()
				.await
				.expect("failed to list packages");
			println!("{:#?}", packages);
		}
		Some(SubCommand::UpdateNow(_)) => {
			println!("update now");
			client
				.request_update()
				.await
				.expect("failed to request update");
			println!("update done");
		}
		Some(SubCommand::ListAccessPoints(_)) => {
			println!("access points");
			let list = client.network_access_points().await.unwrap();
			println!("{:#?}", list);
		}
		Some(SubCommand::ListConnections(_)) => {
			println!("connections");
			let list = client.network_connections().await.unwrap();
			println!("{:#?}", list);
		}
		Some(SubCommand::ConnectWifi(conn)) => {
			println!("connect to wifi {}", conn.ssid);
			let conn = client
				.network_add_connection(
					format!("wifi-{}", conn.ssid),
					AddConnectionKind::Wifi(AddConnectionWifi {
						interface_name: conn.interface_name,
						ssid: conn.ssid,
						password: conn.password,
					}),
				)
				.await
				.unwrap();
			println!("connected to {:?}", conn);
		}
		Some(SubCommand::Screenshot) => {
			println!("taking screenshot");
			let png = client.take_screenshot().await.unwrap();
			fs::write("./screenshot.png", png)
				.await
				.expect("could not write screenshot.png");
			println!("wrote screenshot.png");
		}
		None => {
			println!("opening https://youtube.com in 30s");
			// wait until a network connection could be established
			sleep(Duration::from_secs(30)).await;

			// let's open youtube
			client
				.open_page("https://youtube.com".into())
				.await
				.expect("could not open youtube");

			loop {
				sleep(Duration::from_secs(5 * 60)).await
			}
		}
	}
}
