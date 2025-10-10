use std::error::Error;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use rand::distributions::Alphanumeric;
use rand::Rng;

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

#[derive(Debug)]
pub struct TempPath(PathBuf);

impl TempPath {
	pub fn new() -> Self {
		let mut rng = rand::thread_rng();
		let filename: String = ['.', 's', 'e', 'r', 'v']
			.into_iter()
			.chain((0..7).map(|_| rng.sample(Alphanumeric) as char))
			.collect();

		Self(env::temp_dir().join(filename))
	}
}

impl AsRef<Path> for TempPath {
	fn as_ref(&self) -> &Path {
		&self.0
	}
}

impl Drop for TempPath {
	fn drop(&mut self) {
		let _ = fs::remove_file(&self.0);
	}
}
