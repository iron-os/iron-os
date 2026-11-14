use std::fs::{self, File};
use std::io::{self, Write};

use bootloader_api::requests::{DeviceId, VersionInfo};
use file_db::FileDb;

use crate::io_other;

pub fn version_info() -> io::Result<VersionInfo> {
	FileDb::open_sync("/boot/version.fdb").map(FileDb::into_data)
}

pub fn version_info_db() -> io::Result<FileDb<VersionInfo>> {
	FileDb::open_sync("/boot/version.fdb")
}

/// Writes /boot/version.fdb so that it survives a power loss.
///
/// The boot partition is vfat, without syncing the new content before the
/// rename a power loss can leave an empty version.fdb which the service can't
/// read.
pub fn write_version_info_synced(info: &VersionInfo) -> io::Result<()> {
	let v = serde_json::to_vec(info).map_err(io_other)?;

	let mut file = File::create("/boot/version.fdb.tmp")?;
	file.write_all(&v)?;
	file.sync_all()?;
	fs::rename("/boot/version.fdb.tmp", "/boot/version.fdb")?;

	unsafe {
		libc::sync();
	}

	Ok(())
}

/// updates the version info at the mount location
/// under /mnt
pub fn update_version_info() -> io::Result<()> {
	let mut db = FileDb::<VersionInfo>::open_sync("/mnt/version.fdb")?;
	{
		let data = db.data_mut();
		data.installed = true;
		data.device_id = Some(DeviceId::new());
	}
	db.write_sync()?;

	Ok(())
}
