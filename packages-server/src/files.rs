use crate::config::Config;
use crate::error::{Error, Result};

use std::io::ErrorKind;
use std::path::PathBuf;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{self, AsyncWriteExt};

use crypto::hash::Hash;

pub struct Files {
	// used as a drop holder
	#[allow(dead_code)]
	holder: Box<dyn std::any::Any + Send + Sync>,
	path: PathBuf,
}

impl Files {
	#[cfg(test)]
	pub fn new_temp() -> Self {
		let tmp = tempfile::tempdir().unwrap();

		Self {
			path: tmp.as_ref().into(),
			holder: Box::new(tmp),
		}
	}

	pub async fn create(cfg: &Config) -> Result<Self> {
		if fs::metadata(&cfg.files_dir).await.is_ok() {
			return Self::read(cfg).await;
		}

		fs::create_dir(&cfg.files_dir)
			.await
			.map_err(|e| Error::new("could not create files directory", e))?;

		Self::read(cfg).await
	}

	pub async fn read(cfg: &Config) -> Result<Self> {
		let path = fs::canonicalize(&cfg.files_dir)
			.await
			.map_err(|e| Error::new("files dir not found", e))?;
		Ok(Self {
			holder: Box::new(path.clone()),
			path,
		})
	}

	pub async fn get(&self, hash: &Hash) -> Option<File> {
		let hash = hash.to_string();
		let path = self.path.join(hash);
		File::open(path).await.ok()
	}

	/// the hash needs to correspond with the data
	pub async fn set(&self, hash: &Hash, data: &[u8]) -> io::Result<()> {
		#[cfg(debug_assertions)]
		{
			let n_hash = crypto::hash::Hasher::hash(data);
			assert_eq!(hash, &n_hash);
		}

		let path = self.path.join(hash.to_string());

		let file = OpenOptions::new()
			.create_new(true)
			.write(true)
			.open(path)
			.await;

		let mut file = match file {
			Ok(file) => file,
			Err(e) if e.kind() == ErrorKind::AlreadyExists => return Ok(()),
			Err(e) => return Err(e),
		};

		file.write_all(&data).await?;

		Ok(())
	}

	pub async fn exists(&self, hash: &Hash) -> bool {
		let path = self.path.join(hash.to_string());
		fs::metadata(&path).await.is_ok()
	}
}
