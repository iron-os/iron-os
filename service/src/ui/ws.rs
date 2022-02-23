
use super::Data;

use fire::ws_route;
use fire::ws::Error;

ws_route!{
	MainWs, "/onopenpage", |ws, api| -> Result<(), Error> {
		loop {
			let url = api.on_open_page().await;
			ws.send(url).await?;
		}
	},
	|result| {
		if let Err(e) = result {
			eprintln!("websocket error {:?}", e);
		}
	}
}
