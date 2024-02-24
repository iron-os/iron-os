mod config;
mod error;
mod packages;
mod server;
mod files;
mod auth;
#[cfg(test)]
mod testing_requests;

use config::Config;
use files::Files;
use error::Result;
use auth::AuthDb;
use crate::packages::PackagesDb;

use clap::Parser;
use crypto::signature::Keypair;

#[derive(Parser)]
struct Args {
	#[clap(subcommand)]
	subcmd: Option<SubCommand>,
	#[clap(long, default_value = "packages_server=info,error")]
	tracing: String,
	#[clap(long, default_value = "./config.toml")]
	config: String
}

#[derive(Parser)]
pub enum SubCommand {
	/// create a configuration file
	Create,
	/// Print out the keys
	Keys
}

#[tokio::main]
async fn main() -> Result<()> {
	let args = Args::parse();

	match args.subcmd {
		Some(SubCommand::Create) => {
			println!("creating config files if they dont exist");

			let cfg = Config::create(&args.config).await?;

			let _pack_db = PackagesDb::create(&cfg).await?;

			let _files = Files::create(&cfg).await?;

			let _auths = AuthDb::create(&cfg).await?;

			return Ok(())
		},
		Some(SubCommand::Keys) => {
			// get connection public key
			let cfg = Config::read(&args.config).await?;
			println!("Connection public key: {}", cfg.con_key.public());

			println!("cfg debug: {:?}", cfg.debug);
			println!("cfg alpha: {:?}", cfg.alpha);
			println!("cfg beta: {:?}", cfg.beta);
			println!("cfg release: {:?}", cfg.release);

			let sign = Keypair::new();
			println!("New signature private key: {}", sign);
			println!("New signature public key: {}", sign.public());

			return Ok(())
		},
		None => {}
	}

	server::serve(&args.tracing, &args.config).await
}

