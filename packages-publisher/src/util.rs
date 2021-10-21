
use crate::error::Result;

use std::process::Command;
use std::path::Path;
use std::fmt;

use tokio::fs::{self, read_to_string};
use serde::Serialize;
use serde::de::DeserializeOwned;
use crypto::hash::{Hasher, Hash};

pub async fn read_toml<T>(path: impl AsRef<Path>) -> Result<T>
where T: DeserializeOwned {
	let path = path.as_ref();
	let s = read_to_string(path).await
		.map_err(|e| err!(e, "file {:?} not found", path))?;
	toml::from_str(&s)
		.map_err(|e| err!(e, "toml error in {:?}", path))
}

pub async fn write_toml<T>(path: impl AsRef<Path>, ctn: &T) -> Result<()>
where T: Serialize + fmt::Debug {
	let s = toml::to_string_pretty(ctn)
		.map_err(|e| err!(e, "could not generate toml from {:?}", ctn))?;
	// create the folder first
	let path = path.as_ref();
	let dir = path.with_file_name("");
	fs::create_dir_all(&dir).await
		.map_err(|e| err!(e, "could not create dir {:?}", dir))?;
	fs::write(path, s).await
		.map_err(|e| err!(e, "could not write to {:?}", path))
}

/// first removes the directory and then creates it
/// use the correct path !!!
pub async fn create_dir(path: impl AsRef<Path>) -> Result<()> {
	let path = path.as_ref();
	let _ = fs::remove_dir_all(path).await;
	fs::create_dir_all(path).await
		.map_err(|e| err!(e, "could not create {:?}", path))
}

pub async fn remove_dir(path: impl AsRef<Path>) -> Result<()> {
	let path = path.as_ref();
	fs::remove_dir_all(path).await
		.map_err(|e| err!(e, "could not remove {:?}", path))
}

pub async fn copy(from: &str, to: &str) -> Result<()> {
	fs::copy(from, to).await
		.map(drop)
		.map_err(|e| err!(e, "could not copy {:?} to {:?}", from, to))
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

pub fn extract(path: &str, to: &str) -> Result<()> {
	let stat = Command::new("tar")
		.args(&["-zxvf", path, "-C", to])
		.status()
		.map_err(|e| err!(e, "could not call tar"))?;
	if stat.success() {
		Ok(())
	} else {
		Err(err!("exit status non zero", "extracting failed"))
	}
}

pub async fn hash_file(path: &str) -> Result<Hash> {

	let v = fs::read(path).await
		.map_err(|e| err!(e, "could not hash file {:?}", path))?;

	Ok(Hasher::hash(&v))
}