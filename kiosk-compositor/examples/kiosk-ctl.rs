//! Sends kiosk protocol requests like the Iron OS service does, for testing.
//!
//! kiosk-ctl on|off|brightness <0-255>|click <x> <y>|watch <secs>

use std::time::{Duration, Instant};
use std::{env, thread};

use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};

mod protocol {
	#![allow(dead_code, non_camel_case_types, unused_unsafe)]
	#![allow(unused_variables, non_upper_case_globals, non_snake_case)]
	#![allow(unused_imports, missing_docs, clippy::all)]

	use wayland_client;
	use wayland_client::protocol::*;

	pub mod __interfaces {
		use wayland_client::protocol::__interfaces::*;
		wayland_scanner::generate_interfaces!(
			"protocols/weston-kiosk-shell.xml"
		);
	}
	use self::__interfaces::*;

	wayland_scanner::generate_client_code!("protocols/weston-kiosk-shell.xml");
}

use protocol::weston_kiosk_shell::{self, WestonKioskShell};

#[derive(Default)]
struct App {
	shell: Option<WestonKioskShell>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for App {
	fn event(
		app: &mut Self,
		registry: &wl_registry::WlRegistry,
		event: wl_registry::Event,
		_: &(),
		_: &Connection,
		qh: &QueueHandle<Self>,
	) {
		if let wl_registry::Event::Global {
			name, interface, ..
		} = event
		{
			if interface == "weston_kiosk_shell" {
				app.shell = Some(registry.bind(name, 1, qh, ()));
			}
		}
	}
}

impl Dispatch<WestonKioskShell, ()> for App {
	fn event(
		_: &mut Self,
		_: &WestonKioskShell,
		event: weston_kiosk_shell::Event,
		_: &(),
		_: &Connection,
		_: &QueueHandle<Self>,
	) {
		let weston_kiosk_shell::Event::StateChange { state } = event;
		println!("state_change {state}");
	}
}

fn main() {
	let args: Vec<String> = env::args().skip(1).collect();
	let conn = Connection::connect_to_env().expect("connect");
	let mut queue = conn.new_event_queue();
	let qh = queue.handle();
	conn.display().get_registry(&qh, ());

	let mut app = App::default();
	queue.roundtrip(&mut app).unwrap();
	let shell = app.shell.clone().expect("no weston_kiosk_shell global");

	let mut watch = Duration::ZERO;
	match args.first().map(|s| s.as_str()) {
		Some("on") => shell.set_state(1),
		Some("off") => shell.set_state(0),
		Some("brightness") => shell.set_brightness(args[1].parse().unwrap()),
		Some("click") => shell
			.mouse_click(args[1].parse().unwrap(), args[2].parse().unwrap()),
		Some("watch") => watch = Duration::from_secs(args[1].parse().unwrap()),
		_ => panic!("usage: on|off|brightness n|click x y|watch secs"),
	}

	queue.roundtrip(&mut app).unwrap();
	let start = Instant::now();
	while start.elapsed() < watch {
		queue.dispatch_pending(&mut app).unwrap();
		conn.flush().unwrap();
		if let Some(guard) = queue.prepare_read() {
			let _ = guard.read();
		}
		thread::sleep(Duration::from_millis(50));
	}
	queue.roundtrip(&mut app).unwrap();
}
