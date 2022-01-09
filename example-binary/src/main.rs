
use tokio::time::{sleep, Duration};

mod service_api;

#[tokio::main]
async fn main() {

	let client = service_api::Client::connect().await
		.expect("could not connect to the service api");

	// wait until a connection could be established
	sleep(Duration::from_secs(30).await;

	// let's open youtube
	client.open_page("https://youtube.com").await
		.expect("could not open youtube");

	loop {
		sleep(Duration::from_secs(5 * 60)).await
	}
}