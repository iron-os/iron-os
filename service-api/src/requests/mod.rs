
// - SystemInfo (VersionInfo, Packages)
// - DeviceInfo (Cpu, Disks, Ram, Processes?)
// - OpenPage (open a web page)
// - 
// - DisplayState (turn on off)

#[macro_use]
mod macros;

use crate::message::{Action, Message};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfoReq;

serde_req!(Action::SystemInfo, SystemInfoReq, SystemInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
	// equivalent of version_str
	pub version: String,
	pub packages: Vec<Package>,
	pub installed: bool
}

serde_res!(SystemInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
	pub name: String,
	pub version: String,
	pub path: String
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenPageReq {
	pub url: String
}

serde_req!(Action::OpenPage, OpenPageReq, OpenPage);

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenPage;

serde_res!(OpenPage);

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