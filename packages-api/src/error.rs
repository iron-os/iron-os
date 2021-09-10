
use std::io;
use stream::StreamError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
	Stream(StreamError)
}

impl Error {

	pub(crate) fn io(e: io::Error) -> Self {
		Self::Stream(e.into())
	}

	#[allow(dead_code)]
	pub(crate) fn io_other<E>(e: E) -> Self
	where E: Into<Box<dyn std::error::Error + Send + Sync>> {
		Self::Stream(StreamError::io_other(e))
	}

}