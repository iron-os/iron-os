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
use tokio::process::Command;

use rand::{thread_rng, Rng};

use bootloader_api::{VersionInfoReq, VersionInfo, SystemdRestart};
use packages::packages::{PackagesCfg, PackageCfg, Package, Source, Channel};
use packages::client::Client;
use packages::requests::{PackageInfoReq, GetFileReq};
use file_db::FileDb;

use bootloader_api::{UpdateReq, RestartReq};

const PACKAGES_DIR: &str = "/data/packages";

fn path(s: &str) -> PathBuf {
	Path::new(PACKAGES_DIR).join(s)
}


pub async fn start(client: Bootloader) -> io::Result<JoinHandle<()>> {

	Ok(tokio::spawn(async move {
		// get version info so we know if we should update alot or not
		let version_info = client.request(&VersionInfoReq).await
			.expect("fetching version failed");

		if !version_info.installed {
			// not installed

			// for an installation to be finished
			// a restart is required so we don't need to check it in the loop
			eprintln!("not installed, only update when installed");
			return
		}

		loop {

			let packages = Packages::load().await
				.expect("could not load packages");

			// we do this step on every iteration to
			// always get a new random value
			let time = match packages.cfg.channel.is_debug() {
				// check version 30 seconds
				true => Duration::from_secs(30),
				false => Duration::from_secs(
					// check version every 5-15 minutes
					thread_rng()
						.gen_range((60 * 5)..(60 * 15))
				)
			};

			sleep(time).await;

			let mut updated = Updated::new();

			// update every
			if let Err(e) = update(
				&version_info,
				packages,
				&mut updated,
				&client
			).await {
				eprintln!("update error {:?}", e);
			}

			eprintln!("updated: {:?}", updated);

			// if image was updated
			if updated.image.is_some() {
				client.request(&RestartReq).await
					.expect("could not restart the system");
			// if packages updated
			} else if !updated.packages.is_empty() {
				client.request(&SystemdRestart {
					name: "service-bootloader".into()
				}).await
					.expect("could not restart service-bootloader");
			}

		}
	}))
}

pub async fn update(
	version: &VersionInfo,
	mut packages: Packages,
	updated: &mut Updated,
	bootloader: &Bootloader
) -> io::Result<()> {

	let mut image = Some(version);

	for source in packages.cfg.sources.into_iter().rev() {

		update_from_source(
			source,
			packages.cfg.channel,
			&mut image,
			&mut packages.list,
			updated,
			bootloader
		).await?;

	}

	Ok(())
}

pub async fn update_from_source(
	source: Source,
	channel: Channel,
	image: &mut Option<&VersionInfo>,
	list: &mut Vec<PackageCfg>,
	updated: &mut Updated,
	bootloader: &Bootloader
) -> io::Result<()> {

	if image.is_none() && list.is_empty() {
		return Ok(())
	}

	// build connection
	let client = Client::connect(&source.addr, source.public_key.clone()).await
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

		// validate signature
		if !source.sign_key.verify(
			package.version.as_slice(),
			&package.signature
		) {
			return Err(io_other(format!("signature mismatch {:?}", package)))
		}

		// todo we got an update
		update_package(cfg, package, &client).await?;

		updated.packages.push(cfg.package().clone());

	}

	for rem in to_rem.iter().rev() {
		list.swap_remove(*rem);
	}

	if image.is_some() {
		update_image(
			&source,
			channel,
			image,
			&client,
			updated,
			bootloader
		).await?;
	}

	Ok(())
}

pub async fn update_package(
	cfg: &mut PackageCfg,
	new: Package,
	client: &Client
) -> io::Result<()> {

	// extract
	let path = format!("{}/{}", PACKAGES_DIR, new.name);
	let tar = format!("{}/{}.tar.gz", path, new.name);

	download_file(&new, &client, &tar).await?;

	let other_path = format!("{}/{}", path, cfg.other());
	// remove other folder
	let _ = fs::remove_dir_all(&other_path).await;

	// extract
	extract(&tar, &path).await?;

	let extracted_path = format!("{}/{}", path, new.name);

	fs::rename(extracted_path, other_path).await?;

	fs::remove_file(&tar).await?;

	// update cfg
	cfg.switch(new);

	// write cfg
	let db_path = format!("{}/package.fdb", path);
	let db = FileDb::new(&db_path, cfg.clone());
	db.write().await?;

	Ok(())
}

async fn download_file(
	package: &Package,
	client: &Client,
	path: &str
) -> io::Result<()> {

	// download new file
	let req = GetFileReq {
		hash: package.version.clone()
	};
	let file = client.request(req).await
		.map_err(io_other)?;

	if file.is_empty() {
		return Err(io_other(format!("file empty {:?}", package)));
	}

	// validate hash
	if file.hash() != package.version {
		return Err(io_other(format!("hash mismatch {:?}", package)))
	}

	// remove file if it exists
	let _ = fs::remove_file(&path).await;

	fs::write(&path, file.file()).await?;

	Ok(())
}

pub async fn update_image(
	source: &Source,
	channel: Channel,
	version: &mut Option<&VersionInfo>,
	client: &Client,
	updated: &mut Updated,
	bootloader: &Bootloader
) -> io::Result<()> {
	if version.is_none() {
		return Ok(())
	}

	// check for new version
	let req = PackageInfoReq {
		channel, name: "image".into()
	};
	let info = client.request(req).await
		.map_err(io_other)?;

	let package = match info.package {
		Some(p) => p,
		// package not found
		None => return Ok(())
	};

	let version = version.take().unwrap();

	if version.version == package.version {
		return Ok(())
	}

	// validate signature
	if !source.sign_key.verify(
		package.version.as_slice(),
		&package.signature
	) {
		return Err(io_other(format!("signature mismatch {:?}", package)))
	}

	// /data/tmp/image
	let path = "/data/tmp/image";
	let _ = fs::create_dir_all(&path).await;
	let tar = format!("{}/{}.tar.gz", path, package.name);
	download_file(&package, &client, &tar).await?;

	let img_path = format!("{}/{}", path, package.name);
	let _ = fs::remove_dir_all(&img_path).await;

	// extract
	extract(&tar, &path).await?;
	let _ = fs::remove_file(&tar).await;

	// version info
	let req = UpdateReq {
		version_str: package.version_str.clone(),
		version: package.version.clone(),
		signature: package.signature.clone(),
		path: img_path.clone()
	};
	let version = bootloader.request(&req).await?;

	// remove the folder
	let _ = fs::remove_dir_all(&img_path).await;
	updated.image = Some(version);

	Ok(())
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

	pub fn on_run_binary(&self) -> Option<(String, String)> {
		let on_run = &self.cfg.on_run;
		let package = self.list.iter().find(|p| &p.package().name == on_run)?;
		let binary = package.package().binary.as_ref()?;

		let dir = format!(
			"{}/{}/{}",
			PACKAGES_DIR,
			on_run,
			package.current()
		);

		let bin = format!("{}/{}", dir, binary);

		Some((dir, bin))
	}

}

async fn extract(path: &str, to: &str) -> io::Result<()> {
	let stat = Command::new("tar")
		.args(&["-zxvf", path, "-C", to])
		.status().await?;
	if stat.success() {
		Ok(())
	} else {
		Err(io_other("extraction failed"))
	}
}

#[derive(Debug)]
pub struct Updated {
	packages: Vec<Package>,
	image: Option<VersionInfo>
}

impl Updated {

	fn new() -> Self {
		Self {
			packages: vec![],
			image: None
		}
	}

}