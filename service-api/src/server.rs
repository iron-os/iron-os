
use crate::message::Action;
use crate::error::{Result, Error};

use std::time::Duration;
use std::any::Any;
use std::path::Path;

use stream::basic::{
	self,
	server::RequestHandler
};
use stream::packet::PlainBytes;

use tokio::net::UnixListener;

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);

type BasicServer = basic::Server<Action, PlainBytes, UnixListener, ()>;

pub struct Server {
	inner: BasicServer
}

impl Server {

	pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
		let listener = UnixListener::bind(path)
			.map_err(Error::io)?;

		Ok(Self {
			inner: BasicServer::new(listener, TIMEOUT)
		})
	}

	pub fn register_request<H>(&mut self, handler: H)
	where H: RequestHandler<Action, PlainBytes> + Send + Sync + 'static {
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
			async fn $name<$crate::message::Action, $crate::stream::packet::PlainBytes>
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