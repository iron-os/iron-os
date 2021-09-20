
use crate::command::Command;
use crate::io_other;

use std::io;
use std::path::{Path, PathBuf};
use std::os::unix::io::AsRawFd;
use std::fs::{self, File};

use uuid::Uuid;

use libc::{uid_t, gid_t};

pub fn chown(fd: &File, owner: uid_t, group: gid_t) -> io::Result<()> {
	let r = unsafe {
		libc::fchown(fd.as_raw_fd(), owner, group)
	};

	if r == -1 {
		Err(io::Error::last_os_error())
	} else {
		Ok(())
	}
}

pub fn mount(path: impl AsRef<Path>, dest: impl AsRef<Path>) -> io::Result<()> {
	let dest = dest.as_ref();
	// first unmount
	// but ignore the result since it returns an error of nothing is mounted
	let _ = umount(dest);
	Command::new("mount")
		.arg(path.as_ref())
		.arg(dest)
		.exec()
}

pub fn umount(path: impl AsRef<Path>) -> io::Result<()> {
	Command::new("umount")
		.arg("-f")
		.arg(path.as_ref())
		.exec()
}

pub fn cp(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
	Command::new("cp")
		.args(&["-r", "-p"])
		.arg(from.as_ref())
		.arg(to.as_ref())
		.exec()
}

// returns the file name
pub fn list_files(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
	let mut v = vec![];

	for entry in fs::read_dir(dir)? {
		let e = entry?;
		if e.file_type()?.is_dir() {
			continue
		}

		// so we have a file
		v.push(e.path());
	}

	Ok(v)
}

pub fn root_uuid() -> io::Result<Uuid> {
	// read the cmdline and get the root parameter
	let cmd = fs::read_to_string("/proc/cmdline")?;
	cmd.split_ascii_whitespace()
		.find_map(|p| {
			p.split_once('=')
				.filter(|(a, _)| a == &"root")
				.map(|(_, b)| b)
		})
		.and_then(|v| {
			v.split_once('=')
				.filter(|(a, _)| matches!(*a, "UUID" | "PARTUUID"))
				.map(|(_, b)| b)
		})
		.map(Uuid::parse_str)
		.ok_or_else(|| io_other("no root or uuid"))
		.and_then(|o| o.map_err(io_other))
}

// BOOT_IMAGE=/bzImage
pub fn boot_image() -> io::Result<String> {
	// read the cmdline and get the root parameter
	let cmd = fs::read_to_string("/proc/cmdline")?;
	cmd.split_ascii_whitespace()
		.find_map(|p| {
			p.split_once('=')
				.filter(|(a, _)| a == &"BOOT_IMAGE")
				.map(|(_, b)| b)
		})
		.map(Into::into)
		.ok_or_else(|| io_other("no boot_image"))
}