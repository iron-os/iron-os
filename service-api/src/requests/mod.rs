
// - SystemInfo (VersionInfo, Packages)
// - DeviceInfo (Cpu, Disks, Ram, Processes?)
// - OpenPage (open a web page)
// - 
// - DisplayState (turn on off)

pub mod device;
pub mod network;

use crate::Action;
use crate::error::Error;

use serde::{Serialize, Deserialize};

use stream_api::request::Request;

// todo maybe rename os? or system
pub mod system {

	//! Control various operating system related features.
	//!
	//! - get [SystemInfo](struct.SystemInfoReq.html)
	//! - trigger [InstallOn](struct.InstallOnReq.html)

	use super::*;

	pub use packages_api::packages::Channel;
	pub use packages_api::requests::DeviceId;

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct SystemInfoReq;

	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct SystemInfo {
		// equivalent of version_str
		pub version: String,
		pub board: String,
		pub product: String,
		pub packages: Vec<ShortPackage>,
		pub channel: Channel,
		pub device_id: Option<DeviceId>,
		pub installed: bool
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct ShortPackage {
		pub name: String,
		pub version: String,
		pub path: String
	}

	impl<B> Request<Action, B> for SystemInfoReq {
		type Response = SystemInfo;
		type Error = Error;

		const ACTION: Action = Action::SystemInfo;
	}


	/// This request should only be used if `SystemInfo.installed == false`
	#[derive(Debug, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct InstallOnReq {
		/// The name of a disk that is returned from DisksReq
		pub disk: String
	}

	impl<B> Request<Action, B> for InstallOnReq {
		type Response = ();
		type Error = Error;

		const ACTION: Action = Action::InstallOn;
	}

}

pub mod ui {

	//! Control various web features.
	//!
	//! - trigger [OpenPage](struct.OpenPageReq.html)

	use super::*;

	#[derive(Debug, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct OpenPageReq {
		pub url: String
	}

	impl<B> Request<Action, B> for OpenPageReq {
		type Response = ();
		type Error = Error;

		const ACTION: Action = Action::OpenPage;
	}

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

	/// if you need a detailed list of packages
	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
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

	/// practically the same as packages_api
	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct Package {
		pub name: String,
		pub version_str: String,
		/// blake2s hash of the full compressed file
		pub version: Hash,
		pub signature: Signature,
		pub binary: Option<String>,
		pub path: String
	}

	impl<B> Request<Action, B> for ListPackagesReq {
		type Response = ListPackages;
		type Error = Error;

		const ACTION: Action = Action::ListPackages;
	}


	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct AddPackageReq {
		pub name: String
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct AddPackage {
		/// Returns None if the package was not found
		pub package: Option<Package>
	}

	impl<B> Request<Action, B> for AddPackageReq {
		type Response = AddPackage;
		type Error = Error;

		const ACTION: Action = Action::AddPackage;
	}


	/// Not implemented
	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct RemovePackageReq {
		pub name: String
	}

	impl<B> Request<Action, B> for RemovePackageReq {
		type Response = ();
		type Error = Error;

		const ACTION: Action = Action::RemovePackage;
	}

	/// Not implemented
	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct UpdateReq;

	impl<B> Request<Action, B> for UpdateReq {
		type Response = ();
		type Error = Error;

		const ACTION: Action = Action::Update;
	}
}