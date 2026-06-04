//! Targeted USB reset for the eGalax `0eef:c000` touchscreen.
//!
//! Issues a `USBDEVFS_RESET` ioctl on the device node instead of tearing
//! down the whole xHCI host controller. Drop this file into a crate (it
//! needs the `libc` dependency) and call [`reset_egalax`].

use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::io::AsRawFd;

const USBDEVFS_RESET: libc::c_ulong = 0x5514; // _IO('U', 20), stable across kernels
const EGALAX_VID: &str = "0eef";
const EGALAX_PID: &str = "c000";

/// Scan `/sys/bus/usb/devices` for the eGalax and return its `(busnum, devnum)`.
fn find_egalax() -> io::Result<Option<(u32, u32)>> {
	for entry in fs::read_dir("/sys/bus/usb/devices")? {
		let path = entry?.path();

		let vendor = fs::read_to_string(path.join("idVendor"))
			.ok()
			.map(|s| s.trim().to_lowercase());
		let product = fs::read_to_string(path.join("idProduct"))
			.ok()
			.map(|s| s.trim().to_lowercase());

		if vendor.as_deref() == Some(EGALAX_VID)
			&& product.as_deref() == Some(EGALAX_PID)
		{
			let bus: u32 = fs::read_to_string(path.join("busnum"))?
				.trim()
				.parse()
				.map_err(io::Error::other)?;
			let dev: u32 = fs::read_to_string(path.join("devnum"))?
				.trim()
				.parse()
				.map_err(io::Error::other)?;
			return Ok(Some((bus, dev)));
		}
	}
	Ok(None)
}

/// Issue `USBDEVFS_RESET` on the eGalax touchscreen, if present.
///
/// Returns `Ok(true)` if the reset was issued, `Ok(false)` if the device
/// is not on the bus (nothing to do). Requires write access to the
/// `/dev/bus/usb/BBB/DDD` node (root or `CAP_SYS_RAWIO`).
pub fn reset_egalax() -> io::Result<bool> {
	let Some((bus, dev)) = find_egalax()? else {
		return Ok(false);
	};

	let node = format!("/dev/bus/usb/{bus:03}/{dev:03}");
	let f = OpenOptions::new().write(true).open(&node)?;

	// SAFETY: USBDEVFS_RESET takes no argument; the fd is a valid usbfs node.
	let r = unsafe { libc::ioctl(f.as_raw_fd(), USBDEVFS_RESET) };
	if r != 0 {
		return Err(io::Error::last_os_error());
	}
	Ok(true)
}
