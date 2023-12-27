use super::{
	Update, PACKAGES_DIR, extract, PackageUpdateState,
	ImageUpdateState
};
use crate::Bootloader;
use crate::util::io_other;

use std::{io, mem};
use std::path::Path;

use tokio::fs;
use tokio::process::Command;

use packages::packages::{Package, Source};
use packages::client::Client;
use packages::requests::GetFileBuilder;
use packages::error::Error;

use bootloader_api::requests::{UpdateReq, VersionInfo};

/// if this functions returns Ok(()) the PackageUpdateState::ToUpdate will never
/// be set
pub(super) async fn update(
	sources: &[Source],
	update: &mut Update,
	bootloader: &Bootloader
) -> io::Result<()> {
	for source in sources.iter().rev() {
		if update.is_finished() {
			break
		}

		update_from_source(
			source,
			update,
			bootloader
		).await?;
	}

	// packages that where not found are still in the GatherInfo or DownloadFile
	// state, we need to set them to NotFound
	for (_, state) in update.not_finished_packages() {
		*state = PackageUpdateState::NotFound;
	}

	if !update.image.is_finished() {
		update.image = ImageUpdateState::NotFound;
	}

	Ok(())
}

// 16kb
const FILE_PART_SIZE: u64 = 16384;

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
	let device_id = update.device_id.clone();

	'package_loop: for (name, state) in update.not_finished_packages() {
		let (package, get_file, new_folder) = match state {
			PackageUpdateState::GatherInfo { version, new_folder } => {
				// let's gather some information

				// check package info
				let package = client.package_info(
					channel,
					arch,
					device_id.clone(),
					name.to_string()
				).await.map_err(io_other)?;

				// skip if the package was not found
				let package = match package {
					Some(p) => p,
					None => continue
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
					eprintln!("signature mismatch for {:?}", package);
					// this was a hard error but we don't wan't to brick the
					// whole update process if a package was uploaded with the
					// wrong signature

					// this is not optimal since it might be flagged as NotFound
					continue
				}

				// now we know we should download the file
				*state = PackageUpdateState::DownloadFile {
					get_file: GetFileBuilder::new(
						package.version.clone(),
						FILE_PART_SIZE
					),
					package,
					new_folder: mem::take(new_folder)
				};

				match state {
					PackageUpdateState::DownloadFile {
						package, get_file, new_folder
					} => (package, get_file, new_folder),
					_ => unreachable!()
				}
			},
			PackageUpdateState::DownloadFile {
				package, get_file, new_folder
			} => (package, get_file, new_folder),
			// not_finished_packages only returns packages that are not yet
			// finished, which does not include the following states
			PackageUpdateState::NoUpdate |
			PackageUpdateState::Updated(_) |
			PackageUpdateState::NotFound => unreachable!()
		};

		// download the file
		while !get_file.is_complete() {
			let r = client.get_file_with_builder(get_file).await;

			match r {
				Ok(_) => {},
				Err(Error::FileNotFound) => continue 'package_loop,
				Err(e) => return Err(io_other(e))
			}
		}

		// // todo we got an update
		update_package(new_folder, &package, &get_file).await?;

		*state = PackageUpdateState::Updated(package.clone());
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

/// install the new package and install it
async fn update_package(
	new_folder: &str,
	new: &Package,
	file: &GetFileBuilder
) -> io::Result<()> {

	// extract
	let package_dir = format!("{}/{}", PACKAGES_DIR, new.name);
	let tar = format!("{}/{}.tar.gz", package_dir, new.name);

	// create package directory if it doens't exist
	match fs::create_dir(&package_dir).await {
		Ok(_) => {},
		// don't return an error if the path already exists
		Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {},
		Err(e) => return Err(e)
	}

	// validate hash
	if file.hash() != new.version {
		return Err(io_other!("hash mismatch {:?}", new))
	}

	// remove file if it exists
	let _ = fs::remove_file(&tar).await;
	fs::write(&tar, file.file()).await?;

	let other_path = format!("{}/{}", package_dir, new_folder);
	// remove other folder
	// todo this should be a root call since chromium has a file owned by root
	let _ = fs::remove_dir_all(&other_path).await;

	// extract
	let extracted_path = format!("{}/{}", package_dir, new.name);
	// remove extracted folder if it exists
	let _ = fs::remove_dir_all(&extracted_path).await;
	extract(&tar, &package_dir).await?;


	fs::rename(extracted_path, other_path).await?;

	fs::remove_file(&tar).await?;

	Ok(())
}

