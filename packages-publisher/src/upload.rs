use crate::config::Config;
use crate::error::Result;
use crate::script::Script;
use crate::util::{
	compress, create_dir, get_priv_key, hash_file, read_toml, remove_dir,
};

use std::collections::HashSet;
use std::io;
use std::iter::FromIterator;

use crypto::signature::Keypair;

use tokio::fs::{self, File};

use packages::client::Client;
use packages::packages::{BoardArch, Channel, Package, TargetArch};
use packages::requests::{DeviceId, SetFileReq};

use riji::paint_act;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PackageToml {
	pub name: String,
	pub version: String,
	pub binary: Option<String>,
	/// Default is package.rhai
	pub script: Option<String>,
	#[serde(rename = "tar-file")]
	pub tar_file: Option<String>,
	#[serde(rename = "single-arch")]
	pub single_arch: Option<TargetArch>,
}

impl PackageToml {
	pub fn script(&self) -> &str {
		match &self.script {
			Some(s) => s,
			None => "./package.rhai",
		}
	}
}

/// Upload a package defined in `package.toml`.
/// `package.rhai` is used to build and prepare the package.
#[derive(clap::Parser)]
pub struct Upload {
	/// To what channel should this be updated
	channel: Channel,
	/// if no architecture is selected `Amd64` and `Arm64` are used.
	///
	/// Expect for image.
	#[clap(long)]
	arch: Option<BoardArch>,
	#[clap(long)]
	host_channel: Option<Channel>,
	#[clap(long, default_value = "0")]
	auto_whitelist: u32,
	#[clap(long, num_args(0..))]
	whitelist: Vec<DeviceId>,
	#[clap(long)]
	whitelist_file: Option<String>,
}

pub async fn upload(cfg: Upload) -> Result<()> {
	// check config
	let config = Config::open().await?;
	let source = config.get(&cfg.channel)?;

	let priv_key = get_priv_key(&source).await?;

	// read package toml
	let package: PackageToml = read_toml("./package.toml").await?;

	let mut packages = vec![];

	match (package.single_arch, cfg.arch) {
		(None, None) => {
			for arch in &[TargetArch::Amd64, TargetArch::Arm64] {
				packages.push(
					build(
						arch,
						&cfg.channel,
						cfg.host_channel.as_ref(),
						&package,
						&priv_key,
					)
					.await?,
				);
			}
		}
		(Some(arch), _) => {
			packages.push(
				build(
					&arch,
					&cfg.channel,
					cfg.host_channel.as_ref(),
					&package,
					&priv_key,
				)
				.await?,
			);
		}
		(_, Some(arch)) => {
			packages.push(
				build(
					&arch.into(),
					&cfg.channel,
					cfg.host_channel.as_ref(),
					&package,
					&priv_key,
				)
				.await?,
			);
		}
	}

	let mut whitelist = HashSet::from_iter(cfg.whitelist);

	if let Some(file) = cfg.whitelist_file {
		let ctn = fs::read_to_string(file)
			.await
			.expect("could not open whitelist_file");
		for line in ctn.lines().filter(|l| !l.is_empty()) {
			whitelist.insert(line.parse().unwrap());
		}
	}

	println!();
	println!("do you really wan't to upload package:");
	println!("channel: {}", cfg.channel);
	println!("addr: {}", source.addr);
	for (tar_path, pack) in packages.iter() {
		print_package(tar_path, pack).await;
	}
	println!("auto-whitelist: {:?}", cfg.auto_whitelist);
	println!("whitelist: {:?}", whitelist);
	println!();
	println!("Enter YES to confirm");

	let mut confirm = String::new();
	let stdin = io::stdin();
	stdin
		.read_line(&mut confirm)
		.map_err(|e| err!(e, "could not read confirmation"))?;

	if confirm.trim() != "YES" {
		return Err(err!(confirm, "confirmation not received"));
	}

	println!("connecting to {}", source.addr);

	// build a connection
	let client = Client::connect(&source.addr, source.public_key.clone())
		.await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;

	// authenticate
	client
		.authenticate_writer(&cfg.channel, &priv_key)
		.await
		.map_err(|e| err!(e, "Authentication failed"))?;

	for (tar_path, package) in packages {
		let tar = File::open(&tar_path).await.expect("tar file deleted");
		let file_req = SetFileReq::new(package.signature.clone(), tar)
			.await
			.expect("reading tar failed");
		assert_eq!(file_req.hash(), package.version);

		client
			.set_file(file_req)
			.await
			.map_err(|e| err!(e, "failed to upload file"))?;

		client
			.set_package_info(package, whitelist.clone(), cfg.auto_whitelist)
			.await
			.map_err(|e| err!(e, "failed to upload package"))?;

		fs::remove_file(&tar_path)
			.await
			.expect("could not remove tar");
	}

	println!("package uploaded");

	// wait until the client is closed
	// this is done since the background task has not time to close
	// the connection since this process ends here
	client.close().await;

	Ok(())
}

async fn build(
	arch: &TargetArch,
	channel: &Channel,
	host_channel: Option<&Channel>,
	package: &PackageToml,
	priv_key: &Keypair, // tar file
) -> Result<(String, Package)> {
	// now we need to call build
	let mut script = Script::new(package.script())?;

	paint_act!("calling build {} {}", arch, channel);
	script.build(&arch, &channel, host_channel)?;

	let tar_name = match &package.tar_file {
		Some(n) => n.clone(),
		None => pack(arch, channel, &package, &mut script).await?,
	};

	let hash = hash_file(&tar_name).await?;

	// sign
	let sign = priv_key.sign(&hash);

	// create the package
	let package = Package {
		name: package.name.clone(),
		version_str: package.version.clone(),
		version: hash,
		signature: sign,
		arch: *arch,
		binary: package.binary.clone(),
	};

	Ok((tar_name, package))
}

async fn pack(
	arch: &TargetArch,
	channel: &Channel,
	package: &PackageToml,
	script: &mut Script,
) -> Result<String> {
	let dest_folder = format!("./package_tmp/{}", package.name);
	create_dir(&dest_folder).await?;

	// call package
	paint_act!("calling pack");
	script.pack(&dest_folder, arch, &channel)?;

	let tar_name = format!("{}-{}.tar.gz", &package.name, &arch);

	// now the folder should be compressed
	// tar -zcvf name.tar.gz source
	compress(&tar_name, "./package_tmp", &package.name)?;
	remove_dir("./package_tmp").await?;

	Ok(tar_name)
}

async fn print_package(tar_path: &str, pack: &Package) {
	let size = fs::metadata(tar_path)
		.await
		.expect("failed to read package archive metadata")
		.len();
	println!("name: {}", pack.name);
	println!("version_str: {}", pack.version_str);
	println!("version: {}", pack.version);
	println!("signature: {}", pack.signature);
	println!("arch: {}", pack.arch);
	println!("binary: {:?}", pack.binary);
	println!("tar size: {:.0}mb", size / 1000_000);
}
