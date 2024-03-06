use crate::config::Source;
use crate::error::Result;

use std::path::Path;
use std::process::Command;
use std::str::FromStr;
use std::{fmt, io};

use crypto::hash::{Hash, Hasher};
use crypto::signature::Keypair;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::fs::{self, read_to_string};

pub async fn read_toml<T>(path: impl AsRef<Path>) -> Result<T>
where
	T: DeserializeOwned,
{
	let path = path.as_ref();
	let s = read_to_string(path)
		.await
		.map_err(|e| err!(e, "file {:?} not found", path))?;
	toml::from_str(&s).map_err(|e| err!(e, "toml error in {:?}", path))
}

pub async fn write_toml<T>(path: impl AsRef<Path>, ctn: &T) -> Result<()>
where
	T: Serialize + fmt::Debug,
{
	let s = toml::to_string_pretty(ctn)
		.map_err(|e| err!(e, "could not generate toml from {:?}", ctn))?;
	// create the folder first
	let path = path.as_ref();
	let dir = path.with_file_name("");
	fs::create_dir_all(&dir)
		.await
		.map_err(|e| err!(e, "could not create dir {:?}", dir))?;
	fs::write(path, s)
		.await
		.map_err(|e| err!(e, "could not write to {:?}", path))
}

/// first removes the directory and then creates it
/// use the correct path !!!
pub async fn create_dir(path: impl AsRef<Path>) -> Result<()> {
	let path = path.as_ref();
	let _ = fs::remove_dir_all(path).await;
	fs::create_dir_all(path)
		.await
		.map_err(|e| err!(e, "could not create {:?}", path))
}

pub async fn remove_dir(path: impl AsRef<Path>) -> Result<()> {
	let path = path.as_ref();
	fs::remove_dir_all(path)
		.await
		.map_err(|e| err!(e, "could not remove {:?}", path))
}

pub async fn copy(from: &str, to: &str) -> Result<()> {
	fs::copy(from, to)
		.await
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
	let v = fs::read(path)
		.await
		.map_err(|e| err!(e, "could not hash file {:?}", path))?;

	Ok(Hasher::hash(&v))
}

pub async fn get_priv_key(source: &Source) -> Result<Keypair> {
	if let Some(k) = &source.priv_key {
		println!("using existing private signature key");
		Ok(k.clone())
	} else {
		println!();
		println!("Please enter the private signature key:");

		let mut priv_key_b64 = String::new();
		let stdin = io::stdin();
		stdin
			.read_line(&mut priv_key_b64)
			.map_err(|e| err!(e, "could not read private key"))?;
		Keypair::from_str(priv_key_b64.trim())
			.map_err(|e| err!(format!("{:?}", e), "invalid private key"))
	}
}
