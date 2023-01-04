use crate::action::Action;
use crate::error::{Result, Error};
use crate::packages::{Channel, Package, BoardArch, TargetArch};
use crate::requests::{
	PackageInfoReq, DeviceId,
	SetPackageInfoReq,
	GetFileReq, GetFile, GetFileBuilder,
	SetFileReq,
	AuthKey, AuthenticateReaderReq,
	AuthenticateWriter1Req, AuthenticateWriter2Req,
	NewAuthKeyReaderReq,
	ChangeWhitelistReq
};

use std::time::Duration;
use std::collections::HashSet;

use stream_api::client::{Config, Client as StreamClient, EncryptedBytes};

use crypto::signature::{PublicKey, Keypair};
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

	/// can be called by anyone
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

	/// can only be called if you authenticated as a writer
	pub async fn set_package_info(
		&self,
		package: Package,
		whitelist: HashSet<DeviceId>
	) -> Result<()> {
		let req = SetPackageInfoReq { package, whitelist };
		self.inner.request(req).await
	}

	/// can be called by anyone
	/// does not return FileNotFound
	pub async fn get_file(&self, hash: Hash) -> Result<GetFile> {
		let req = GetFileReq { hash };
		self.inner.raw_request(req).await
	}

	/// If this function returns Ok(())
	/// and the builder is not completed you can call this function again
	/// immediately
	/// 
	/// can return FileNotFound
	pub async fn get_file_with_builder(
		&self,
		builder: &mut GetFileBuilder
	) -> Result<()> {
		let r = builder.next_req();
		let resp = self.inner.raw_request(r).await?;
		builder.add_resp(resp);

		Ok(())
	}

	/// you need to be authentiacated as a writer
	pub async fn set_file(&self, req: SetFileReq) -> Result<()> {
		self.inner.raw_request(req).await
	}

	/// authenticate as reader
	pub async fn authenticate_reader(&self, key: AuthKey) -> Result<()> {
		self.inner.request(AuthenticateReaderReq { key }).await
	}

	/// authenticate as writer
	pub async fn authenticate_writer(
		&self,
		channel: &Channel,
		key: &Keypair
	) -> Result<()> {
		let resp = self.inner.request(AuthenticateWriter1Req {
			channel: *channel
		}).await?;
		self.inner.request(AuthenticateWriter2Req {
			signature: key.sign(&resp.challenge)
		}).await
	}

	/// need to be authenticate as a writer
	pub async fn new_auth_key_reader(&self) -> Result<AuthKey> {
		self.inner.request(NewAuthKeyReaderReq).await
	}

	/// need to be authenticate as a writer
	pub async fn change_whitelist(
		&self,
		arch: TargetArch,
		name: String,
		version: Hash,
		whitelist: HashSet<DeviceId>
	) -> Result<()> {
		self.inner.request(ChangeWhitelistReq {
			arch, name, version, whitelist
		}).await
	}

	pub async fn close(self) {
		self.inner.close().await
	}
}