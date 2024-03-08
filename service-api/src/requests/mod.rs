// - SystemInfo (VersionInfo, Packages)
// - DeviceInfo (Cpu, Disks, Ram, Processes?)
// - OpenPage (open a web page)
// -
// - DisplayState (turn on off)

pub mod device;
pub mod network;

use crate::error::Error;
use crate::Action;

use serde::{Deserialize, Serialize};

use stream_api::{request::Request, FromMessage, IntoMessage};

#[derive(
	Debug, Clone, Default, Serialize, Deserialize, IntoMessage, FromMessage,
)]
#[message(json)]
pub struct EmptyJson;

// todo maybe rename os? or system
pub mod system {

	//! Control various operating system related features.
	//!
	//! - get [SystemInfo](struct.SystemInfoReq.html)
	//! - trigger [InstallOn](struct.InstallOnReq.html)

	use super::*;

	pub use packages_api::packages::Channel;
	pub use packages_api::requests::DeviceId;

	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[message(json)]
	pub struct SystemInfoReq;

	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct SystemInfo {
		// equivalent of version_str
		pub version: String,
		pub board: String,
		pub product: String,
		pub packages: Vec<ShortPackage>,
		pub channel: Channel,
		pub device_id: Option<DeviceId>,
		pub installed: bool,
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct ShortPackage {
		pub name: String,
		pub version: String,
		pub path: String,
	}

	impl Request for SystemInfoReq {
		type Action = Action;
		type Response = SystemInfo;
		type Error = Error;

		const ACTION: Action = Action::SystemInfo;
	}

	/// This request should only be used if `SystemInfo.installed == false`
	#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct InstallOnReq {
		/// The name of a disk that is returned from DisksReq
		pub disk: String,
	}

	impl Request for InstallOnReq {
		type Action = Action;
		type Response = EmptyJson;
		type Error = Error;

		const ACTION: Action = Action::InstallOn;
	}
}

pub mod ui {

	//! Control various web features.
	//!
	//! - trigger [OpenPage](struct.OpenPageReq.html)

	use super::*;

	#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct OpenPageReq {
		pub url: String,
	}

	impl Request for OpenPageReq {
		type Action = Action;
		type Response = EmptyJson;
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

	pub use packages_api::packages::{Channel, Hash, Signature, Source};

	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[message(json)]
	pub struct ListPackagesReq;

	/// if you need a detailed list of packages
	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct ListPackages {
		pub packages: Vec<Package>,
		pub sources: Vec<Source>,
		pub channel: Channel,
		pub on_run: String,
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
		pub path: String,
	}

	impl Request for ListPackagesReq {
		type Action = Action;
		type Response = ListPackages;
		type Error = Error;

		const ACTION: Action = Action::ListPackages;
	}

	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct AddPackageReq {
		pub name: String,
	}

	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct AddPackage {
		/// Returns None if the package was not found
		pub package: Option<Package>,
	}

	impl Request for AddPackageReq {
		type Action = Action;
		type Response = AddPackage;
		type Error = Error;

		const ACTION: Action = Action::AddPackage;
	}

	/// Not implemented
	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct RemovePackageReq {
		pub name: String,
	}

	impl Request for RemovePackageReq {
		type Action = Action;
		type Response = EmptyJson;
		type Error = Error;

		const ACTION: Action = Action::RemovePackage;
	}

	/// Not implemented
	#[derive(
		Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage,
	)]
	#[serde(rename_all = "camelCase")]
	#[message(json)]
	pub struct UpdateReq;

	impl Request for UpdateReq {
		type Action = Action;
		type Response = EmptyJson;
		type Error = Error;

		const ACTION: Action = Action::Update;
	}
}
