use crate::action::Action;
use crate::error::{Result, Error};

use std::time::Duration;
use std::any::Any;

use stream_api::request::RequestHandler;
pub use stream_api::server::{Session, Config, EncryptedBytes};
pub use stream::handler::Configurator;

use crypto::signature::Keypair;

use tokio::net::{TcpListener, ToSocketAddrs};

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);
const BODY_LIMIT: u32 = 4096;// 4kb request limit

type StreamServer = stream_api::server::Server<
	Action, EncryptedBytes, TcpListener, Keypair
>;

pub struct Server {
	inner: StreamServer
}

impl Server {
	pub async fn new<A>(addr: A, priv_key: Keypair) -> Result<Self>
	where A: ToSocketAddrs {
		let listener = TcpListener::bind(addr).await
			.map_err(|e| Error::Other(format!("could not bind {}", e)))?;

		Ok(Self {
			inner: StreamServer::new_encrypted(listener, Config {
				timeout: TIMEOUT,
				body_limit: BODY_LIMIT
			}, priv_key)
		})
	}

	pub fn register_request<H>(&mut self, handler: H)
	where H: RequestHandler<EncryptedBytes, Action=Action> + Send + Sync + 'static {
		self.inner.register_request(handler);
	}

	pub fn register_data<D>(&mut self, data: D)
	where D: Any + Send + Sync {
		self.inner.register_data(data);
	}

	/// Panics if one of the request handlers panics
	pub async fn run(self) -> Result<()> {
		self.inner.run().await
			.map_err(|e| Error::Other(format!("server failed {}", e)))
	}
}
