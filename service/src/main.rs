
mod ui;

use bootloader_api::AsyncClient;

#[tokio::main]
async fn main() {

	// initialize api
	let mut client = AsyncClient::new();

	// start the ui
	let ui_bg_task = ui::start(&mut client).await
		.expect("ui start failed");

	ui_bg_task.await.expect("ui task failed");


	// start basic ui


	// then we either need to start the next package
	// or we start the service installer
	// run factory tests









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