//! A wayland compositor for kiosk devices running Iron OS.
//!
//! It shows exactly one fullscreen application (chromium) and implements the
//! weston kiosk protocol (display on/off, brightness, mouse click) and the
//! weston capture protocol so it can be used as a drop-in replacement for our
//! weston fork.

mod backlight;
mod capture;
mod debug;
mod drm;
mod headless;
mod input;
mod kiosk;
mod render;
mod state;
mod stats;
mod touch_stats;

use state::{Backend, State};

use std::ffi::OsStr;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use tracing::{error, info, warn};

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
	/// name of the wayland socket in $XDG_RUNTIME_DIR
	#[arg(short = 'S', long, default_value = "wayland-0")]
	socket: String,
	/// render offscreen without a display (for testing)
	#[arg(long)]
	headless: bool,
	/// output size in headless mode
	#[arg(long, default_value = "1920x1080")]
	headless_size: String,
	#[arg(long, default_value = "kiosk_compositor=info,warn")]
	tracing: String,
	#[command(subcommand)]
	cmd: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
enum Cmd {
	/// talk to the running compositor: events, dump, stats,
	/// damage on|off, hud on|off
	Debug { command: Vec<String> },
}

fn main() -> ExitCode {
	let args = Args::parse();

	if let Some(Cmd::Debug { command }) = &args.cmd {
		return match debug::client(&args.socket, command) {
			Ok(()) => ExitCode::SUCCESS,
			Err(e) => {
				eprintln!("{e}");
				ExitCode::FAILURE
			}
		};
	}

	tracing_subscriber::fmt()
		.with_env_filter(args.tracing.as_str())
		.without_time()
		.with_ansi(false)
		.init();

	// a panic is an error in the journal, that's what we look for on devices
	std::panic::set_hook(Box::new(|info| {
		error!("panic: {info}");
	}));

	info!("starting kiosk-compositor {}", env!("CARGO_PKG_VERSION"));

	match run(args) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			error!("{e}");
			ExitCode::FAILURE
		}
	}
}

fn run(args: Args) -> Result<(), String> {
	let mut event_loop: EventLoop<State> =
		EventLoop::try_new().map_err(|e| format!("event loop {e}"))?;
	let display: Display<State> =
		Display::new().map_err(|e| format!("display {e}"))?;

	let backend = if args.headless {
		let size = parse_size(&args.headless_size)
			.ok_or("invalid --headless-size, expected WxH")?;
		Backend::Headless(Box::new(headless::init(size)?))
	} else {
		Backend::Drm(Box::new(drm::init(&event_loop.handle())?))
	};

	let mut state =
		State::new(display, &event_loop.handle(), backend, &args.socket)?;

	touch_stats::start(&mut state);

	if let Err(e) =
		debug::listen(&mut state, &event_loop.handle(), &args.socket)
	{
		warn!("no debug socket {e}");
	}

	state.start_backend();
	notify_ready();

	info!("listening on {}", args.socket);

	event_loop
		.run(None, &mut state, |state| {
			state.space.refresh();
			state.popups.cleanup();
			let _ = state.display_handle.flush_clients();
		})
		.map_err(|e| format!("event loop failed {e}"))
}

/// Tells systemd (Type=notify) that the wayland socket is ready, so
/// chromium.service doesn't start before it exists.
fn notify_ready() {
	let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
		return;
	};
	let path = path.as_bytes();
	let addr = match path.strip_prefix(b"@") {
		Some(name) => SocketAddr::from_abstract_name(name),
		None => SocketAddr::from_pathname(OsStr::from_bytes(path)),
	};
	let res = UnixDatagram::unbound()
		.and_then(|s| addr.and_then(|a| s.send_to_addr(b"READY=1", &a)));
	if let Err(e) = res {
		warn!("could not notify systemd {e}");
	}
}

fn parse_size(s: &str) -> Option<(i32, i32)> {
	let (w, h) = s.split_once('x')?;
	Some((w.parse().ok()?, h.parse().ok()?))
}
