
use super::Data;

use fire::ws_route;
use fire::ws::{Message, Error};

ws_route!{
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
