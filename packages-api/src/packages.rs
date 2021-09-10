
use crypto::signature::PublicKey;

use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
	/// example packages.lvgd.ch:9281
	pub url: String,
	/// if public == false an authentication token is sent?
	pub public: bool,
	/// the connection signature key
	pub public_key: PublicKey,
	/// the package signature key
	pub sign_key: PublicKey

	// todo add whitelist that only specific packages can be fetched
	// from this source
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesCfg {
	list: Vec<String>,
	/// sources to fetch for updates
	/// updates are checked in reverse order
	/// until some source is found that contains that package
	pub sources: Vec<Source>,
	/// if this is true that last source will return realtime updates
	pub fetch_realtime: bool,
	/// the package that should be run when installing
	pub on_install: String,
	/// the package that should be run normally
	pub on_run: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
	pub name: String,
	pub version_str: String,
	/// blake2s hash of the full compressed file
	pub version: String,
	pub current: String,
	pub binary: Option<String>
}