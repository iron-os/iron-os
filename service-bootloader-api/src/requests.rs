
use crate::Request;

use serde::{Serialize, Deserialize};

use crypto::hash::Hash;
use crypto::signature::Signature;
use crypto::token::Token;

kind!{
	SystemdRestart,
	Restart,
	Shutdown,
	Disks,
	InstallOn,
	VersionInfo,
	MakeRoot,
	Update
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct InstallOn {
	pub disk: String
}

impl Request for InstallOn {
	type Response = ();
	fn kind() -> Kind { Kind::InstallOn }
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VersionInfoReq;

impl Request for VersionInfoReq {
	type Response = VersionInfo;
	fn kind() -> Kind { Kind::VersionInfo }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Architecture {
	Amd64,
	Arm64
}

// Should be equivalent to the DeviceId in packages-api
pub type DeviceId = Token<32>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
	pub board: String,
	pub arch: Architecture,
	pub product: String,
	pub version_str: String,
	pub version: Hash,
	pub signature: Option<Signature>,
	/// device id should exist if the device is installed
	pub device_id: Option<DeviceId>,
	pub installed: bool
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakeRoot {
	pub path: String
}

impl Request for MakeRoot {
	type Response = ();
	fn kind() -> Kind { Kind::MakeRoot }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReq {
	pub version_str: String,
	pub version: Hash,
	pub signature: Signature,
	// path to folder where the following files are located:
	// - bzImage
	// - rootfs.ext2
	pub path: String
}

impl Request for UpdateReq {
	type Response = VersionInfo;
	fn kind() -> Kind { Kind::Update }
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RestartReq;

impl Request for RestartReq {
	type Response = ();
	fn kind() -> Kind { Kind::Restart }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ShutdownReq;

impl Request for ShutdownReq {
	type Response = ();
	fn kind() -> Kind { Kind::Shutdown }
}