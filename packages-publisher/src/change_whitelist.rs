use crate::config::Config;
use crate::error::Result;
use crate::util::{get_priv_key, read_toml};

use std::collections::HashSet;
use std::io;

use packages::client::Client;
use packages::error::Error as ApiError;
use packages::packages::{BoardArch, Channel, Hash, TargetArch};
use packages::requests::DeviceId;

use riji::{paint_err, paint_ok};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct PackageToml {
	pub name: String,
	#[serde(rename = "single-arch")]
	pub single_arch: Option<TargetArch>,
}

#[derive(clap::Parser)]
pub struct ChangeWhitelistOpts {
	channel: Channel,
	version: Hash,
	#[clap(long)]
	arch: Option<BoardArch>,
	#[clap(long, default_value = "0")]
	auto_whitelist: u32,
	#[clap(long)]
	add: bool,
	#[clap(long, num_args(0..))]
	whitelist: Vec<DeviceId>,
}

pub async fn change_whitelist(opts: ChangeWhitelistOpts) -> Result<()> {
	// check config
	let config = Config::open().await?;
	let source = config.get(&opts.channel)?;

	// read package toml
	let package: PackageToml = read_toml("./package.toml").await?;

	let target_archs = match (package.single_arch, opts.arch) {
		(None, None) => vec![TargetArch::Amd64, TargetArch::Arm64],
		(Some(arch), _) => vec![arch],
		(_, Some(arch)) => vec![arch.into()],
	};

	println!("connecting to {}", source.addr);

	// build a connection
	let client = Client::connect(&source.addr, source.public_key.clone())
		.await
		.map_err(|e| err!(e, "connect to {} failed", source.addr))?;

	let key = get_priv_key(&source).await?;

	// authenticate
	client
		.authenticate_writer(&opts.channel, &key)
		.await
		.map_err(|e| err!(e, "Authentication failed"))?;

	let whitelist: HashSet<_> = opts.whitelist.into_iter().collect();

	println!();
	println!("do you really wan't to change the whitelist for package:");
	println!("channel: {}", opts.channel);
	println!("version: {:?}", opts.version);
	println!("archs: {:?}", target_archs);
	println!("add: {:?}", if opts.add { "yes" } else { "no" });
	println!("auto-whitelist: {:?}", opts.auto_whitelist);
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

	for arch in target_archs {
		let r = client
			.change_whitelist(
				arch,
				package.name.clone(),
				opts.version.clone(),
				whitelist.clone(),
				opts.add,
				opts.auto_whitelist,
			)
			.await;
		match r {
			Ok(_) => {
				paint_ok!("whitelist for arch {} changed", arch);
			}
			Err(ApiError::VersionNotFound) => {
				paint_err!("version for arch {} not found", arch);
			}
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
