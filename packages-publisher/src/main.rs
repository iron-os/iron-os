
// what can we do?
/*

upload package info from package.toml

download packages folder for image from packages.toml

*/

mod upload;

use upload::Upload;

use packages::packages::Channel;
use clap::{AppSettings, Clap};

pub struct PackageToml {
	pub name: String,
	pub version_str: String,
	pub binary: Option<String>,
	/// Default is package.rhai
	pub script: Option<String>
}



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

	match opts.subcmd {
		SubCommand::Upload(u) => {
			upload::upload(u).await
		},
		SubCommand::Download(d) => {
			todo!("download")
		}
	}

}
