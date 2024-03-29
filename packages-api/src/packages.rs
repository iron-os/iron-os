use std::str::FromStr;
use std::{fmt, mem};

pub use crypto::hash::Hash;
pub use crypto::signature::{PublicKey, Signature};

use serde::de::{value, IntoDeserializer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
	Debug,
	Alpha,
	Beta,
	Release,
}

impl FromStr for Channel {
	type Err = value::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::deserialize(s.into_deserializer())
	}
}

impl fmt::Display for Channel {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.serialize(f)
	}
}

impl Channel {
	pub fn is_debug(&self) -> bool {
		matches!(self, Self::Debug)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
	/// example packages.lvgd.ch:9281
	pub addr: String,
	/// if public == false an authentication token is sent?
	pub public: bool,
	/// the connection signature key
	pub public_key: PublicKey,
	/// the package signature key
	pub sign_key: PublicKey, // todo add whitelist that only specific packages can be fetched
	                         // from this source
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagesCfg {
	/// sources to fetch for updates
	/// updates are checked in reverse order
	/// until some source is found that contains that package
	pub sources: Vec<Source>,
	/// if this is true that last source will return realtime updates
	///
	/// Todo
	pub fetch_realtime: bool,
	/// the package that should be run normally
	pub on_run: String,
	/// this information get's overriden if the image is in Debug
	pub channel: Channel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetArch {
	/// should only be used for example for web only
	/// packages, or script only
	Any,
	Amd64,
	Arm64,
}

impl From<BoardArch> for TargetArch {
	fn from(b: BoardArch) -> Self {
		match b {
			BoardArch::Amd64 => TargetArch::Amd64,
			BoardArch::Arm64 => TargetArch::Arm64,
		}
	}
}

impl FromStr for TargetArch {
	type Err = value::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::deserialize(s.into_deserializer())
	}
}

impl fmt::Display for TargetArch {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.serialize(f)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BoardArch {
	Amd64,
	Arm64,
}

impl FromStr for BoardArch {
	type Err = value::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::deserialize(s.into_deserializer())
	}
}

impl fmt::Display for BoardArch {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.serialize(f)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
	pub name: String,
	pub version_str: String,
	/// blake2s hash of the full compressed file
	pub version: Hash,
	pub signature: Signature,
	pub arch: TargetArch,
	pub binary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageCfg {
	// do i need to other package??
	Left(Package),
	Right(Package),
}

impl PackageCfg {
	pub fn package(&self) -> &Package {
		match self {
			Self::Left(p) => p,
			Self::Right(p) => p,
		}
	}

	/// Returns the current side of the package
	pub fn current(&self) -> &'static str {
		match self {
			Self::Left(_) => "left",
			Self::Right(_) => "right",
		}
	}

	/// returns the other side of the package
	pub fn other(&self) -> &'static str {
		match self {
			Self::Left(_) => "right",
			Self::Right(_) => "left",
		}
	}

	/// Switches the side of the current package and stores it there
	pub fn switch(&mut self, new: Package) {
		let new = match self {
			Self::Left(_) => PackageCfg::Right(new),
			Self::Right(_) => PackageCfg::Left(new),
		};
		let _ = mem::replace(self, new);
	}
}
