use crate::error::{Error, Result};

use std::result::Result as StdResult;
use std::collections::{HashMap, HashSet};
use std::borrow::Cow;

use tokio::fs;
use tokio::sync::RwLock;

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{Error as SerdeError, IntoDeserializer};

use packages::packages::{Package, Channel, Hash, TargetArch, BoardArch};
use packages::requests::DeviceId;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesDbFile {
	indexes: HashMap<IndexKey, PackagesIndex>
}

impl PackagesDbFile {
	fn new() -> Self {
		Self {
			indexes: HashMap::new()
		}
	}

	fn get(
		&self,
		arch: &BoardArch,
		channel: &Channel,
		name: &str,
		device_id: &Option<DeviceId>
	) -> Option<&PackageEntry> {
		self.get_inner(&(*arch).into(), channel, name, device_id)
			.or_else(|| {
				self.get_inner(&TargetArch::Any, channel, name, device_id)
			})
	}

	fn get_inner(
		&self,
		arch: &TargetArch,
		channel: &Channel,
		name: &str,
		device_id: &Option<DeviceId>
	) -> Option<&PackageEntry> {
		self.indexes.get(&IndexKey {
			channel: *channel,
			arch: *arch
		}).and_then(|idx| idx.get_latest(name, device_id))
	}

	fn push(&mut self, channel: Channel, entry: PackageEntry) {
		// entry: PackageEntry
		let index = self.indexes.entry(IndexKey {
			channel,
			arch: entry.package.arch
		}).or_insert_with(|| PackagesIndex::new());
		index.push(entry);
	}

	fn change_whitelist(
		&mut self,
		channel: &Channel,
		arch: &TargetArch,
		name: &str,
		version: &Hash,
		whitelist: HashSet<DeviceId>,
		add: bool
	) -> bool {
		let entry = self.indexes.get_mut(&IndexKey {
			channel: *channel,
			arch: *arch
		}).and_then(|i| i.mut_with_version(name, version));

		if let Some(entry) = entry {
			if add {
				for dev in whitelist {
					entry.whitelist.insert(dev);
				}
			} else {
				entry.whitelist = whitelist;
			}

			true
		} else {
			false
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
	pub package: Package,
	// if the whitelist is empty this means that all devices are allowed
	// to use the package
	pub whitelist: HashSet<DeviceId>
}

impl PackageEntry {
	pub fn in_whitelist(&self, device_id: &Option<DeviceId>) -> bool {
		match device_id {
			None => self.whitelist.is_empty(),
			Some(_) if self.whitelist.is_empty() => true,
			Some(device_id) => self.whitelist.contains(device_id)
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesIndex {
	// last package Entry is the current entry
	list: HashMap<String, Vec<PackageEntry>>
}

impl PackagesIndex {
	fn new() -> Self {
		Self {
			list: HashMap::new()
		}
	}

	fn get_latest(
		&self,
		name: &str,
		device_id: &Option<DeviceId>
	) -> Option<&PackageEntry> {
		self.list.get(name)?
			.iter().rev()
			.find(|e| {
				e.in_whitelist(device_id)
			})
	}

	fn mut_with_version(
		&mut self,
		name: &str,
		version: &Hash
	) -> Option<&mut PackageEntry> {
		self.list.get_mut(name)?
			.iter_mut().rev()
			.find(|e| {
				&e.package.version == version
			})
	}

	// push or updates a existing Entry
	fn push(&mut self, entry: PackageEntry) {
		let name = entry.package.name.clone();
		let list = self.list.entry(name)
			.or_insert(vec![]);

		let maybe_entry = list.iter_mut()
			.find(|e| e.package.version == entry.package.version);

		match maybe_entry {
			Some(stored_entry) => {
				// the version is the same
				// so override the current package
				*stored_entry = entry;
			},
			None => {
				list.push(entry);
			}
		}
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
		name: &str,
		device_id: &Option<DeviceId>
	) -> Option<PackageEntry> {
		let lock = self.inner.read().await;
		let db = lock.data();
		db.get(arch, channel, name, device_id)
			.map(Clone::clone)
	}

	pub async fn push_package(&self, channel: Channel, entry: PackageEntry) {
		let mut lock = self.inner.write().await;
		let db = lock.data_mut();
		db.push(channel, entry);
		lock.write().await
			.expect("writing failed unexpectetly")
	}

	// returns true if the whitelist could be changed
	pub async fn change_whitelist(
		&self,
		channel: &Channel,
		arch: &TargetArch,
		name: &str,
		version: &Hash,
		whitelist: HashSet<DeviceId>,
		add: bool
	) -> bool {
		let mut lock = self.inner.write().await;
		let db = lock.data_mut();
		let r = db.change_whitelist(
			channel,
			arch,
			name,
			version,
			whitelist,
			add
		);
		lock.write().await
			.expect("writing failed unexpectetly");
		r
	}
}

// process of updating a package
// create the file
// update packagesDb
// delete old file