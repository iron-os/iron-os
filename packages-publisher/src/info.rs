use crate::config::Config;
use crate::error::Result;

use packages::client::Client;
use packages::packages::{BoardArch, Channel};
use packages::requests::DeviceId;

#[derive(clap::Parser)]
pub struct Info {
	channel: Channel,
	name: String,

	/// if no architecture is selected `Amd64` and `Arm64` are used.
	#[clap(long)]
	arch: Option<BoardArch>,

	/// If you want to specificy as which device we make the request
	#[clap(long)]
	device_id: Option<DeviceId>,
}

pub async fn info(cfg: Info) -> Result<()> {
	// check config
	let config = Config::open().await?;
	let source = config.get(&cfg.channel)?;

	let archs: &[BoardArch] = cfg
		.arch
		.as_ref()
		.map(std::slice::from_ref)
		.unwrap_or_else(|| &[BoardArch::Amd64, BoardArch::Arm64]);

	println!("connecting to {}", source.addr);

	// build a connection
	let client = Client::connect(&source.addr, source.public_key.clone())
		.await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;

	let mut packages = vec![];

	for arch in archs {
		let package = client
			.package_info(
				cfg.channel,
				*arch,
				cfg.device_id.clone(),
				cfg.name.clone(),
			)
			.await
			.map_err(|e| err!(e, "failed to get package info"))?;

		if let Some(package) = package {
			packages.push(package);
		}
	}

	println!();
	println!("channel: {}", cfg.channel);
	println!("addr: {}", source.addr);
	for pack in packages {
		println!();
		println!("name: {}", pack.name);
		println!("version_str: {}", pack.version_str);
		println!("version: {}", pack.version);
		println!("signature: {}", pack.signature);
		println!("arch: {}", pack.arch);
		println!("binary: {:?}", pack.binary);
	}

	// wait until the client is closed
	// this is done since the background task has not time to close
	// the connection since this process ends here
	client.close().await;

	Ok(())
}
