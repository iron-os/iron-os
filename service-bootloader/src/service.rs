
use crate::command::Command;
use crate::io_other;
use crate::disks::{api_disks, install_on};
use crate::version_info::{version_info, version_info_db};
use crate::util::chown;

use std::io;
use std::path::Path;
use std::fs::File;
use std::os::unix::fs::PermissionsExt;

use bootloader_api::{
	Server, request_handler, SystemdRestart, Disks,
	Disk, InstallOn, VersionInfoReq, VersionInfo,
	MakeRoot, RestartReq, UpdateReq
};
use file_db::FileDb;

use serde::Deserialize;



#[derive(Debug, Deserialize)]
pub struct Package {
	pub name: String,
	pub binary: String
}

#[derive(Debug, Deserialize)]
pub enum PackageCfg {
	// do i need to other package??
	Left(Package),
	Right(Package)
}

impl PackageCfg {
	pub fn current(&self) -> &'static str {
		match self {
			Self::Left(_) => "left",
			Self::Right(_) => "right"
		}
	}

	pub fn pack(&self) -> &Package {
		match self {
			Self::Left(p) => p,
			Self::Right(p) => p
		}
	}
}

// open packages folder
// and then open the folder
// service


request_handler!{
	fn systemd_restart(req: SystemdRestart) -> io::Result<()> {
		Command::new("systemctl")
			.args(&["restart", &req.name])
			.exec()?;
		Ok(())
	}
}

request_handler!{
	fn disks(_d: Disks) -> io::Result<Vec<Disk>> {
		api_disks()
	}
}

request_handler!{
	fn install_on_handler(req: InstallOn) -> io::Result<()> {
		install_on(req.name)
	}
}

request_handler!{
	fn version_info_handle(_r: VersionInfoReq) -> io::Result<VersionInfo> {
		version_info()
	}
}

request_handler!{
	fn make_root(req: MakeRoot) -> io::Result<()> {
		let MakeRoot { path } = req;

		let file = File::open(&path)?;

		// set root
		chown(&file, 0, 0)?;

		let mut perms = file.metadata()?.permissions();
		perms.set_mode(0o4755);

		file.set_permissions(perms)?;

		Ok(())
	}
}

request_handler!{
	fn restart(_req: RestartReq) -> io::Result<()> {
		Command::new("shutdown")
			.args(&["-r", "now"])
			.exec()?;
		Ok(())
	}
}

request_handler!{
	fn update(req: UpdateReq) -> io::Result<VersionInfo> {
		let version = version_info()?;
		if version.version == req.version {
			return Err(io_other("already updated"));
		}

		if !version.installed {
			return Err(io_other("before updating you need to install"))
		}

		crate::disks::update(&req.path)?;

		let mut db = version_info_db()?;
		let data = db.data_mut();
		data.version_str = req.version_str;
		data.version = req.version;
		data.signature = Some(req.signature);
		db.write_sync()?;

		Ok(db.into_data())
	}
}

pub fn start() -> io::Result<()> {

	/*
## Chnobli service bootloader

- manage pssplash
- start chnobli service package

- supports api via stdin
 - can switch boot img
 - update images from img file
 - can restart
 - watchdog for chnobli service
	 restart if chnobli service does not send
	 connected for a given period
*/

	let service_package = Path::new("/data/packages/service");
	let package_file = service_package.join("package.fdb");
	let package: PackageCfg = FileDb::<PackageCfg>::open_sync(package_file)?
		.into_data();
	let curr_path = service_package.join(package.current());
	let bin_path = curr_path.join(&package.pack().binary);

	let mut child = Command::new(bin_path)
		.current_dir(curr_path)
		.as_user()
		.env("XDG_RUNTIME_DIR", "/run/user/14")
		.env("WAYLAND_DISPLAY", "wayland-0")
		.stdin_piped()
		.stdout_piped()
		.spawn()?;
	child.kill_on_drop(true);

	// todo kill the process if the child is dropped

	let mut server = Server::new(&mut child)
		.ok_or_else(|| io_other("could not get stdin or stdout"))?;

	server.register(systemd_restart);
	server.register(disks);
	server.register(install_on_handler);
	server.register(version_info_handle);
	server.register(make_root);
	server.register(restart);
	server.register(update);

	server.run()?;

	let s = child.wait()?;
	s.success().then(|| ())
		.ok_or_else(|| io_other("command exited with non success status"))?;

	Ok(())
}