
use crate::Bootloader;
use crate::ui::{ApiSender};
use crate::util::io_other;

use std::io;

use api::server::Server;
use api::requests::{OpenPageReq, OpenPage};
use api::request_handler;
use api::error::{Result as ApiResult};

use tokio::task::JoinHandle;

use tokio::fs;


pub async fn start(
	client: Bootloader,
	ui_api: ApiSender
) -> io::Result<JoinHandle<()>> {

	// since there is only one instance of service running this is fine
	let _ = fs::remove_file("/data/service-api").await;

	let mut server = Server::new("/data/service-api").await
		.map_err(io_other)?;
	server.register_data(ui_api);
	server.register_data(client);
	server.register_request(open_page);

	Ok(tokio::spawn(async move {
		server.run().await
			.expect("could not run api server")
	}))
}

request_handler!(
	async fn open_page(
		req: OpenPageReq,
		ui_api: ApiSender
	) -> ApiResult<OpenPage> {
		ui_api.open_page(req.url);
		Ok(OpenPage)
	}
);