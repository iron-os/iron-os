//! This api is unstable and should only be used by packages-server

use crate::action::Action;
use crate::error::{Result, Error};

use std::time::Duration;
use std::any::Any;
use std::sync::Arc;

pub use stream_api::server::{Session, Config, EncryptedBytes};
use stream_api::request::{RequestHandler, Request};
use stream_api::server::{BuiltServer};
use stream_api::message::{IntoMessage, FromMessage};
pub use stream::handler::Configurator;
use stream::util::testing::PanicListener;
use stream::util::{Listener, SocketAddr};

use crypto::signature::Keypair;

use tokio::net::{TcpListener, ToSocketAddrs};

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);
const BODY_LIMIT: u32 = 4096;// 4kb request limit

type StreamServer<L> = stream_api::server::Server<
	Action, EncryptedBytes, L, Keypair
>;

pub struct Server<L> {
	inner: StreamServer<L>
}

impl<L> Server<L> {
	pub fn register_request<H>(&mut self, handler: H)
	where H: RequestHandler<EncryptedBytes, Action=Action> + Send + Sync + 'static {
		self.inner.register_request(handler);
	}

	pub fn register_data<D>(&mut self, data: D)
	where D: Any + Send + Sync {
		self.inner.register_data(data);
	}

	pub fn into_inner(self) -> StreamServer<L> {
		self.inner
	}
}

impl<L> Server<L>
where L: Listener {
	/// Panics if one of the request handlers panics
	pub async fn run(self) -> Result<()> {
		self.inner.run().await
			.map_err(|e| Error::Other(format!("server failed {}", e)))
	}
}

impl Server<TcpListener> {
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
}

impl Server<PanicListener> {
	pub fn new_testing(priv_key: Keypair) -> Self {
		Self {
			inner: StreamServer::new_encrypted(PanicListener::new(), Config {
				timeout: TIMEOUT,
				body_limit: BODY_LIMIT
			}, priv_key)
		}
	}

	pub fn build(self) -> TestingServer {
		TestingServer {
			inner: self.inner.build(),
			session: Arc::new(Session::new(
				SocketAddr::V4("127.0.0.1:8080".parse().unwrap())
			))
		}
	}
}

pub struct TestingServer {
	inner: BuiltServer<Action, EncryptedBytes, PanicListener, Keypair>,
	session: Arc<Session>
}

impl TestingServer {
	pub fn session(&self) -> &Session {
		&self.session
	}

	pub fn reset_session(&mut self) {
		self.session = Arc::new(Session::new(
			SocketAddr::V4("127.0.0.1:8080".parse().unwrap())
		));
	}

	pub fn get_data<D: std::any::Any>(&self) -> &D {
		self.inner.get_data().unwrap()
	}

	pub async fn request<R>(
		&self,
		r: R
	) -> std::result::Result<R::Response, R::Error>
	where
		R: Request<Action=Action>,
		R: IntoMessage<Action, EncryptedBytes>,
		R::Response: FromMessage<Action, EncryptedBytes>,
		R::Error: FromMessage<Action, EncryptedBytes>
	{
		self.inner.request(r, &self.session).await
	}
}