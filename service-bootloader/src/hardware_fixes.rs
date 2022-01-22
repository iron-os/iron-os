
use std::fs;
use std::io::{self, Write};

use linux_info::bios::Bios;

/// this is not allowed to panic
pub fn hardware_fixes() {
	// detect on what hardware we are running
	let (manufacturer, product_name) = match system_info() {
		Some(s) => s,
		None => {
			eprintln!("[hardware_fixes] failed to get system info");
			return
		}
	};

	match (manufacturer.trim(), product_name.trim()) {
		("AAEON", "FAY-003") => {
			eprintln!("applying fix: mask gpe07 interrupt");
			if let Err(e) = mask_gpe07_interrupt() {
				eprintln!("masking gpe07 failed with {:?}", e);
			}
		},
		_ => {}
	}
}

/// returns (Manufacturer, Product Name)
fn system_info() -> Option<(String, String)> {
	let bios = Bios::read().ok()?;
	let sys = bios.system_info()?;

	Some((sys.manufacturer.into(), sys.product_name.into()))
}

fn mask_gpe07_interrupt() -> io::Result<()> {
	fs::OpenOptions::new()
		.write(true)
		.open("/sys/firmware/acpi/interrupts/gpe07")?
		.write_all(b"mask\n")
}