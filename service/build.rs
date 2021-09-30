
use std::env;
use std::path::Path;

use wayland_scanner::{generate_code, Side};


const PROTO_FILE: &str = "./weston-kiosk-shell.xml";

fn main() {

	let out_dir = env::var("OUT_DIR").unwrap();

	generate_code(
		PROTO_FILE,
		Path::new(&out_dir).join("weston_kiosk_shell.rs"),
		Side::Client
	)

}