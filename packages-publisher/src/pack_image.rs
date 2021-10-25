
use crate::error::{Result};
use crate::util::{create_dir, hash_file, read_toml, remove_dir, compress, copy};

use file_db::FileDb;

use bootloader_api::{VersionInfo, Architecture};

use serde::{Deserialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ImageToml {
	pub version: String,
	pub board: String,
	pub arch: Architecture
}

/// Downloads and fills a full packages folder
/// with the packages listed in `packages.toml`
/// the address and the channel should be in `packages.toml`
#[derive(clap::Parser)]
pub struct PackImage {}

pub async fn pack_image(_: PackImage) -> Result<()> {

	let cfg: ImageToml = read_toml("./image.toml").await?;

	let tmp_path = "./image_tmp/image";
	create_dir(tmp_path).await?;

	copy("./bzImage", &format!("{}/bzImage", tmp_path)).await?;
	copy("./rootfs.ext2", &format!("{}/rootfs.ext2", tmp_path)).await?;
	match cfg.arch {
		Architecture::Amd64 => {
			copy(
				"./efi-part/EFI/BOOT/bootx64.efi",
				&format!("{}/bootx64.efi", tmp_path)
			).await?;
		},
		Architecture::Arm64 => {
			copy(
				"./efi-part/EFI/BOOT/bootaa64.efi",
				&format!("{}/bootaa64.efi", tmp_path)
			).await?;
		}
	}

	// todo: maybe use the version as a name?
	compress("image.tar.gz", "./image_tmp", "image")?;
	remove_dir("./image_tmp").await?;

	let hash = hash_file("./image.tar.gz").await?;

	let version = VersionInfo {
		board: cfg.board,
		arch: cfg.arch,
		version_str: cfg.version,
		version: hash,
		signature: None,
		installed: false
	};

	let db = FileDb::new("./version.fdb", version);
	db.write().await
		.map_err(|e| err!(e, "could not store version.fdb"))?;

	Ok(())
}