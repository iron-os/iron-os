
use std::io;
use std::os::unix::io::AsRawFd;
use std::fs::File;

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