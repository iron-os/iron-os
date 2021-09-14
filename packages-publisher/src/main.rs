
// what can we do?
/*

upload package info from package.toml

download packages folder for image from packages.toml

*/

#[macro_use]
mod error;
mod util;
mod script;
mod upload;

use upload::Upload;

use std::process;

use packages::packages::Channel;
use clap::{AppSettings, Clap};

use riji::paint_err;


#[derive(Clap)]
#[clap(version = "0.1")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
	#[clap(subcommand)]
	subcmd: SubCommand
}

#[derive(Clap)]
enum SubCommand {
	Upload(Upload),
	Download(Download)
}

/// Downloads and fills a full packages folder
/// with the packages listed in `packages.toml`
/// the address and the channel should be in `packages.toml`
#[derive(Clap)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Download {}


#[tokio::main]
async fn main() {

	let opts = Opts::parse();

	let r = match opts.subcmd {
		SubCommand::Upload(u) => {
			upload::upload(u).await
		},
		SubCommand::Download(d) => {
			todo!("download")
		}
	};

	match r {
		Ok(_) => {},
		Err(e) => {
			paint_err!("{}", e.description);
			eprintln!("{:?}", e.error);
			process::exit(1);
		}
	}

}
