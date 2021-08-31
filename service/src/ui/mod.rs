
mod chromium;

use tokio::task::JoinHandle;

// todo should add watchdog
pub fn spawn() -> JoinHandle<()> {

	// this function should never exit
	tokio::spawn(async move {
		chromium::start("https://youtube.com").await
			.expect("chromium task failed")
			.expect("chromium not exited correctly");
	})
}