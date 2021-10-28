//! Read and control various device features
//!
//! - get [DeviceInfos](struct.DeviceInfoReq.html) read cpu, ram, disk usage.  
//! - set [DisplayState](struct.SetDisplayStateReq.html)  
//! - set [PowerState](struct.SetPowerStateReq.html)

use crate::message::{Action, Message};

use serde::{Serialize, Deserialize};

/// ## Important!!
///
/// Device info not implemented 
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfoReq;

serde_req!(Action::DeviceInfo, DeviceInfoReq, DeviceInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
	pub cpu: CpuLoad,
	pub ram: RamUsage,
	pub full_disk: DiskUsage,
	pub data: DiskUsage,
	// display
	// touch
	// network
	// todo complete
}

serde_res!(DeviceInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuLoad {
	pub load_avg: (f32, f32, f32),
	pub cores: usize
	// tasks
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RamUsage {// in bytes
	pub total: u32,
	pub avail: u32
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskUsage {
	pub used: u32,
	pub avail: u32
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetPowerStateReq {
	pub state: PowerState
}

serde_req!(Action::SetPowerState, SetPowerStateReq, SetPowerState);

#[derive(Debug, Serialize, Deserialize)]
pub struct SetPowerState;

serde_res!(SetPowerState);

#[derive(Debug, Serialize, Deserialize)]
pub enum PowerState {
	Shutdown,
	Restart
}

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