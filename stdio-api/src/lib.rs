//! A mini protocol based on stdio  
//! a request consists of the following: `:>:<key> <data>\n`  
//! a response consists of the following: `:<:<key> <data>\n`  
//!

use std::mem;

mod sync;
pub use sync::*;

#[cfg(any(feature = "async", test))]
mod r#async;
#[cfg(any(feature = "async", test))]
pub use r#async::*;

#[cfg(feature = "serde")]
mod serde;
#[cfg(feature = "serde")]
pub use serde::Error as SerdeError;
#[cfg(feature = "serde")]
pub use serde::{deserialize, serialize};

// Represents an api line
// <kind><key> <data>\n
#[derive(Debug, Clone)]
pub struct Line {
	kind: Kind,
	/// contains the length of the key
	key: usize,
	inner: String,
}

impl Line {
	pub fn new(kind: Kind, key: &str, data: &str) -> Self {
		Self {
			key: key.len(),
			inner: format!("{}{} {}\n", kind.as_str(), key, data),
			kind,
		}
	}

	// the first three bytes need to be :<: or :>:
	fn new_raw(kind: Kind, inner: &mut String) -> Self {
		let inner = mem::take(inner);
		debug_assert!(inner.len() > 3);
		let key = inner.find(' ').unwrap_or(0).max(3) - 3; // skips the first 3 bytes

		Self { kind, key, inner }
	}

	pub fn kind(&self) -> Kind {
		self.kind
	}

	pub fn key(&self) -> &str {
		&self.inner[Kind::LEN..][..self.key]
	}

	pub fn data(&self) -> &str {
		&self.inner[Kind::LEN + self.key..].trim()
	}

	// return a line with the newline
	pub fn as_str(&self) -> &str {
		&self.inner
	}
}

// stdin represents the request
// stdout represents the response

//:>:Request
//:<:Response

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
	Request,
	Response,
}

impl Kind {
	pub const LEN: usize = 3;

	/// only looks at the first 3 bytes
	pub fn from_str(s: &str) -> Option<Self> {
		let start = s.get(..3)?;
		match start {
			":>:" => Some(Self::Request),
			":<:" => Some(Self::Response),
			_ => None,
		}
	}

	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Request => ":>:",
			Self::Response => ":<:",
		}
	}
}

#[derive(Debug)]
struct Buffer {
	inner: String,
}

impl Buffer {
	pub fn new() -> Self {
		Self {
			inner: String::new(),
		}
	}

	pub fn as_mut(&mut self) -> &mut String {
		&mut self.inner
	}

	/// Returns none if the line was no an api line
	/// then the line is outputed again to stderr
	pub fn parse_line(&mut self) -> Option<Line> {
		if self.inner.is_empty() {
			return None;
		}

		let kind = Kind::from_str(&self.inner);
		match kind {
			Some(kind) => Some(Line::new_raw(kind, &mut self.inner)),
			None => {
				eprint!("{}", &self.inner);
				self.inner.clear();
				None
			}
		}
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn from_str() {
		let mut buffer = Buffer::new();
		buffer.as_mut().push_str(":>:SomeKey data\n");
		let line = buffer.parse_line().unwrap();

		assert_eq!(line.kind, Kind::Request);
		assert_eq!(line.key(), "SomeKey");
		assert_eq!(line.data(), "data");
	}

	#[test]
	fn new() {
		let line = Line::new(Kind::Request, "SomeKey", "data");
		assert_eq!(line.kind, Kind::Request);
		assert_eq!(line.key(), "SomeKey");
		assert_eq!(line.data(), "data");
	}
}
