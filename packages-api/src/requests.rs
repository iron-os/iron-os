use crate::action::Action;
use crate::error::{Error, Result};
use crate::packages::{BoardArch, Channel, Package, TargetArch};

use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::result::Result as StdResult;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

use stream_api::error::MessageError;
use stream_api::message::{FromMessage, IntoMessage, Message, PacketBytes};
use stream_api::request::Request;
use stream_api::{FromMessage, IntoMessage};

use serde::{Deserialize, Serialize};

use crypto::hash::{Hash, Hasher};
use crypto::signature::Signature;
use crypto::token::Token;

use bytes::{BytesRead, BytesReadRef, BytesWrite};

// type Message = stream_api::message::Message<Action, EncryptedBytes>;

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

#[derive(
	Debug, Clone, Default, Serialize, Deserialize, IntoMessage, FromMessage,
)]
#[message(json)]
pub struct EmptyJson;

fn is_false(b: &bool) -> bool {
	!b
}

/// Package Info
///
/// Can be called by anyone
#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct PackageInfoReq {
	pub channel: Channel,
	pub arch: BoardArch,
	pub name: String,
	/// device_id gives the possibility to target an update
	/// specific for only one device
	#[serde(skip_serializing_if = "Option::is_none")]
	pub device_id: Option<DeviceId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub image_version: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub package_versions: Option<HashMap<String, String>>,
	/// if true the whitelist and version requirements are ignored
	#[serde(default, skip_serializing_if = "is_false")]
	pub ignore_requirements: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct PackageInfo {
	// there may not exist any info
	pub package: Option<Package>,
}

impl Request for PackageInfoReq {
	type Action = Action;
	type Response = PackageInfo;
	type Error = Error;

	const ACTION: Action = Action::PackageInfo;
}

/// Needs to be authenticated as a writer
#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct SetPackageInfoReq {
	pub package: Package,
	// if empty no whitelist is applied
	pub whitelist: HashSet<DeviceId>,
	#[serde(default)]
	pub auto_whitelist_limit: u32,
}

impl Request for SetPackageInfoReq {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::SetPackageInfo;
}

/// Get File
///
/// Can be accessed by anyone
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFileReq<B> {
	pub hash: Hash,
	#[serde(skip)]
	_bytes: PhantomData<B>,
}

impl<B> GetFileReq<B> {
	pub fn new(hash: Hash) -> Self {
		Self {
			hash,
			_bytes: PhantomData,
		}
	}
}

impl<B> IntoMessage<Action, B> for GetFileReq<B>
where
	B: PacketBytes,
{
	fn into_message(self) -> StdResult<Message<Action, B>, MessageError> {
		stream_api::encdec::json::encode(self)
	}
}

impl<B> FromMessage<Action, B> for GetFileReq<B>
where
	B: PacketBytes,
{
	fn from_message(msg: Message<Action, B>) -> StdResult<Self, MessageError> {
		stream_api::encdec::json::decode(msg)
	}
}

#[derive(Debug)]
pub struct GetFile<B> {
	// we keep the message so no new allocation is done
	inner: Message<Action, B>,
}

