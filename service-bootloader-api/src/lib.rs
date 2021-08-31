
use std::io;
use std::borrow::Cow;

use stdio_api::{Stdio, Line, Kind};

pub struct Server {
	inner: Stdio
}

impl Server {

	pub fn receive(&mut self) -> io::Result<Option<Request>> {
		let r = self.inner.read()?
			.map(Request::from_line);
		Ok(r)
	}

}


// if you receive a message it will be this
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

	pub fn from_line(line: Line) -> Self {
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

impl From<Request> for Line {
	fn from(req: Request) -> Line {
		Line::new(Kind::Request, req.key(), req.data().as_ref())
	}
}