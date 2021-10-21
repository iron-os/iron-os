
use crate::error::{Error, Result};

use std::result::Result as StdResult;
use std::collections::HashMap;
use std::borrow::Cow;

use tokio::fs;
use tokio::sync::RwLock;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{Error as SerdeError, IntoDeserializer};
use packages::packages::{Package, Channel, TargetArch, BoardArch};
use file_db::FileDb;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct IndexKey {
	channel: Channel,
	arch: TargetArch
}

impl Serialize for IndexKey {
	fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
	where S: Serializer {
		let s = format!("{}-{}", self.channel, self.arch);
		serializer.serialize_str(&s)
	}
}

impl<'de> Deserialize<'de> for IndexKey {
	fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
	where D: Deserializer<'de> {
		let s: Cow<'_, str> = Deserialize::deserialize(deserializer)?;
		let s = s.as_ref();
		let (channel, arch) = s.split_once('-')
			.ok_or_else(|| D::Error::custom("expected <channel>-<arch>"))?;
		let channel = Channel::deserialize(channel.into_deserializer())?;
		let arch = TargetArch::deserialize(arch.into_deserializer())?;

		Ok(Self { channel, arch })
	}
}

type IndexesV2 = HashMap<IndexKey, PackagesIndex>;

fn default_indexes_v2() -> IndexesV2 {
	IndexesV2::new()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesDbFile {
	// this index is threated as if everyone has TargetArch::Amd64
	indexes: HashMap<Channel, PackagesIndex>,
	// todo migrate after all packages where updated at least once
	#[serde(default = "default_indexes_v2")]
	indexes_v2: IndexesV2
}

impl PackagesDbFile {
	fn new() -> Self {
		Self {
			indexes: HashMap::new(),
			indexes_v2: IndexesV2::new()
		}
	}

	fn get(
		&self,
		arch: &BoardArch,
		channel: &Channel,
		name: &str
	) -> Option<&Package> {
		// first try indexes_v2

		let pack_v2 = self.get_v2(
			&(*arch).into(),
			channel,
			name
		).or_else(|| {
			self.get_v2(
				&TargetArch::Any,
				channel,
				name
			)
		});
		if let Some(p) = pack_v2 {
			return Some(p)
		}

		// since we introduces arm after we got indexes v2
		// we don't check v1
		if matches!(arch, BoardArch::Arm64) {
			return None
		}

		self.indexes.get(channel)
			.and_then(|idx| idx.get(name))
	}

	fn get_v2(
		&self,
		arch: &TargetArch,
		channel: &Channel,
		name: &str
	) -> Option<&Package> {
		self.indexes_v2.get(&IndexKey {
			channel: *channel,
			arch: *arch
		}).and_then(|idx| idx.get(name))
	}

	fn set(&mut self, channel: Channel, package: Package) {
		let index = self.indexes_v2.entry(IndexKey {
			channel,
			arch: package.arch
		}).or_insert_with(|| PackagesIndex::new());
		index.set(package);
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesIndex {
	list: HashMap<String, Package>
}

impl PackagesIndex {

	fn new() -> Self {
		Self {
			list: HashMap::new()
		}
	}

	fn get(&self, name: &str) -> Option<&Package> {
		self.list.get(name)
	}

	fn set(&mut self, package: Package) {
		self.list.insert(package.name.clone(), package);
	}

}

#[derive(Debug)]
pub struct PackagesDb {
	inner: RwLock<FileDb<PackagesDbFile>>
}

impl PackagesDb {

	pub async fn create() -> Result<Self> {
		if fs::metadata("./packages.fdb").await.is_ok() {
			return Self::read().await;
		}

		let db = FileDb::new("./packages.fdb", PackagesDbFile::new());
		db.write().await
			.map_err(|e| Error::new("could not write packages.fdb", e))?;

		Ok(Self {
			inner: RwLock::new(db)
		})
	}

	pub async fn read() -> Result<Self> {
		let db = FileDb::open("./packages.fdb").await
			.map_err(|e| Error::new("packages.fdb could not be opened", e))?;

		Ok(Self {
			inner: RwLock::new(db)
		})
	}

	pub async fn get_package(
		&self,
		arch: &BoardArch,
		channel: &Channel,
		name: &str
	) -> Option<Package> {
		let lock = self.inner.read().await;
		let db = lock.data();
		db.get(arch, channel, name)
			.map(Clone::clone)
	}

	pub async fn set_package(&self, channel: Channel, package: Package) {
		let mut lock = self.inner.write().await;
		let db = lock.data_mut();
		db.set(channel, package);
		lock.write().await
			.expect("writing failed unexpectetly")
	}

}

// process of updating a package
// create the file
// update packagesDb
// delete old file