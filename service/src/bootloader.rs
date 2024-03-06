use std::sync::Arc;

use bootloader_api::requests::{Disk, UpdateReq, VersionInfo};
use bootloader_api::{error::Result, AsyncClient};

use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Bootloader {
	inner: Arc<Mutex<AsyncClient>>,
}

impl Bootloader {
	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(AsyncClient::new())),
		}
	}

	pub async fn systemd_restart(&self, name: impl Into<String>) -> Result<()> {
		self.inner.lock().await.systemd_restart(name).await
	}

	pub async fn disks(&self) -> Result<Vec<Disk>> {
		self.inner.lock().await.disks().await
	}

	pub async fn install_on(&self, disk: impl Into<String>) -> Result<()> {
		self.inner.lock().await.install_on(disk).await
	}

	pub async fn version_info(&self) -> Result<VersionInfo> {
		self.inner.lock().await.version_info().await
	}

	pub async fn make_root(&self, path: impl Into<String>) -> Result<()> {
		self.inner.lock().await.make_root(path).await
	}

	pub async fn update(&self, req: &UpdateReq) -> Result<VersionInfo> {
		self.inner.lock().await.update(req).await
	}

	pub async fn restart(&self) -> Result<()> {
		self.inner.lock().await.restart().await
	}

	pub async fn shutdown(&self) -> Result<()> {
		self.inner.lock().await.shutdown().await
	}
}
