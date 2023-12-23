use crate::packages::Channel;

use std::fmt;

use serde::{Serialize, Deserialize};

use stream_api::{IntoMessage, FromMessage};
use stream_api::error::{ApiError, RequestError, MessageError};

pub type Result<T> = std::result::Result<T, Error>;


// todo i don't wan't to change the enum at the moment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(IntoMessage, FromMessage)]
#[message(json)]
#[non_exhaustive]
pub enum Error {
	// deprecated
	ConnectionClosed,
	// deprecated
	RequestDropped,
	AuthKeyUnknown,
	NoSignKeyForChannel(Channel),
	NotAuthenticated,
	SignatureIncorrect,
	VersionNotFound,
	StartUnreachable,
	FileNotFound,
	Internal(String),
	Request(String),
	// deprecated
	Response(String),
	Other(String)
}

impl ApiError for Error {
	fn from_request_error(e: RequestError) -> Self {
		match e {
			RequestError::ConnectionAlreadyClosed => Self::ConnectionClosed,
			RequestError::NoResponse => Self::RequestDropped,
			RequestError::ResponsePacket(p) => Self::Response(p.to_string()),
			e => Self::Request(e.to_string())
		}
	}

	fn from_message_error(e: MessageError) -> Self {
		Self::Other(e.to_string())
	}
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(self, f)
	}
}