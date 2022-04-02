
use crate::Bootloader;
use crate::packages::Packages;
use crate::context;

use std::io;
use std::os::unix::fs::PermissionsExt;

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

const CMD: &str = include_str!("start_process.templ");
const SERVICE_PACKAGE: &str = "/data/packages/service";

pub async fn start(packages: Packages, client: Bootloader) -> io::Result<()> {

	let (on_run_dir, on_run_binary) = packages.on_run_binary().await
		.expect("no on run binary found");
	eprintln!("starting binary {:?}", on_run_binary);

	// create file start-subprocess.sh
	// in /data/packages/service/

	let mut cmd = CMD.replace("CURRENT_DIR", &on_run_dir)
		.replace("BINARY", &on_run_binary);

	if context::is_headless() {
		cmd = cmd.replace("MORE_CMD", "export HEADLESS=\"yes\"");
	} else {
		cmd = cmd.replace("MORE_CMD", "");
	}

	// create start script
	let mut script = File::create(
		format!("{}/start-subprocess.sh", SERVICE_PACKAGE)
	).await?;
	script.write_all(cmd.as_bytes()).await?;
	script.flush().await?;
	let mut permission = script.metadata().await?.permissions();
	permission.set_mode(0o755);
	script.set_permissions(permission).await?;
	// this is done to free start.sh so the service can start chromium
	drop(script);

	// now restart service
	client.systemd_restart("subprocess").await
		.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

	Ok(())
}