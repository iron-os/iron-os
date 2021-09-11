
use std::{io, fmt, error};
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

	pub fn into_stream(self) -> StreamError {
		match self {
			Self::Stream(s) => s
		}
	}

}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(self, f)
	}
}

impl error::Error for Error {}