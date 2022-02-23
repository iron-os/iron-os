
mod chromium;
mod ws;

use crate::context;
use crate::Bootloader;

use std::io;
use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio::sync::watch;

use fire::{data_struct, static_file, static_files};

// start chromium and the server manually
// but then return a task which contains the serevr
pub async fn start(
	client: Bootloader,
	receiver: ApiReceiver
) -> io::Result<JoinHandle<()>> {

	// let's first start the server and then chromium
	// so when chromes loads the page already exists
	let server = start_server(8888, client.clone(), receiver);

	// only start chromium if we have a context
	if context::get().is_release() {
		chromium::start("http://127.0.0.1:8888", &client).await?;
	}

	Ok(server)
}

static_file!(Index, "/" => "./www/index.html");

static_files!(Js, "/js" => "./www/js");
static_files!(Css, "/css" => "./www/css");

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
		bootloader: Bootloader,
		api: ApiReceiver
	}
}

pub fn api_new() -> (ApiSender, ApiReceiver) {
	let (p_tx, p_rx) = watch::channel(String::new());

	(
		ApiSender {
			page: Arc::new(p_tx)
		},
		ApiReceiver {
			page: p_rx
		}
	)
}

#[derive(Debug, Clone)]
pub struct ApiSender {
	page: Arc<watch::Sender<String>>
}

impl ApiSender {
	pub fn open_page(&self, url: String) {
		self.page.send(url).expect("ui api receiver closed")
	}
}

#[derive(Debug, Clone)]
pub struct ApiReceiver {
	page: watch::Receiver<String>
}

impl ApiReceiver {
	pub fn open_page(&self) -> String {
		self.page.borrow().clone()
	}

	pub async fn on_open_page(&mut self) -> String {
		self.page.changed().await.expect("ui sender closed");
		self.open_page()
	}
}

pub fn start_server(
	port: u16,
	bootloader: Bootloader,
	receiver: ApiReceiver
) -> JoinHandle<()> {

	let data = Data {
		bootloader,
		api: receiver
	};


	let mut server = fire::build(("127.0.0.1", port), data)
		.expect("address not parseable");

	server.add_route(Index::new());
	server.add_route(Js::new());
	server.add_route(Css::new());
	server.add_raw_route(ws::MainWs);

	tokio::spawn(async move {
		server.light().await
			.expect("lighting server failed")
	})
}