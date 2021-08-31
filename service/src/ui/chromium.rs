
use std::io;
use std::path::Path;

use tokio::task::JoinHandle;
use tokio::process::Command;
use tokio::fs;

use serde::Deserialize;
use file_db::FileDb;

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
	current: String,
	binary: String
	// we don't care about all other fields
}

const CMD: &str = include_str!("start_chrome.templ");
const CHROME_PACKAGE: &str = "/data/packages/chromium";
const START_SCRIPT: &str = concat!(CHROME_PACKAGE, "/start.sh");

// the url needs https or http
pub fn start(url: impl Into<String>) -> io::Result<()> {
	// do we need to setsid?
	// so chrome closes if this process closes??

	let package: Package = FileDb::open(
		concat!(CHROME_PACKAGE, "/package.fdb")
	).await?.into_data();
	let curr_path = format!("{}/{}", CHROME_PACKAGE, package.current);
	let bin_path = format!("{}/{}", CHROME_PACKAGE, package.binary);


	// todo: there should be a way to not display the out of storage
	// message

	// this is not really efficient
	let cmd = CMD.replace("CURRENT_DIR", &curr_path)
		.replace("BINARY", &bin_path)
		.replace("URL", url);

	// start script
	let script = OpenOptions::new()
		.write(true)
		.truncate(true)
		.open(START_SCRIPT)
		.await?;
	script.write_all(cmd).await?;
	script.flush().await?;
	let mut permission = script.metadata().await?.permissions();
	permission.set_mode(0o755);
	script.set_permissions(permission).await?;

	// now restart service

	Ok(())
}

/*
Imago | Pictura | 
*/

// async fn start_async(url: &str) -> io::Result<()> {



// 	let status = Command::new(bin_path)
// 		.current_dir(curr_path)
// 		.env("XDG_RUNTIME_DIR", "/run/user/14")
// 		.env("WAYLAND_DISPLAY", "wayland-0")
// 		// just a security measure
// 		.kill_on_drop(true)
// 		.args(&[
// 			// set all folders to the tmp
// 			"--disk-cache-dir=/tmp/",
// 			"--user-profile=/tmp/",
// 			"--disable-infobars",
// 			"--disable-rollback-option",
// 			"--disable-speech-api",
// 			"--disable-sync",
// 			"--disable-pinch",
// 			"--kiosk"
// 			// --disable-restore-session-state --disable-session-storage
// 		])
// 		.arg(&format!("--app=\"{}\"", url))
// 		// todo add extension loading
// 		.status().await?;

// 	status.success().then(|| ())
// 		.ok_or_else(|| io::Error::new(io::ErrorKind::Other, "chromium: exit status non zero"))
// }


/*


to start chromium a bash script would be created
which contains all arguments and the location of chromium
which then is started via a systemd service


make a service which

Requires=weston.service
BindsTo=weston.service
After=weston.serivice


# this should now restart if weston failes
Restarts=