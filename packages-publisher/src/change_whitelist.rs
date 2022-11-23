
use crate::error::Result;
use crate::util::read_toml;
use crate::config::Config;

use std::collections::HashSet;

use packages::client::Client;
use packages::requests::DeviceId;
use packages::packages::{Channel, Hash, TargetArch, BoardArch};
use packages::error::Error as ApiError;

use riji::{paint_ok, paint_err};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct PackageToml {
	pub name: String,
	#[serde(rename = "single-arch")]
	pub single_arch: Option<TargetArch>
}

#[derive(clap::Parser)]
pub struct ChangeWhitelistOpts {
	channel: Channel,
	version: Hash,
	#[clap(long)]
	arch: Option<BoardArch>,
	#[clap(long, num_args(0..))]
	whitelist: Vec<DeviceId>
}

pub async fn change_whitelist(opts: ChangeWhitelistOpts) -> Result<()> {

	// check config
	let config = Config::open().await?;
	let source = config.get(&opts.channel)?;

	if source.auth_key.is_none() {
		println!("please first call auth <channel> to get an auth key");
		return Ok(())
	}

	// read package toml
	let package: PackageToml = read_toml("./package.toml").await?;

	let target_archs = match (package.single_arch, opts.arch) {
		(None, None) => vec![TargetArch::Amd64, TargetArch::Arm64],
		(Some(arch), _) => vec![arch],
		(_, Some(arch)) => vec![arch.into()]
	};

	println!("connecting to {}", source.addr);

	// build a connection
	let client = Client::connect(&source.addr, source.public_key.clone()).await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;

	// authenticate
	client.authenticate(source.auth_key.clone().unwrap()).await
		.map_err(|e| err!(e, "Authentication failed"))?;

	let whitelist: HashSet<_> = opts.whitelist.into_iter().collect();

	for arch in target_archs {
		let r = client.change_whitelist(
			opts.channel,
			arch,
			package.name.clone(),
			opts.version.clone(),
			whitelist.clone()
		).await;
		match r {
			Ok(_) => {
				paint_ok!("whitelist for arch {} changed", arch);
			},
			Err(ApiError::VersionNotFound) => {
				paint_err!("version for arch {} not found", arch);
			},
			Err(e) => {
				return Err(err!(e, "Could not change whitelist {}", arch))
			}
		}
	}

	// wait until the client is closed
	// this is done since the background task has not time to close
	// the connection since this process ends here
	client.close().await;

	Ok(())
}