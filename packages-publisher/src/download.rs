
use crate::error::{Result};
use crate::util::{create_dir, read_toml, extract};

use tokio::fs;

use file_db::FileDb;

use clap::{AppSettings, Clap};
use riji::paint_act;

use crypto::signature::PublicKey;

use packages::packages::{Channel, Source, PackageCfg, PackagesCfg};
use packages::client::Client;
use packages::requests::{PackageInfoReq, GetFileReq};

use serde::{Deserialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PackagesToml {
	/// a list of all packages that should be downloaded
	list: Vec<String>,
	/// the address from which the files should be downloaded
	address: String,
	/// the public key used for the connection
	#[serde(rename = "pub-key")]
	pub_key: PublicKey,
	/// the public key used for signing packages
	#[serde(rename = "sign-key")]
	sign_key: PublicKey,
	/// the channel from which it should be downloaded
	channel: Channel,
	/// what package to execute on installation
	#[serde(rename = "on-install")]
	on_install: String,
	/// what package to execute on running
	#[serde(rename = "on-run")]
	on_run: String
}

/// Downloads and fills a full packages folder
/// with the packages listed in `packages.toml`
/// the address and the channel should be in `packages.toml`
#[derive(Clap)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Download {}

pub async fn download(_: Download) -> Result<()> {

	// read packages.toml
	let cfg: PackagesToml = read_toml("./packages.toml").await?;

	// creates packages folder
	create_dir("./packages").await?;

	// should we delete the packages folder
	let client = Client::connect(&cfg.address, cfg.pub_key.clone()).await
		.map_err(|e| err!(e, "connect to {} failed", cfg.address))?;

	let mut packs = vec![];

	for pack in &cfg.list {

		paint_act!("downloading {}", pack);

		let req = PackageInfoReq {
			channel: cfg.channel,
			name: pack.to_string()
		};
		let res = client.request(req).await
			.map_err(|e| err!(e, "could not get package info"))?;
		let pack = match res.package {
			Some(p) => p,
			None => {
				return Err(err!("not found", "package {} not found", pack));
			}
		};

		// now get the file
		let req = GetFileReq {
			hash: pack.version.clone()
		};
		let res = client.request(req).await
			.map_err(|e| err!(e, "could not get file"))?;
		if res.is_empty() {
			return Err(err!("not found", "file {} not found", pack.name));
		}

		if res.hash() != pack.version ||
			!cfg.sign_key.verify(pack.version.as_slice(), &pack.signature)
		{
			return Err(err!("hash / sig", "file {} not correct", pack.name));
		}

		// write to
		let path = format!("./packages/{}.tar.gz", pack.name);
		fs::write(&path, res.file()).await
			.map_err(|e| err!(e, "could not write to {}", path))?;

		packs.push(pack);
	}

	drop(client);

	//
	// extract packages
	// and create package.fdb
	for pack in packs {

		let path = format!("./packages/{}", pack.name);
		let tar = format!("./packages/{}.tar.gz", pack.name);

		// create folder
		fs::create_dir(&path).await
			.map_err(|e| err!(e, "could not create dir {}", path))?;

		// extract
		extract(&tar, &path)?;

		fs::remove_file(&tar).await.expect("could not delete");

		// rename extracted folder
		let left = format!("{}/left", path);
		fs::rename(&format!("{}/{}", path, pack.name), &left).await
			.map_err(|e| err!(e, "could not rename folder"))?;

		// build package and store it
		let fdb = format!("{}/package.fdb", path);
		let db = FileDb::new(fdb, PackageCfg::Left(pack));
		db.write().await
			.map_err(|e| err!(e, "could not store file db"))?;

	}


	let source = Source {
		addr: cfg.address.clone(),
		public: true,
		public_key: cfg.pub_key.clone(),
		sign_key: cfg.sign_key.clone()
	};

	let packs_cfg = PackagesCfg {
		sources: vec![source],
		fetch_realtime: false,
		on_install: cfg.on_install.clone(),
		on_run: cfg.on_run.clone(),
		channel: cfg.channel
	};

	let db = FileDb::new("./packages/packages.fdb", packs_cfg);
	db.write().await
		.map_err(|e| err!(e, "could not store packages.fdb"))?;

	Ok(())
}