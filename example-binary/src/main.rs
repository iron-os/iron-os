use tokio::time::{sleep, Duration};

use clap::Parser;

use service_api::client::Client;


#[derive(Debug, Parser)]
struct Args {
	#[clap(subcommand)]
	subcmd: Option<SubCommand>
}

#[derive(Debug, Parser)]
enum SubCommand {
	/// List all disks that could be used to install on
	ListDisks(ListDisks),
	/// Install on a specific disk
	Install(Install),
	/// View Device Info
	DeviceInfo(DeviceInfo),
	/// List all packages
	ListPackages(ListPackages),
	/// UpdateNow
	UpdateNow(UpdateNow)
}

#[derive(Debug, Parser)]
struct ListDisks {}

#[derive(Debug, Parser)]
struct Install {
	disk: String
}

#[derive(Debug, Parser)]
struct DeviceInfo {}

#[derive(Debug, Parser)]
struct ListPackages {}

#[derive(Debug, Parser)]
struct UpdateNow {}

#[tokio::main]
async fn main() {
	let args = Args::parse();

	let client = Client::connect("/data/service-api").await
		.expect("could not connect to the service api");

	match args.subcmd {
		Some(SubCommand::ListDisks(_)) => {
			let disks = client.disks().await
				.expect("failed to list disks");

			println!("{:>8} | {:>4} | {:>8}", "Name", "Init", "Size");
			for disk in disks {
				println!(
					"{:>8} | {:>4} | {:>8.0}gb",
					disk.name,
					disk.initialized,
					disk.size / 1000_000_000
				);
			}

			return;
		},
		Some(SubCommand::Install(i)) => {
			println!("Installing... to {}", i.disk);
			client.install_on(i.disk).await
				.expect("failed to install");
			println!("Installation completed");
			return;
		},
		Some(SubCommand::DeviceInfo(_)) => {
			println!("Display device info");
			let device_info = client.device_info().await
				.expect("failed to get device info");
			println!("{:#?}", device_info);
			return;
		},
		Some(SubCommand::ListPackages(_)) => {
			println!("Display packages");
			let packages = client.list_packages().await
				.expect("failed to list packages");
			println!("{:#?}", packages);
			return
		},
		Some(SubCommand::UpdateNow(_)) => {
			eprintln!("update now");
			client.request_update().await
				.expect("failed to request update");
			println!("update done");
			return;
		}
		None => {}
	}

	// wait until a network connection could be established
	sleep(Duration::from_secs(30)).await;

	// let's open youtube
	client.open_page("https://youtube.com".into()).await
		.expect("could not open youtube");

	loop {
		sleep(Duration::from_secs(5 * 60)).await
	}
}