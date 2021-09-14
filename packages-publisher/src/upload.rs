
use crate::error::{Result, Error};
use crate::util::{read_toml, create_dir, compress, remove_dir, hash_file};
use crate::script::Script;

use packages::packages::Channel;
use clap::{AppSettings, Clap};

use riji::paint_act;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PackageToml {
	pub name: String,
	pub version: String,
	pub binary: Option<String>,
	/// Default is package.rhai
	pub script: Option<String>
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
	/// The address of the package server
	address: String,
	/// To what channel should this be updated
	channel: Channel
}

pub async fn upload(cfg: Upload) -> Result<()> {

	// read package toml
	let package: PackageToml = read_toml("./package.toml").await?;

	// now we need to call build
	let mut script = Script::new(package.script())?;

	paint_act!("calling build");
	script.build(&cfg.channel)?;

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

	let hash = hash_file(&tar_name).await?;

	println!("hash {}", hash);

	// sign

	// signed

	// and uploaded

	todo!("")
}