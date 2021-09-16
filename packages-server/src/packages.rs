
use crate::error::{Error, Result};

use std::collections::HashMap;

use tokio::fs;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use packages::packages::{Package, Channel};
use file_db::FileDb;

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesDbFile {
	indexes: HashMap<Channel, PackagesIndex>
}

impl PackagesDbFile {
	fn new() -> Self {
		Self {
			indexes: HashMap::new()
		}
	}

	fn get(&self, channel: &Channel) -> Option<&PackagesIndex> {
		self.indexes.get(channel)
	}

	fn set(&mut self, channel: Channel, package: Package) {
		let index = self.indexes.entry(channel)
			.or_insert_with(|| PackagesIndex::new());
		index.set(package);
		// 
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

	pub async fn get_package(&self, channel: &Channel, name: &str) -> Option<Package> {
		let lock = self.inner.read().await;
		let db = lock.data();
		let index = db.get(channel)?;
		index.get(name)
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