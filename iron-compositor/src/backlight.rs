//! Display brightness over /sys/class/backlight (a udev rule makes it
//! writable for the video group).

use std::fs;
use std::io;
use std::path::PathBuf;

const BACKLIGHT_DIR: &str = "/sys/class/backlight";

/// Sets the brightness, 0-255 like the kiosk protocol.
pub fn set(brightness: u32) -> io::Result<()> {
	let dir = find()?;
	let max: u64 = fs::read_to_string(dir.join("max_brightness"))?
		.trim()
		.parse()
		.map_err(io::Error::other)?;

	let value = max * brightness.min(255) as u64 / 255;
	fs::write(dir.join("brightness"), value.to_string())
}

/// Picks the backlight device, preferring firmware over platform over raw
/// interfaces like the kernel documentation recommends.
fn find() -> io::Result<PathBuf> {
	let mut devices = vec![];
	for entry in fs::read_dir(BACKLIGHT_DIR)? {
		let path = entry?.path();
		let kind = fs::read_to_string(path.join("type")).unwrap_or_default();
		let prio = match kind.trim() {
			"firmware" => 0,
			"platform" => 1,
			_ => 2,
		};
		devices.push((prio, path));
	}

	devices.sort();
	devices
		.into_iter()
		.next()
		.map(|(_, path)| path)
		.ok_or_else(|| io::Error::other("no backlight device"))
}
