
mod chromium;

use std::io;

use tokio::task::JoinHandle;

use bootloader_api::AsyncClient;
use fire::static_files;

// start chromium and the server manually
// but then return a task which contains the serevr
pub async fn start(client: &mut AsyncClient) -> io::Result<JoinHandle<()>> {

	chromium::start("https://127.0.0.1:8888", client).await?;

	Ok(start_server(8888))
}



static_files!(Index, "/" => "./www/index.html");


pub fn start_server(port: u16) -> JoinHandle<()> {
	let server = fire::build(("127.0.0.1", port), ())
		.expect("address not parseable");

	server.add_route(Index);

	tokio::spawn(async move {
		server.light().await
			.expect("lighting server failed")
	})
}