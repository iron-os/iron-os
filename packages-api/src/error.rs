
use std::{fmt, error};

use serde::{Serialize, Deserialize};

use stream_api::error::{ApiError, Error as ErrorTrait};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
	ConnectionClosed,
	RequestDropped,
	AuthKeyUnknown,
	NotAuthenticated,
	SignatureIncorrect,
	Internal(String),
	Request(String),
	Response(String),
	Other(String)
}

impl ApiError for Error {

	fn connection_closed() -> Self {
		Self::ConnectionClosed
	}

	fn request_dropped() -> Self {
		Self::RequestDropped
	}

	fn internal<E: ErrorTrait>(e: E) -> Self {
		Self::Internal(e.to_string())
	}

	fn request<E: ErrorTrait>(e: E) -> Self {
		Self::Request(e.to_string())
	}

	fn response<E: ErrorTrait>(e: E) -> Self {
		Self::Response(e.to_string())
	}

	fn other<E: ErrorTrait>(e: E) -> Self {
		Self::Other(e.to_string())
	}

}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(self, f)
	}
}

impl error::Error for Error {}