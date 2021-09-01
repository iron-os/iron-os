
use std::io;
use std::path::Path;
use std::os::unix::fs::PermissionsExt;

use tokio::task::JoinHandle;
use tokio::process::Command;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncWriteExt};

use serde::Deserialize;
use file_db::FileDb;

use bootloader_api::{AsyncClient, Request};

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
	current: String,
	binary: String
	// we don't care about all other fields
}

const CMD: &str = include_str!("start_chrome.templ");
const CHROME_PACKAGE: &str = "/data/packages/chromium";

// the url needs https or http
pub async fn start(url: &str, client: &mut AsyncClient) -> io::Result<()> {
	// do we need to setsid?
	// so chrome closes if this process closes??

	let package: Package = FileDb::open(
		format!("{}/package.fdb", CHROME_PACKAGE)
	).await?.into_data();
	let curr_path = format!("{}/{}", CHROME_PACKAGE, package.current);
	let bin_path = format!("{}/{}", CHROME_PACKAGE, package.binary);


	// todo: there should be a way to not display the: out of storage
	// message

	// this is not really efficient
	let cmd = CMD.replace("CURRENT_DIR", &curr_path)
		.replace("BINARY", &bin_path)
		.replace("URL", url);

	// start script
	let mut script = OpenOptions::new()
		.write(true)
		.truncate(true)
		.open(format!("{}/start.sh", CHROME_PACKAGE))
		.await?;
	script.write_all(cmd.as_bytes()).await?;
	script.flush().await?;
	let mut permission = script.metadata().await?.permissions();
	permission.set_mode(0o755);
	script.set_permissions(permission).await?;

	// now restart service
	client.send(Request::SystemdRestart { name: "chromium".into() }).await?;

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