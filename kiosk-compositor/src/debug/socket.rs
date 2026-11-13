//! The debug socket and the `kiosk-compositor debug` client.

use super::{COMMAND_TIMEOUT, WRITE_TIMEOUT};

use crate::state::State;

use std::cell::Cell;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use std::{env, fs, thread};

use serde_json::{json, Value};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use tracing::{info, warn};

fn socket_path(wayland_socket: &str) -> io::Result<PathBuf> {
	let dir = env::var_os("XDG_RUNTIME_DIR")
		.ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR not set"))?;
	Ok(Path::new(&dir).join(format!("{wayland_socket}-debug.sock")))
}

/// Listens on the debug socket next to the wayland socket.
pub fn listen(
	state: &mut State,
	loop_handle: &LoopHandle<'static, State>,
	wayland_socket: &str,
) -> io::Result<()> {
	let path = socket_path(wayland_socket)?;
	// the wayland socket is locked already, so this one is ours
	let _ = fs::remove_file(&path);

	let listener = UnixListener::bind(&path)?;
	listener.set_nonblocking(true)?;
	state.debug.socket_path = Some(path);

	let token = Rc::new(Cell::new(None));
	let source_token = token.clone();
	let res = loop_handle.insert_source(
		Generic::new(listener, Interest::READ, Mode::Level),
		move |_, listener, state| loop {
			match listener.accept() {
				Ok((stream, _)) => accept(state, stream),
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					return Ok(PostAction::Continue);
				}
				// e.g. no file descriptors left, the listener stays readable
				// so pause it instead of spinning
				Err(e) => {
					warn!("debug socket accept {e}, pausing it for 1s");
					let token = source_token.get();
					let res = state.loop_handle.insert_source(
						Timer::from_duration(Duration::from_secs(1)),
						move |_, _, state| {
							if let Some(t) = token {
								let _ = state.loop_handle.enable(&t);
							}
							TimeoutAction::Drop
						},
					);
					if res.is_err() {
						return Ok(PostAction::Continue);
					}
					return Ok(PostAction::Disable);
				}
			}
		},
	);
	let t = res.map_err(|e| io::Error::other(e.to_string()))?;
	token.set(Some(t));

	Ok(())
}

fn accept(state: &mut State, stream: UnixStream) {
	if stream.set_nonblocking(true).is_err() {
		return;
	}

	let done = Rc::new(Cell::new(false));
	let mut buf = Vec::new();
	let read_done = done.clone();
	let res = state.loop_handle.insert_source(
		Generic::new(stream, Interest::READ, Mode::Level),
		move |_, stream, state| {
			if read_done.get() {
				return Ok(PostAction::Remove);
			}
			let mut chunk = [0; 256];
			// safety: the stream is not dropped
			let stream = unsafe { stream.get_mut() };
			match stream.read(&mut chunk) {
				Ok(0) => {
					read_done.set(true);
					return Ok(PostAction::Remove);
				}
				Ok(n) => buf.extend_from_slice(&chunk[..n]),
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					return Ok(PostAction::Continue);
				}
				Err(_) => {
					read_done.set(true);
					return Ok(PostAction::Remove);
				}
			}

			let Some(end) = buf.iter().position(|b| *b == b'\n') else {
				if buf.len() > 1024 {
					read_done.set(true);
					return Ok(PostAction::Remove);
				}
				return Ok(PostAction::Continue);
			};
			let line = String::from_utf8_lossy(&buf[..end]).to_string();
			read_done.set(true);
			command(state, stream, line.trim());
			Ok(PostAction::Remove)
		},
	);
	let token = match res {
		Ok(t) => t,
		Err(e) => {
			warn!("debug socket {e}");
			return;
		}
	};

	// a client which never sends a command is closed
	let res = state.loop_handle.insert_source(
		Timer::from_duration(COMMAND_TIMEOUT),
		move |_, _, state| {
			if !done.get() {
				done.set(true);
				state.loop_handle.remove(token);
			}
			TimeoutAction::Drop
		},
	);
	if let Err(e) = res {
		warn!("debug socket timer {e}");
	}
}

fn reply(stream: &mut UnixStream, v: Value) {
	let _ = writeln!(stream, "{v}");
}

