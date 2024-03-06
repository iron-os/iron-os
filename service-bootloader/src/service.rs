use crate::command::Command;
use crate::disks::{api_disks, install_on};
use crate::io_other;
use crate::util::chown;
use crate::version_info::{version_info, version_info_db};

use std::fs::File;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use bootloader_api::error::Error;
use bootloader_api::requests::{
	Disk, Disks, InstallOn, MakeRoot, RestartReq, ShutdownReq, SystemdRestart,
	UpdateReq, VersionInfo, VersionInfoReq,
};
use bootloader_api::{request_handler, Server};
use file_db::FileDb;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Package {
	pub name: String,
	pub binary: String,
}

#[derive(Debug, Deserialize)]
pub enum PackageCfg {
	// do i need to other package??
	Left(Package),
	Right(Package),
}

impl PackageCfg {
	pub fn current(&self) -> &'static str {
		match self {
			Self::Left(_) => "left",
			Self::Right(_) => "right",
		}
	}

	pub fn pack(&self) -> &Package {
		match self {
			Self::Left(p) => p,
			Self::Right(p) => p,
		}
	}
}

// open packages folder
// and then open the folder
// service

request_handler! {
	fn systemd_restart(req: SystemdRestart) -> Result<(), Error> {
		Command::new("systemctl")
			.args(&["restart", &req.name])
			.exec()
			.map_err(Error::internal_display)
	}
}

request_handler! {
	fn disks(_d: Disks) -> Result<Vec<Disk>, Error> {
		api_disks().map_err(Error::internal_display)
	}
}

request_handler! {
	fn install_on_handler(req: InstallOn) -> Result<(), Error> {
		install_on(req.disk).map_err(Error::internal_display)
	}
}

request_handler! {
	fn version_info_handle(_r: VersionInfoReq) -> Result<VersionInfo, Error> {
		version_info().map_err(Error::internal_display)
	}
}

request_handler! {
	fn make_root(req: MakeRoot) -> Result<(), Error> {
		let MakeRoot { path } = req;

		let file = File::open(&path)
			.map_err(Error::internal_display)?;

		// set root
		chown(&file, 0, 0)
			.map_err(Error::internal_display)?;

		let mut perms = file.metadata()
			.map_err(Error::internal_display)?
			.permissions();
		perms.set_mode(0o4755);

		file.set_permissions(perms)
			.map_err(Error::internal_display)?;

		Ok(())
	}
}

request_handler! {
	fn restart(_req: RestartReq) -> Result<(), Error> {
		Command::new("shutdown")
			.args(&["-r", "now"])
			.exec()
			.map_err(Error::internal_display)
	}
}

request_handler! {
	fn shutdown(_req: ShutdownReq) -> Result<(), Error> {
		Command::new("shutdown")
			.arg("now")
			.exec()
			.map_err(Error::internal_display)
	}
}

request_handler! {
	fn update(req: UpdateReq) -> Result<VersionInfo, Error> {
		let version = version_info()
			.map_err(Error::internal_display)?;

		if version.version == req.version {
			return Err(Error::AlreadyUpdated);
		}

		if !version.installed {
			return Err(Error::InternalError(
				"before updating you need to install".into()
			))
		}

		crate::disks::update(&req.path, &version)
			.map_err(Error::internal_display)?;

		let mut db = version_info_db()
			.map_err(Error::internal_display)?;
		let data = db.data_mut();
		data.version_str = req.version_str;
		data.version = req.version;
		data.signature = Some(req.signature);
		db.write_sync()
			.map_err(Error::internal_display)?;

		Ok(db.into_data())
	}
}

pub fn start() -> io::Result<()> {
	let service_package = Path::new("/data/packages/service");
	let package_file = service_package.join("package.fdb");
	let package: PackageCfg =
		FileDb::<PackageCfg>::open_sync(package_file)?.into_data();
	let curr_path = service_package.join(package.current());
	let bin_path = curr_path.join(&package.pack().binary);

	let mut child = Command::new(bin_path);

	child
		.current_dir(curr_path)
		.as_user()
		.env("XDG_RUNTIME_DIR", "/run/user/14")
		.env("WAYLAND_DISPLAY", "wayland-0");

	#[cfg(feature = "headless")]
	child.env("HEADLESS", "yes");

	#[cfg(feature = "image-debug")]
	child.env("IMAGE_DEBUG", "yes");

	let mut child = child.stdin_piped().stdout_piped().spawn()?;
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
	server.register(shutdown);
	server.register(update);

	server.run()?;

	let s = child.wait()?;
	s.success()
		.then(|| ())
		.ok_or_else(|| io_other("command exited with non success status"))?;

	Ok(())
}
