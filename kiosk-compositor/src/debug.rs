//! Debugging a running compositor.
//!
//! Everything that happens is an event, sent as json lines to the clients
//! of a unix socket next to the wayland socket (`kiosk-compositor debug
//! <command>`):
//! - `events` streams all events live
//! - `dump` prints the last rare events (display, kiosk requests, touches,
//!   devices, windows, output, captures), frequent ones (commits with their
//!   damage, frames, flips, touch motion) are only sent live
//! - `stats` prints frame statistics of the last seconds
//! - `damage on|off` flashes every area a client repaints
//! - `hud on|off` shows a graph of frame intervals and render times

use crate::state::State;
use crate::stats::MinuteStats;

use std::cell::Cell;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use serde_json::{json, Map, Value};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::desktop::get_popup_toplevel_coords;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Physical, Point, Rectangle};
use smithay::wayland::compositor::{
	get_parent, with_states, Damage, SurfaceAttributes,
};
use tracing::{info, warn};

const HISTORY: usize = 2000;
const FRAMES: usize = 120;
const DAMAGE_SHOWN: Duration = Duration::from_millis(400);
const MAX_DAMAGE: usize = 256;
/// a client has to send its command within this time
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
/// replies are small, a client which doesn't read them is dropped
const WRITE_TIMEOUT: Duration = Duration::from_millis(100);

struct Frame {
	at: Instant,
	render: Duration,
	commits: u32,
}

pub struct Debug {
	start: Instant,
	history: VecDeque<String>,
	subscribers: Vec<UnixStream>,
	pub damage_overlay: bool,
	pub hud: bool,
	damage: VecDeque<(Instant, Rectangle<i32, Logical>)>,
	frames: VecDeque<Frame>,
	commits: u32,
	minute: MinuteStats,
	socket_path: Option<PathBuf>,
}

impl Debug {
	pub fn new() -> Self {
		Self {
			start: Instant::now(),
			history: VecDeque::with_capacity(HISTORY),
			subscribers: vec![],
			damage_overlay: false,
			hud: false,
			damage: VecDeque::new(),
			frames: VecDeque::with_capacity(FRAMES),
			commits: 0,
			minute: MinuteStats::new(),
			socket_path: None,
		}
	}

	/// If someone watches the live events, frequent events are only built
	/// then.
	pub fn live(&self) -> bool {
		!self.subscribers.is_empty()
	}

	/// Records a rare event, data has to be a json object.
	pub fn event(&mut self, kind: &str, data: Value) {
		let line = self.line(kind, data);
		if self.history.len() == HISTORY {
			self.history.pop_front();
		}
		self.history.push_back(line.clone());
		self.send(line);
	}

	/// Sends a frequent event to the live subscribers only.
	pub fn frequent(&mut self, kind: &str, data: Value) {
		if self.live() {
			let line = self.line(kind, data);
			self.send(line);
		}
	}

	fn line(&self, kind: &str, data: Value) -> String {
		let mut obj = match data {
			Value::Object(m) => m,
			_ => Map::new(),
		};
		let t = self.start.elapsed().as_secs_f64() * 1000.0;
		obj.insert("t".into(), json!((t * 10.0).round() / 10.0));
		obj.insert("event".into(), json!(kind));
		Value::Object(obj).to_string()
	}

	fn send(&mut self, mut line: String) {
		if self.subscribers.is_empty() {
			return;
		}
		line.push('\n');
		// a subscriber which can't keep up is dropped
		self.subscribers
			.retain_mut(|s| s.write_all(line.as_bytes()).is_ok());
	}

	/// A frame was drawn.
	pub fn frame(&mut self, render: Duration, queued: bool) {
		self.minute.frame(render, queued);

		let now = Instant::now();
		let interval = self.frames.back().map(|f| now - f.at);
		if self.frames.len() == FRAMES {
			self.frames.pop_front();
		}
		self.frames.push_back(Frame {
			at: now,
			render,
			commits: self.commits,
		});
		self.frequent(
			"frame",
			json!({
				"render_ms": ms(render),
				"interval_ms": interval.map(ms),
				"commits": self.commits,
				"queued": queued,
			}),
		);
		self.commits = 0;
	}

