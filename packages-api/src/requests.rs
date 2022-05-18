
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

use crate::error::{Error, Result};
use crate::action::Action;
use crate::packages::{Channel, Package, BoardArch, TargetArch};

use std::collections::HashSet;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

use stream_api::derive_serde_message;
use stream_api::message::{SerdeMessage, EncryptedBytes};
use stream_api::request::{Request, RawRequest};

use serde::{Serialize, Deserialize};

use crypto::signature::Signature;
use crypto::hash::{Hasher, Hash};
use crypto::token::Token;

use bytes::{BytesRead, BytesWrite};

type Message = stream_api::message::Message<Action, EncryptedBytes>;

pub type DeviceId = Token<32>;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfoReq {
	pub channel: Channel,
	pub arch: BoardArch,
	pub name: String,
	/// device_id gives the possibility to target an update
	/// specific for only one device
	pub device_id: Option<DeviceId>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
	// there may not exist any info
	pub package: Option<Package>
}

impl<B> Request<Action, B> for PackageInfoReq {
	type Response = PackageInfo;
	type Error = Error;
	const ACTION: Action = Action::PackageInfo;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPackageInfoReq {
	pub channel: Channel,
	pub package: Package,
	// if empty no whitelist is applied
	pub whitelist: HashSet<DeviceId>
}

impl<B> Request<Action, B> for SetPackageInfoReq {
	type Response = ();
	type Error = Error;
	const ACTION: Action = Action::SetPackageInfo;
}

// Get File

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFileReq {
	pub hash: Hash,
	// pub start: Option<u64>,
	// pub end: Option<u64>
}

derive_serde_message!(GetFileReq);

#[derive(Debug)]
pub struct GetFile {
	// we keep the message so no new allocation is done
	inner: Message
}

impl GetFile {
	/// Before sending this response you should check the hash
	pub async fn from_file(mut file: File) -> Result<Self> {

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
				.map_err(|e| Error::Other(
					format!("could not read file {}", e)
				))?;
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

impl SerdeMessage<Action, EncryptedBytes, Error> for GetFile {
	fn into_message(self) -> Result<Message> {
		Ok(self.inner)
	}

	fn from_message(msg: Message) -> Result<Self> {
		Ok(Self { inner: msg })
	}
}

impl RawRequest<Action, EncryptedBytes> for GetFileReq {
	type Response = GetFile;
	type Error = Error;
	const ACTION: Action = Action::GetFile;
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

		body.write(&sign);

		unsafe {
			// safe because file.read_to_end only appends
			let v = body.as_mut_vec();
			file.read_to_end(v).await
				.map_err(|e| Error::Other(
					format!("could not read file {}", e)
				))?;
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

impl SerdeMessage<Action, EncryptedBytes, Error> for SetFileReq {
	fn into_message(self) -> Result<Message> {
		Ok(self.message)
	}

	fn from_message(msg: Message) -> Result<Self> {
		if msg.body().len() <= Signature::LEN {
			return Err(Error::Request("no signature".into()))
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

impl RawRequest<Action, EncryptedBytes> for SetFileReq {
	type Response = ();
	type Error = Error;
	const ACTION: Action = Action::SetFile;
}

pub type AuthKey = Token<32>;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticationReq {
	pub key: AuthKey
}

impl<B> Request<Action, B> for AuthenticationReq {
	type Response = ();
	type Error = Error;
	const ACTION: Action = Action::Authentication;
}


#[derive(Debug, Serialize, Deserialize)]
pub struct NewAuthKeyReq {
	pub sign: Option<Signature>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewAuthKey {
	pub kind: NewAuthKeyKind,
	pub key: AuthKey
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NewAuthKeyKind {
	Challenge,
	NewKey
}

impl<B> Request<Action, B> for NewAuthKeyReq {
	type Response = NewAuthKey;
	type Error = Error;
	const ACTION: Action = Action::NewAuthKey;
}

// Changewhitelist

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeWhitelistReq {
	pub channel: Channel,
	pub arch: TargetArch,
	pub name: String,
	pub version: Hash,
	pub whitelist: HashSet<DeviceId>
}

impl<B> Request<Action, B> for ChangeWhitelistReq {
	type Response = ();
	type Error = Error;
	const ACTION: Action = Action::ChangeWhitelist;
}