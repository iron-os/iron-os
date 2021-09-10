
use crate::message::Action;
use crate::error::{Result, Error};

use std::time::Duration;

use stream::basic::{
	self,
	request::Request
};
use stream::packet::EncryptedBytes;

use crypto::signature::PublicKey;

use tokio::net::{TcpStream, ToSocketAddrs};

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
	inner: basic::Client<Action, EncryptedBytes>
}

impl Client {

	pub async fn connect<A>(addr: A, pub_key: PublicKey) -> Result<Self>
	where A: ToSocketAddrs {
		let stream = TcpStream::connect(addr).await
			.map_err(Error::io)?;
		Ok(Self {
			inner: basic::Client::<_, EncryptedBytes>::new(stream, TIMEOUT, pub_key)
		})
	}

	pub async fn request<R>(&self, req: R) -> Result<R::Response>
	where R: Request<Action, EncryptedBytes> {
		self.inner.request(req).await
			.map_err(Error::Stream)
	}

}