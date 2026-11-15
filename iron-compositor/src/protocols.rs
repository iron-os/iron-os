//! Server code for the protocols in protocols/.

macro_rules! server_protocol {
	($name:ident, $file:literal) => {
		pub mod $name {
			#![allow(dead_code, non_camel_case_types, unused_unsafe)]
			#![allow(unused_variables, non_upper_case_globals, non_snake_case)]
			#![allow(unused_imports, missing_docs, clippy::all)]

			use wayland_server;
			use wayland_server::protocol::*;

			pub mod __interfaces {
				use wayland_server::protocol::__interfaces::*;
				wayland_scanner::generate_interfaces!($file);
			}
			use self::__interfaces::*;

			wayland_scanner::generate_server_code!($file);
		}
	};
}

server_protocol!(kiosk_shell, "protocols/weston-kiosk-shell.xml");
server_protocol!(output_capture, "protocols/weston-output-capture.xml");
