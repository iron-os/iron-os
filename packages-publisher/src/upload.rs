
use crate::error::Result;
use crate::util::{read_toml, create_dir, compress, remove_dir, hash_file};
use crate::script::Script;

use std::io;

use crypto::signature::{Keypair, PublicKey};

use tokio::fs::{self, File};

use packages::client::Client;
use packages::requests::{SetFileReq, SetPackageInfoReq};
use packages::packages::{Channel, Package};
use clap::{AppSettings, Clap};

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
	pub tar_file: Option<String>
}

impl PackageToml {
	pub fn script(&self) -> &str {
		match &self.script {
			Some(s) => s,
			None => "./package.rhai"
		}
	}
}

/// Upload a package defined in `package.toml`.
/// `package.rhai` is used to build and prepare the package.
#[derive(Clap)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Upload {
	/// To what channel should this be updated
	channel: Channel,
	/// The address of the package server
	address: String,
	/// The public key of the package server
	public_key: PublicKey
}

pub async fn upload(cfg: Upload) -> Result<()> {

	// read package toml
	let package: PackageToml = read_toml("./package.toml").await?;

	// now we need to call build
	let mut script = Script::new(package.script())?;

	paint_act!("calling build");
	script.build(&cfg.channel)?;

	let tar_name = match package.tar_file {
		Some(n) => n,
		None => pack(&cfg, &package, &mut script).await?
	};

	let hash = hash_file(&tar_name).await?;

	println!();
	println!("Please enter the private signature key:");

	let mut priv_key_b64 = String::new();
	let stdin = io::stdin();
	stdin.read_line(&mut priv_key_b64)
		.map_err(|e| err!(e, "could not read private key"))?;
	let priv_key = Keypair::from_b64(priv_key_b64.trim())
		.map_err(|e| err!(format!("{:?}", e), "invalid private key"))?;

	// sign
	let sign = priv_key.sign(hash.as_slice());

	// create the package
	let package = Package {
		name: package.name,
		version_str: package.version,
		version: hash,
		signature: sign.clone(),
		binary: package.binary
	};

	println!();
	println!("do you really wan't to upload package:");
	println!("channel: {}", cfg.channel);
	print_package(&package);
	println!();
	println!("Enter YES to confirm");

	let mut confirm = String::new();
	let stdin = io::stdin();
	stdin.read_line(&mut confirm)
		.map_err(|e| err!(e, "could not read private key"))?;

	if confirm.trim() != "YES" {
		return Err(err!(confirm, "confirmation not received"))
	}

	// build a connection
	let client = Client::connect(&cfg.address, cfg.public_key.clone()).await
		.map_err(|e| err!(e, "connect to {} failed", cfg.address))?;

	let tar = File::open(&tar_name).await
		.expect("tar file deleted");
	let file_req = SetFileReq::new(sign, tar).await
		.expect("reading tar failed");
	assert_eq!(file_req.hash(), package.version);

	client.request(file_req).await
		.map_err(|e| err!(e, "failed to upload file"))?;


	let pack_req = SetPackageInfoReq {
		channel: cfg.channel,
		package
	};
	client.request(pack_req).await
		.map_err(|e| err!(e, "failed to upload package"))?;

	fs::remove_file(&tar_name).await
		.expect("could not remove tar");

	println!("package uploaded");

	Ok(())
}

pub async fn pack(cfg: &Upload, package: &PackageToml, script: &mut Script) -> Result<String> {

	let dest_folder = format!("./package_tmp/{}", package.name);
	create_dir(&dest_folder).await?;

	// call package
	paint_act!("calling pack");
	script.pack(&dest_folder, &cfg.channel)?;

	let tar_name = format!("{}.tar.gz", &package.name);

	// now the folder should be compressed
	// tar -zcvf name.tar.gz source
	compress(&tar_name, "./package_tmp", &package.name)?;
	remove_dir("./package_tmp").await?;

	Ok(tar_name)
}

fn print_package(pack: &Package) {
	println!("name: {}", pack.name);
	println!("version_str: {}", pack.version_str);
	println!("version: {}", pack.version);
	println!("signature: {}", pack.signature);
	println!("binary: {:?}", pack.binary);
}