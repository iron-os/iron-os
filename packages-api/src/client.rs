
use crate::message::Message;
use crate::error::{Result, Error};
use crate::request::{Request, Response};

use std::io;
use std::time::Duration;

use stream::packet::EncryptedBytes;
use stream::{encrypted};

use crypto::signature::PublicKey;

use tokio::net::{TcpStream, ToSocketAddrs};

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
	inner: stream::Client<Message>
}

impl Client {

	pub async fn connect<A>(addr: A, pub_key: PublicKey) -> Result<Self>
	where A: ToSocketAddrs {
		let stream = TcpStream::connect(addr).await
			.map_err(Error::io)?;
		let inner = encrypted::client(stream, TIMEOUT, pub_key);
		Ok(Self { inner })
	}

	pub async fn request<R>(&self, req: R) -> Result<R::Response>
	where R: Request {
		let req = req.into_message()?;
		let res = self.inner.request(req).await
			.map_err(Error::Stream)?;
		R::Response::from_message(res)
	}

}