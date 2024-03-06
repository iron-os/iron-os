use crate::error::Error;
use crate::requests::Kind;
use crate::{serialize, RequestHandler};

use std::collections::HashMap;
use std::io;
use std::process::Child;

use stdio_api::{Kind as LineKind, Line, Stdio};

pub struct Server {
	handlers: HashMap<Kind, Box<dyn RequestHandler>>,
	inner: Stdio,
}

impl Server {
	/// Returns none if the child doesn't have stdin and stdout
	pub fn new(child: &mut Child) -> Option<Self> {
		Stdio::from_child(child).map(|inner| Self {
			handlers: HashMap::new(),
			inner,
		})
	}

	// matching
	pub fn register<R>(&mut self, handler: R)
	where
		R: RequestHandler + 'static,
	{
		self.handlers.insert(R::kind(), Box::new(handler));
	}

	pub fn run(mut self) -> io::Result<()> {
		while let Some(line) = self.inner.read()? {
			#[cfg(feature = "debug")]
			{
				eprintln!("received: {:?}", line);
			}

			if let LineKind::Response = line.kind() {
				eprintln!("received response {:?}", line);
				continue;
			}

			let key = line.key().to_string();

			let handler =
				Kind::from_str(line.key()).and_then(|k| self.handlers.get(&k));

			let r = match handler {
				Some(handler) => handler.handle(line),
				None => serialize(&Error::UnknownKind).unwrap(),
			};

			let line = Line::new(LineKind::Response, &key, &r);
			#[cfg(feature = "debug")]
			{
				eprintln!("sending: {:?}", line);
			}
			self.inner.write(&line)?;
		}

		Ok(())
	}
}
