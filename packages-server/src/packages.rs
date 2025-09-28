use crate::error::{Error, Result};
use crate::Config;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::result::Result as StdResult;

use semver::VersionReq;
use tokio::fs;
use tokio::sync::RwLock;

use serde::de::{Error as SerdeError, IntoDeserializer};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use packages::packages::{BoardArch, Channel, Hash, Package, TargetArch};
use packages::requests::{DeviceId, WhitelistChange};

use file_db::FileDb;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct IndexKey {
	channel: Channel,
	arch: TargetArch,
}

impl Serialize for IndexKey {
	fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let s = format!("{}-{}", self.channel, self.arch);
		serializer.serialize_str(&s)
	}
}

impl<'de> Deserialize<'de> for IndexKey {
	fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s: Cow<'_, str> = Deserialize::deserialize(deserializer)?;
		let s = s.as_ref();
		let (channel, arch) = s
			.split_once('-')
			.ok_or_else(|| D::Error::custom("expected <channel>-<arch>"))?;
		let channel = Channel::deserialize(channel.into_deserializer())?;
		let arch = TargetArch::deserialize(arch.into_deserializer())?;

		Ok(Self { channel, arch })
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryIndex {
	packages_index: IndexKey,
	name: String,
	idx: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesDbFile {
	indexes: HashMap<IndexKey, PackagesIndex>,
}

impl PackagesDbFile {
	fn new() -> Self {
		Self {
			indexes: HashMap::new(),
		}
	}

	pub fn get(&self, idx: &EntryIndex) -> Option<&PackageEntry> {
		self.indexes
			.get(&idx.packages_index)
			.and_then(|packages| packages.get(&idx.name, idx.idx))
	}

	pub fn get_mut(&mut self, idx: &EntryIndex) -> Option<&mut PackageEntry> {
		self.indexes
			.get_mut(&idx.packages_index)
			.and_then(|packages| packages.get_mut(&idx.name, idx.idx))
	}

	/// Returns a package which the device has either access to
	/// or might be able to get access to with get_mut
	///
	/// ## Important
	/// Do not just return this check `PackageEntry::needs_write(device_id)`
	/// if you need to call get_mut
	// todo this api needs to be improved
	fn get_latest(
		&self,
		arch: &BoardArch,
		channel: &Channel,
		name: &str,
		device_id: &Option<DeviceId>,
	) -> Option<EntryIndex> {
		// first try with the provided arch
		// else try with the any arch

		self.get_latest_inner(&(*arch).into(), channel, name, device_id)
			.or_else(|| {
				self.get_latest_inner(
					&TargetArch::Any,
					channel,
					name,
					device_id,
				)
			})
	}

	fn get_latest_inner(
		&self,
		arch: &TargetArch,
		channel: &Channel,
		name: &str,
		device_id: &Option<DeviceId>,
	) -> Option<EntryIndex> {
		let mut idx = EntryIndex {
			packages_index: IndexKey {
				channel: *channel,
				arch: *arch,
			},
			name: name.into(),
			idx: 0,
		};

		let packages = self.indexes.get(&idx.packages_index)?;
		idx.idx = packages.get_latest(name, device_id)?;

		Some(idx)
	}

	fn push(&mut self, channel: Channel, entry: PackageEntry) {
		// entry: PackageEntry
		let index = self
			.indexes
			.entry(IndexKey {
				channel,
				arch: entry.package.arch,
			})
			.or_insert_with(|| PackagesIndex::new());
		index.push(entry);
	}

	fn change_whitelist(
		&mut self,
		channel: &Channel,
		arch: &TargetArch,
		name: &str,
		version: &Hash,
		change: &WhitelistChange,
	) -> bool {
		let entry = self
			.indexes
			.get_mut(&IndexKey {
				channel: *channel,
				arch: *arch,
			})
			.and_then(|i| i.mut_with_version(name, version));

		let entry = match entry {
			Some(e) => e,
			None => return false,
		};

		match change {
			WhitelistChange::Set(whitelist) => {
				entry.whitelist = whitelist.clone();
			}
			WhitelistChange::Add(whitelist) => {
				entry.whitelist.extend(whitelist.iter().cloned());
			}
			WhitelistChange::SetMinAuto(auto) => {
				entry.auto_whitelist_limit =
					(*auto).max(entry.auto_whitelist_limit);
			}
			WhitelistChange::AddAuto(auto) => {
				entry.auto_whitelist_limit += auto;
			}
		}

		true
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InWhitelist {
	Yes,
	CanBe,
	No,
}

impl InWhitelist {
	pub fn is_or_can_be(&self) -> bool {
		!matches!(self, Self::No)
	}

	pub fn can_be(&self) -> bool {
		matches!(self, Self::CanBe)
	}
}

impl From<bool> for InWhitelist {
	fn from(b: bool) -> Self {
		match b {
			true => Self::Yes,
			false => Self::No,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
	pub package: Package,
	#[serde(default)]
	pub requirements: HashMap<String, VersionReq>,
	// if the whitelist is empty this means that all devices are allowed
	// to use the package
	pub whitelist: HashSet<DeviceId>,
	// if this is > 0 every device which request info about the package will be
	// added to the whitelist until the whitelist.len is >= than this value
	#[serde(default)]
	pub auto_whitelist_limit: u32,
}

impl PackageEntry {
	fn in_whitelist(&self, device_id: &Option<DeviceId>) -> InWhitelist {
		match device_id {
			None => self.whitelist.is_empty().into(),
			Some(_) if self.whitelist.is_empty() => InWhitelist::Yes,
			Some(device_id) if self.whitelist.contains(device_id) => {
				InWhitelist::Yes
			}
			// might be added to the whitelist
			Some(_) => {
				if self.auto_whitelist_limit as usize > self.whitelist.len() {
					InWhitelist::CanBe
				} else {
					InWhitelist::No
				}
			}
		}
	}

	/// Only call this if it is fine to add the user to the whitelist
	pub fn update_whitelist(&mut self, device_id: &Option<DeviceId>) {
		debug_assert!(self.in_whitelist(device_id).is_or_can_be());

		if let Some(device_id) = device_id {
			self.whitelist.insert(device_id.clone());
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesIndex {
	// last package Entry is the current entry
	list: HashMap<String, Vec<PackageEntry>>,
}

impl PackagesIndex {
	fn new() -> Self {
		Self {
			list: HashMap::new(),
		}
	}

	fn get(&self, name: &str, idx: usize) -> Option<&PackageEntry> {
		self.list.get(name).and_then(|list| list.get(idx))
	}

	fn get_mut(&mut self, name: &str, idx: usize) -> Option<&mut PackageEntry> {
		self.list.get_mut(name).and_then(|list| list.get_mut(idx))
	}

	/// returns the index of the lastest entry
	fn get_latest(
		&self,
		name: &str,
		device_id: &Option<DeviceId>,
	) -> Option<usize> {
		self.list
			.get(name)?
			.iter()
			.enumerate()
			.rev()
			.find(|(_, e)| e.in_whitelist(device_id).is_or_can_be())
			.map(|(idx, _)| idx)
	}

	fn mut_with_version(
		&mut self,
		name: &str,
		version: &Hash,
	) -> Option<&mut PackageEntry> {
		self.list
			.get_mut(name)?
			.iter_mut()
			.rev()
			.find(|e| &e.package.version == version)
	}

	// push or updates a existing Entry
	fn push(&mut self, entry: PackageEntry) {
		let name = entry.package.name.clone();
		let list = self.list.entry(name).or_insert(vec![]);

		let maybe_entry = list
			.iter_mut()
			.find(|e| e.package.version == entry.package.version);

		match maybe_entry {
			Some(stored_entry) => {
				// the version is the same
				// so override the current package
				*stored_entry = entry;
			}
			None => {
				list.push(entry);
			}
		}
	}
}

#[derive(Debug)]
pub struct PackagesDb {
	inner: RwLock<FileDb<PackagesDbFile>>,
	write: bool,
}

impl PackagesDb {
	/// Creates a new empty PackagesDb without writing anything to file
	#[cfg(test)]
	pub fn new(cfg: &Config) -> Self {
		let db = FileDb::new(&cfg.packages_file, PackagesDbFile::new());

		Self {
			inner: RwLock::new(db),
			write: false,
		}
	}

	pub async fn create(cfg: &Config) -> Result<Self> {
		if fs::metadata(&cfg.packages_file).await.is_ok() {
			return Self::read(cfg).await;
		}

		let db = FileDb::new(&cfg.packages_file, PackagesDbFile::new());
		db.write()
			.await
			.map_err(|e| Error::new("could not write packages.fdb", e))?;

		Ok(Self {
			inner: RwLock::new(db),
			write: true,
		})
	}

	pub async fn read(cfg: &Config) -> Result<Self> {
		let db = FileDb::open(&cfg.packages_file)
			.await
			.map_err(|e| Error::new("packages.fdb could not be opened", e))?;

		Ok(Self {
			inner: RwLock::new(db),
			write: true,
		})
	}

	pub async fn get_package(
		&self,
		arch: &BoardArch,
		channel: &Channel,
		name: &str,
		device_id: &Option<DeviceId>,
	) -> Option<PackageEntry> {
		{
			let lock = self.inner.read().await;
			let db = lock.data();

			let idx = db.get_latest(arch, channel, name, device_id)?;
			let package = db.get(&idx).unwrap();

			// check if we need to add us to the whitelist
			if !package.in_whitelist(device_id).can_be() {
				return Some(package.clone());
			}
		};

		let mut lock = self.inner.write().await;
		let db = lock.data_mut();

		let idx = db.get_latest(arch, channel, name, device_id)?;
		let package = db.get_mut(&idx).unwrap();

		package.update_whitelist(device_id);

		Some(package.clone())
	}

	pub async fn push_package(&self, channel: Channel, entry: PackageEntry) {
		let mut lock = self.inner.write().await;
		let db = lock.data_mut();
		db.push(channel, entry);

		if self.write {
			lock.write().await.expect("writing failed unexpectetly")
		}
	}

	// returns true if the whitelist could be changed
	pub async fn change_whitelist(
		&self,
		channel: &Channel,
		arch: &TargetArch,
		name: &str,
		version: &Hash,
		change: &WhitelistChange,
	) -> bool {
		let mut lock = self.inner.write().await;
		let db = lock.data_mut();
		let r = db.change_whitelist(channel, arch, name, version, change);

		if self.write {
			lock.write().await.expect("writing failed unexpectetly");
		}

		r
	}
}

// process of updating a package
// create the file
// update packagesDb
// delete old file
