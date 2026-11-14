//! Detects an image update which was written but never booted.
//!
//! /boot/version.fdb describes the image the boot config points to, not the
//! running one. If the reboot after an image update doesn't happen (the
//! restart request fails, the service restarts before it), the device keeps
//! running the old image while reporting the new one, and no update fixes
//! that since the version already matches.

use std::io;

use tokio::fs;

const BOOT_CONFIGS: &[&str] =
	&["/boot/EFI/BOOT/grub.cfg", "/boot/extlinux/extlinux.conf"];

/// Remembers the image version a reboot was already tried for, so a wrong
/// detection can't end in a reboot loop.
const MARKER: &str = "/data/reboot-pending";

/// Returns true if the boot config points to another root partition than the
/// running one and no reboot was tried for this image version yet.
pub async fn should_reboot(version_str: &str) -> bool {
	if !reboot_pending().await {
		let _ = fs::remove_file(MARKER).await;
		return false;
	}

	let marker = fs::read_to_string(MARKER).await.ok();
	if marker.as_deref() == Some(version_str) {
		eprintln!(
			"image {version_str} still not running after a reboot, not \
			 rebooting again"
		);
		return false;
	}

	if let Err(e) = write_marker(version_str).await {
		eprintln!("could not write {MARKER} {e:?}, not rebooting");
		return false;
	}

	true
}

async fn write_marker(version_str: &str) -> io::Result<()> {
	let file = fs::File::create(MARKER).await?;
	let mut file = file.into_std().await;
	io::Write::write_all(&mut file, version_str.as_bytes())?;
	file.sync_all()
}

async fn reboot_pending() -> bool {
	let Ok(cmdline) = fs::read_to_string("/proc/cmdline").await else {
		return false;
	};
	let Some(running) = root_partuuid(&cmdline) else {
		return false;
	};

	for path in BOOT_CONFIGS {
		if let Ok(cfg) = fs::read_to_string(path).await {
			return match root_partuuid(&cfg) {
				Some(next) if next != running => {
					eprintln!(
						"running root {running} but {path} boots {next}, the \
						 image update was not booted"
					);
					true
				}
				_ => false,
			};
		}
	}

	false
}

/// The root=PARTUUID= parameter of a kernel command line or boot config.
fn root_partuuid(s: &str) -> Option<String> {
	s.split_ascii_whitespace()
		.find_map(|p| p.strip_prefix("root=PARTUUID="))
		.map(|uuid| uuid.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
	use super::root_partuuid;

	#[test]
	fn finds_the_root_partuuid() {
		let grub = "menuentry \"Iron Os\" {\n linux /bzImageB \
			root=PARTUUID=AB12-cd rootwait quiet\n}";
		assert_eq!(root_partuuid(grub).as_deref(), Some("ab12-cd"));

		let cmdline = "BOOT_IMAGE=/bzImage root=PARTUUID=ab12-CD rootwait\n";
		assert_eq!(root_partuuid(cmdline).as_deref(), Some("ab12-cd"));

		assert_eq!(root_partuuid("root=/dev/vda2 rootwait"), None);
	}
}
