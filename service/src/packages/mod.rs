//! ## packages folder
//! packages
//!  - packages.fdb
//!  - chnobli_ui
//!   - package.fdb // json_db containing information about the package
//!   - left
//!   - right
//! 
//! package.fdb
//!  - name
//!  - version_str
//!  - version // hash
//!  - signature // signature of the current version
//!  - current // folder of the current left|right
//!  - binary // Option<String>

// use crypto::signature::PublicKey;
use crate::Bootloader;
use crate::util::io_other;

use std::io;
use std::path::{Path, PathBuf};

use tokio::fs;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

use rand::{thread_rng, Rng};

use bootloader_api::{VersionInfoReq, VersionInfo};
use packages::packages::{PackagesCfg, PackageCfg, Package, Source, Channel};
use packages::client::Client;
use packages::requests::PackageInfoReq;
use file_db::FileDb;

const PACKAGES_DIR: &str = "/data/packages";

fn path(s: &str) -> PathBuf {
	Path::new(PACKAGES_DIR).join(s)
}


pub async fn start(client: Bootloader) -> io::Result<JoinHandle<()>> {

	Ok(tokio::spawn(async move {
		loop {

			// get version info so we know if we should update alot or not
			let version_info = client.request(&VersionInfoReq).await
				.expect("fetching version failed");

				if !version_info.installed {
					// not installed
					eprintln!("not installed, only update when installed");
					return
				}

			// we do this step on every iteration to
			// always get a new random value
			let time = match version_info.channel.is_debug() {
				// check version every minute
				true => Duration::from_secs(60),
				false => Duration::from_secs(
					// check version every 5-15 minutes
					thread_rng()
						.gen_range((60 * 5)..(60 * 15))
				)
			};

			sleep(time).await;

			// update every
			match update(version_info).await {
				Ok(()) => {},
				Err(e) => {
					eprintln!("update error {:?}", e);
				}
			}

		}
	}))
}

pub async fn update(version: VersionInfo) -> io::Result<()> {

	let mut packages = Packages::load().await?;
	let mut image = Some(version);

	for source in packages.cfg.sources.into_iter().rev() {

		update_from_source(
			source,
			packages.cfg.channel,
			&mut image,
			&mut packages.list
		).await?;

	}

	Ok(())
}

pub async fn update_from_source(
	source: Source,
	channel: Channel,
	image: &mut Option<VersionInfo>,
	list: &mut Vec<PackageCfg>
) -> io::Result<()> {

	if image.is_none() && list.is_empty() {
		return Ok(())
	}

	// build connection
	let client = Client::connect(source.addr, source.public_key).await
		.map_err(io_other)?;

	let mut to_rem = vec![];

	for (id, cfg) in list.iter_mut().enumerate() {
		let pack = cfg.package();

		// check package info
		let req = PackageInfoReq {
			channel, name: pack.name.clone()
		};
		let info = client.request(req).await
			.map_err(io_other)?;

		let package = match info.package {
			Some(p) => p,
			None => continue
		};

		to_rem.push(id);

		if pack.version == package.version {
			continue
		}

		// todo we got an update
		update_package(cfg, package, &client).await?;

	}

	todo!("check image for update")

}

pub async fn update_package(
	cfg: &mut PackageCfg,
	new: Package,
	client: &Client
) -> io::Result<()> {

	todo!("make update")

	// download new file
	// validate hash
	// validate signature
	// extract
	// update cfg
	// write cfg

}


pub struct Packages {
	cfg: PackagesCfg,
	list: Vec<PackageCfg>
}

impl Packages {

	pub async fn load() -> io::Result<Self> {

		let cfg = FileDb::open(path("packages.fdb")).await?
			.into_data();

		let mut list = vec![];
		// read all directories

		let mut dirs = fs::read_dir(PACKAGES_DIR).await?;
		while let Some(entry) = dirs.next_entry().await? {
			if !entry.file_type().await?.is_dir() {
				continue
			}

			let mut path = entry.path();
			path.push("package.fdb");
			let cfg = FileDb::open(path).await?
				.into_data();

			list.push(cfg);
		}

		Ok(Self { cfg, list })
	}

}