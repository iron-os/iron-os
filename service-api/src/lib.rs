#![doc = include_str!("../README.md")]

#[cfg(target_family = "unix")]
pub mod client;
pub mod error;
pub mod requests;
#[cfg(target_family = "unix")]
pub mod server;

pub use stream;

use stream_api::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Action)]
#[repr(u16)]
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
	NetworkRemoveConnection = 53,
	NetworkAccessPoints = 54,
	// screenshots
	TakeScreenshot = 60,
}
