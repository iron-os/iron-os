
use crate::Bootloader;
use crate::packages::Packages;

use std::io;
use std::os::unix::fs::PermissionsExt;

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use bootloader_api::SystemdRestart;

const CMD: &str = include_str!("start_process.templ");
const SERVICE_PACKAGE: &str = "/data/packages/service";

pub async fn start(client: Bootloader) -> io::Result<()> {

	let packages = Packages::load().await?;

	let (on_run_dir, on_run_binary) = packages.on_run_binary()
		.expect("no on run binary found");
	eprintln!("starting binary {:?}", on_run_binary);

	// create file start-subprocess.sh
	// in /data/packages/service/

	let cmd = CMD.replace("CURRENT_DIR", &on_run_dir)
		.replace("BINARY", &on_run_binary);

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
	client.request(&SystemdRestart { name: "subprocess".into() }).await?;

	Ok(())
}