
use crypto::signature::{PublicKey, Signature};
use crypto::hash::Hash;

use serde::{Serialize, Deserialize};


// todo should we use this??
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
	Debug,
	Alpha,
	Beta,
	Release
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
	/// example packages.lvgd.ch:9281
	pub addr: String,
	/// if public == false an authentication token is sent?
	pub public: bool,
	/// the connection signature key
	pub public_key: PublicKey,
	/// the package signature key
	pub sign_key: PublicKey

	// todo add whitelist that only specific packages can be fetched
	// from this source
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesCfg {
	/// sources to fetch for updates
	/// updates are checked in reverse order
	/// until some source is found that contains that package
	pub sources: Vec<Source>,
	/// if this is true that last source will return realtime updates
	pub fetch_realtime: bool,
	/// the package that should be run when installing
	pub on_install: String,
	/// the package that should be run normally
	pub on_run: String,
	/// this information get's overriden if the image is in Debug
	pub channel: Channel
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
	pub name: String,
	pub version_str: String,
	/// blake2s hash of the full compressed file
	pub version: Hash,
	pub signature: Signature,
	// pub size: u64,
	pub binary: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageCfg {
	// do i need to other package??
	Left(Package),
	Right(Package)
}

impl PackageCfg {
	pub fn package(&self) -> &Package {
		match self {
			Self::Left(p) => p,
			Self::Right(p) => p
		}
	}

	pub fn current(&self) -> &'static str {
		match self {
			Self::Left(_) => "left",
			Self::Right(_) => "right"
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
	pub buildroot_version: String,
	pub version: u64,
	pub hash: Hash,
	pub signature: Signature,
	// pub size: u64
}