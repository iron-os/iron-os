
use std::fmt;
use std::borrow::Cow;
use std::str::FromStr;

use rand::{RngCore, rngs::OsRng};

use base64::{encode_config, decode_config_slice, URL_SAFE_NO_PAD, DecodeError};

use serde::{Serialize, Deserialize, Serializer, Deserializer};
use serde::de::Error;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AuthKey {
	bytes: [u8; 32]
}

impl AuthKey {

	pub fn new() -> Self {
		let mut bytes = [0u8; 32];

		OsRng.fill_bytes(&mut bytes);

		Self { bytes }
	}

	pub fn to_b64(&self) -> String {
		encode_config(&self.bytes, URL_SAFE_NO_PAD)
	}

	/// ## Panics
	/// if b64 has not a length of 43
	pub fn parse_from_b64<T>(b64: T) -> Result<Self, DecodeError>
	where T: AsRef<[u8]> {
		let mut bytes = [0u8; 32];
		decode_config_slice(b64, URL_SAFE_NO_PAD, &mut bytes)?;
		Ok(Self { bytes })
	}

}

impl fmt::Debug for AuthKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("AuthKey")
			.field(&self.to_b64())
			.finish()
	}
}

impl fmt::Display for AuthKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.to_b64().fmt(f)
	}
}

impl AsRef<[u8]> for AuthKey {
	fn as_ref(&self) -> &[u8] {
		&self.bytes
	}
}

impl FromStr for AuthKey {
	type Err = DecodeError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::parse_from_b64(s)
	}
}

impl Serialize for AuthKey {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where S: Serializer {
		serializer.serialize_str(&self.to_b64())
	}
}

impl<'de> Deserialize<'de> for AuthKey {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: Deserializer<'de> {
		let s: Cow<'_, str> = Deserialize::deserialize(deserializer)?;
		let s = s.as_ref();
		if s.len() == 43 {
			Self::parse_from_b64(s)
				.map_err(D::Error::custom)
		} else {
			Err(D::Error::custom("expected string with exactly 43 characters"))
		}
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn test() {
		let key = AuthKey::new();
		assert_ne!(key, AuthKey::new());
		let copy_key = key.clone();
		assert_eq!(key, copy_key);

		let b64 = key.to_b64();
		assert_eq!(copy_key, AuthKey::parse_from_b64(b64).unwrap());
	}

}