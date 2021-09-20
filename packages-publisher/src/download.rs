
use crate::error::{Result};
use crate::util::{create_dir, read_toml, extract};

use tokio::fs;

use file_db::FileDb;

use clap::{AppSettings, Clap};
use riji::paint_act;

use crypto::signature::PublicKey;

use packages::packages::{Channel, Source, Package, PackageCfg, PackagesCfg};
use packages::client::Client;
use packages::requests::{PackageInfoReq, GetFileReq};

use serde::{Deserialize};

#[derive(Debug, Clone, Deserialize)]
pub struct SourceToml {
	/// the address from which the files should be downloaded
	address: String,
	/// the public key used for the connection
	#[serde(rename = "pub-key")]
	pub_key: PublicKey,
	/// the public key used for signing packages
	#[serde(rename = "sign-key")]
	sign_key: PublicKey,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackagesToml {
	/// a list of all packages that should be downloaded
	list: Vec<String>,
	/// the channel from which it should be downloaded
	channel: Channel,
	/// what package to execute on running (the first parameter will be the state)
	#[serde(rename = "on-run")]
	on_run: String,
	#[serde(rename = "source")]
	sources: Vec<SourceToml>
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

	if cfg.sources.is_empty() {
		return Err(err!("no sources", "misssing cfg sources"))
	}

	let mut list: Vec<_> = cfg.list.into_iter()
		.map(Some)
		.collect();

	let mut packs = vec![];

	for source in cfg.sources.iter().rev() {
		download_from_source(&mut list, &mut packs, &cfg.channel, &source).await?;
	}

	let sources: Vec<_> = cfg.sources.into_iter()
		.map(|source| Source {
			addr: source.address,
			public: true,
			public_key: source.pub_key,
			sign_key: source.sign_key
		})
		.collect();

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

	let packs_cfg = PackagesCfg {
		sources,
		fetch_realtime: false,
		on_run: cfg.on_run.clone(),
		channel: cfg.channel
	};

	let db = FileDb::new("./packages/packages.fdb", packs_cfg);
	db.write().await
		.map_err(|e| err!(e, "could not store packages.fdb"))?;

	Ok(())
}

async fn download_from_source(
	list: &mut Vec<Option<String>>,
	packs: &mut Vec<Package>,
	channel: &Channel,
	source: &SourceToml
) -> Result<()> {

	// should we delete the packages folder
	let client = Client::connect(&source.address, source.pub_key.clone()).await
		.map_err(|e| err!(e, "connect to {} failed", source.address))?;

	for list_name in list.iter_mut() {
		let name = match list_name.as_ref() {
			Some(n) => n,
			None => continue
		};

		paint_act!("checking {}", name);

		let req = PackageInfoReq {
			channel: *channel,
			name: name.clone()
		};
		let res = client.request(req).await
			.map_err(|e| err!(e, "could not get package info"))?;
		let pack = match res.package {
			Some(p) => p,
			None => continue
		};

		list_name.take();

		paint_act!("downloading {}", pack.name);

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
			!source.sign_key.verify(pack.version.as_slice(), &pack.signature)
		{
			return Err(err!("hash / sig", "file {} not correct", pack.name));
		}

		// write to
		let path = format!("./packages/{}.tar.gz", pack.name);
		fs::write(&path, res.file()).await
			.map_err(|e| err!(e, "could not write to {}", path))?;

		packs.push(pack);
	}

	Ok(())
}