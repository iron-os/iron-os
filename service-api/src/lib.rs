#![doc = include_str!("../README.md")]

pub mod requests;
#[cfg(target_family = "unix")]
pub mod client;
#[cfg(target_family = "unix")]
pub mod server;
pub mod error;

pub use stream;

pub use stream_api::request_handler;
use stream_api::action;

action! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub enum Action {
		SystemInfo = 4,
		DeviceInfo = 7,
		OpenPage = 10,
		SetDisplayState = 13,
		SetDisplayBrightness = 14,
		Disks = 16,
		InstallOn = 17,
		SetPowerState = 20,
		// packages
		ListPackages = 30,
		AddPackage = 32,
		RemovePackage = 34,
		Update = 36,
		// storage
		GetStorage = 40,
		SetStorage = 42,
		RemoveStorage = 44,
		// network
		NetworkConnections = 50,
		NetworkAddConnection = 52,
		NetworkAccessPoints = 54
	}
}