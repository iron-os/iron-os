use super::Data;

use fire::ws::{Error, Message};
use fire::ws_route;

ws_route! {
	MainWs, "/service-stream", |ws, api, web_watchdog| -> Result<(), Error> {
		loop {
			tokio::select! {
				msg = ws.receive() => {
					let msg = msg?;
					match msg {
						Some(Message::Text(t)) if t == "StillAlive" => {
							web_watchdog.set_still_alive();
						},
						Some(m) => {
							eprintln!("unknown ws service-stream msg {:?}", m);
						},
						None => return Ok(())
					}
				},
				// this will always trigger the first time
				// because ws route clones the ApiReceiver and
				// nobody previously read from it
				// and since this is a copy nobody will ever
				// read from the api in data
				url = api.on_open_page() => {
					ws.send(url).await?;
				}
			}
		}
	},
	|result| {
		if let Err(e) = result {
			eprintln!("websocket service-stream error {:?}", e);
		}
	}
}
