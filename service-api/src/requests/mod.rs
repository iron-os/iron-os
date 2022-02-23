
// - SystemInfo (VersionInfo, Packages)
// - DeviceInfo (Cpu, Disks, Ram, Processes?)
// - OpenPage (open a web page)
// - 
// - DisplayState (turn on off)

#[macro_use]
mod macros;

pub mod device;

use crate::message::{Action, Message};
use serde::{Serialize, Deserialize};

// todo maybe rename os? or system
pub mod system {

	//! Control various operating system related features.
	//!
	//! - get [SystemInfo](struct.SystemInfoReq.html)
	//! - trigger [InstallOn](struct.InstallOnReq.html)

	use super::*;

	pub use packages_api::packages::Channel;

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct SystemInfoReq;

	serde_req!(Action::SystemInfo, SystemInfoReq, SystemInfo);

	fn default_board() -> String {
		"image".into()
	}

	fn default_product() -> String {
		"sputnik".into()
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct SystemInfo {
		// equivalent of version_str
		pub version: String,
		#[serde(default = "default_board")]
		pub board: String,
		#[serde(default = "default_product")]
		pub product: String,
		pub packages: Vec<ShortPackage>,
		pub channel: Channel,
		pub installed: bool
	}

	serde_res!(SystemInfo);

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct ShortPackage {
		pub name: String,
		pub version: String,
		pub path: String
	}

	/// This request should only be used if `SystemInfo.installed == false`
	#[derive(Debug, Serialize, Deserialize)]
	pub struct InstallOnReq {
		/// The name of a disk that is returned from DisksReq
		pub name: String
	}

	serde_req!(Action::InstallOn, InstallOnReq, InstallOn);

	#[derive(Debug, Serialize, Deserialize)]
	pub struct InstallOn;

	serde_res!(InstallOn);

}

pub mod ui {

	//! Control various web features.
	//!
	//! - trigger [OpenPage](struct.OpenPageReq.html)

	use super::*;

	#[derive(Debug, Serialize, Deserialize)]
	pub struct OpenPageReq {
		pub url: String
	}

	serde_req!(Action::OpenPage, OpenPageReq, OpenPage);

	#[derive(Debug, Serialize, Deserialize)]
	pub struct OpenPage;

	serde_res!(OpenPage);

}


pub mod packages {

	//! Manages and read all packages.
	//!
	//! - get [ListPackages](struct.ListPackagesReq.html)
	//! - add [AddPackage](struct.AddPackageReq.html)
	//! - rm [RemovePackage](struct.RemovePackageReq.html)

	use super::*;

	pub use packages_api::packages::{Source, Channel, Hash, Signature};

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct ListPackagesReq;

	serde_req!(Action::ListPackages, ListPackagesReq, ListPackages);

	/// if you need a detailed list of packages
	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct ListPackages {
		pub packages: Vec<Package>,
		pub sources: Vec<Source>,
		pub channel: Channel,
		pub on_run: String
	}

	impl ListPackages {
		pub fn get(&self, name: &str) -> Option<&Package> {
			self.packages.iter().find(|p| p.name == name)
		}
	}

	serde_res!(ListPackages);

	/// practically the same as packages_api
	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct Package {
		pub name: String,
		pub version_str: String,
		/// blake2s hash of the full compressed file
		pub version: Hash,
		pub signature: Signature,
		pub binary: Option<String>,
		pub path: String
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct AddPackageReq {
		pub name: String
	}

	serde_req!(Action::AddPackage, AddPackageReq, AddPackage);

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct AddPackage {
		pub package: Package
	}

	serde_res!(AddPackage);

	/// Not implemented
	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct RemovePackageReq {
		pub name: String
	}

	serde_req!(Action::RemovePackage, RemovePackageReq, RemovePackage);

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct RemovePackage;

	serde_res!(RemovePackage);

}