
use crate::message::Action;
use crate::error::{Result, Error};

use std::time::Duration;
use std::path::Path;

use stream::basic::{
	self,
	request::Request
};
use stream::packet::PlainBytes;

use tokio::net::UnixStream;

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
	inner: basic::Client<Action, PlainBytes>
}

impl Client {

	pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
		let stream = UnixStream::connect(path).await
			.map_err(Error::io)?;
		Ok(Self {
			inner: basic::Client::<_, PlainBytes>::new(stream, TIMEOUT)
		})
	}

	pub async fn request<R>(&self, req: R) -> Result<R::Response>
	where R: Request<Action, PlainBytes> {
		self.inner.request(req).await
			.map_err(Error::Stream)
	}

}