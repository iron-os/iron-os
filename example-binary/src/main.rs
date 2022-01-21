
mod service_api;

use tokio::time::{sleep, Duration};

use clap::Parser;


#[derive(Parser, Debug)]
struct Args {
	/// List all disks that could be used to install on
	#[clap(long = "disks")]
	list_disks: bool,
	/// Install on a specific disk
	#[clap(long)]
	install: Option<String>
}

#[tokio::main]
async fn main() {
	let args = Args::parse();

	let client = service_api::Client::connect().await
		.expect("could not connect to the service api");

	if args.list_disks {
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
	}

	if let Some(disk) = args.install {
		println!("Installing... to {}", disk);
		client.install_on(disk).await
			.expect("failed to install");
		println!("Installation completed");
		return;
	}

	// wait until a connection could be established
	sleep(Duration::from_secs(30)).await;

	// let's open youtube
	client.open_page("https://youtube.com").await
		.expect("could not open youtube");

	loop {
		sleep(Duration::from_secs(5 * 60)).await
	}
}