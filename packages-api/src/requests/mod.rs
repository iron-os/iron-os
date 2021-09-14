
/*
what api do we provide??

authenticate
- authenticate stream

all packages
- list all packages

package info
- get info about a package

image info (contains bzImage and rootfs)
- 

get file
- by hash (returns the file + a signature)
*/
#[macro_use]
mod macros;

use crate::error::{Error, Result};
use crate::message::{Action, Message};
use crate::packages::{Channel, Package, Image};

use tokio::fs::File;
use tokio::io::AsyncReadExt;

use stream::basic::request::{Request, Response};
use stream::packet::{EncryptedBytes, PacketError};

use serde::{Serialize, Deserialize};

use crypto::signature::Signature;
use crypto::hash::{Hasher, Hash};

use bytes::{BytesRead, BytesWrite};


// All packages

// #[derive(Debug, Serialize, Deserialize)]
// pub struct AllPackagesReq {
// 	pub channel: Channel
// }

// serde_req!(Action::AllPackages, AllPackagesReq, AllPackages);

// #[derive(Debug, Serialize, Deserialize)]
// pub struct AllPackages {
// 	pub list: Vec<Package>
// }

// serde_res!(AllPackages);


// Package Info

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageInfoReq {
	pub channel: Channel,
	pub name: String
}

serde_req!(Action::PackageInfo, PackageInfoReq, PackageInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageInfo {
	// there may not exist any info
	pub package: Option<Package>
}

serde_res!(PackageInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct SetPackageInfoReq {
	pub channel: Channel,
	pub package: Package
}

serde_req!(Action::SetPackageInfo, SetPackageInfoReq, SetPackageInfo);

#[derive(Debug)]
pub struct SetPackageInfo;

impl Response<Action, EncryptedBytes> for SetPackageInfo {
	fn into_message(self) -> stream::Result<Message> {
		Ok(Message::new())
	}
	fn from_message(_: Message) -> stream::Result<Self> {
		Ok(Self)
	}
}

// Image Info

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageInfoReq {
	// only Debug and Release are supported
	pub channel: Channel
}

serde_req!(Action::ImageInfo, ImageInfoReq, ImageInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageInfo {
	pub image: Option<Image>
}

serde_res!(ImageInfo);


// Get File

#[derive(Debug, Serialize, Deserialize)]
pub struct GetFileReq {
	pub hash: Hash,
	// pub start: Option<u64>,
	// pub end: Option<u64>
}

serde_req!(Action::GetFile, GetFileReq, GetFile);

#[derive(Debug)]
pub struct GetFile {
	inner: Message
}

impl GetFile {
	pub async fn new(_req: GetFileReq, mut file: File) -> Result<Self> {

		let mut msg = Message::new();

		// check how big the file is then allocate
		let buf_size = file.metadata().await
			.map(|m| m.len() as usize + 1)
			.unwrap_or(0);

		let mut body = msg.body_mut();
		body.reserve(buf_size);

		unsafe {
			// safe because file.read_to_end only appends
			let v = body.as_mut_vec();
			file.read_to_end(v).await
				.map_err(Error::io)?;
		}

		Ok(Self { inner: msg })
	}

	pub fn empty() -> Self {
		Self { inner: Message::new() }
	}

	pub fn is_empty(&self) -> bool {
		self.inner.body().len() == 0
	}

	pub fn file(&self) -> &[u8] {
		self.inner.body().inner()
	}

	/// creates a hash of the file
	pub fn hash(&self) -> Hash {
		Hasher::hash(self.file())
	}
}

impl Response<Action, EncryptedBytes> for GetFile {
	fn into_message(self) -> stream::Result<Message> {
		Ok(self.inner)
	}
	fn from_message(msg: Message) -> stream::Result<Self> {
		Ok(Self { inner: msg })
	}
}

/// This is temporary will be replaced with streams
#[derive(Debug)]
pub struct SetFileReq {
	signature: Signature,
	// message contains signature + 
	message: Message
}

impl SetFileReq {

	pub async fn new(sign: Signature, mut file: File) -> Result<Self> {

		let mut msg = Message::new();

		// check how big the file is then allocate
		let buf_size = file.metadata().await
			.map(|m| m.len() as usize + 1)
			.unwrap_or(0);

		let mut body = msg.body_mut();
		body.reserve(buf_size + Signature::LEN);

		body.write(sign.as_slice());

		unsafe {
			// safe because file.read_to_end only appends
			let v = body.as_mut_vec();
			file.read_to_end(v).await
				.map_err(Error::io)?;
		}

		Ok(Self {
			signature: sign,
			message: msg
		})
	}

	pub fn signature(&self) -> &Signature {
		&self.signature
	}

	/// creates a hash of the file
	pub fn hash(&self) -> Hash {
		Hasher::hash(self.file())
	}

	pub fn file(&self) -> &[u8] {
		let body = self.message.body();
		&body.inner()[Signature::LEN..]
	}

}

impl Request<Action, EncryptedBytes> for SetFileReq {
	type Response = SetFile;

	fn action() -> Action {
		Action::SetFile
	}
	fn into_message(self) -> stream::Result<Message> {
		Ok(self.message)
	}
	fn from_message(msg: Message) -> stream::Result<Self> {
		if msg.body().len() <= Signature::LEN {
			return Err(PacketError::Body("no signature".into()).into());
		}

		let sign = Signature::from_slice(
			msg.body().read(Signature::LEN)
		);

		Ok(Self {
			signature: sign,
			message: msg
		})
	}
}

#[derive(Debug)]
pub struct SetFile;

impl Response<Action, EncryptedBytes> for SetFile {
	fn into_message(self) -> stream::Result<Message> {
		Ok(Message::new())
	}
	fn from_message(_: Message) -> stream::Result<Self> {
		Ok(Self)
	}
}