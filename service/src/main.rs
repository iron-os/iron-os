
mod ui;
mod context;
mod bootloader;
mod packages;
mod util;

use context::Context;
use bootloader::Bootloader;

use std::env;

#[tokio::main]
async fn main() {

	let mut args = env::args();
	let _ = args.next();
	let ctx = args.next();
	if matches!(ctx, Some(a) if a == "debug") || cfg!(debug_assertions) {
		unsafe {
			// safe because multithreading hasn't started
			context::set(Context::Debug);
		}
	}

	// initialize api
	let bootloader = Bootloader::new();

	// if we are in debug only start the ui
	if context::get().is_debug() {
		eprintln!("Service started in Debug context, chromium will not start");
		if !cfg!(debug_assertions) {
			// only show address in release since else
			// fire displays it
			eprintln!("Access the page via 127.0.0.1:8888");
		}

		// start the ui
		let ui_bg_task = ui::start(bootloader).await
			.expect("ui start failed");

		ui_bg_task.await.expect("ui task failed");

		return;
	}


	// start the ui
	let ui_bg_task = ui::start(bootloader.clone()).await
		.expect("ui start failed");

	// start packages api
	let packages_bg_task = packages::start(bootloader.clone()).await
		.expect("packages failed");

	// start service api


	// detect what package should be run
	// and run it

	// now wait until some task fails and restart
	let (_ui, _packages) = tokio::try_join!(
		ui_bg_task,
		packages_bg_task
	).expect("some task failed");



/*
## Chnobli service

- start chnobli_ui (or chnobli_shell)
- start chromium
- start chnobli_packages
- maybe need chromium debug protocol (to be able to log console.logs warnings etc)

- api to start other packages
- api to interact with ui (reset, show display)

- start installer if not installed

- start chnobli_core
- start frame package
*/

}