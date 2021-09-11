
mod config;
mod error;
mod packages;
mod server;

use config::Config;
use error::{Result, Error};
use crate::packages::PackagesDb;

use std::env;
use crypto::signature::Keypair;




fn help() {
	println!("unkown command");
	println!("use `create` to create a configuration file");
	println!("use `` or `serve` to run the server");
}


pub enum Command {
	Create,
	Serve,
	GenSignature
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
			Some("gen-signature") => Some(Self::GenSignature),
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
		Command::GenSignature => {
			let sign = Keypair::new();
			println!("Private Key: {}", sign.to_b64());
			println!("Public Key: {}", sign.public());
			Ok(())
		}
	}
}


async fn create() -> Result<()> {
	println!("creating config.toml file if it doesn't exist");

	let _cfg = Config::create().await?;

	let _pack_db = PackagesDb::create().await?;

	Ok(())
}