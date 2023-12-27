use crate::version_info::{version_info, version_info_db};

use bootloader_api::requests::UpdateReq;
use stdio_api::deserialize;

pub fn update_image_fix_10(arg: &str) {
	let req: UpdateReq = deserialize(arg).unwrap();

	let version = version_info().expect("failed to load version_info");

	if !version.installed {
		panic!("executed on a device which is not installed")
	}

	crate::disks::update(&req.path, &version).expect("failed to call update");

	let mut db = version_info_db().expect("failed to get version_db");
	let data = db.data_mut();
	data.version_str = req.version_str;
	data.version = req.version;
	data.signature = Some(req.signature);
	db.write_sync().expect("failed to write version_db");
}