
use super::WsData;

use bootloader_api::{VersionInfoReq, Disks, InstallOn};
use super::ws_api::{self, ConnectionBuilder, Connection, Name, Result};

// version info
msg_handler!( for Name::VersionInfo,
	async fn version_info<WsData>(_req: String, sender, bootloader) -> Result<()> {
		let r = bootloader.request(&VersionInfoReq).await
			.map_err(ws_api::Error::Io)?;
		sender.send(r).await?;
		Ok(())
	}
);

msg_handler!( for Name::Disks,
	async fn disks<WsData>(_req: String, sender, bootloader) -> Result<()> {
		let r = bootloader.request(&Disks).await
			.map_err(ws_api::Error::Io)?;
		sender.send(r).await?;
		Ok(())
	}
);

msg_handler!( for Name::InstallOn,
	async fn install_on<WsData>(req: String, sender, bootloader) -> Result<()> {
		let req = InstallOn { name: req };
		bootloader.request(&req).await
			.map_err(ws_api::Error::Io)?;
		sender.send(true).await?;
		Ok(())
	}
);

pub fn build() -> Connection<WsData> {
	let mut builder = ConnectionBuilder::new();
	builder.register(version_info);
	builder.register(disks);
	builder.register(install_on);
	builder.build()
}

route!(
	MainWs<super::Data>, "/websocket", ws_data, ws_con
);