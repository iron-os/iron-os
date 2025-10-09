use std::path::Path;
use std::{env, fs};

use serde::Deserialize;

use wayland_scanner::{generate_code, Side};

#[derive(Debug, Clone, Deserialize)]
struct ConfigValues {
	whitelist: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
	debug: Option<ConfigValues>,
	alpha: Option<ConfigValues>,
	beta: Option<ConfigValues>,
	release: Option<ConfigValues>,
}

const KIOSK_PROTO_FILE: &str = "./weston-kiosk-shell.xml";
const CAPTURE_PROTO_FILE: &str = "./weston-output-capture.xml";

fn write_extension_config(values: &ConfigValues) {
	let whitelist_arr = format!(
		"[{}]",
		values
			.whitelist
			.iter()
			.map(|v| format!("\"{}\"", v))
			.collect::<Vec<_>>()
			.join(",")
	);

	let file = format!(
		"\
		export default {{\n\
			whitelist: {}\n\
		}}\
		",
		whitelist_arr
	);

	fs::write("./extension/config.js", file)
		.expect("failed to write extension config.js");
}

fn main() {
	println!("cargo:rerun-if-changed={}", KIOSK_PROTO_FILE);
	println!("cargo:rerun-if-changed={}", CAPTURE_PROTO_FILE);
	println!("cargo:rerun-if-changed=./build-config.toml");
	println!("cargo:rerun-if-env-changed=BUILD_CHANNEL");

	let build_channel =
		env::var("BUILD_CHANNEL").unwrap_or_else(|_| "Debug".into());

	// read toml
	let ctn = fs::read_to_string("./build-config.toml")
		.expect("./build-config.toml not found");
	let cfg: Config =
		toml::from_str(&ctn).expect("failed to read ./build-config.toml");

	let values = match &*build_channel {
		"Debug" => cfg.debug.expect("debug config not found"),
		"Alpha" => cfg.alpha.expect("alpha config not found"),
		"Beta" => cfg.beta.expect("beta config not found"),
		"Release" => cfg.release.expect("release config not found"),
		v => panic!("channel {} unkown", v),
	};

	let out_dir = env::var("OUT_DIR").unwrap();

	write_extension_config(&values);

	generate_code(
		KIOSK_PROTO_FILE,
		Path::new(&out_dir).join("weston_kiosk_shell.rs"),
		Side::Client,
	);
	generate_code(
		CAPTURE_PROTO_FILE,
		Path::new(&out_dir).join("weston_output_capture.rs"),
		Side::Client,
	);
}
