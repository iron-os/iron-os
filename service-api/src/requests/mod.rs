
// - SystemInfo (VersionInfo, Packages)
// - DeviceInfo (Cpu, Disks, Ram, Processes?)
// - OpenPage (open a web page)
// - 
// - DisplayState (turn on off)

#[macro_use]
mod macros;

use crate::message::{Action, Message};
use serde::{Serialize, Deserialize};

pub mod system_info {

	use super::*;

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct SystemInfoReq;

	serde_req!(Action::SystemInfo, SystemInfoReq, SystemInfo);

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct SystemInfo {
		// equivalent of version_str
		pub version: String,
		pub packages: Vec<Package>,
		pub channel: Channel,
		pub installed: bool
	}

	serde_res!(SystemInfo);

	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct Package {
		pub name: String,
		pub version: String,
		pub path: String
	}

	#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
	pub enum Channel {
		Debug,
		Alpha,
		Beta,
		Release
	}

}

/// Not implemented
pub mod device_info {

	use super::*;

	// DeviceInfo
	#[derive(Debug, Serialize, Deserialize)]
	pub struct DeviceInfoReq;

	serde_req!(Action::DeviceInfo, DeviceInfoReq, DeviceInfo);

	#[derive(Debug, Serialize, Deserialize)]
	pub struct DeviceInfo {
		pub cpu: Cpu,
		pub ram: Ram,
		pub full_disk: Disk,
		pub data: Disk,
		// display
		// touch
		// network
	}

	serde_res!(DeviceInfo);

	#[derive(Debug, Serialize, Deserialize)]
	pub struct Cpu {
		pub load_avg: (f32, f32, f32),
		pub cores: usize
		// tasks
	}

	#[derive(Debug, Serialize, Deserialize)]
	pub struct Ram {// in bytes
		pub total: u32,
		pub avail: u32
	}

	#[derive(Debug, Serialize, Deserialize)]
	pub struct Disk {
		pub used: u32,
		pub avail: u32
	}

}

pub mod open_page {

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

pub mod set_display_state {

	use super::*;

	#[derive(Debug, Serialize, Deserialize)]
	pub struct SetDisplayStateReq {
		// 0-1
		pub brightness: f32,
		pub on: bool
	}

	serde_req!(Action::SetDisplayState, SetDisplayStateReq, SetDisplayState);

	#[derive(Debug, Serialize, Deserialize)]
	pub struct SetDisplayState;

	serde_res!(SetDisplayState);

}

pub mod disks {

	use super::*;

	/// This request should only be used if `SystemInfo.installed == false`
	#[derive(Debug, Serialize, Deserialize)]
	pub struct DisksReq;

	serde_req!(Action::Disks, DisksReq, Disks);

	/// The active disk will not be returned
	#[derive(Debug, Serialize, Deserialize)]
	pub struct Disks(pub Vec<Disk>);

	serde_res!(Disks);

	#[derive(Debug, Serialize, Deserialize)]
	pub struct Disk {
		pub name: String,
		/// if this disk already has a valid gpt header
		pub initialized: bool,
		/// the size in bytes
		pub size: u64
	}

}

pub mod install_on {

	use super::*;

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