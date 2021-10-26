
use crate::message::Action;
use crate::error::{Result, Error};

use std::time::Duration;
use std::any::Any;

use stream::basic::{
	self,
	server::RequestHandler
};
pub use stream::basic::server::Session;
use stream::packet::EncryptedBytes;
pub use stream::server::Config;
pub use stream::handler::Configurator;

use crypto::signature::Keypair;

use tokio::net::{TcpListener, ToSocketAddrs};

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);
const BODY_LIMIT: usize = 4096;// 4kb request limit

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
			inner: BasicServer::new(listener, Config {
				timeout: TIMEOUT,
				body_limit: BODY_LIMIT
			}, priv_key)
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

///
/// ```dont_run
/// async fn test(req: Request, data: Data) -> Result<Response> { todo!() }
/// ```
#[macro_export]
macro_rules! request_handler {
	(async fn $name:ident( $($args:tt)* ) $($tt:tt)*) => (
		$crate::request_handler!(
			async fn $name<$crate::message::Action, $crate::stream::packet::EncryptedBytes>
			( $($args)* )
			$($tt)*
		);
	);
	(async fn $name:ident<$a:ty, $b:ty>($req:ident: $req_ty:ty) $($tt:tt)*) => (
		$crate::request_handler!(
			async fn $name<$a, $b>($req: $req_ty,) $($tt)*
		);
	);
	(
		async fn $name:ident<$a:ty, $b:ty>(
			$req:ident: $req_ty:ty,
			$($data:ident: $data_ty:ty),*
		) -> $ret_ty:ty
		$block:block
	) => (
		$crate::stream::request_handler!(
			async fn $name<$a, $b>(
				$req: $req_ty,
				$($data: $data_ty),*
			) -> $crate::stream::Result<<$req_ty as $crate::StreamRequest<$a, $b>>::Response> {
				async fn __req_handle(
					$req: $req_ty,
					$($data: &$data_ty),*
				) -> $ret_ty {
					$block
				}

				let resp = __req_handle($req, $($data),*).await;

				let resp: $crate::error::Result<<$req_ty as $crate::StreamRequest<$a, $b>>::Response> = resp;
				resp.map_err(|e| e.into_stream())
			}
			// __req_handle()
		);
	);
}