
use crate::error::Result;

use std::process::Command;
use tokio::fs::{self, read_to_string};
use serde::de::DeserializeOwned;
use crypto::hash::{Hasher, Hash};

pub async fn read_toml<T>(path: &str) -> Result<T>
where T: DeserializeOwned {
	let s = read_to_string(path).await
		.map_err(|e| err!(e, "file {:?} not found", path))?;
	toml::from_str(&s)
		.map_err(|e| err!(e, "toml error in {:?}", path))
}

/// first removes the directory and then creates it
/// use the correct path !!!
pub async fn create_dir(path: &str) -> Result<()> {
	let _ = fs::remove_dir_all(path).await;
	fs::create_dir_all(path).await
		.map_err(|e| err!(e, "could not create {:?}", path))
}

pub async fn remove_dir(path: &str) -> Result<()> {
	fs::remove_dir_all(path).await
		.map_err(|e| err!(e, "could not remove {:?}", path))
}

pub fn compress(name: &str, path: &str, inner: &str) -> Result<()> {
	// tar -zcvf name.tar.gz -C source name
	let stat = Command::new("tar")
		.args(&["-zcvf", name, "-C", path, inner])
		.status()
		.map_err(|e| err!(e, "could not call tar"))?;
	if stat.success() {
		Ok(())
	} else {
		Err(err!("exit status non zero", "compressing failed"))
	}
}

pub async fn hash_file(path: &str) -> Result<Hash> {

	let v = fs::read(path).await
		.map_err(|e| err!(e, "could not hash file {:?}", path))?;

	Ok(Hasher::hash(&v))
}