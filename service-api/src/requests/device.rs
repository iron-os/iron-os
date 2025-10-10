//! Read and control various device features
//!
//! - get [DeviceInfos](struct.DeviceInfoReq.html) read cpu, ram, disk usage.
//! - set [DisplayState](struct.SetDisplayStateReq.html)
//! - set [PowerState](struct.SetPowerStateReq.html)

use crate::error::Error;
use crate::Action;

use bytes::BytesWrite;
use serde::{Deserialize, Serialize};

use stream::packet::PacketBytes;
use stream_api::{
	error::MessageError,
	message::{FromMessage, IntoMessage, Message},
	request::Request,
	FromMessage, IntoMessage,
};

use super::EmptyJson;

/// ## Important!!
///
/// Device info not implemented
#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct DeviceInfoReq;

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct DeviceInfo {
	pub cpu: CpuLoad,
	pub memory: MemoryUsage,
	pub active_disk: ActiveDisk,
	pub data: DataDisk, // display
	                    // touch
	                    // network
	                    // todo complete
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
	pub idletime: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUsage {
	// in bytes
	pub total: u64,
	pub available: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDisk {
	// in bytes
	pub name: String,
	pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDisk {
	// in bytes
	pub total: u64,
	pub used: u64,
}

impl Request for DeviceInfoReq {
	type Action = Action;
	type Response = DeviceInfo;
	type Error = Error;

	const ACTION: Action = Action::DeviceInfo;
}

/// not implemented
#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct SetPowerStateReq {
	pub state: PowerState,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PowerState {
	Shutdown,
	Restart,
}

impl Request for SetPowerStateReq {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::SetPowerState;
}

/// This request should only be used if `SystemInfo.installed == false`
#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct DisksReq;

/// The active disk will not be returned
#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct Disks(pub Vec<Disk>);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
	pub name: String,
	/// if this disk already has a valid gpt header
	pub initialized: bool,
	/// the size in bytes
	pub size: u64,
}

impl Request for DisksReq {
	type Action = Action;
	type Response = Disks;
	type Error = Error;

	const ACTION: Action = Action::Disks;
}

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct SetDisplayStateReq {
	pub on: bool,
}

impl Request for SetDisplayStateReq {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::SetDisplayState;
}

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct SetDisplayBrightnessReq {
	// 0-255
	pub brightness: u8,
}

impl Request for SetDisplayBrightnessReq {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::SetDisplayBrightness;
}

// Todo maybe a DisplayStateChange stream should be added
// for when the display get's waken up by touch

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct TakeScreenshotReq;

#[derive(Debug)]
pub struct Screenshot(pub Vec<u8>); // PNG data

impl Request for TakeScreenshotReq {
	type Action = Action;
	type Response = Screenshot;
	type Error = Error;

	const ACTION: Action = Action::TakeScreenshot;
}

impl<B> FromMessage<Action, B> for Screenshot
where
	B: PacketBytes,
{
	fn from_message(msg: Message<Action, B>) -> Result<Self, MessageError> {
		Ok(Self(msg.body().inner().to_vec()))
	}
}

impl<B> IntoMessage<Action, B> for Screenshot
where
	B: PacketBytes,
{
	fn into_message(self) -> Result<Message<Action, B>, MessageError> {
		let mut msg = Message::new();
		msg.body_mut().write(self.0);

		Ok(msg)
	}
}
