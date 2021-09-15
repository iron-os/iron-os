
mod command;
mod service;
mod disks;
mod version_info;
mod util;

use command::Command;

use std::io;
use std::error::Error as StdError;
use std::thread;
use std::time::Duration;

// get's started as root
fn main() {

	Command::new("systemctl")
		.args(&["start", "weston"])
		.exec()
		.expect("could not start weston");

	// let weston startup
	thread::sleep(Duration::from_millis(200));

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

// io_comm io error with comments
// fn io_comm<E