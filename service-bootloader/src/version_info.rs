
use std::io;

use bootloader_api::VersionInfo;
use file_db::FileDb;

pub fn version_info() -> io::Result<VersionInfo> {
	FileDb::open_sync("/etc/version.fdb")
		.map(FileDb::into_data)
}