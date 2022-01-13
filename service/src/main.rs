//! ## Perfomance
//! there are probably multiple places where performance could be imporved
//! but that would probably not do a big difference.
//! Since this code runs on not that limited hardware
//!

#[macro_use]
mod util;
mod ui;
mod context;
mod bootloader;
mod packages;
mod api;
mod subprocess;
mod display;

/// Todo
/// make it possible to run without a ui

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

	let (ui_api_tx, ui_api_rx) = ui::api_new();

	let display = display::Display::new();

	// if we are in debug only start the ui
	if context::get().is_debug() {
		eprintln!("Service started in Debug context, chromium will not start");
		if !cfg!(debug_assertions) {
			// only show address in release since in debug fire-http displays it
			eprintln!("Access the page via 127.0.0.1:8888");
		}

		ui_api_tx.open_page("https://livgood.ch".into());

		// start the ui
		let ui_bg_task = ui::start(bootloader, ui_api_rx).await
			.expect("ui start failed");

		ui_bg_task.await.expect("ui task failed");

		return;
	}

	// start the ui
	let ui_bg_task = ui::start(bootloader.clone(), ui_api_rx).await
		.expect("ui start failed");

	// start packages api
	let (packages, packages_bg_task) = packages::start(bootloader.clone()).await
		.expect("packages failed");

	// start service api
	let service_bg_task = crate::api::start(
		bootloader.clone(),
		ui_api_tx,
		display.clone(),
		packages.clone()
	).await.expect("service api failed");

	// start display api
	let display_bg_task = display::start(display).await;

	// detect what package should be run
	// and run it
	subprocess::start(packages, bootloader).await
		.expect("failed to start subprocess");

	// now wait until some task fails and restart
	let (_ui, _packages, _api, _display) = tokio::try_join!(
		ui_bg_task,
		packages_bg_task,
		service_bg_task,
		display_bg_task
	).expect("some task failed");

}