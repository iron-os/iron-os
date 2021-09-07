
use super::WsData;

use bootloader_api::VersionInfoReq;
use ws_api::{route, ConnectionBuilder, Connection, msg_handler, Name, Result};

// version info
msg_handler!( for Name::VersionInfo,
	async fn version_info<WsData>(req: String, sender, bootloader) -> Result<()> {
		println!("requested");
		let r = bootloader.request(&VersionInfoReq).await
			.map_err(ws_api::Error::Io)?;
		println!("received Version info {:?}", r);
		sender.send(r).await?;
		Ok(())
	}
);

pub fn build() -> Connection<WsData> {
	let mut builder = ConnectionBuilder::new();
	builder.register(version_info);
	builder.build()
}

route!(
	MainWs<super::Data>, "/websocket", ws_data, ws_con
);