	/// A queued frame is on the display.
	pub fn flipped(&mut self, took: Duration) {
		self.minute.flipped(took);
		self.frequent("flip", json!({ "ms": ms(took) }));
	}

	/// If the overlays need a new frame all the time.
	pub fn animating(&self) -> bool {
		self.hud || !self.damage.is_empty()
	}

	fn stats(&self) -> Value {
		let frames = self.frames.len();
		let span = match (self.frames.front(), self.frames.back()) {
			(Some(a), Some(b)) if frames > 1 => b.at - a.at,
			_ => Duration::ZERO,
		};
		let render: Vec<_> = self.frames.iter().map(|f| ms(f.render)).collect();
		let max = render.iter().cloned().fold(0.0, f64::max);
		let avg = render.iter().sum::<f64>() / render.len().max(1) as f64;
		let commits: u32 = self.frames.iter().map(|f| f.commits).sum();
		let secs = span.as_secs_f64();
		let per_s = |n: f64| if secs > 0.0 { n / secs } else { 0.0 };

		json!({
			"frames": frames,
			"seconds": (secs * 100.0).round() / 100.0,
			"fps": per_s(frames.saturating_sub(1) as f64),
			"commits_per_s": per_s(commits as f64),
			"render_avg_ms": avg,
			"render_max_ms": max,
			"last_frame_ago_ms": self.frames.back().map(|f| ms(f.at.elapsed())),
		})
	}

	/// Overlay elements, front to back.
	pub fn elements(
		&mut self,
		output: Rectangle<i32, Logical>,
	) -> Vec<SolidColorRenderElement> {
		let mut elements = vec![];
		let now = Instant::now();

		if self.hud {
			self.hud_elements(output, &mut elements);
		}

		self.damage.retain(|(at, _)| now - *at < DAMAGE_SHOWN);
		for (at, rect) in self.damage.iter().rev() {
			let age = (now - *at).as_secs_f32() / DAMAGE_SHOWN.as_secs_f32();
			let a = 0.4 * (1.0 - age);
			elements.push(solid(rect.to_physical(1), [a, 0.0, a, a]));
		}

		elements
	}

	/// A graph in the top right corner: one bar per frame, its height is the
	/// time since the frame before (the line is 60 fps), green the render
	/// time, a blue dot if a client committed.
	fn hud_elements(
		&self,
		output: Rectangle<i32, Logical>,
		elements: &mut Vec<SolidColorRenderElement>,
	) {
		const W: i32 = 2;
		const H: i32 = 80;
		// pixels per millisecond
		const SCALE: f64 = 2.0;

		let width = W * FRAMES as i32;
		let x0 = output.loc.x + output.size.w - width - 10;
		let y0 = output.loc.y + 10;
		let base = y0 + H;

		let line = base - (16.7 * SCALE) as i32;
		elements.push(solid(rect(x0, line, width, 1), [0.8, 0.0, 0.0, 0.8]));

		let mut prev: Option<Instant> = None;
		for (i, f) in self.frames.iter().enumerate() {
			let x = x0 + i as i32 * W;
			if let Some(prev) = prev {
				let h = ((ms(f.at - prev) * SCALE) as i32).clamp(1, H);
				let grey = [0.4, 0.4, 0.4, 0.8];
				elements.push(solid(rect(x, base - h, W, h), grey));
			}
			let r = ((ms(f.render) * SCALE * 4.0) as i32).clamp(1, H);
			elements.push(solid(rect(x, base - r, W, r), [0.0, 0.7, 0.0, 0.9]));
			if f.commits > 0 {
				let blue = [0.1, 0.3, 1.0, 1.0];
				elements.push(solid(rect(x, base + 2, W, 3), blue));
			}
			prev = Some(f.at);
		}

		elements.push(solid(
			rect(x0 - 4, y0 - 4, width + 8, H + 12),
			[0.0, 0.0, 0.0, 0.6],
		));
	}
}

impl Drop for Debug {
	fn drop(&mut self) {
		if let Some(path) = &self.socket_path {
			let _ = fs::remove_file(path);
		}
	}
}

