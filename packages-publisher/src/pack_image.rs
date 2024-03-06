use crate::error::Result;
use crate::util::{
	compress, copy, create_dir, hash_file, read_toml, remove_dir,
};

use tokio::fs;

use file_db::FileDb;

use bootloader_api::requests::{Architecture, VersionInfo};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ImageToml {
	pub version: String,
	pub board: String,
	pub arch: Architecture,
	pub name: String,
}

/// The toml file which get's used between the download and pack-image command
#[derive(Debug, Clone, Deserialize)]
pub struct ProductToml {
	product: String,
}

/// creates a tar.gz of the kernel and the rootfs
/// this can then be uploaded to the packages server.
///
/// There is also an option to only create the version.fdb file
/// to add to the boot partition.
#[derive(clap::Parser)]
pub struct PackImage {
	/// If the command should use the existing image.tar.gz to create the
	/// version.fdb file
	#[clap(long)]
	use_existing_image: bool,
	#[clap(long)]
	create_version_file: bool,
}

pub async fn pack_image(opts: PackImage) -> Result<()> {
	let cfg: ImageToml = read_toml("./image.toml").await?;

	if !opts.use_existing_image {
		create_tar_gz(&cfg).await?;
	}

	if !opts.create_version_file {
		// nothing more to done
		return Ok(());
	}

	let hash = hash_file("./image.tar.gz").await?;

	let product: ProductToml = read_toml("./product.toml").await?;

	let version = VersionInfo {
		board: cfg.board,
		arch: cfg.arch,
		product: product.product,
		version_str: cfg.version,
		version: hash,
		signature: None,
		device_id: None,
		installed: false,
	};

	let db = FileDb::new("./version.fdb", version);
	db.write()
		.await
		.map_err(|e| err!(e, "could not store version.fdb"))?;

	Ok(())
}

async fn create_tar_gz(cfg: &ImageToml) -> Result<()> {
	let tmp_path = format!("./image_tmp/{}", cfg.name);
	create_dir(&tmp_path).await?;

	// copy the rootfs to the image
	copy("./rootfs.ext2", &format!("{tmp_path}/rootfs.ext2")).await?;

	// copy the kernel
	let kernel_img = match cfg.arch {
		Architecture::Amd64 => {
			let img_path = format!("{tmp_path}/bzImage");
			// copy kernel
			copy("./bzImage", &img_path).await?;
			// copy bootloader
			copy(
				"./efi-part/EFI/BOOT/bootx64.efi",
				&format!("{tmp_path}/bootx64.efi"),
			)
			.await?;
			// copy grub cfg
			copy(
				"./efi-part/EFI/BOOT/grub.templ",
				&format!("{tmp_path}/grub.templ"),
			)
			.await?;

			img_path
		}
		Architecture::Arm64 => {
			let img_path = format!("{tmp_path}/Image.gz");
			// copy kernel
			copy("./Image.gz", &img_path).await?;
			// copy bootloader
			copy("./u-boot.bin", &format!("{tmp_path}/u-boot.bin")).await?;
			// copy uboot config
			copy(
				"./extlinux/extlinux.templ",
				&format!("{tmp_path}/extlinux.templ"),
			)
			.await?;

			img_path
		}
	};

	let kernel_image_size = fs::metadata(kernel_img)
		.await
		.map_err(|e| {
			err!(format!("{e:?}"), "could not read kernel image metadata")
		})?
		.len();

	// check that kernel is not to big since we have limited space
	// on the boot partition which needs to contain two kernel (a/b) 20mb
	if kernel_image_size > 20 * 1_000_000 {
		return Err(err!(
			format!("{:.0}mb", kernel_image_size / 1_000_000),
			"kernel image size to big"
		));
	}

	// todo: maybe use the version as a name?
	compress("image.tar.gz", "./image_tmp", &cfg.name)?;
	remove_dir("./image_tmp").await?;

	Ok(())
}
