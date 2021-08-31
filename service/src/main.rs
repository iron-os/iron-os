
mod ui;

use tokio::time::{sleep, Duration};
use stdio_api::AsyncStdio;

#[tokio::main]
async fn main() {

	// initialize api
	let stdio = AsyncStdio::from_env();

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

	for i in 0..20 {
		eprintln!("hey");

		sleep(Duration::from_secs(30)).await;

		println!(":>:Hi some data");
	}

}