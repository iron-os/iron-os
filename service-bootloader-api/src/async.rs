use crate::error::{Error, Result};
use crate::requests::*;
use crate::Request;

use stdio_api::{deserialize, serialize, AsyncStdio, Kind as LineKind, Line};

pub struct AsyncClient {
	inner: AsyncStdio,
}

impl AsyncClient {
	pub fn new() -> Self {
		Self {
			inner: AsyncStdio::from_env(),
		}
	}

	async fn request<R>(&mut self, req: &R) -> Result<R::Response>
	where
		R: Request,
	{
		let line = Line::new(
			LineKind::Request,
			R::kind().as_str(),
			&serialize(req).map_err(|_| Error::SerializationError)?,
		);
		self.inner
			.write(&line)
			.await
			.map_err(|e| Error::ConnectionError(e.to_string()))?;
		let line = self
			.inner
			.read()
			.await
			.map_err(|e| Error::ConnectionError(e.to_string()))?
			.ok_or_else(|| Error::ConnectionClosed)?;

		if let LineKind::Request = line.kind() {
			return Err(Error::ConnectionError(
				"received request instead of response".into(),
			));
		}

		if line.key() != R::kind().as_str() {
			return Err(Error::ConnectionError(
				"received other key than requested".into(),
			));
		}

		// deserialized to Result<Response>
		deserialize(line.data()).map_err(|_| Error::DeserializationError)?
	}

	pub async fn systemd_restart(
		&mut self,
		name: impl Into<String>,
	) -> Result<()> {
		self.request(&SystemdRestart { name: name.into() }).await
	}

	pub async fn disks(&mut self) -> Result<Vec<Disk>> {
		self.request(&Disks).await
	}

	pub async fn install_on(&mut self, disk: impl Into<String>) -> Result<()> {
		self.request(&InstallOn { disk: disk.into() }).await
	}

	pub async fn version_info(&mut self) -> Result<VersionInfo> {
		self.request(&VersionInfoReq).await
	}

	pub async fn make_root(&mut self, path: impl Into<String>) -> Result<()> {
		self.request(&MakeRoot { path: path.into() }).await
	}

	pub async fn update(&mut self, req: &UpdateReq) -> Result<VersionInfo> {
		self.request(req).await
	}

	pub async fn restart(&mut self) -> Result<()> {
		self.request(&RestartReq).await
	}

	pub async fn shutdown(&mut self) -> Result<()> {
		self.request(&ShutdownReq).await
	}
}
