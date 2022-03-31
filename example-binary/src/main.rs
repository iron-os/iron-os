
mod service_api;

use tokio::time::{sleep, Duration};

use clap::Parser;


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
	DeviceInfo(DeviceInfo)
}

#[derive(Debug, Parser)]
struct ListDisks {}

#[derive(Debug, Parser)]
struct Install {
	disk: String
}

#[derive(Debug, Parser)]
struct DeviceInfo {}

#[tokio::main]
async fn main() {
	let args = Args::parse();

	let client = service_api::Client::connect().await
		.expect("could not connect to the service api");

	match args.subcmd {
		Some(SubCommand::ListDisks(_)) => {
			let disks = client.list_disks().await
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
			eprintln!("{:#?}", device_info);
			return;
		},
		_ => {}
	}

	// wait until a network connection could be established
	sleep(Duration::from_secs(30)).await;

	// let's open youtube
	client.open_page("https://youtube.com").await
		.expect("could not open youtube");

	loop {
		sleep(Duration::from_secs(5 * 60)).await
	}
}