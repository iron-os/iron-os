
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {

	// initialize api

/*
## Chnobli service

- start chnobli_ui (or chnobli_shell)
- start chromium
- start chnobli_packages
- send logs
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