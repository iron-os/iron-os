
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


/// Example
/// ```ignore
/// request_handler!{
/// 	fn disks(req: Disks) -> Result<Disks> {
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
			fn kind() -> $crate::requests::Kind {
				<$req_ty as $crate::Request>::kind()
			}

			fn handle(&self, line: $crate::Line) -> String {
				type __Response = <$req_ty as $crate::Request>::Response;
				type __Error = $crate::error::Error;

				fn __inner($req: $req_ty) -> $ret_ty {
					$block
				}

				fn __handle(
					line: $crate::Line
				) -> std::result::Result<__Response, __Error> {
					let req: $req_ty = $crate::deserialize(line.data())
						.map_err(|_| __Error::DeserializationError)?;

					__inner(req)
				}

				let res = __handle(line);
				match $crate::serialize(&res) {
					Ok(s) => s,
					Err(_) => $crate::serialize(&__Error::SerializationError)
						// this should not be able to fail
						.unwrap()
				}
			}
		}
	)
}