use std::io;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Error {
	description: String,
	error: io::Error,
}

impl Error {
	pub fn new(desc: impl Into<String>, error: io::Error) -> Self {
		Self {
			description: desc.into(),
			error,
		}
	}

	pub fn other<E>(desc: impl Into<String>, error: E) -> Self
	where
		E: Into<Box<dyn std::error::Error + Send + Sync>>,
	{
		Self {
			description: desc.into(),
			error: io::Error::new(io::ErrorKind::Other, error),
		}
	}
}

pub type Result<T> = std::result::Result<T, Error>;
