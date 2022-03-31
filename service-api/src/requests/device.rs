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
	pub memory: MemoryUsage,
	pub active_disk: ActiveDisk,
	pub data: DataDisk
	// display
	// touch
	// network
	// todo complete
}

serde_res!(DeviceInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuLoad {
	pub cores: usize,
	/// Get the average of jobs in the queue or waiting for disk I/O.
	/// The values are averaged over (1 min, 5 min, 15 min).
	pub load_avg_1min: f32,
	pub load_avg_5min: f32,
	pub load_avg_15min: f32,
	pub runnable_threads: usize,
	pub running_threads: usize,
	/// uptime in seconds
	pub uptime: u64,
	/// Get the sum of how much time each core has spent idle.
	/// Should be idletime / cores to get the real idle time.
	pub idletime: u64
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryUsage {// in bytes
	pub total: u64,
	pub available: u64
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActiveDisk {// in bytes
	pub name: String,
	pub size: u64
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataDisk {// in bytes
	pub total: u64,
	pub used: u64
}

/// not implemented
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