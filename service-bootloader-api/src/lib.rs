//!
//! ## Note
//! Performance or efficient is not a focus of this crate since it is not used
//! in a performance sensitive environment.
//!

#[macro_use]
mod macros;
#[cfg(any(feature = "async", test))]
mod r#async;
pub mod error;
pub mod requests;
mod server;

use requests::Kind;
pub use server::Server;

#[cfg(any(feature = "async", test))]
pub use r#async::*;

use serde::de::DeserializeOwned;
use serde::Serialize;

#[doc(hidden)]
pub use stdio_api::{deserialize, serialize, Line};

pub trait RequestHandler {
	fn kind() -> Kind
	where
		Self: Sized;
	/// result should only be returned if the serialization or deserialization failed
	fn handle(&self, line: Line) -> String;
}

pub trait Request: Serialize + DeserializeOwned {
	type Response: Serialize + DeserializeOwned;
	fn kind() -> Kind;
}
