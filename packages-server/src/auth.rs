
use crate::error::{Error, Result};

use std::collections::HashSet;

use tokio::fs;
use tokio::sync::RwLock;

use file_db::FileDb;

use packages::auth::AuthKey;

use serde::{Serialize, Deserialize};



#[derive(Debug, Serialize, Deserialize)]
struct AuthDbFile {
	keys: HashSet<AuthKey>
}

impl AuthDbFile {
	fn new() -> Self {
		Self {
			keys: HashSet::new()
		}
	}

	fn insert(&mut self, key: AuthKey) {
		self.keys.insert(key);
	}

	fn contains(&self, key: &AuthKey) -> bool {
		self.keys.contains(key)
	}
}

pub struct AuthDb {
	inner: RwLock<FileDb<AuthDbFile>>
}

impl AuthDb {

	pub async fn create() -> Result<Self> {
		if fs::metadata("./auths.fdb").await.is_ok() {
			return Self::read().await;
		}

		let db = FileDb::new("./auths.fdb", AuthDbFile::new());
		db.write().await
			.map_err(|e| Error::new("could not write auths.fdb", e))?;

		Ok(Self {
			inner: RwLock::new(db)
		})
	}

	pub async fn read() -> Result<Self> {
		let db = FileDb::open("./auths.fdb").await
			.map_err(|e| Error::new("auths.fdb could not be opened", e))?;

		Ok(Self {
			inner: RwLock::new(db)
		})
	}

	pub async fn insert(&self, key: AuthKey) {
		let mut lock = self.inner.write().await;
		let db = lock.data_mut();
		db.insert(key);
		lock.write().await
			.expect("writing failed");
	}

	pub async fn contains(&self, key: &AuthKey) -> bool {
		let lock = self.inner.read().await;
		lock.data().contains(key)
	}

}