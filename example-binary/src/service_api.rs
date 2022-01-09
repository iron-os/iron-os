
use std::sync::Arc;

use service_api::error::Result;
use service_api::client::Client as ApiClient;

use service_api::requests::ui::OpenPageReq;

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
}