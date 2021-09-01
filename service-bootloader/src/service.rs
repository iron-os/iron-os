
use crate::command::Command;
use crate::io_other;

use std::io;
use std::path::Path;

use bootloader_api::{Server, Request};
use file_db::FileDb;

use serde::Deserialize;



#[derive(Debug, Clone, Deserialize)]
pub struct Package {
	current: String,
	binary: String
	// we don't care about all other fields
}


// open packages folder
// and then open the folder
// service

pub fn start() -> io::Result<()> {

	/*
## Chnobli service bootloader

- manage pssplash
- start chnobli service package

- supports api via stdin
 - can switch boot img
 - update images from img file
 - can restart
 - watchdog for chnobli service
	 restart if chnobli service does not send
	 connected for a given period
 - start weston service
 - api for setuid root
*/

	let service_package = Path::new("/data/packages/service");
	let package_file = service_package.join("package.fdb");
	let package: Package = FileDb::open_sync(package_file)?.into_data();
	let curr_path = service_package.join(&package.current);
	let bin_path = curr_path.join(&package.binary);

	let mut child = Command::new(bin_path)
		.current_dir(curr_path)
		.as_user()
		.stdin_piped()
		.stdout_piped()
		.spawn()?;
	child.kill_on_drop(true);

	// todo kill the process if the child is dropped

	let mut server = Server::new(&mut child)
		.ok_or_else(|| io_other("could not get stdin or stdout"))?;

	while let Some(req) = server.receive()? {
		match req {
			// receiving request to restart a systemd service
			Request::SystemdRestart { name } => {
				Command::new("systemctl")
					.args(&["restart", &name])
					.exec()?;
			},
			// got an unknown request
			// maybe should return something
			e => eprintln!("got req {:?}", e)
		}

		// should handle SystemctlStart service
	}

	let s = child.wait()?;
	s.success().then(|| ())
		.ok_or_else(|| io_other("command exited with non success status"))?;

	Ok(())
}