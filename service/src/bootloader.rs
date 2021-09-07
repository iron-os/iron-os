
use std::io;
use std::sync::Arc;

use bootloader_api::{AsyncClient, Request};

use tokio::sync::Mutex;


#[derive(Clone)]
pub struct Bootloader {
	inner: Arc<Mutex<AsyncClient>>
}

impl Bootloader {

	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(AsyncClient::new()))
		}
	}

	pub async fn request<R>(&self, req: &R) -> io::Result<R::Response>
	where R: Request {
		let mut client = self.inner.lock().await;
		client.request(req).await
	}

}