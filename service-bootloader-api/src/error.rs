use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Error {
	/// unrecoverable error
	ConnectionClosed,
	/// unrecoverable error
	ConnectionError(String),
	/// get's returned when we receive an unkown key/kind
	UnknownKind,
	SerializationError,
	DeserializationError,
	InternalError(String),
	// update errors
	AlreadyUpdated,
}

impl Error {
	pub fn internal_display(e: impl fmt::Display) -> Self {
		Self::InternalError(e.to_string())
	}
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fmt::Debug::fmt(self, f)
	}
}

pub type Result<T> = std::result::Result<T, Error>;
