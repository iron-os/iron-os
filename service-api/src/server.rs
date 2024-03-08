use crate::error::{Error, Result};
use crate::Action;

use std::any::Any;
use std::path::Path;
use std::time::Duration;

use stream::packet::PlainBytes;
use stream::server::Config;
use stream_api::request::RequestHandler;
use stream_api::server::{self};

use tokio::net::UnixListener;

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);

type ApiServer = server::Server<Action, PlainBytes, UnixListener, ()>;

pub struct Server {
	inner: ApiServer,
}

impl Server {
	pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
		let listener = UnixListener::bind(path)
			.map_err(|e| Error::Internal(e.to_string()))?;

		Ok(Self {
			inner: ApiServer::new(
				listener,
				Config {
					timeout: TIMEOUT,
					body_limit: 0,
				},
			),
		})
	}

	pub fn register_request<H>(&mut self, handler: H)
	where
		H: RequestHandler<PlainBytes, Action = Action> + Send + Sync + 'static,
	{
		self.inner.register_request(handler);
	}

	pub fn register_data<D>(&mut self, data: D)
	where
		D: Any + Send + Sync,
	{
		self.inner.register_data(data);
	}

	/// Panics if one of the request handlers panics
	pub async fn run(self) -> Result<()> {
		self.inner
			.run()
			.await
			.map_err(|e| Error::Internal(e.to_string()))
	}
}