impl<B> GetFile<B>
where
	B: PacketBytes,
{
	/// Before sending this response you should check the hash
	pub async fn from_file(mut file: File) -> Result<Self> {
		let mut msg = Message::new();

		// check how big the file is then allocate
		let buf_size = file
			.metadata()
			.await
			.map(|m| m.len() as usize + 1)
			.unwrap_or(0);

		let mut body = msg.body_mut();
		body.reserve(buf_size);

		unsafe {
			// safe because file.read_to_end only appends
			let v = body.as_mut_vec();
			file.read_to_end(v).await.map_err(|e| {
				Error::Other(format!("could not read file {}", e))
			})?;
		}

		Ok(Self { inner: msg })
	}

	pub fn empty() -> Self {
		Self {
			inner: Message::new(),
		}
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

impl<B> IntoMessage<Action, B> for GetFile<B>
where
	B: PacketBytes,
{
	fn into_message(self) -> StdResult<Message<Action, B>, MessageError> {
		Ok(self.inner)
	}
}

impl<B> FromMessage<Action, B> for GetFile<B>
where
	B: PacketBytes,
{
	fn from_message(msg: Message<Action, B>) -> StdResult<Self, MessageError> {
		Ok(Self { inner: msg })
	}
}

impl<B> Request for GetFileReq<B> {
	type Action = Action;
	type Response = GetFile<B>;
	type Error = Error;

	const ACTION: Action = Action::GetFile;
}

/// Get File Part
///
/// Can be accessed by anyone
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFilePartReq<B> {
	pub hash: Hash,
	pub start: u64,
	// can be longer that the file itself the returned file will tell you how
	// long it is
	pub len: u64,
	#[serde(skip)]
	_bytes: PhantomData<B>,
}

impl<B> GetFilePartReq<B> {
	pub fn new(hash: Hash, start: u64, len: u64) -> Self {
		Self {
			hash,
			start,
			len,
			_bytes: PhantomData,
		}
	}
}

impl<B> IntoMessage<Action, B> for GetFilePartReq<B>
where
	B: PacketBytes,
{
	fn into_message(self) -> StdResult<Message<Action, B>, MessageError> {
		stream_api::encdec::json::encode(self)
	}
}

impl<B> FromMessage<Action, B> for GetFilePartReq<B>
where
	B: PacketBytes,
{
	fn from_message(msg: Message<Action, B>) -> StdResult<Self, MessageError> {
		stream_api::encdec::json::decode(msg)
	}
}

#[derive(Debug)]
pub struct GetFilePart<B> {
	// we keep the message so no new allocation is done
	// +------------------------+
	// |          Body          |
	// +--------------+---------+
	// |Total File Len|File Part|
	// +--------------+---------+
	// |     u64      |   ...   |
	// +--------------+---------+
	inner: Message<Action, B>,
}

impl<B> GetFilePart<B>
where
	B: PacketBytes,
{
	fn new(msg: Message<Action, B>) -> Result<Self> {
		// make sure the body is at least 8bytes long
		if msg.body().len() < 8 {
			return Err(Error::Request(
				"GetFilePart expects at least 8bytes".into(),
			));
		}

		Ok(Self { inner: msg })
	}

	/// Before sending this response you should check the hash
	pub async fn from_file(
		mut file: File,
		start: u64,
		len: u64,
	) -> Result<Self> {
		let mut msg = Message::new();

		let total_file_len = file
			.metadata()
			.await
			.map_err(|e| Error::Internal(e.to_string()))?
			.len();

		// let's calculate how much we need to read
		let rem_max_len = total_file_len
			.checked_sub(start)
			.ok_or(Error::StartUnreachable)?;

		let len = len.min(rem_max_len);

		let mut body = msg.body_mut();
		body.reserve((8 + len + 1) as usize);
		body.write_u64(total_file_len);

		if len == 0 {
			return Ok(Self { inner: msg });
		}

		file.seek(SeekFrom::Start(start)).await.map_err(|e| {
			Error::Internal(format!("seeking file failed {}", e))
		})?;

		// make sure only to read at max len
		let mut file_reader = file.take(len);

		unsafe {
			// safe because file.read_to_end only appends
			let v = body.as_mut_vec();
			file_reader.read_to_end(v).await.map_err(|e| {
				Error::Internal(format!("could not read file {}", e))
			})?;
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

impl<B> IntoMessage<Action, B> for GetFilePart<B>
where
	B: PacketBytes,
{
	fn into_message(self) -> StdResult<Message<Action, B>, MessageError> {
		Ok(self.inner)
	}
}

impl<B> FromMessage<Action, B> for GetFilePart<B>
where
	B: PacketBytes,
{
	fn from_message(msg: Message<Action, B>) -> StdResult<Self, MessageError> {
		Self::new(msg).map_err(|e| MessageError::Other(e.to_string().into()))
	}
}

impl<B> Request for GetFilePartReq<B> {
	type Action = Action;
	type Response = GetFilePart<B>;
	type Error = Error;

	const ACTION: Action = Action::GetFilePart;
}

#[derive(Debug, Clone)]
pub struct GetFileBuilder {
	hash: Hash,
	total_len: Option<u64>,
	bytes: Vec<u8>,
	// how big should each packet be
	part_size: u64,
}

impl GetFileBuilder {
	/// part_size: how big should each part be, which we request
	pub fn new(hash: Hash, part_size: u64) -> Self {
		Self {
			hash,
			part_size,
			total_len: None,
			bytes: vec![],
		}
	}

	#[doc(hidden)]
	pub fn next_req<B>(&self) -> GetFilePartReq<B> {
		GetFilePartReq::new(
			self.hash.clone(),
			// start
			self.bytes.len() as u64,
			// len
			self.part_size,
		)
	}

	#[doc(hidden)]
	pub fn add_resp<B>(&mut self, resp: GetFilePart<B>)
	where
		B: PacketBytes,
	{
		self.total_len = Some(resp.total_file_len());
		self.bytes.extend_from_slice(resp.file_part());
	}

	pub fn is_complete(&self) -> bool {
		self.total_len
			.map(|l| self.bytes.len() as u64 >= l)
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
pub struct SetFileReq<B> {
	signature: Signature,
	// message contains signature +
	message: Message<Action, B>,
}

impl<B> SetFileReq<B>
where
	B: PacketBytes,
{
	pub async fn new(sign: Signature, mut file: File) -> Result<Self> {
		let mut msg = Message::new();

		// check how big the file is then allocate
		let buf_size = file
			.metadata()
			.await
			.map(|m| m.len() as usize + 1)
			.unwrap_or(0);

		let mut body = msg.body_mut();
		body.reserve(buf_size + Signature::LEN);

		body.write(sign.to_bytes());

		unsafe {
			// safe because file.read_to_end only appends
			let v = body.as_mut_vec();
			file.read_to_end(v).await.map_err(|e| {
				Error::Other(format!("could not read file {}", e))
			})?;
		}

		Ok(Self {
			signature: sign,
			message: msg,
		})
	}

	pub fn from_bytes(sign: Signature, ctn: &[u8]) -> Self {
		let mut msg = Message::new();

		let mut body = msg.body_mut();
		body.write(sign.to_bytes());
		body.write(ctn);

		Self {
			signature: sign,
			message: msg,
		}
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

impl<B> IntoMessage<Action, B> for SetFileReq<B>
where
	B: PacketBytes,
{
	fn into_message(self) -> StdResult<Message<Action, B>, MessageError> {
		Ok(self.message)
	}
}

impl<B> FromMessage<Action, B> for SetFileReq<B>
where
	B: PacketBytes,
{
	fn from_message(msg: Message<Action, B>) -> StdResult<Self, MessageError> {
		if msg.body().len() <= Signature::LEN {
			return Err(MessageError::Other("no signature".into()));
		}

		let sign = Signature::from_slice(msg.body().read(Signature::LEN));

		Ok(Self {
			signature: sign,
			message: msg,
		})
	}
}

impl<B> Request for SetFileReq<B> {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::SetFile;
}

pub type AuthKey = Token<32>;

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct AuthenticateReaderReq {
	pub key: AuthKey,
}

impl Request for AuthenticateReaderReq {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::AuthenticateReader;
}

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct AuthenticateWriter1Req {
	pub channel: Channel,
}

pub type Challenge = Token<32>;

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct AuthenticateWriter1 {
	pub challenge: Challenge,
}

impl Request for AuthenticateWriter1Req {
	type Action = Action;
	type Response = AuthenticateWriter1;
	type Error = Error;

	const ACTION: Action = Action::AuthenticateWriter1;
}

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct AuthenticateWriter2Req {
	pub signature: Signature,
}

impl Request for AuthenticateWriter2Req {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::AuthenticateWriter2;
}

/// Needs to be authenticated as a writer
#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct NewAuthKeyReaderReq;

#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
#[repr(transparent)]
pub struct NewAuthKeyReader(pub AuthKey);

impl Request for NewAuthKeyReaderReq {
	type Action = Action;
	type Response = NewAuthKeyReader;
	type Error = Error;

	const ACTION: Action = Action::NewAuthKeyReader;
}

/// Changewhitelist
///
/// Needs to be authenticated as a writer
#[derive(Debug, Serialize, Deserialize, IntoMessage, FromMessage)]
#[message(json)]
pub struct ChangeWhitelistReq {
	pub arch: TargetArch,
	pub name: String,
	pub version: Hash,
	pub change: WhitelistChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WhitelistChange {
	Set(HashSet<DeviceId>),
	Add(HashSet<DeviceId>),
	// this will not reduce the whitelist amount
	SetMinAuto(u32),
	AddAuto(u32),
}

impl Request for ChangeWhitelistReq {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::ChangeWhitelist;
}
