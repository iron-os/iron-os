
use crate::error::Result;
use crate::util::{read_toml, create_dir, compress, remove_dir, hash_file};
use crate::script::Script;
use crate::config::Config;

use std::io;

use crypto::signature::Keypair;

use tokio::fs::{self, File};

use packages::client::Client;
use packages::requests::{SetFileReq, SetPackageInfoReq};
use packages::packages::{Channel, Package, TargetArch, BoardArch};

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
	pub single_arch: Option<TargetArch>
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
#[derive(clap::Parser)]
pub struct Upload {
	/// To what channel should this be updated
	channel: Channel,
	/// if no architecture is selected `Amd64` and `Arm64` are used.
	/// 
	/// Expect for image.
	#[clap(long)]
	arch: Option<BoardArch>
}

pub async fn upload(cfg: Upload) -> Result<()> {

	// check config
	let config = Config::open().await?;
	let source = config.get(&cfg.channel)?;

	// read package toml
	let package: PackageToml = read_toml("./package.toml").await?;

	let priv_key = if let Some(k) = &source.priv_key {
		println!("using existing private signature key");
		k.clone()
	} else {
		println!();
		println!("Please enter the private signature key:");

		let mut priv_key_b64 = String::new();
		let stdin = io::stdin();
		stdin.read_line(&mut priv_key_b64)
			.map_err(|e| err!(e, "could not read private key"))?;
		Keypair::from_b64(priv_key_b64.trim())
			.map_err(|e| err!(format!("{:?}", e), "invalid private key"))?
	};

	let mut packages = vec![];

	match (package.single_arch, cfg.arch) {
		(None, None) => {
			for arch in &[TargetArch::Amd64, TargetArch::Arm64] {
				packages.push(build(
					arch,
					&cfg.channel,
					&package,
					&priv_key
				).await?);
			}
		},
		(Some(arch), _) => {
			packages.push(build(
				&arch,
				&cfg.channel,
				&package,
				&priv_key
			).await?);
		},
		(_, Some(arch)) => {
			packages.push(build(
				&arch.into(),
				&cfg.channel,
				&package,
				&priv_key
			).await?);
		}
	}

	println!();
	println!("do you really wan't to upload package:");
	println!("channel: {}", cfg.channel);
	println!("addr: {}", source.addr);
	for (_, pack) in packages.iter() {
		print_package(pack);
	}
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
	let client = Client::connect(&source.addr, source.public_key.clone()).await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;

	for (tar_path, package) in packages {

		let tar = File::open(&tar_path).await
			.expect("tar file deleted");
		let file_req = SetFileReq::new(package.signature.clone(), tar).await
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

		fs::remove_file(&tar_path).await
			.expect("could not remove tar");

	}

	println!("package uploaded");

	Ok(())
}


async fn build(
	arch: &TargetArch,
	channel: &Channel,
	package: &PackageToml,
	priv_key: &Keypair
	// tar file
) -> Result<(String, Package)> {
	// now we need to call build
	let mut script = Script::new(package.script())?;

	paint_act!("calling build {} {}", arch, channel);
	script.build(&arch, &channel)?;

	let tar_name = match &package.tar_file {
		Some(n) => n.clone(),
		None => pack(arch, channel, &package, &mut script).await?
	};

	let hash = hash_file(&tar_name).await?;

	// sign
	let sign = priv_key.sign(hash.as_slice());

	// create the package
	let package = Package {
		name: package.name.clone(),
		version_str: package.version.clone(),
		version: hash,
		signature: sign,
		arch: *arch,
		binary: package.binary.clone()
	};

	Ok((tar_name, package))
}


async fn pack(
	arch: &TargetArch,
	channel: &Channel,
	package: &PackageToml,
	script: &mut Script
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

fn print_package(pack: &Package) {
	println!("name: {}", pack.name);
	println!("version_str: {}", pack.version_str);
	println!("version: {}", pack.version);
	println!("signature: {}", pack.signature);
	println!("arch: {}", pack.arch);
	println!("binary: {:?}", pack.binary);
}