fn ms(d: Duration) -> f64 {
	(d.as_secs_f64() * 1000.0 * 100.0).round() / 100.0
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Physical> {
	Rectangle::new((x, y).into(), (w, h).into())
}

fn solid(
	geo: Rectangle<i32, Physical>,
	color: [f32; 4],
) -> SolidColorRenderElement {
	SolidColorRenderElement::new(
		Id::new(),
		geo,
		CommitCounter::default(),
		color,
		Kind::Unspecified,
	)
}

/// A client committed new content: counts it for the stats, remembers its
/// damage for the overlay and sends it to the live subscribers.
///
/// Has to run before the buffer handler, it takes the buffer and the damage.
pub fn commit(state: &mut State, surface: &WlSurface) {
	let debug = &mut state.debug;
	debug.commits += 1;
	if !debug.damage_overlay && !debug.live() {
		return;
	}

	let (damage, buffer) = with_states(surface, |states| {
		let mut attrs = states.cached_state.get::<SurfaceAttributes>();
		let attrs = attrs.current();
		let damage: Vec<Rectangle<i32, Logical>> = attrs
			.damage
			.iter()
			.take(MAX_DAMAGE)
			.map(|d| match d {
				Damage::Surface(r) => *r,
				// buffer scale and transform are always 1 and normal
				Damage::Buffer(r) => Rectangle::new(
					(r.loc.x, r.loc.y).into(),
					(r.size.w, r.size.h).into(),
				),
			})
			.collect();
		(damage, attrs.buffer.is_some())
	});

	// subsurfaces are shown at the position of their parent
	let mut root = surface.clone();
	while let Some(parent) = get_parent(&root) {
		root = parent;
	}
	let (kind, offset): (_, Point<i32, Logical>) =
		if let Some(popup) = state.popups.find_popup(&root) {
			let toplevel = get_popup_toplevel_coords(&popup);
			("popup", toplevel + popup.geometry().loc)
		} else if let Some(window) = state
			.space
			.elements()
			.find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(&root))
		{
			let loc = state.space.element_location(window).unwrap_or_default();
			("window", loc - window.geometry().loc)
		} else {
			("other", (0, 0).into())
		};
	let kind = if &root != surface { "subsurface" } else { kind };

	// client values, never overflow
	let damage: Vec<Rectangle<i32, Logical>> = damage
		.into_iter()
		.map(|r| {
			Rectangle::new(
				(
					r.loc.x.saturating_add(offset.x),
					r.loc.y.saturating_add(offset.y),
				)
					.into(),
				r.size,
			)
		})
		.collect();

	let debug = &mut state.debug;
	if debug.damage_overlay {
		let now = Instant::now();
		debug.damage.retain(|(at, _)| now - *at < DAMAGE_SHOWN);
		debug.damage.extend(damage.iter().map(|r| (now, *r)));
		while debug.damage.len() > MAX_DAMAGE {
			debug.damage.pop_front();
		}
	}

	if !debug.live() {
		return;
	}
	let area: i64 = damage
		.iter()
		.map(|r| (r.size.w as i64).saturating_mul(r.size.h as i64))
		.fold(0, i64::saturating_add);
	let pid = state
		.display_handle
		.get_client(surface.id())
		.ok()
		.and_then(|c| c.get_credentials(&state.display_handle).ok())
		.map(|c| c.pid);

	state.debug.frequent(
		"commit",
		json!({
			"surface": surface.id().protocol_id(),
			"pid": pid,
			"kind": kind,
			"buffer": buffer,
			"damage_px": area,
			"damage": damage
				.iter()
				.take(8)
				.map(|r| [r.loc.x, r.loc.y, r.size.w, r.size.h])
				.collect::<Vec<_>>(),
		}),
	);
}

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

	loop_handle
		.insert_source(
			Generic::new(listener, Interest::READ, Mode::Level),
			|_, listener, state| {
				while let Ok((stream, _)) = listener.accept() {
					accept(state, stream);
				}
				Ok(PostAction::Continue)
			},
		)
		.map_err(|e| io::Error::other(e.to_string()))?;

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
				thread::spawn(move || {
					let _ = s.write_all(text.as_bytes());
				});
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
		Some("hud") => {
			state.debug.hud = on(parts.next());
			reply(stream, json!({ "hud": state.debug.hud }));
			state.schedule_render();
		}
		_ => reply(
			stream,
			json!({
				"commands": [
					"events", "dump", "stats", "damage on|off", "hud on|off"
				]
			}),
		),
	}
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
