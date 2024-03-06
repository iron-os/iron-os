use std::fmt;
use std::ops::Not;

use _serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Error {
	Json(serde_json::Error),
	ContainsInvalidChars,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Json(e) => e.fmt(f),
			Self::ContainsInvalidChars => f.write_str("ContainsInvalidChars"),
		}
	}
}

impl std::error::Error for Error {}

/// Serializes a value
///
/// Returns an error if the serialization did not succeed or the result contained
/// a newline character `\n`
pub fn serialize<T>(value: &T) -> Result<String, Error>
where
	T: Serialize + ?Sized,
{
	let s = serde_json::to_string(value).map_err(Error::Json)?;

	s.contains('\n')
		.not()
		.then(|| s)
		.ok_or(Error::ContainsInvalidChars)
}

pub fn deserialize<'a, T>(s: &'a str) -> Result<T, Error>
where
	T: Deserialize<'a>,
{
	serde_json::from_str(s).map_err(Error::Json)
}
