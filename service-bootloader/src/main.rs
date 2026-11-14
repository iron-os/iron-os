mod command;
mod disks;
mod hardware_fixes;
mod layout;
mod service;
mod util;
mod version_info;
mod fix_10;

#[cfg(not(feature = "headless"))]
use command::Command;
use hardware_fixes::hardware_fixes;

use std::error::Error as StdError;
use std::thread;
use std::time::Duration;
use std::{fs, io};

// get's started as root
fn main() {
	let args: Vec<_> = std::env::args().collect();

	if args.len() >= 2 {
		if args[1] == "version" {
			eprintln!("service-bootloader {}", env!("CARGO_PKG_VERSION"));
			return;
		}
	}

	if args.len() >= 3 {
		if args[1] == "update_image_fix_10" {
			fix_10::update_image_fix_10(&args[2]);
			return
		}
	}

	hardware_fixes();

	// the service has to run even if the compositor doesn't, it installs
	// the updates which fix it, so don't wait for it and don't fail
	#[cfg(not(feature = "headless"))]
	{
		let res = Command::new("systemctl")
			.args(&["start", "--no-block", "weston"])
			.exec();
		if let Err(e) = res {
			eprintln!("could not start the compositor {e}");
		}
	}

	// make sure /data is owned by the user
	if let Ok(f) = fs::File::open("/data") {
		let _ = util::chown(&f, 14, 15);
	}

	// let the compositor start up
	thread::sleep(Duration::from_millis(400));

	// now we need to get the service binary
	loop {
		let e = service::start();
		if let Err(e) = e {
			eprintln!("service error {:?}", e);
		}

		thread::sleep(Duration::from_secs(1));
	}
}

fn io_other<E>(e: E) -> io::Error
where
	E: Into<Box<dyn StdError + Send + Sync>>,
{
	io::Error::new(io::ErrorKind::Other, e)
}
