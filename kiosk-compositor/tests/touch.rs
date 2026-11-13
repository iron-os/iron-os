//! Starts the compositor headless with a wayland client and checks which
//! touch events the client gets, especially for lost releases and touches
//! still down when the display turns off.
//!
//! Needs a render node and the test-input feature:
//! `cargo test --features test-input`

#![cfg(feature = "test-input")]

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use wayland_client::protocol::{
	wl_buffer, wl_compositor, wl_registry, wl_seat, wl_shm, wl_shm_pool,
	wl_surface, wl_touch,
};
use wayland_client::{
	delegate_noop, Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols::xdg::shell::client::{
	xdg_surface, xdg_toplevel, xdg_wm_base,
};

const W: i32 = 640;
const H: i32 = 480;

#[derive(Default)]
struct Client {
	compositor: Option<wl_compositor::WlCompositor>,
	shm: Option<wl_shm::WlShm>,
	wm_base: Option<xdg_wm_base::XdgWmBase>,
	seat: Option<wl_seat::WlSeat>,
	touch: Option<wl_touch::WlTouch>,
	configured: bool,
	events: Vec<String>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Client {
	fn event(
		c: &mut Self,
		registry: &wl_registry::WlRegistry,
		event: wl_registry::Event,
		_: &(),
		_: &Connection,
		qh: &QueueHandle<Self>,
	) {
		let wl_registry::Event::Global {
			name, interface, ..
		} = event
		else {
			return;
		};
		match interface.as_str() {
			"wl_compositor" => {
				c.compositor = Some(registry.bind(name, 4, qh, ()));
			}
			"wl_shm" => c.shm = Some(registry.bind(name, 1, qh, ())),
			"xdg_wm_base" => c.wm_base = Some(registry.bind(name, 1, qh, ())),
			"wl_seat" => c.seat = Some(registry.bind(name, 5, qh, ())),
			_ => {}
		}
	}
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Client {
	fn event(
		_: &mut Self,
		base: &xdg_wm_base::XdgWmBase,
		event: xdg_wm_base::Event,
		_: &(),
		_: &Connection,
		_: &QueueHandle<Self>,
	) {
		if let xdg_wm_base::Event::Ping { serial } = event {
			base.pong(serial);
		}
	}
}

impl Dispatch<xdg_surface::XdgSurface, ()> for Client {
	fn event(
		c: &mut Self,
		surface: &xdg_surface::XdgSurface,
		event: xdg_surface::Event,
		_: &(),
		_: &Connection,
		_: &QueueHandle<Self>,
	) {
		if let xdg_surface::Event::Configure { serial } = event {
			surface.ack_configure(serial);
			c.configured = true;
		}
	}
}

impl Dispatch<wl_seat::WlSeat, ()> for Client {
	fn event(
		c: &mut Self,
		seat: &wl_seat::WlSeat,
		event: wl_seat::Event,
		_: &(),
		_: &Connection,
		qh: &QueueHandle<Self>,
	) {
		if let wl_seat::Event::Capabilities {
			capabilities: WEnum::Value(caps),
		} = event
		{
			if caps.contains(wl_seat::Capability::Touch) && c.touch.is_none() {
				c.touch = Some(seat.get_touch(qh, ()));
			}
		}
	}
}

impl Dispatch<wl_touch::WlTouch, ()> for Client {
	fn event(
		c: &mut Self,
		_: &wl_touch::WlTouch,
		event: wl_touch::Event,
		_: &(),
		_: &Connection,
		_: &QueueHandle<Self>,
	) {
		let e = match event {
			wl_touch::Event::Down { id, .. } => format!("down {id}"),
			wl_touch::Event::Up { id, .. } => format!("up {id}"),
			wl_touch::Event::Motion { id, .. } => format!("motion {id}"),
			wl_touch::Event::Frame => "frame".into(),
			wl_touch::Event::Cancel => "cancel".into(),
			_ => return,
		};
		c.events.push(e);
	}
}

delegate_noop!(Client: ignore wl_compositor::WlCompositor);
delegate_noop!(Client: ignore wl_shm::WlShm);
delegate_noop!(Client: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Client: ignore wl_buffer::WlBuffer);
delegate_noop!(Client: ignore wl_surface::WlSurface);
delegate_noop!(Client: ignore xdg_toplevel::XdgToplevel);

struct Compositor {
	child: Child,
	dir: PathBuf,
}

impl Drop for Compositor {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
		let _ = std::fs::remove_dir_all(&self.dir);
	}
}

fn start() -> Compositor {
	let dir = std::env::temp_dir()
		.join(format!("kiosk-compositor-test-{}", std::process::id()));
	std::fs::create_dir_all(&dir).unwrap();
	let child = Command::new(env!("CARGO_BIN_EXE_kiosk-compositor"))
		.args(["--headless", "--headless-size", &format!("{W}x{H}")])
		.args(["-S", "wayland-test"])
		.env("XDG_RUNTIME_DIR", &dir)
		.spawn()
		.unwrap();
	let comp = Compositor { child, dir };

	let start = Instant::now();
	while !comp.dir.join("wayland-test-debug.sock").exists() {
		assert!(start.elapsed() < Duration::from_secs(10), "no socket");
		sleep(Duration::from_millis(20));
	}
	comp
}

/// Sends a debug command and returns its reply.
fn debug(dir: &Path, cmd: &str) -> String {
	let mut s =
		UnixStream::connect(dir.join("wayland-test-debug.sock")).unwrap();
	writeln!(s, "{cmd}").unwrap();
	let mut line = String::new();
	BufReader::new(s).read_line(&mut line).unwrap();
	line
}

/// A fullscreen window with a white buffer and touch.
fn client(dir: &Path) -> (Client, EventQueue<Client>) {
	let stream = UnixStream::connect(dir.join("wayland-test")).unwrap();
	let conn = Connection::from_socket(stream).unwrap();
	let mut queue = conn.new_event_queue();
	let qh = queue.handle();
	conn.display().get_registry(&qh, ());

	let mut c = Client::default();
	queue.roundtrip(&mut c).unwrap();

	let surface = c.compositor.as_ref().unwrap().create_surface(&qh, ());
	let xdg = c
		.wm_base
		.as_ref()
		.unwrap()
		.get_xdg_surface(&surface, &qh, ());
	let _toplevel = xdg.get_toplevel(&qh, ());
	surface.commit();
	while !c.configured {
		queue.blocking_dispatch(&mut c).unwrap();
	}

	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(true)
		.open(dir.join("buffer"))
		.unwrap();
	file.set_len((W * H * 4) as u64).unwrap();
	let pool =
		c.shm
			.as_ref()
			.unwrap()
			.create_pool(file.as_fd(), W * H * 4, &qh, ());
	let buffer =
		pool.create_buffer(0, W, H, W * 4, wl_shm::Format::Xrgb8888, &qh, ());
	surface.attach(Some(&buffer), 0, 0);
	surface.damage_buffer(0, 0, W, H);
	surface.commit();
	queue.roundtrip(&mut c).unwrap();
	queue.roundtrip(&mut c).unwrap();
	assert!(c.touch.is_some(), "no touch capability");

	(c, queue)
}

/// Runs the commands and returns the touch events the client got.
fn touch(
	dir: &Path,
	c: &mut Client,
	queue: &mut EventQueue<Client>,
	cmds: &[&str],
) -> Vec<String> {
	for cmd in cmds {
		let reply = debug(dir, cmd);
		assert!(!reply.contains("\"inject\":false"), "{cmd}: {reply}");
	}
	queue.roundtrip(c).unwrap();
	std::mem::take(&mut c.events)
}

#[test]
fn touch_events() {
	if !Path::new("/dev/dri/renderD128").exists() {
		eprintln!("no render node, skipping");
		return;
	}

	let comp = start();
	let dir = comp.dir.clone();
	let (mut c, mut queue) = client(&dir);

	// a normal tap
	let ev = touch(
		&dir,
		&mut c,
		&mut queue,
		&[
			"inject down 0 0.5 0.5",
			"inject frame",
			"inject up 0",
			"inject frame",
		],
	);
	assert_eq!(ev, ["down 0", "frame", "up 0", "frame"]);

	// the controller loses the release: the next down ends the old touch
	let ev = touch(
		&dir,
		&mut c,
		&mut queue,
		&[
			"inject down 0 0.5 0.5",
			"inject frame",
			"inject down 0 0.2 0.2",
			"inject frame",
		],
	);
	assert_eq!(ev, ["down 0", "frame", "up 0", "down 0", "frame"]);

	// a finger still down when the display turns off is cancelled
	let ev = touch(&dir, &mut c, &mut queue, &["display off"]);
	assert_eq!(ev, ["motion 0", "cancel"]);

	// the release the controller sends later is not forwarded
	let ev = touch(&dir, &mut c, &mut queue, &["inject up 0", "inject frame"]);
	assert!(ev.is_empty(), "{ev:?}");

	// a touch on the dark display only wakes it up
	let ev = touch(
		&dir,
		&mut c,
		&mut queue,
		&[
			"inject down 1 0.5 0.5",
			"inject frame",
			"inject up 1",
			"inject frame",
		],
	);
	assert!(ev.is_empty(), "{ev:?}");
	assert!(debug(&dir, "stats").contains("\"awake\":true"));

	// the touchscreen cancels while a finger is down
	let ev = touch(
		&dir,
		&mut c,
		&mut queue,
		&["inject down 2 0.5 0.5", "inject frame", "inject cancel"],
	);
	assert_eq!(ev, ["down 2", "frame", "motion 2", "cancel"]);
}
