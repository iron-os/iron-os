
use crate::{Bootloader, context};

use std::{io, env, fmt};
use std::os::unix::fs::PermissionsExt;

use tokio::fs::File;
use tokio::io::{AsyncWriteExt};
use tokio::time::{sleep, Duration};

use file_db::FileDb;

use packages::packages::PackageCfg;

const CMD: &str = include_str!("start_chrome.templ");
const CHROME_PACKAGE: &str = "/data/packages/chromium";

fn io_other(s: impl fmt::Display) -> io::Error {
	io::Error::new(io::ErrorKind::Other, s.to_string())
}

macro_rules! write_arg {
	($s:expr, $($tt:tt)*) => (
		{
			use std::fmt::Write;
			write!($s, $($tt)*).unwrap();
			write!($s, " ").unwrap();
		}
	)
}

// the url needs https or http
pub async fn start(url: &str, client: &Bootloader) -> io::Result<()> {

	let package_cfg = FileDb::<PackageCfg>::open(
		format!("{}/package.fdb", CHROME_PACKAGE)
	).await?.into_data();
	let package = package_cfg.package();
	let curr_path = format!("{}/{}", CHROME_PACKAGE, package_cfg.current());
	let binary = package.binary.as_ref()
		.expect("chromium package does not have a binary");
	let bin_path = format!("{}/{}", curr_path, binary);

	let my_curr_path = env::current_dir()?
		.into_os_string().into_string()
		.map_err(|_| io_other("invalid package path"))?;
	let extension_path = format!("{}/{}", my_curr_path, "extension");

	let mut args = String::new();
	write_arg!(args, "--disable-infobars");
	write_arg!(args, "--disable-restore-session-state");
	write_arg!(args, "--disable-session-storage");
	write_arg!(args, "--disable-rollback-option");
	write_arg!(args, "--disable-speech-api");
	write_arg!(args, "--disable-sync");
	write_arg!(args, "--disable-pinch");
	write_arg!(args, "--kiosk \"{}\"", url);
	write_arg!(args, "--load-extension=\"{}\"", extension_path);

	// todo: there should be a way to not display the: out of storage message

	if context::is_image_debug() {
		write_arg!(args, "--remote-debugging-port=9222");
	};

	let product = client.version_info().await
		.map_err(|e| io_other(format!("failed to get version info {:?}", e)))?
		.product;

	if product == "sputnik" {
		// this fixes an issue we had where the display could freeze up
		// will be removed after
		// https://bugs.chromium.org/p/chromium/issues/detail?id=1336078 gets
		// resolved
		write_arg!(args, "--disable-gpu");
	}

	let cmd = CMD.replace("CURRENT_DIR", &curr_path)
		.replace("BINARY", &bin_path)
		.replace("ARGS", &args);

	// create start script
	{
		let mut script = File::create(format!("{}/start.sh", CHROME_PACKAGE)).await?;
		script.write_all(cmd.as_bytes()).await?;
		script.flush().await?;
		let mut permission = script.metadata().await?.permissions();
		permission.set_mode(0o755);
		script.set_permissions(permission).await?;
		// drop script
		// this is done to free start.sh so the service can start chromium
	}

	// now make chrome-sandbox root
	client.make_root(format!("{}/chrome-sandbox", curr_path)).await
		.map_err(|e| io_other(format!("make root failed {:?}", e)))?;

	// now restart service
	client.systemd_restart("chromium").await
		.map_err(|e| io_other(format!("could not restart chromium {:?}", e)))?;

	// wait until chromium is loaded (does probably not take that long)
	sleep(Duration::from_millis(400)).await;

	Ok(())
}