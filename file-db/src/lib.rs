
use std::path::PathBuf;
use std::{io, fs};

use serde::{de::DeserializeOwned, Serialize};

pub struct FileDb<T> {
	path: PathBuf,
	tmp_path: PathBuf,
	data: T
}

impl<T> FileDb<T> {

	fn sanitze_paths(mut main: PathBuf) -> (PathBuf, PathBuf) {
		if main.file_name().is_none() {
			main.set_file_name("");
		}
		main.set_extension("fdb");
		let mut tmp = main.clone();
		tmp.set_extension("fdb.tmp");
		(main, tmp)
	}

	// the file extension of path will be set to fdb
	pub fn new(path: impl Into<PathBuf>, data: T) -> Self {
		let (path, tmp_path) = Self::sanitze_paths(path.into());

		Self { path, tmp_path, data }
	}

	/// if the file does not exists this will return an error.
	pub fn open_sync(path: impl Into<PathBuf>) -> io::Result<Self>
	where T: DeserializeOwned {
		let (path, tmp_path) = Self::sanitze_paths(path.into());

		// because we are lazy just read the whole file
		let v = fs::read(&path)?;
		let data = serde_json::from_slice(&v)
			.map_err(json_err)?;

		Ok(Self { path, tmp_path, data })
	}

	/// if the file does not exists this will return an error.
	pub fn read_sync(&mut self) -> io::Result<()>
	where T: DeserializeOwned {
		let v = fs::read(&self.path)?;
		let data = serde_json::from_slice(&v)
			.map_err(json_err)?;

		self.data = data;
		Ok(())
	}

	pub fn write_sync(&self) -> io::Result<()>
	where T: Serialize {
		let v = serde_json::to_vec(&self.data)
			.map_err(json_err)?;

		// write to the tmp file
		fs::write(&self.tmp_path, v)?;
		// now rename the file atomically
		fs::rename(&self.tmp_path, &self.path)
	}

	pub fn data(&self) -> &T {
		&self.data
	}

	pub fn data_mut(&mut self) -> &mut T {
		&mut self.data
	}

	pub fn into_data(self) -> T {
		self.data
	}

}

#[cfg(any(feature = "async", test))]
impl<T> FileDb<T> {

	/// if the file does not exists this will return an error.
	pub async fn open(path: impl Into<PathBuf>) -> io::Result<Self>
	where T: DeserializeOwned {
		let (path, tmp_path) = Self::sanitze_paths(path.into());

		// because we are lazy just read the whole file
		let v = tokio::fs::read(&path).await?;
		let data = serde_json::from_slice(&v)
			.map_err(json_err)?;

		Ok(Self { path, tmp_path, data })
	}

	/// if the file does not exists this will return an error.
	pub async fn read(&mut self) -> io::Result<()>
	where T: DeserializeOwned {
		let v = tokio::fs::read(&self.path).await?;
		let data = serde_json::from_slice(&v)
			.map_err(json_err)?;

		self.data = data;
		Ok(())
	}

	pub async fn write(&self) -> io::Result<()>
	where T: Serialize {
		let v = serde_json::to_vec(&self.data)
			.map_err(json_err)?;

		// write to the tmp file
		tokio::fs::write(&self.tmp_path, v).await?;
		// now rename the file atomically
		tokio::fs::rename(&self.tmp_path, &self.path).await
	}

}

fn json_err(e: serde_json::Error) -> io::Error {
	io::Error::new(io::ErrorKind::Other, e)
}


#[cfg(test)]
mod tests {

	use super::*;
	use serde::Deserialize;
	use std::path::PathBuf;
	use std::fs;

	#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
	pub struct Data {
		inner: Vec<u32>
	}

	const TEST_TMP: &str = "./tests_tmp";

	fn test_tmp() -> PathBuf {
		let p = PathBuf::from(TEST_TMP);
		if !p.is_dir() {
			let _ = fs::create_dir(&p);
		}
		p
	}

	#[test]
	fn test_sync() {
		let mut file = test_tmp();
		file.push("test_sync.fdb");

		let data = Data {
			inner: vec![1, 4, 8, 14]
		};

		let mut db = FileDb::new(file.clone(), data.clone());
		db.write_sync().unwrap();

		// now open
		let mut db_2: FileDb<Data> = FileDb::open_sync(file.clone()).unwrap();
		assert_eq!(db_2.data(), &data);

		// update one
		db.data_mut().inner.push(420);
		db.write_sync().unwrap();

		// now read
		db_2.read_sync().unwrap();
		assert_ne!(db_2.data(), &data);
		assert_eq!(db_2.data().inner.last().unwrap(), &420);

	}

	#[tokio::test]
	async fn test_async() {
		let mut file = test_tmp();
		file.push("test_async.fdb");

		let data = Data {
			inner: vec![1, 4, 8, 14]
		};

		let mut db = FileDb::new(file.clone(), data.clone());
		db.write().await.unwrap();

		// now open
		let mut db_2: FileDb<Data> = FileDb::open(file.clone()).await.unwrap();
		assert_eq!(db_2.data(), &data);

		// update one
		db.data_mut().inner.push(420);
		db.write().await.unwrap();

		// now read
		db_2.read().await.unwrap();
		assert_ne!(db_2.data(), &data);
		assert_eq!(db_2.data().inner.last().unwrap(), &420);

	}

}