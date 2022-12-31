use crate::error::{Error, Result};

use std::collections::HashMap;

use tokio::fs;
use tokio::sync::RwLock;

use file_db::FileDb;

use packages::requests::AuthKey;
use packages::packages::Channel;

use serde::{Serialize, Deserialize};


fn default_keys() -> HashMap<AuthKey, Channel> {
	HashMap::new()
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthDbFile {
	#[serde(rename = "keys_v2", default = "default_keys")]
	keys: HashMap<AuthKey, Channel>
}

impl AuthDbFile {
	fn new() -> Self {
		Self {
			keys: HashMap::new()
		}
	}

	fn insert(&mut self, key: AuthKey, channel: Channel) {
		self.keys.insert(key, channel);
	}

	fn get(&self, key: &AuthKey) -> Option<Channel> {
		self.keys.get(key).map(|c| *c)
	}
}

pub struct AuthDb {
	inner: RwLock<FileDb<AuthDbFile>>
}

const AUTH_PATH: &'static str = "./auths.fdb";

impl AuthDb {
	pub async fn create() -> Result<Self> {
		if fs::metadata(AUTH_PATH).await.is_ok() {
			return Self::read().await;
		}

		let db = FileDb::new(AUTH_PATH, AuthDbFile::new());
		db.write().await
			.map_err(|e| Error::new("could not write auths.fdb", e))?;

		Ok(Self {
			inner: RwLock::new(db)
		})
	}

	pub async fn read() -> Result<Self> {
		let db = FileDb::open(AUTH_PATH).await
			.map_err(|e| Error::new("auths.fdb could not be opened", e))?;

		Ok(Self {
			inner: RwLock::new(db)
		})
	}

	pub async fn insert(&self, key: AuthKey, channel: Channel) {
		let mut lock = self.inner.write().await;
		let db = lock.data_mut();
		db.insert(key, channel);
		lock.write().await
			.expect("writing failed");
	}

	pub async fn get(&self, key: &AuthKey) -> Option<Channel> {
		let lock = self.inner.read().await;
		lock.data().get(key)
	}
}