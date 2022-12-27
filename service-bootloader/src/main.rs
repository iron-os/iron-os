mod command;
mod service;
mod disks;
mod version_info;
mod util;
mod hardware_fixes;

#[cfg(not(feature = "headless"))]
use command::Command;
use hardware_fixes::hardware_fixes;

use std::{io, fs};
use std::error::Error as StdError;
use std::thread;
use std::time::Duration;

// get's started as root
fn main() {

	hardware_fixes();

	#[cfg(not(feature = "headless"))]
	{
		Command::new("systemctl")
			.args(&["start", "weston"])
			.exec()
			.expect("could not start weston");
	}

	// make sure /data is owned by the user
	if let Ok(f) = fs::File::open("/data") {
		let _ = util::chown(&f, 14, 15);
	}

	// let weston startup
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
where E: Into<Box<dyn StdError + Send + Sync>> {
	io::Error::new(io::ErrorKind::Other, e)
}