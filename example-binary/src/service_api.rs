
use std::sync::Arc;

use service_api::error::Result;
use service_api::client::Client as ApiClient;

use service_api::requests::ui::OpenPageReq;
use service_api::requests::device::{
	DisksReq, Disk,
	DeviceInfoReq, DeviceInfo
};
use service_api::requests::system::InstallOnReq;

/// This does not reconnect, since if the connection closes we expect
/// to be restarted
#[derive(Clone)]
pub struct Client {
	inner: Arc<ApiClient>
}

impl Client {
	pub async fn connect() -> Result<Self> {
		let client = ApiClient::connect("/data/service-api").await?;
		Ok(Self {
			inner: Arc::new(client)
		})
	}

	pub async fn open_page(&self, url: impl Into<String>) -> Result<()> {
		self.inner.request(OpenPageReq { url: url.into() }).await
			.map(|_| ())
	}

	pub async fn list_disks(&self) -> Result<Vec<Disk>> {
		self.inner.request(DisksReq).await
			.map(|d| d.0)
	}

	pub async fn install_on(&self, disk: String) -> Result<()> {
		self.inner.request(InstallOnReq { name: disk }).await
			.map(|_| ())
	}

	pub async fn device_info(&self) -> Result<DeviceInfo> {
		self.inner.request(DeviceInfoReq).await
	}
}