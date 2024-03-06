use std::fs;
use std::io::{self, Write};
use std::path::Path;

use linux_info::bios::Bios;

/// this is not allowed to panic
pub fn hardware_fixes() {
	// detect on what hardware we are running
	let (manufacturer, product_name) = match system_info() {
		Some(s) => s,
		None => {
			eprintln!("[hardware_fixes] failed to get system info");
			return;
		}
	};

	match (manufacturer.trim(), product_name.trim()) {
		("AAEON", prod) if prod.starts_with("FAY") => {
			eprintln!("applying fix: mask gpe07 interrupt");
			if let Err(e) = mask_gpe07_interrupt() {
				eprintln!("masking gpe07 failed with {:?}", e);
			}

			if let Err(e) = change_sim7600e_mode() {
				eprintln!("changing sim7600e mode failed with {:?}", e);
			}
		}
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

/// this changes the sim7600e mode from mbim to ndis (mbim is not compatible
/// with linux)
fn change_sim7600e_mode() -> io::Result<()> {
	// set 1e0e:9011 to 1e0e:9001
	const PATH: &str = "/sys/bus/usb/devices/usb1/1-6";

	let vendor = fs::read_to_string(Path::new(PATH).join("idVendor"))?;
	let product = fs::read_to_string(Path::new(PATH).join("idProduct"))?;

	if vendor.trim() != "1e0e" || product.trim() != "9011" {
		return Ok(());
	}

	eprintln!("applying fix: change sim7600e mode");

	// change mode
	let mut file = fs::OpenOptions::new().write(true).open("/dev/ttyUSB3")?;

	file.write_all(b"AT+CUSBPIDSWITCH=9001,1,1\r\n")?;

	// we should receive OK but don't liste, since we could listen forever
	Ok(())
}