/// you need to make sure the image is still not in the finished state
async fn update_image(
	source: &Source,
	update: &mut Update,
	client: &Client,
	bootloader: &Bootloader
) -> io::Result<()> {
	let state = &mut update.image;

	let (package, file) = match state {
		ImageUpdateState::GatherInfo { version } => {
			// check for new version
			let package = client.package_info(
				update.channel,
				update.arch,
				update.device_id.clone(),
				format!("image-{}", update.board)
			).await.map_err(io_other)?;

			// skip if the package was not found
			let package = match package {
				Some(p) => p,
				None => return Ok(())
			};

			if version == &package.version {
				*state = ImageUpdateState::NoUpdate;
				return Ok(())
			}

			// validate signature
			if !source.sign_key.verify(
				&package.version,
				&package.signature
			) {
				eprintln!("signature mismatch {:?}", package);
				// might be flagged as not found
				return Ok(())
			}

			*state = ImageUpdateState::DownloadFile {
				get_file: GetFileBuilder::new(
					package.version.clone(),
					FILE_PART_SIZE
				),
				package
			};

			match state {
				ImageUpdateState::DownloadFile { package, get_file } => {
					(package, get_file)
				},
				_ => unreachable!()
			}
		},
		ImageUpdateState::DownloadFile { package, get_file } => {
			(package, get_file)
		},
		// theses states are marked as finished and since this function
		// only get's called with unfinished states, unreachable
		ImageUpdateState::NoUpdate |
		ImageUpdateState::Updated(_) |
		ImageUpdateState::NotFound => unreachable!()
	};

	// download the file
	while !file.is_complete() {
		let r = client.get_file_with_builder(file).await;

		match r {
			Ok(_) => {},
			Err(Error::FileNotFound) => return Ok(()),
			Err(e) => return Err(io_other(e))
		}
	}

	// /data/tmp/image
	let path = "/data/tmp/image";
	let _ = fs::create_dir_all(&path).await;
	let tar = format!("{}/{}.tar.gz", path, package.name);

	// validate hash
	if file.hash() != package.version {
		return Err(io_other!("hash mismatch {:?}", package))
	}

	// remove file if it exists
	let _ = fs::remove_file(&tar).await;
	fs::write(&tar, file.file()).await?;


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

	// let version = bootloader.update(&req).await
	// 	.map_err(io_other)?;

	// because the native service-bootloader is broken we ship our own for the
	// moment see #10
	let version = fix_10_update_image(bootloader, &req).await?;

	// remove the folder
	let _ = fs::remove_dir_all(&img_path).await;

	*state = ImageUpdateState::Updated(version);

	Ok(())
}

// see #10
async fn fix_10_update_image(
	bootloader: &Bootloader,
	req: &UpdateReq
) -> io::Result<VersionInfo> {
	// make sure we have the new service-bootloader in the correct folder
	let path = "/data/tmp-service-bootloader";
	let _ = fs::remove_dir_all(&path).await;
	let _ = fs::create_dir_all(&path).await;

	let service_bootloader_file = Path::new(path).join("service_bootloader");

	fs::copy("./service_bootloader", &service_bootloader_file).await?;

	// make sure it can be executed as root
	bootloader.make_root(service_bootloader_file.to_str().unwrap()).await
		.map_err(io_other)?;

	eprintln!("executing command update_image_fix_10 {}", stdio_api::serialize(req).unwrap());

	// now call the bootloader
	let output = Command::new(service_bootloader_file)
		.arg("update_image_fix_10")
		.arg(stdio_api::serialize(req).unwrap())
		.output().await?;

	if !output.status.success() {
		eprintln!(
			"update_image_fix_10 failed: {}",
			String::from_utf8_lossy(&output.stderr)
		);

		return Err(io_other("failed to update image"))
	}

	// now parse the version info
	let output = String::from_utf8(output.stdout).map_err(io_other)?;

	stdio_api::deserialize(&output)
		.map_err(io_other)
}