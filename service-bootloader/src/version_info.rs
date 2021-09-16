
use std::io;

use bootloader_api::VersionInfo;
use file_db::FileDb;

pub fn version_info() -> io::Result<VersionInfo> {
	FileDb::open_sync("/boot/version.fdb")
		.map(FileDb::into_data)
}

/// updates the version info at the mount location
/// under /mnt
pub fn update_version_info() -> io::Result<()> {
	let mut db = FileDb::<VersionInfo>::open_sync("/mnt/version.fdb")?;
	db.data_mut().installed = true;
	db.write_sync()?;

	Ok(())
}