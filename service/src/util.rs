use std::error::Error;
use std::io;

pub fn io_other<E>(e: E) -> io::Error
where
	E: Into<Box<dyn Error + Send + Sync>>,
{
	io::Error::new(io::ErrorKind::Other, e)
}

macro_rules! io_other {
	($e:expr) => {
		$crate::util::io_other($e)
	};
	($($arg:tt)*) => {
		$crate::util::io_other(format!($($arg)*))
	}
}
