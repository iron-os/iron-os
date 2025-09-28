use crate::config::Config;
use crate::error::Result;

use tokio::fs;

use packages::client::Client;
use packages::packages::BoardArch;
use packages::requests::PackageInfoReq;

#[derive(clap::Parser)]
pub struct GetFile {
	server_name: String,
	name: String,

	/// Either `Amd64` or `Arm64`.
	arch: BoardArch,

	/// Where should the file be downloaded to
	dest: Option<String>,
}

pub async fn get_file(cfg: GetFile) -> Result<()> {
	// check config
	let config = Config::open().await?;
	let source = config.get(&cfg.server_name)?;

	println!("connecting to {}", source.addr);

	// build a connection
	let client = Client::connect(&source.addr, source.public_key.clone())
		.await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;

	let package = client
		.package_info(PackageInfoReq {
			channel: source.channel,
			name: cfg.name.clone(),
			arch: cfg.arch,
			device_id: None,
			image_version: None,
			package_versions: None,
			ignore_requirements: true,
		})
		.await
		.map_err(|e| err!(e, "failed to get package info"))?;

	let Some(package) = package else {
		return Err(err!("not found", "package {} not found", cfg.name));
	};

	println!();
	println!("channel: {}", source.channel);
	println!("addr: {}", source.addr);

	println!("name: {}", package.name);
	println!("version_str: {}", package.version_str);
	println!("version: {}", package.version);
	println!("signature: {}", package.signature);
	println!("arch: {}", package.arch);
	println!("binary: {:?}", package.binary);

	let file = client
		.get_file(package.version.clone())
		.await
		.map_err(|e| err!(e, "failed to download file"))?;
	if file.is_empty() {
		return Err(err!("not found", "file {} not found", package.name));
	}

	// todo how do we get a signature key?
	// !source.sign_key.verify(&package.version, &package.signature)
	if file.hash() != package.version {
		return Err(err!("hash / sig", "file {} not correct", package.name));
	}

	// wait until the client is closed
	// this is done since the background task has not time to close
	// the connection since this process ends here
	client.close().await;

	// write file to disk
	let dest = cfg
		.dest
		.unwrap_or_else(|| format!("./{}.tar.gz", package.name));

	fs::write(&dest, file.file())
		.await
		.map_err(|e| err!(e, "could not write to {dest}"))?;

	println!("file written to {dest}");

	Ok(())
}
