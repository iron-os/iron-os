//! ## Perfomance
//! there are probably multiple places where performance could be imporved
//! but that would probably not do a big difference.
//! Since this code runs on not that limited hardware
//!

#[macro_use]
mod util;
mod api;
mod bootloader;
mod context;
mod display;
mod packages;
mod subprocess;
mod ui;

use bootloader::Bootloader;

#[tokio::main]
async fn main() {
	unsafe {
		context::init();
	}

	// initialize api
	let bootloader = Bootloader::new();

	let (ui_api_tx, ui_api_rx) = ui::api_new();

	// if we are in debug only start the ui
	if context::is_debug() {
		eprintln!("Service started in Debug context, chromium will not start");
		if !cfg!(debug_assertions) {
			// only show address in release since in debug fire-http displays it
			eprintln!("Access the page via 127.0.0.1:8888");
		}

		ui_api_tx.open_page("http://127.0.0.1:8080".into());

		// start the ui
		let ui_bg_task = ui::start(bootloader, ui_api_rx)
			.await
			.expect("ui start failed");

		ui_bg_task.await.expect("ui task failed");

		return;
	}

	// start the ui
	let ui_bg_task = ui::start(bootloader.clone(), ui_api_rx)
		.await
		.expect("ui start failed");

	// start packages api
	let (packages, packages_bg_task) = packages::start(bootloader.clone())
		.await
		.expect("packages failed");

	// start display api
	let (display_bg_task, display) = display::start();

	// start service api
	let service_bg_task = crate::api::start(
		bootloader.clone(),
		ui_api_tx,
		display,
		packages.clone(),
	)
	.await
	.expect("service api failed");

	// detect what package should be run
	// and run it
	subprocess::start(packages, bootloader)
		.await
		.expect("failed to start subprocess");

	// now wait until some task fails and restart
	let (_ui, _packages, _api, _display) = tokio::try_join!(
		ui_bg_task,
		packages_bg_task,
		service_bg_task,
		display_bg_task
	)
	.expect("some task failed");
}
