
mod chromium;

use std::io;

use tokio::task::JoinHandle;

use bootloader_api::AsyncClient;
use fire::{static_file, static_files};

// start chromium and the server manually
// but then return a task which contains the serevr
pub async fn start(client: &mut AsyncClient) -> io::Result<JoinHandle<()>> {

	chromium::start("http://127.0.0.1:8888", client).await?;

	Ok(start_server(8888))
}



static_file!(Index, "/" => "./www/index.html");

static_files!(Js, "/js" => "./www/js");

static_files!(FireHtml, "/fire-html" => "./www/fire-html");

pub fn start_server(port: u16) -> JoinHandle<()> {
	let mut server = fire::build(("127.0.0.1", port), ())
		.expect("address not parseable");

	server.add_route(Index::new());
	server.add_route(Js::new());
	server.add_route(FireHtml::new());

	tokio::spawn(async move {
		server.light().await
			.expect("lighting server failed")
	})
}