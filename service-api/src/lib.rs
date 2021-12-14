#![doc = include_str!("../README.md")]

pub mod requests;
pub mod message;
#[cfg(target_family = "unix")]
pub mod client;
#[cfg(target_family = "unix")]
pub mod server;
pub mod error;

pub use stream;

#[doc(hidden)]
pub use stream::basic::request::Request as StreamRequest;