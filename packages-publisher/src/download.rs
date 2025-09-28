use crate::error::Result;
use crate::util::{create_dir, extract, read_toml, remove_dir, write_toml};

use tokio::fs;

use file_db::FileDb;

use riji::paint_act;

use crypto::signature::PublicKey;

use packages::client::Client;
use packages::packages::{
	BoardArch, Channel, Package, PackageCfg, PackagesCfg, Source,
};
use packages::requests::PackageInfoReq;

use serde::{Deserialize, Serialize};

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
	/// the product name for which this image is meant
	product: String,
	/// the channel from which it should be downloaded
	channel: Channel,
	/// what package to execute on running (the first parameter will be the state)
	#[serde(rename = "on-run")]
	on_run: String,
	#[serde(rename = "source")]
	sources: Vec<SourceToml>,
}

/// The toml in which the buildsystem stores information about the image
#[derive(Debug, Clone, Deserialize)]
pub struct ImageToml {
	arch: BoardArch,
}

/// The toml file which get's used between the download and pack-image command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductToml {
	product: String,
}

/// Downloads and fills a full packages folder `./packages`
/// with the packages listed in the provided configuration file
#[derive(clap::Parser)]
pub struct Download {
	/// the location of packages.toml
	/// should be an absolute path
	config: String,
}

pub async fn download(opts: Download) -> Result<()> {
	// read packages.toml
	let cfg: PackagesToml = read_toml(opts.config).await?;

	let product_toml = ProductToml {
		product: cfg.product.clone(),
	};

	// store the product name
	write_toml("./product.toml", &product_toml).await?;

	// read image.toml
	let image_cfg: ImageToml = read_toml("./image.toml").await?;

	let local_packages = "./packages";

	// delete packages dir
	let _ = remove_dir(&local_packages).await;
	create_dir(&local_packages).await?;

	if cfg.sources.is_empty() {
		return Err(err!("no sources", "misssing cfg sources"));
	}

	let mut list: Vec<_> = cfg.list.into_iter().map(Some).collect();

	let mut packs = vec![];

	for source in cfg.sources.iter().rev() {
		download_from_source(
			&mut list,
			&mut packs,
			image_cfg.arch,
			cfg.channel,
			&source,
			&local_packages,
		)
		.await?;
	}

	let mut unfinished = false;
	for pack in list {
		if let Some(pack) = pack {
			println!("package not downloaded {:?}", pack);
			unfinished = true;
		}
	}

	if unfinished {
		return Err(err!("unfinished", "not all packages could be downloaded"));
	}

	let sources: Vec<_> = cfg
		.sources
		.into_iter()
		.map(|source| Source {
			addr: source.address,
			public: true,
			public_key: source.pub_key,
			sign_key: source.sign_key,
		})
		.collect();

	//
	// extract packages
	// and create package.fdb
	for pack in packs {
		let path = format!("{}/{}", local_packages, pack.name);
		let tar = format!("{}/{}.tar.gz", local_packages, pack.name);

		// create folder
		create_dir(&path).await?;

		// extract
		extract(&tar, &path)?;

		fs::remove_file(&tar).await.expect("could not delete");

		// rename extracted folder
		let left = format!("{}/left", path);
		fs::rename(&format!("{}/{}", path, pack.name), &left)
			.await
			.map_err(|e| err!(e, "could not rename folder"))?;

		// build package and store it
		let fdb = format!("{}/package.fdb", path);
		let db = FileDb::new(fdb, PackageCfg::Left(pack));
		db.write()
			.await
			.map_err(|e| err!(e, "could not store file db"))?;
	}

	let packs_cfg = PackagesCfg {
		sources,
		fetch_realtime: false,
		on_run: cfg.on_run.clone(),
		channel: cfg.channel,
	};

	let db = FileDb::new(format!("{}/packages.fdb", local_packages), packs_cfg);
	db.write()
		.await
		.map_err(|e| err!(e, "could not store packages.fdb"))?;

	Ok(())
}

async fn download_from_source(
	list: &mut Vec<Option<String>>,
	packs: &mut Vec<Package>,
	arch: BoardArch,
	channel: Channel,
	source: &SourceToml,
	packages_dir: &str,
) -> Result<()> {
	paint_act!("connecting to {}", source.address);

	// should we delete the packages folder
	let client = Client::connect(&source.address, source.pub_key.clone())
		.await
		.map_err(|e| err!(e, "connect to {} failed", source.address))?;

	for list_name in list.iter_mut() {
		// todo: replace with let Some(name) = .. else
		let name = match list_name.as_ref() {
			Some(n) => n,
			None => continue,
		};

		paint_act!("checking {}", name);

		let pack = client
			.package_info(PackageInfoReq {
				channel,
				arch,
				name: name.clone(),
				device_id: None,
				image_version: None,
				package_versions: None,
				ignore_requirements: true,
			})
			.await
			.map_err(|e| err!(e, "could not get package info"))?;
		let pack = match pack {
			Some(p) => p,
			None => continue,
		};

		list_name.take();

		paint_act!("downloading {}", pack.name);

		// now get the file
		let res = client
			.get_file(pack.version.clone())
			.await
			.map_err(|e| err!(e, "could not get file"))?;
		if res.is_empty() {
			return Err(err!("not found", "file {} not found", pack.name));
		}

		if res.hash() != pack.version
			|| !source.sign_key.verify(&pack.version, &pack.signature)
		{
			return Err(err!("hash / sig", "file {} not correct", pack.name));
		}

		// write to
		let path = format!("{}/{}.tar.gz", packages_dir, pack.name);
		fs::write(&path, res.file())
			.await
			.map_err(|e| err!(e, "could not write to {}", path))?;

		packs.push(pack);
	}

	Ok(())
}
