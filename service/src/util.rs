
use std::io;
use std::error::Error;

pub fn io_other<E>(e: E) -> io::Error
where E: Into<Box<dyn Error + Send + Sync>> {
	io::Error::new(io::ErrorKind::Other, e)
}