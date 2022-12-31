#[macro_use]
mod error;
mod util;
mod script;
mod upload;
mod download;
mod pack_image;
mod config;
mod auth;
mod change_whitelist;

use upload::Upload;
use download::Download;
use pack_image::PackImage;
use config::ConfigOpts;
use auth::AuthOpts;
use change_whitelist::ChangeWhitelistOpts;

use std::process;

use riji::paint_err;

use clap::Parser;


#[derive(Parser)]
#[clap(version = "0.1")]
struct Opts {
	#[clap(subcommand)]
	subcmd: SubCommand
}

#[derive(clap::Parser)]
enum SubCommand {
	Upload(Upload),
	Download(Download),
	PackImage(PackImage),
	Config(ConfigOpts),
	Auth(AuthOpts),
	ChangeWhitelist(ChangeWhitelistOpts)
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
		},
		SubCommand::Config(opts) => {
			config::configure(opts).await
		},
		SubCommand::Auth(opts) => {
			auth::authenticate(opts).await
		},
		SubCommand::ChangeWhitelist(opts) => {
			change_whitelist::change_whitelist(opts).await
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
