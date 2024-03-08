use std::{error, fmt};

use stream::error::RequestError;
pub use stream_api::error::{ApiError, Error as ErrorTrait};
use stream_api::{error::MessageError, FromMessage, IntoMessage};

use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub enum Error {
	#[deprecated]
	ConnectionClosed,
	#[deprecated]
	RequestDropped,
	Internal(String),
	Request(String),
	#[deprecated]
	Response(String),
	Other(String),
}

impl Error {
	pub fn internal_display(e: impl fmt::Display) -> Self {
		Self::Internal(e.to_string())
	}
}

impl ApiError for Error {
	fn from_message_error(e: MessageError) -> Self {
		Self::Request(e.to_string())
	}

	fn from_request_error(e: RequestError) -> Self {
		Self::Request(e.to_string())
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(self, f)
	}
}

impl error::Error for Error {}
