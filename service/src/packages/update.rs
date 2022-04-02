
use super::{
	Update, PACKAGES_DIR, extract, PackageUpdateState,
	ImageUpdateState
};
use crate::Bootloader;
use crate::util::io_other;

use std::io;

use tokio::fs;

use packages::packages::{Package, Source};
use packages::client::Client;
use packages::requests::{PackageInfoReq, GetFileReq};

use bootloader_api::requests::UpdateReq;

pub(super) async fn update(
	sources: &[Source],
	update: &mut Update,
	bootloader: &Bootloader
) -> io::Result<()> {

	for source in sources.iter().rev() {

		if update.is_finished() {
			return Ok(())
		}

		update_from_source(
			source,
			update,
			bootloader
		).await?;

	}

	Ok(())
}

async fn update_from_source(
	source: &Source,
	update: &mut Update,
	bootloader: &Bootloader
) -> io::Result<()> {

	// build connection
	let client = Client::connect(&source.addr, source.public_key.clone()).await
		.map_err(io_other)?;

	let arch = update.arch;
	let channel = update.channel;

	for (name, state) in update.to_update() {

		// check package info
		let req = PackageInfoReq {
			arch,
			channel,
			name: name.to_string()
		};
		let info = client.request(req).await
			.map_err(io_other)?;

		// skip if the package as not changed
		let package = match info.package {
			Some(p) => p,
			None => continue
		};

		let (version, new_folder) = match state {
			PackageUpdateState::ToUpdate { version, new_folder } => {
				(version, new_folder)
			},
			// Update only returns packages that are not yet updated
			_ => unreachable!()
		};

		// if the version is the same
		// skip downloading and updating
		if matches!(version, Some(v) if *v == package.version) {
			*state = PackageUpdateState::NoUpdate;
			continue
		}

		// validate signature
		if !source.sign_key.verify(
			&package.version,
			&package.signature
		) {
			return Err(io_other(format!("signature mismatch {:?}", package)))
		}

		// todo we got an update
		update_package(new_folder, &package, &client).await?;

		*state = PackageUpdateState::Updated(package);

	}

	if !update.image.is_finished() {
		update_image(
			source,
			update,
			&client,
			bootloader
		).await?;
	}

	Ok(())
}

/// Downloads the new package and install it
async fn update_package(
	new_folder: &str,
	new: &Package,
	client: &Client
) -> io::Result<()> {

	// extract
	let path = format!("{}/{}", PACKAGES_DIR, new.name);
	let tar = format!("{}/{}.tar.gz", path, new.name);

	// create path if it doens't exist
	match fs::create_dir(&path).await {
		Ok(_) => {},
		// don't return an error if the path already exists
		Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {},
		Err(e) => return Err(e)
	}

	download_file(&new, &client, &tar).await?;

	let other_path = format!("{}/{}", path, new_folder);
	// remove other folder
	let _ = fs::remove_dir_all(&other_path).await;

	// extract
	extract(&tar, &path).await?;

	let extracted_path = format!("{}/{}", path, new.name);

	fs::rename(extracted_path, other_path).await?;

	fs::remove_file(&tar).await?;

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

async fn update_image(
	source: &Source,
	update: &mut Update,
	client: &Client,
	bootloader: &Bootloader
) -> io::Result<()> {

	// check for new version
	let req = PackageInfoReq {
		arch: update.arch,
		channel: update.channel,
		name: format!("image-{}", update.board)
	};
	let info = client.request(req).await
		.map_err(io_other)?;

	let package = match info.package {
		Some(p) => p,
		// package not found
		None => return Ok(())
	};

	let version = match &update.image {
		ImageUpdateState::ToUpdate { version } => version,
		// update_from_source checks if this image needs to be updated
		_ => unreachable!()
	};

	if version == &package.version {
		update.image = ImageUpdateState::NoUpdate;
		return Ok(())
	}

	// validate signature
	if !source.sign_key.verify(
		&package.version,
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
	let version = bootloader.update(&req).await
		.map_err(io_other)?;

	// remove the folder
	let _ = fs::remove_dir_all(&img_path).await;

	update.image = ImageUpdateState::Updated(version);

	Ok(())
}