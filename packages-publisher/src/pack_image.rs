
use crate::error::{Result};
use crate::util::{create_dir, hash_file, read_toml, remove_dir, compress, copy};

use file_db::FileDb;

use clap::{AppSettings, Clap};

use bootloader_api::VersionInfo;

use serde::{Deserialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PackageToml {
	pub version: String
}

/// Downloads and fills a full packages folder
/// with the packages listed in `packages.toml`
/// the address and the channel should be in `packages.toml`
#[derive(Clap)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct PackImage {}

pub async fn pack_image(_: PackImage) -> Result<()> {

	// read packages.toml
	let cfg: PackageToml = read_toml("./package.toml").await?;

	let tmp_path = "./image_tmp/image";
	create_dir(tmp_path).await?;

	copy("./bzImage", &format!("{}/bzImage", tmp_path)).await?;
	copy("./rootfs.ext2", &format!("{}/rootfs.ext2", tmp_path)).await?;
	copy("./efi-part/EFI/BOOT/bootx64.efi", &format!("{}/bootx64.efi", tmp_path)).await?;

	compress("image.tar.gz", "./image_tmp", "image")?;
	remove_dir("./image_tmp").await?;

	let hash = hash_file("./image.tar.gz").await?;

	let version = VersionInfo {
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