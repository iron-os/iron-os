
use crate::error::{Result};
use crate::util::{create_dir, hash_file, read_toml, remove_dir, compress, copy};

use tokio::fs;

use file_db::FileDb;

use bootloader_api::requests::{VersionInfo, Architecture};

use serde::{Deserialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ImageToml {
	pub version: String,
	pub board: String,
	pub arch: Architecture,
	pub name: String
}

/// The toml file which get's used between the download and pack-image command
#[derive(Debug, Clone, Deserialize)]
pub struct ProductToml {
	product: String
}

/// Downloads and fills a full packages folder
/// with the packages listed in `packages.toml`
/// the address and the channel should be in `packages.toml`
#[derive(clap::Parser)]
pub struct PackImage {}

pub async fn pack_image(_: PackImage) -> Result<()> {

	let cfg: ImageToml = read_toml("./image.toml").await?;

	let product: ProductToml = read_toml("./product.toml").await?;

	let tmp_path = format!("./image_tmp/{}", cfg.name);
	create_dir(&tmp_path).await?;

	copy("./rootfs.ext2", &format!("{}/rootfs.ext2", tmp_path)).await?;
	let kernel_img = match cfg.arch {
		Architecture::Amd64 => {
			let img_path = format!("{}/bzImage", tmp_path);
			copy("./bzImage", &img_path).await?;
			copy(
				"./efi-part/EFI/BOOT/bootx64.efi",
				&format!("{}/bootx64.efi", tmp_path)
			).await?;

			img_path
		},
		Architecture::Arm64 => {
			let img_path = format!("{}/Image.gz", tmp_path);
			copy("./Image.gz", &img_path).await?;
			copy(
				"./u-boot.bin",
				&format!("{}/u-boot.bin", tmp_path)
			).await?;

			img_path
		}
	};

	let kernel_image_size = fs::metadata(kernel_img).await
		.map_err(|e| {
			err!(format!("{:?}", e), "could not read kernel image metadata")
		})?
		.len();

	// 20mb
	if kernel_image_size > 20 * 1_000_000 {
		return Err(err!(
			format!("{:.0}mb", kernel_image_size / 1_000_000),
			"kernel image size to big"
		));
	}

	// todo: maybe use the version as a name?
	compress("image.tar.gz", "./image_tmp", &cfg.name)?;
	remove_dir("./image_tmp").await?;

	let hash = hash_file("./image.tar.gz").await?;

	let version = VersionInfo {
		board: cfg.board,
		arch: cfg.arch,
		product: product.product,
		version_str: cfg.version,
		version: hash,
		signature: None,
		device_id: None,
		installed: false
	};

	let db = FileDb::new("./version.fdb", version);
	db.write().await
		.map_err(|e| err!(e, "could not store version.fdb"))?;

	Ok(())
}