use std::error::Error as StdError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
	pub description: String,
	pub error: Box<dyn StdError + Send + Sync>,
}

impl Error {
	pub fn new<D, E>(desc: D, error: E) -> Self
	where
		D: Into<String>,
		E: Into<Box<dyn StdError + Send + Sync>>,
	{
		Self {
			description: desc.into(),
			error: error.into(),
		}
	}
}

macro_rules! err {
	($e:expr, $($tt:tt)*) => (
		$crate::error::Error::new(format!($($tt)*), $e)
	)
}
