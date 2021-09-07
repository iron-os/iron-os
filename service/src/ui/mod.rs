
mod chromium;
mod ws;

use crate::context;
use crate::Bootloader;

use std::io;

use tokio::task::JoinHandle;

use fire::{data_struct, static_file, static_files};

// start chromium and the server manually
// but then return a task which contains the serevr
pub async fn start(client: Bootloader) -> io::Result<JoinHandle<()>> {

	// only start chromium if we have a context
	if context::get().is_release() {
		chromium::start("http://127.0.0.1:8888", &client).await?;
	}

	Ok(start_server(8888, client))
}



static_file!(Index, "/" => "./www/index.html");

static_files!(Js, "/js" => "./www/js");

static_files!(FireHtml, "/fire-html" => "./www/fire-html");


/*
struct Message {
	id: RandomToken,
	kind: Request|Push|Response,
	name: String, // the name that identifiers this message
				// for example DisksInfo
				// or InstallTo
	data: T
}
*/


/*
receive

*/

data_struct!{
	pub struct Data {
		ws_con: ws_api::Connection<WsData>,
		ws_data: WsData
	}
}

impl Data {
	pub fn bootloader(&self) -> &Bootloader {
		&self.ws_data.bootloader
	}
}

data_struct!{
	#[derive(Clone)]
	pub struct WsData {
		bootloader: Bootloader
	}
}



pub fn start_server(port: u16, bootloader: Bootloader) -> JoinHandle<()> {

	let data = Data {
		ws_con: ws::build(),
		ws_data: WsData {
			bootloader
		}
	};


	let mut server = fire::build(("127.0.0.1", port), data)
		.expect("address not parseable");

	server.add_route(Index::new());
	server.add_route(Js::new());
	server.add_route(FireHtml::new());
	server.add_raw_route(ws::MainWs);

	// tokio::spawn(async move {
	// 	ws::handle_ws_messages(con_listener).await
	// 		.expect("todo should this be retried");
	// });

	tokio::spawn(async move {
		server.light().await
			.expect("lighting server failed")
	})
}

