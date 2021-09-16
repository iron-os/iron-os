
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
mod download;
mod pack_image;

use upload::Upload;
use download::Download;
use pack_image::PackImage;

use std::process;

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
	Download(Download),
	PackImage(PackImage)
}

#[tokio::main]
async fn main() {

	let opts = Opts::parse();

	let r = match opts.subcmd {
		SubCommand::Upload(u) => {
			upload::upload(u).await
		},
		SubCommand::Download(d) => {
			download::download(d).await
		},
		SubCommand::PackImage(p) => {
			pack_image::pack_image(p).await
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
