use crate::action::Action;
use crate::error::{Error, Result};
use crate::packages::{BoardArch, Channel, Package, TargetArch};
use crate::requests::{
	AuthKey, AuthenticateReaderReq, AuthenticateWriter1Req,
	AuthenticateWriter2Req, ChangeWhitelistReq, DeviceId, GetFile,
	GetFileBuilder, GetFileReq, NewAuthKeyReaderReq, PackageInfoReq,
	SetFileReq, SetPackageInfoReq,
};

use std::collections::HashSet;
use std::time::Duration;

use stream_api::client::{Client as StreamClient, Config, EncryptedBytes};

use crypto::hash::Hash;
use crypto::signature::{Keypair, PublicKey};

use tokio::net::{TcpStream, ToSocketAddrs};

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
	inner: StreamClient<Action, EncryptedBytes>,
}

impl Client {
	pub async fn connect<A>(addr: A, pub_key: PublicKey) -> Result<Self>
	where
		A: ToSocketAddrs,
	{
		let stream = TcpStream::connect(addr)
			.await
			.map_err(|e| Error::Other(format!("could not connect {}", e)))?;
		Ok(Self {
			inner: StreamClient::<_, EncryptedBytes>::new_encrypted(
				stream,
				Config {
					timeout: TIMEOUT,
					body_limit: 0,
				},
				None,
				pub_key,
			),
		})
	}

	/// can be called by anyone
	pub async fn package_info(
		&self,
		channel: Channel,
		arch: BoardArch,
		device_id: Option<DeviceId>,
		name: String,
	) -> Result<Option<Package>> {
		let req = PackageInfoReq {
			channel,
			arch,
			name,
			device_id,
		};
		self.inner.request(req).await.map(|r| r.package)
	}

	/// can only be called if you authenticated as a writer
	pub async fn set_package_info(
		&self,
		package: Package,
		whitelist: HashSet<DeviceId>,
		auto_whitelist_limit: u32,
	) -> Result<()> {
		let req = SetPackageInfoReq {
			package,
			whitelist,
			auto_whitelist_limit,
		};
		self.inner.request(req).await.map(|_r| ())
	}

	/// can be called by anyone
	/// does not return FileNotFound
	pub async fn get_file(
		&self,
		hash: Hash,
	) -> Result<GetFile<EncryptedBytes>> {
		let req = GetFileReq::new(hash);
		self.inner.request(req).await
	}

	/// If this function returns Ok(())
	/// and the builder is not completed you can call this function again
	/// immediately
	///
	/// can return FileNotFound
	pub async fn get_file_with_builder(
		&self,
		builder: &mut GetFileBuilder,
	) -> Result<()> {
		let r = builder.next_req();
		let resp = self.inner.request(r).await?;
		builder.add_resp(resp);

		Ok(())
	}

	/// you need to be authentiacated as a writer
	pub async fn set_file(
		&self,
		req: SetFileReq<EncryptedBytes>,
	) -> Result<()> {
		self.inner.request(req).await.map(|_| ())
	}

	/// authenticate as reader
	pub async fn authenticate_reader(&self, key: AuthKey) -> Result<()> {
		self.inner
			.request(AuthenticateReaderReq { key })
			.await
			.map(|_| ())
	}

	/// authenticate as writer
	pub async fn authenticate_writer(
		&self,
		channel: &Channel,
		key: &Keypair,
	) -> Result<()> {
		let resp = self
			.inner
			.request(AuthenticateWriter1Req { channel: *channel })
			.await?;
		self.inner
			.request(AuthenticateWriter2Req {
				signature: key.sign(&resp.challenge),
			})
			.await
			.map(|_| ())
	}

	/// need to be authenticate as a writer
	pub async fn new_auth_key_reader(&self) -> Result<AuthKey> {
		self.inner.request(NewAuthKeyReaderReq).await.map(|r| r.0)
	}

	/// Allows to change the whitelist. The whitelist can either be replaced
	/// or can be additive.
	///
	/// ## Auth
	/// need to be authenticate as a writer.
	pub async fn change_whitelist(
		&self,
		arch: TargetArch,
		name: String,
		version: Hash,
		whitelist: HashSet<DeviceId>,
		// if the whitelist should added or replaced
		add: bool,
		auto_whitelist_limit: u32,
	) -> Result<()> {
		self.inner
			.request(ChangeWhitelistReq {
				arch,
				name,
				version,
				whitelist,
				add,
				auto_whitelist_limit,
			})
			.await
			.map(|_| ())
	}

	pub async fn close(self) {
		let _ = self.inner.close().await;
	}
}