fn command(state: &mut State, stream: &mut UnixStream, cmd: &str) {
	// replies are written blocking but never for long
	let _ = stream.set_nonblocking(false);
	let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));

	let mut parts = cmd.split_whitespace();
	let on = |arg: Option<&str>| arg != Some("off");
	match parts.next() {
		Some("events") => {
			info!("debug client subscribed to events");
			match stream.try_clone() {
				Ok(s) => {
					// a slow reader must not block the compositor
					let _ = s.set_nonblocking(true);
					state.debug.subscribers.push(s);
				}
				Err(e) => reply(stream, json!({ "error": e.to_string() })),
			}
		}
		Some("dump") => {
			// the history is bigger than the socket buffer, a thread writes
			// it so a slow reader can't block the compositor
			let mut text = String::new();
			for line in &state.debug.history {
				text.push_str(line);
				text.push('\n');
			}
			if let Ok(mut s) = stream.try_clone() {
				let _ = s.set_write_timeout(Some(Duration::from_secs(30)));
				let res = thread::Builder::new()
					.name("debug-dump".into())
					.spawn(move || {
						let _ = s.write_all(text.as_bytes());
					});
				if let Err(e) = res {
					warn!("could not write the dump {e}");
				}
			}
		}
		Some("stats") => {
			let mut stats = state.debug.stats();
			stats["awake"] = json!(state.awake);
			stats["windows"] = json!(state.space.elements().count());
			stats["output"] = json!(state.output.as_ref().and_then(|o| {
				o.current_mode()
					.map(|m| format!("{}x{}@{}", m.size.w, m.size.h, m.refresh))
			}));
			reply(stream, stats);
		}
		Some("damage") => {
			state.debug.damage_overlay = on(parts.next());
			reply(stream, json!({ "damage": state.debug.damage_overlay }));
			state.schedule_render();
		}
		Some("display") => {
			let awake = on(parts.next());
			if state.set_awake(awake) {
				state.kiosk.send_state_change(awake);
			}
			reply(stream, json!({ "display": state.awake }));
		}
		#[cfg(feature = "test-input")]
		Some("inject") => {
			let res = inject(state, parts);
			reply(stream, json!({ "inject": res }));
		}
		Some("hud") => {
			state.debug.hud = on(parts.next());
			reply(stream, json!({ "hud": state.debug.hud }));
			state.schedule_render();
		}
		_ => reply(
			stream,
			json!({
				"commands": [
					"events", "dump", "stats", "damage on|off", "hud on|off",
					"display on|off"
				]
			}),
		),
	}
}

/// Touch events for the integration tests:
/// `inject down|motion <slot> <x> <y>`, `inject up <slot>`, `inject frame`,
/// `inject cancel`, x and y from 0 to 1.
#[cfg(feature = "test-input")]
fn inject<'a>(
	state: &mut State,
	mut parts: impl Iterator<Item = &'a str>,
) -> bool {
	use smithay::backend::input::TouchSlot;

	let kind = parts.next();
	let mut num = || parts.next().and_then(|p| p.parse::<f64>().ok());
	let slot = num().map(|s| TouchSlot::from(Some(s as u32)));
	let pos = match (num(), num(), state.output_size()) {
		(Some(x), Some(y), Some(geo)) => {
			Some((x * geo.size.w as f64, y * geo.size.h as f64).into())
		}
		_ => None,
	};
	let time = state.start_time.elapsed().as_millis() as u32;

	match (kind, slot) {
		(Some("down"), Some(slot)) => state.touch_down(slot, pos, time),
		(Some("motion"), Some(slot)) => state.touch_motion(slot, pos, time),
		(Some("up"), Some(slot)) => state.touch_up(slot, time),
		(Some("frame"), _) => state.touch_frame(),
		(Some("cancel"), _) => state.touch_device_cancel(),
		_ => return false,
	}
	true
}

/// `kiosk-compositor debug <command>`: sends the command to the running
/// compositor and prints its answer.
pub fn client(wayland_socket: &str, command: &[String]) -> Result<(), String> {
	if env::var_os("XDG_RUNTIME_DIR").is_none() {
		// the default of the compositor running in weston.service
		env::set_var("XDG_RUNTIME_DIR", "/run/user/14");
	}
	let path = socket_path(wayland_socket).map_err(|e| e.to_string())?;
	let mut stream = UnixStream::connect(&path)
		.map_err(|e| format!("could not connect to {}: {e}", path.display()))?;

	let cmd = if command.is_empty() {
		"help".to_string()
	} else {
		command.join(" ")
	};
	writeln!(stream, "{cmd}").map_err(|e| e.to_string())?;

	let mut out = io::stdout().lock();
	io::copy(&mut stream, &mut out).map_err(|e| e.to_string())?;
	Ok(())
}
