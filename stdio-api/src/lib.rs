/*

*/

use std::mem;

mod sync;
pub use sync::*;

#[cfg(any(feature = "async", test))]
mod r#async;
#[cfg(any(feature = "async", test))]
pub use r#async::*;

// Represents an api line
// <kind><key> <data>\n
#[derive(Debug, Clone)]
pub struct Line {
	kind: Kind,
	key: usize,
	inner: String
}

impl Line {

	pub fn new(kind: Kind, key: &str, data: &str) -> Self {
		Self {
			key: key.len() + 3,
			inner: format!("{}{} {}\n", kind.as_str(), key, data),
			kind
		}
	}

	// the first three bytes need to be :<: or :>:
	fn new_raw(kind: Kind, inner: &mut String) -> Self {
		let inner = mem::take(inner);
		debug_assert!(inner.len() > 3);
		let key = inner.find(' ')
			.unwrap_or(0)
			.max(3);// skips the first 3 chars

		Self { kind, key, inner }
	}

	pub fn kind(&self) -> Kind {
		self.kind
	}

	pub fn key(&self) -> &str {
		&self.inner[3..self.key]
	}

	pub fn data(&self) -> &str {
		&self.inner[self.key..].trim()
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

#[cfg(feature = "serde")]
mod serde {

	use std::fmt;
	use std::ops::Not;

	use _serde::{Serialize, Deserialize};

	#[derive(Debug)]
	pub enum Error {
		Json(serde_json::Error),
		ContainsInvalidChars
	}

	impl fmt::Display for Error {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			match self {
				Self::Json(e) => e.fmt(f),
				Self::ContainsInvalidChars => f.write_str("ContainsInvalidChars")
			}
		}
	}

	impl std::error::Error for Error {}

	// not allowed to contain \n
	pub fn serialize<T>(value: &T) -> Result<String, Error>
	where T: Serialize + ?Sized {
		let s = serde_json::to_string(value)
			.map_err(Error::Json)?;
		s.contains('\n').not()
			.then(|| s)
			.ok_or(Error::ContainsInvalidChars)
	}

	pub fn deserialize<'a, T>(s: &'a str) -> Result<T, Error>
	where T: Deserialize<'a> {
		serde_json::from_str(s)
			.map_err(Error::Json)
	}

}
#[cfg(feature = "serde")]
pub use serde::{serialize, deserialize};
pub use serde::Error as SerdeError;



#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
	Request,
	Response
}

impl Kind {
	pub fn from_str(s: &str) -> Option<Self> {
		if s.starts_with(":>:") {
			Some(Self::Request)
		} else if s.starts_with(":<:") {
			Some(Self::Response)
		} else {
			None
		}
	}

	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Request => ":>:",
			Self::Response => ":<:"
		}
	}
}


#[derive(Debug)]
struct Buffer {
	inner: String
}

impl Buffer {
	pub fn new() -> Self {
		Self { inner: String::new() }
	}

	pub fn as_mut(&mut self) -> &mut String {
		&mut self.inner
	}

	/// Returns none if the line was no an api line
	/// then the line is outputed again to stderr
	pub fn parse_line(&mut self) -> Option<Line> {
		if self.inner.is_empty() {
			return None
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