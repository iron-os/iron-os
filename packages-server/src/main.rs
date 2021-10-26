
mod config;
mod error;
mod packages;
mod server;
mod files;
mod auth;

use config::Config;
use files::Files;
use error::Result;
use auth::AuthDb;
use crate::packages::PackagesDb;

use std::env;
use crypto::signature::Keypair;



/*
Todo: use clap
*/


fn help() {
	println!("unkown command");
	println!("use `create` to create a configuration file");
	println!("use `` or `serve` to run the server");
}


pub enum Command {
	Create,
	Serve,
	Keys
}

impl Command {
	fn from_args() -> Option<Self> {
		let mut args = env::args();
		// ignore filename
		let _ = args.next();
		let cmd = args.next();
		match cmd.as_ref().map(|a| a.as_str()) {
			Some("create") => Some(Self::Create),
			Some("serve") => Some(Self::Serve),
			Some("keys") => Some(Self::Keys),
			None => Some(Self::Serve),
			_ => None
		}
	}
}

#[tokio::main]
async fn main() -> Result<()> {

	let cmd = match Command::from_args() {
		Some(cmd) => cmd,
		None => {
			help();
			return Ok(())
		}
	};

	match cmd {
		Command::Create => create().await,
		Command::Serve => server::serve().await,
		Command::Keys => {
			// get connection public key
			let cfg = Config::read().await?;
			println!("Connection public key: {}", cfg.con_key.public());

			if let Some(sign_key) = cfg.sign_key {
				println!("Current signature public key: {}", sign_key);
			}

			let sign = Keypair::new();
			println!("New signature private key: {}", sign.to_b64());
			println!("New signature public key: {}", sign.public());

			Ok(())
		}
	}
}


async fn create() -> Result<()> {
	println!("creating config.toml file if it doesn't exist");

	let cfg = Config::create().await?;

	let _pack_db = PackagesDb::create().await?;

	let _files = Files::create(&cfg).await?;

	let _auths = AuthDb::create().await?;

	Ok(())
}