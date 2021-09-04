//!
//! ## Note
//! Performance or efficient is not a focus of this crate since it is not used
//! in a performance sensitive environment.
//!

use std::{io, fmt};
use std::process::Child;
use std::error::Error as StdError;
use std::collections::HashMap;

use stdio_api::{Stdio, Kind as LineKind, SerdeError};

use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;


#[doc(hidden)]
pub use stdio_api::{serialize, deserialize, Line};

/// Example
/// ```ignore
/// request_handler!{
/// 	fn disks(req: Disks) -> io::Result<Disks> {
/// 		todo!()
/// 	}
/// }
/// ```
#[macro_export]
macro_rules! request_handler {
	(
		fn $name:ident($req:ident: $req_ty:ty) -> $ret_ty:ty $block:block
	) => (
		#[allow(non_camel_case_types)]
		pub struct $name;

		impl $crate::RequestHandler for $name {
			fn kind() -> $crate::Kind { <$req_ty as $crate::Request>::kind() }
			fn handle(&self, line: $crate::Line) -> std::io::Result<String> {
				let req: $req_ty = $crate::deserialize(line.data())
					.map_err($crate::io_other)?;
				$crate::assert_ty!(std::io::Result<<$req_ty as $crate::Request>::Response>, $ret_ty);
				fn inner($req: $req_ty) -> $ret_ty {
					$block
				}
				let r = inner(req)?;
				$crate::serialize(&r)
					.map_err($crate::io_other)
			}
		}
	)
}

#[doc(hidden)]
#[macro_export]
macro_rules! assert_ty {
	($ty1:ty, $ty2:ty) => ({
		// this can maybe be done better
		#[allow(dead_code)]
		fn __assert_ty(a: $ty1, b: $ty2) {
			__assert_ty_gen(a, b);
		}
		#[allow(dead_code)]
		fn __assert_ty_gen<A>(a: A, b: A) {}
	})
}



pub trait RequestHandler {
	fn kind() -> Kind
	where Self: Sized;
	/// result should only be returned if the serialization or deserialization failed
	fn handle(&self, line: Line) -> io::Result<String>;
}




pub struct Server {
	handlers: HashMap<Kind, Box<dyn RequestHandler>>,
	inner: Stdio
}

impl Server {

	/// Returns none if the child doesn't have stdin and stdout
	pub fn new(child: &mut Child) -> Option<Self> {
		Stdio::from_child(child)
			.map(|inner| Self {
				handlers: HashMap::new(),
				inner
			})
	}

	// matching 
	pub fn register<R>(&mut self, handler: R)
	where R: RequestHandler + 'static {
		self.handlers.insert(R::kind(), Box::new(handler));
	}

	pub fn run(mut self) -> io::Result<()> {
		while let Some(line) = self.inner.read()? {

			if let LineKind::Response = line.kind() {
				eprintln!("received response {:?}", line);
				continue
			}

			let handler = Kind::from_str(line.key())
				.and_then(|k| {
					self.handlers.get(&k)
						.map(|h| (k, h))
				});

			if let Some((kind, handler)) = handler {
				let data = handler.handle(line);
				// what error should be returned??
				let data = match data {
					Ok(d) => d,
					Err(e) => {
						eprintln!("handler had error {:?}", e);
						// this will fail while serialization
						"error".into()
					}
				};
				let line = Line::new(LineKind::Response, kind.as_str(), &data);
				self.inner.write(&line)?;
			} else {
				eprintln!("handler for line {:?} not found", line);
			}
		}

		Ok(())
	}

}


/// used in the macro
#[doc(hidden)]
pub fn io_other<E>(e: E) -> io::Error
where E: Into<Box<dyn StdError + Send + Sync>> {
	io::Error::new(io::ErrorKind::Other, e)
}


#[cfg(any(feature = "async", test))]
mod r#async {

	use super::*;
	use stdio_api::AsyncStdio;

	pub struct AsyncClient {
		inner: AsyncStdio
	}

	impl AsyncClient {

		pub fn new() -> Self {
			Self {
				inner: AsyncStdio::from_env()
			}
		}

		pub async fn request<R>(&mut self, req: &R) -> io::Result<R::Response>
		where R: Request {
			let line = Line::new(
				LineKind::Request,
				R::kind().as_str(),
				&serialize(req)
					.map_err(io_other)?
			);
			self.inner.write(&line).await?;
			let line = self.inner.read().await?;
			if let LineKind::Request = line.kind() {
				return Err(io_other("received request instead of response"))
			}

			if line.key() != R::kind().as_str() {
				return Err(io_other("received other key that requested"))
			}

			deserialize(line.data())
					.map_err(io_other)
		}

	}

}
#[cfg(any(feature = "async", test))]
pub use r#async::*;





pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
	Json(SerdeError),
	Unknown,
	Other(&'static str)
}

impl From<SerdeError> for Error {
	fn from(e: SerdeError) -> Self {
		Self::Json(e)
	}
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Json(e) => e.fmt(f),
			Self::Unknown => f.write_str("Unknown"),
			Self::Other(o) => f.write_str(o)
		}
	}
}



pub trait Request: Serialize + DeserializeOwned {
	type Response: Serialize + DeserializeOwned;
	fn kind() -> Kind;
}




macro_rules! kind {
	($($name:ident),*) => (
		#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
		pub enum Kind {
			$($name),*
		}

		impl Kind {

			pub fn as_str(&self) -> &'static str {
				match self {
					$(Self::$name => stringify!($name)),*
				}
			}

			pub fn from_str(s: &str) -> Option<Self> {
				match s {
					$(stringify!($name) => Some(Self::$name)),*,
					_ => None
				}
			}

		}
	)
}

kind!{
	SystemdRestart,
	Disks,
	InstallOn,
}




#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdRestart {
	pub name: String
}

impl Request for SystemdRestart {
	type Response = ();
	fn kind() -> Kind { Kind::SystemdRestart }
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Disks;

impl Request for Disks {
	type Response = Vec<Disk>;
	fn kind() -> Kind { Kind::Disks }
}

// data for disks info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disk {
	pub name: String,
	// if this is the disk we are running on
	pub active: bool,
	// if the this disk has a gpt partition table
	pub initialized: bool,
	// how many bytes this disk has
	pub size: u64
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallOn {
	pub name: String
}

impl Request for InstallOn {
	type Response = ();
	fn kind() -> Kind { Kind::InstallOn }
}
}