use crate::error::{Error, Result};
use crate::action::Action;
use crate::packages::{Channel, Package, BoardArch, TargetArch};

use std::collections::HashSet;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

use stream_api::derive_serde_message;
use stream_api::message::{SerdeMessage, EncryptedBytes};
use stream_api::request::{Request, RawRequest};

use serde::{Serialize, Deserialize};

use crypto::signature::Signature;
use crypto::hash::{Hasher, Hash};
use crypto::token::Token;

use bytes::{BytesRead, BytesReadRef, BytesWrite};

type Message = stream_api::message::Message<Action, EncryptedBytes>;

pub type DeviceId = Token<32>;

// /// All packages
// ///
// /// Can be called by anyone
// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct AllPackagesReq {
// 	pub channel: Channel
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct AllPackages {
// 	pub list: Vec<Package>
// }

// impl<B> Request<Action, B> for AllPackagesReq {
// 	type Response = AllPackages;
// 	type Error = Error;
// 	const ACTION: Action = Action::AllPackages;
// }


/// Package Info
///
/// Can be called by anyone
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


/// Needs to be authenticated as a writer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPackageInfoReq {
	pub package: Package,
	// if empty no whitelist is applied
	pub whitelist: HashSet<DeviceId>
}

impl<B> Request<Action, B> for SetPackageInfoReq {
	type Response = ();
	type Error = Error;
	const ACTION: Action = Action::SetPackageInfo;
}


/// Get File
///
/// Can be accessed by anyone
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFileReq {
	pub hash: Hash
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
	start: u64,
	// can be longer that the file itself the returned file will tell you how
	// long it is
	len: u64
}


/// Get File Part
///
/// Can be accessed by anyone
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFilePartReq {
	pub hash: Hash,
	pub start: u64,
	// can be longer that the file itself the returned file will tell you how
	// long it is
	pub len: u64
}

derive_serde_message!(GetFilePartReq);

#[derive(Debug)]
pub struct GetFilePart {
	// we keep the message so no new allocation is done
	// +------------------------+
	// |          Body          |
	// +--------------+---------+
	// |Total File Len|File Part|
	// +--------------+---------+
	// |     u64      |   ...   |
	// +--------------+---------+
	inner: Message
}

impl GetFilePart {
	fn new(msg: Message) -> Result<Self> {
		// make sure the body is at least 8bytes long
		if msg.body().len() < 8 {
			return Err(Error::Request(
				"GetFilePart expects at least 8bytes".into()
			))
		}

		Ok(Self { inner: msg })
	}

	/// Before sending this response you should check the hash
	pub async fn from_file(
		mut file: File,
		start: u64,
		len: u64
	) -> Result<Self> {
		let mut msg = Message::new();

		let total_file_len = file.metadata().await
			.map_err(|e| Error::Internal(e.to_string()))?
			.len();

		// let's calculate how much we need to read
		let rem_max_len = total_file_len.checked_sub(start)
			.ok_or(Error::StartUnreachable)?;

		let len = len.min(rem_max_len);

		let mut body = msg.body_mut();
		body.reserve((8 + len + 1) as usize);
		body.write_u64(total_file_len);

		if len == 0 {
			return Ok(Self { inner: msg })
		}

		file.seek(SeekFrom::Start(start)).await
			.map_err(|e| Error::Internal(
				format!("seeking file failed {}", e)
			))?;

		// make sure only to read at max len
		let mut file_reader = file.take(len);

		unsafe {
			// safe because file.read_to_end only appends
			let v = body.as_mut_vec();
			file_reader.read_to_end(v).await
				.map_err(|e| Error::Internal(
					format!("could not read file {}", e)
				))?;
		}

		Ok(Self { inner: msg })
	}

	pub fn total_file_len(&self) -> u64 {
		let mut body = self.inner.body();
		body.read_u64()
	}

	pub fn file_part(&self) -> &[u8] {
		let mut body = self.inner.body();
		let _ = body.read_u64();
		body.remaining_ref()
	}
}

impl SerdeMessage<Action, EncryptedBytes, Error> for GetFilePart {
	fn into_message(self) -> Result<Message> {
		Ok(self.inner)
	}

	fn from_message(msg: Message) -> Result<Self> {
		Self::new(msg)
	}
}

impl RawRequest<Action, EncryptedBytes> for GetFilePartReq {
	type Response = GetFilePart;
	type Error = Error;
	const ACTION: Action = Action::GetFilePart;
}

/*
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFilePartReq {
	pub hash: Hash,
	pub start: u64,
	// can be longer that the file itself the returned file will tell you how
	// long it is
	pub len: u64
}
*/

#[derive(Debug, Clone)]
pub struct GetFileBuilder {
	hash: Hash,
	total_len: Option<u64>,
	bytes: Vec<u8>,
	// how big should each packet be
	part_size: u64
}

impl GetFileBuilder {
	/// part_size: how big should each part be, which we request
	pub fn new(hash: Hash, part_size: u64) -> Self {
		Self {
			hash, part_size,
			total_len: None,
			bytes: vec![]
		}
	}

	pub(crate) fn next_req(&self) -> GetFilePartReq {
		GetFilePartReq {
			hash: self.hash.clone(),
			start: self.bytes.len() as u64,
			len: self.part_size
		}
	}

	pub(crate) fn add_resp(&mut self, resp: GetFilePart) {
		self.total_len = Some(resp.total_file_len());
		self.bytes.extend_from_slice(resp.file_part());
	}

	pub fn is_complete(&self) -> bool {
		self.total_len.map(|l| self.bytes.len() as u64 >= l)
			.unwrap_or(false)
	}

	/// you need to make sure the file is complete
	pub fn file(&self) -> &[u8] {
		&self.bytes
	}

	/// creates a hash of the file
	pub fn hash(&self) -> Hash {
		Hasher::hash(self.file())
	}
}


/// Set a file
///
/// Needs to be authenticated as a writer
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
pub struct AuthenticateReaderReq {
	pub key: AuthKey
}

impl<B> Request<Action, B> for AuthenticateReaderReq {
	type Response = ();
	type Error = Error;
	const ACTION: Action = Action::AuthenticateReader;
}


#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticateWriter1Req {
	pub channel: Channel
}

pub type Challenge = Token<32>;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticateWriter1 {
	pub challenge: Challenge
}

impl<B> Request<Action, B> for AuthenticateWriter1Req {
	type Response = AuthenticateWriter1;
	type Error = Error;
	const ACTION: Action = Action::AuthenticateWriter1;
}


#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticateWriter2Req {
	pub signature: Signature
}

impl<B> Request<Action, B> for AuthenticateWriter2Req {
	type Response = ();
	type Error = Error;
	const ACTION: Action = Action::AuthenticateWriter2;
}


/// Needs to be authenticated as a writer
#[derive(Debug, Serialize, Deserialize)]
pub struct NewAuthKeyReaderReq;

impl<B> Request<Action, B> for NewAuthKeyReaderReq {
	type Response = AuthKey;
	type Error = Error;
	const ACTION: Action = Action::NewAuthKeyReader;
}


/// Changewhitelist
///
/// Needs to be authenticated as a writer
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeWhitelistReq {
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