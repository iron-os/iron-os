
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesIndex {
	list: HashMap<String, Package>
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

}

// process of updating a package
// create the file
// update packagesDb
// delete old file