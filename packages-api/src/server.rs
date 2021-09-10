
use crate::message::Action;
use crate::error::{Result, Error};

use std::time::Duration;
use std::any::Any;

use stream::basic::{
	self,
	server::RequestHandler
};
use stream::packet::EncryptedBytes;

use crypto::signature::Keypair;

use tokio::net::{TcpListener, ToSocketAddrs};

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);

type BasicServer = basic::Server<Action, EncryptedBytes, TcpListener, Keypair>;

pub struct Server {
	inner: BasicServer
}

impl Server {

	pub async fn new<A>(addr: A, priv_key: Keypair) -> Result<Self>
	where A: ToSocketAddrs {
		let listener = TcpListener::bind(addr).await
			.map_err(Error::io)?;

		Ok(Self {
			inner: BasicServer::new(listener, TIMEOUT, priv_key)
		})
	}

	pub fn register_request<H>(&mut self, handler: H)
	where H: RequestHandler<Action, EncryptedBytes> + Send + Sync + 'static {
		self.inner.register_request(handler);
	}

	pub fn register_data<D>(&mut self, data: D)
	where D: Any + Send + Sync {
		self.inner.register_data(data);
	}

	/// Panics if one of the request handlers panics
	pub async fn run(self) -> Result<()> {
		self.inner.run().await
			.map_err(Error::Stream)
	}

}