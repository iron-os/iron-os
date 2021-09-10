
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

use crate::message::{Action, Message};
use crate::packages::{Package};

use stream::Result;
use stream::basic::request::Response;
use stream::packet::EncryptedBytes;

use serde::{Serialize, Deserialize};



// todo should we use this??
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
	Debug,
	Alpha,
	Beta,
	Release
}


// All packages

#[derive(Debug, Serialize, Deserialize)]
pub struct AllPackagesReq {
	pub channel: Channel
}

serde_req!(Action::AllPackages, AllPackagesReq, AllPackages);

#[derive(Debug, Serialize, Deserialize)]
pub struct AllPackages {
	pub list: Vec<Package>
}

serde_res!(AllPackages);


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


// Image Info

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageInfoReq {
	// only Debug and Release are supported
	pub channel: Channel
}

serde_req!(Action::ImageInfo, ImageInfoReq, ImageInfo);

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageInfo {

}

serde_res!(ImageInfo);


// Get File

#[derive(Debug, Serialize, Deserialize)]
pub struct GetFileReq {
	// only Debug and Release are supported
	pub hash: String,
	pub start: Option<u64>,
	pub end: Option<u64>
}

serde_req!(Action::GetFile, GetFileReq, GetFile);

#[derive(Debug)]
pub struct GetFile {
	inner: Message
}

impl Response<Action, EncryptedBytes> for GetFile {
	fn into_message(self) -> Result<Message> {
		Ok(self.inner)
	}
	fn from_message(msg: Message) -> Result<Self> {
		Ok(Self { inner: msg })
	}
}