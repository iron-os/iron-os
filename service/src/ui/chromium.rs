
use crate::Bootloader;

use std::{io, env};
use std::os::unix::fs::PermissionsExt;

use tokio::fs::File;
use tokio::io::{AsyncWriteExt};

use file_db::FileDb;

use bootloader_api::{SystemdRestart, MakeRoot};

use packages::packages::PackageCfg;

const CMD: &str = include_str!("start_chrome.templ");
const CHROME_PACKAGE: &str = "/data/packages/chromium";

// the url needs https or http
pub async fn start(url: &str, client: &Bootloader) -> io::Result<()> {
	// do we need to setsid?
	// so chrome closes if this process closes??

	let package_cfg: PackageCfg = FileDb::open(
		format!("{}/package.fdb", CHROME_PACKAGE)
	).await?.into_data();
	let package = package_cfg.package();
	let curr_path = format!("{}/{}", CHROME_PACKAGE, package_cfg.current());
	let bin_path = format!("{}/{}", curr_path, package.binary.as_ref().unwrap());

	let my_curr_path = env::current_dir()?
		.into_os_string().into_string()
		.map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid package path"))?;
	let extension_path = format!("{}/{}", my_curr_path, "extension");


	// todo: there should be a way to not display the: out of storage
	// message

	// this is not really efficient
	let cmd = CMD.replace("CURRENT_DIR", &curr_path)
		.replace("BINARY", &bin_path)
		.replace("URL", url)
		.replace("EXTENSION", &extension_path);

	// create start script
	let mut script = File::create(format!("{}/start.sh", CHROME_PACKAGE)).await?;
	script.write_all(cmd.as_bytes()).await?;
	script.flush().await?;
	let mut permission = script.metadata().await?.permissions();
	permission.set_mode(0o755);
	script.set_permissions(permission).await?;
	// this is done to free start.sh so the service can start chromium
	drop(script);

	// now make chrome-sandbox root
	client.request(&MakeRoot {
		path: format!("{}/chrome-sandbox", curr_path)
	}).await?;

	// now restart service
	client.request(&SystemdRestart { name: "chromium".into() }).await?;

	Ok(())
}