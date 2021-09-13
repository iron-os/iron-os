
use packages::packages::Channel;
use clap::{AppSettings, Clap};

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

pub async fn upload(cfg: Upload) {
	
}