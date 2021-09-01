
use std::io;
use std::borrow::Cow;
use std::process::Child;

use stdio_api::{Stdio, Line, Kind};

pub struct Server {
	inner: Stdio
}

impl Server {

	/// Returns none if the child doesn't have stdin and stdout
	pub fn new(child: &mut Child) -> Option<Self> {
		Stdio::from_child(child)
			.map(|inner| Self { inner })
	}

	pub fn receive(&mut self) -> io::Result<Option<Request>> {
		let r = self.inner.read()?
			.map(Request::from);
		Ok(r)
	}

}


#[cfg(any(feature = "async", test))]
mod r#async {

	use super::*;
	use stdio_api::AsyncStdio;

	pub struct AsyncClient {
		inner: AsyncStdio
	}

	impl AsyncClient {

		pub fn new() -> Self {
			Self {
				inner: AsyncStdio::from_env()
			}
		}

		pub async fn send(&mut self, req: Request) -> io::Result<()> {
			self.inner.write(&req.into()).await
		}

	}

}
#[cfg(any(feature = "async", test))]
pub use r#async::*;



// if you receive a message it will be this
#[derive(Debug, Clone)]
pub enum Request {
	SystemdRestart { name: String },
	// this can even be a response
	Unknown(Line)
}

impl Request {

	pub fn key(&self) -> &str {
		match self {
			Self::SystemdRestart {..} => "SystemdRestart",
			Self::Unknown(l) => l.key()
		}
	}

	pub fn data(&self) -> Cow<'_, str> {
		match self {
			Self::SystemdRestart { name } => name.into(),
			Self::Unknown(l) => l.data().into()
		}
	}

}

impl From<Request> for Line {
	fn from(req: Request) -> Line {
		Line::new(Kind::Request, req.key(), req.data().as_ref())
	}
}

impl From<Line> for Request {
	fn from(line: Line) -> Self {
		if !matches!(line.kind(), Kind::Request) {
			return Self::Unknown(line)
		}

		match line.key() {
			"SystemdRestart" => Self::SystemdRestart {
				name: line.data().into()
			},
			_ => Self::Unknown(line)
		}
	}
}