
use crate::Action;
use crate::error::{Result, Error};
use crate::requests::system::{SystemInfoReq, SystemInfo, InstallOnReq};
use crate::requests::ui::OpenPageReq;
use crate::requests::packages::{
	ListPackagesReq, ListPackages,
	AddPackageReq, Package,
	RemovePackageReq,
	UpdateReq
};
use crate::requests::device::{
	DeviceInfoReq, DeviceInfo,
	SetPowerStateReq, PowerState,
	DisksReq, Disk,
	SetDisplayStateReq, SetDisplayBrightnessReq
};
use crate::requests::network::{
	AccessPointsReq, AccessPoints,
	ConnectionsReq, Connection,
	AddConnectionReq, AddConnectionKind,
	RemoveConnectionReq
};

use std::time::Duration;
use std::path::Path;

use stream::packet::PlainBytes;
use stream::client::Config;
use stream_api::client;

use tokio::net::UnixStream;

// long since pings are not implemented yet
const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
	inner: client::Client<Action, PlainBytes>
}

impl Client {
	pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
		let stream = UnixStream::connect(path).await
			.map_err(|e| Error::Internal(e.to_string()))?;

		Ok(Self {
			inner: client::Client::<_, PlainBytes>::new(stream, Config {
				timeout: TIMEOUT,
				body_limit: 0
			}, None)
		})
	}

	pub async fn system_info(&self) -> Result<SystemInfo> {
		self.inner.request(SystemInfoReq).await
	}

	pub async fn install_on(&self, disk: String) -> Result<()> {
		self.inner.request(InstallOnReq { disk }).await
	}

	pub async fn open_page(&self, url: String) -> Result<()> {
		self.inner.request(OpenPageReq { url }).await
	}

	pub async fn list_packages(&self) -> Result<ListPackages> {
		self.inner.request(ListPackagesReq).await
	}

	pub async fn add_package(&self, name: String) -> Result<Option<Package>> {
		self.inner.request(AddPackageReq { name }).await
			.map(|a| a.package)
	}

	pub async fn remove_package(&self, name: String) -> Result<()> {
		self.inner.request(RemovePackageReq { name }).await
	}

	pub async fn request_update(&self) -> Result<()> {
		self.inner.request(UpdateReq).await
	}

	pub async fn device_info(&self) -> Result<DeviceInfo> {
		self.inner.request(DeviceInfoReq).await
	}

	pub async fn set_power_state(&self, state: PowerState) -> Result<()> {
		self.inner.request(SetPowerStateReq { state }).await
	}

	pub async fn disks(&self) -> Result<Vec<Disk>> {
		self.inner.request(DisksReq).await
			.map(|d| d.0)
	}

	pub async fn set_display_state(&self, on: bool) -> Result<()> {
		self.inner.request(SetDisplayStateReq { on }).await
	}

	pub async fn set_display_brightness(&self, brightness: u8) -> Result<()> {
		self.inner.request(SetDisplayBrightnessReq { brightness }).await
	}

	pub async fn network_access_points(&self) -> Result<AccessPoints> {
		self.inner.request(AccessPointsReq).await
	}

	pub async fn network_connections(&self) -> Result<Vec<Connection>> {
		self.inner.request(ConnectionsReq).await
			.map(|c| c.list)
	}

	pub async fn network_add_connection(
		&self,
		id: String,
		kind: AddConnectionKind
	) -> Result<Connection> {
		self.inner.request(AddConnectionReq { id, kind }).await
	}

	pub async fn network_remove_connection(
		&self,
		uuid: String
	) -> Result<()> {
		self.inner.request(RemoveConnectionReq { uuid }).await
	}
}