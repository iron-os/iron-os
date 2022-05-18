
use crate::action::Action;
use crate::error::{Result, Error};
use crate::packages::{Channel, Package, BoardArch, TargetArch};
use crate::requests::{
	PackageInfoReq, DeviceId,
	SetPackageInfoReq,
	GetFileReq, GetFile,
	SetFileReq,
	AuthKey, AuthenticationReq,
	NewAuthKeyReq, NewAuthKeyKind,
	ChangeWhitelistReq
};

use std::time::Duration;
use std::collections::HashSet;

use stream_api::client::{Config, Client as StreamClient, EncryptedBytes};

use crypto::signature::{PublicKey, Signature};
use crypto::hash::Hash;

use tokio::net::{TcpStream, ToSocketAddrs};

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
	inner: StreamClient<Action, EncryptedBytes>
}

impl Client {

	pub async fn connect<A>(addr: A, pub_key: PublicKey) -> Result<Self>
	where A: ToSocketAddrs {
		let stream = TcpStream::connect(addr).await
			.map_err(|e| Error::Other(format!("could not connect {}", e)))?;
		Ok(Self {
			inner: StreamClient::<_, EncryptedBytes>::new(
				stream,
				Config {
					timeout: TIMEOUT,
					body_limit: 0
				},
				None,
				pub_key
			)
		})
	}

	// pub async fn request<R>(&self, req: R) -> Result<R::Response>
	// where R: Request<Action, EncryptedBytes> {
	// 	self.inner.request(req).await
	// 		.map_err(Error::Stream)
	// }

	pub async fn package_info(
		&self,
		channel: Channel,
		arch: BoardArch,
		device_id: Option<DeviceId>,
		name: String
	) -> Result<Option<Package>> {
		let req = PackageInfoReq { channel, arch, name, device_id };
		self.inner.request(req).await
			.map(|r| r.package)
	}

	pub async fn set_package_info(
		&self,
		channel: Channel,
		package: Package,
		whitelist: HashSet<DeviceId>
	) -> Result<()> {
		let req = SetPackageInfoReq { channel, package, whitelist };
		self.inner.request(req).await
	}

	pub async fn get_file(&self, hash: Hash) -> Result<GetFile> {
		let req = GetFileReq { hash };
		self.inner.raw_request(req).await
	}

	pub async fn set_file(&self, req: SetFileReq) -> Result<()> {
		self.inner.raw_request(req).await
	}

	pub async fn authenticate(&self, key: AuthKey) -> Result<()> {
		self.inner.request(AuthenticationReq { key }).await
	}

	pub async fn auth_challenge(&self) -> Result<AuthKey> {
		let resp = self.inner.request(NewAuthKeyReq { sign: None }).await?;
		match resp.kind {
			NewAuthKeyKind::Challenge => Ok(resp.key),
			NewAuthKeyKind::NewKey => Err(Error::Response(
				"expected Challenge".into()
			))
		}
	}

	/// you need to sign the challenge
	pub async fn auth_key(&self, sign: Signature) -> Result<AuthKey> {
		let req = NewAuthKeyReq { sign: Some(sign) };
		let resp = self.inner.request(req).await?;
		match resp.kind {
			NewAuthKeyKind::NewKey => Ok(resp.key),
			NewAuthKeyKind::Challenge => Err(Error::Response(
				"expected Key".into()
			))
		}
	}

	pub async fn change_whitelist(
		&self,
		channel: Channel,
		arch: TargetArch,
		name: String,
		version: Hash,
		whitelist: HashSet<DeviceId>
	) -> Result<()> {
		self.inner.request(ChangeWhitelistReq {
			channel, arch, name, version, whitelist
		}).await
	}

	pub async fn close(self) {
		self.inner.close().await
	}

}