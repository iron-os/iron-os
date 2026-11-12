//! Debugging a running compositor.
//!
//! Everything that happens (client commits with their damage, frames, page
//! flips, display state, input, kiosk requests) is an event. The last
//! events are kept in memory and can be streamed live over a unix socket next
//! to the wayland socket (`kiosk-compositor debug <command>`):
//! - `events` streams the events as json lines
//! - `dump` prints the last events
//! - `stats` prints frame statistics of the last seconds
//! - `damage on|off` flashes every area a client repaints
//! - `hud on|off` shows a graph of frame intervals and render times

use crate::state::State;

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{env, fs};

use serde_json::{json, Map, Value};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::utils::{Logical, Physical, Rectangle};
use tracing::{info, warn};

pub const SOCKET_NAME: &str = "kiosk-compositor.sock";

const HISTORY: usize = 2000;
const FRAMES: usize = 120;
const DAMAGE_SHOWN: Duration = Duration::from_millis(400);

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
			socket_path: None,
		}
	}

	/// Records an event, data has to be a json object.
	pub fn event(&mut self, kind: &str, data: Value) {
		let mut obj = match data {
			Value::Object(m) => m,
			_ => Map::new(),
		};
		let t = self.start.elapsed().as_secs_f64() * 1000.0;
		obj.insert("t".into(), json!((t * 10.0).round() / 10.0));
		obj.insert("event".into(), json!(kind));
		let mut line = Value::Object(obj).to_string();

		if self.history.len() == HISTORY {
			self.history.pop_front();
		}
		self.history.push_back(line.clone());

		if !self.subscribers.is_empty() {
			line.push('\n');
			// a subscriber which can't keep up is dropped
			self.subscribers
				.retain_mut(|s| s.write_all(line.as_bytes()).is_ok());
		}
	}

	/// A client committed new content.
	pub fn commit(&mut self, damage: &[Rectangle<i32, Logical>]) {
		self.commits += 1;
		if self.damage_overlay {
			let now = Instant::now();
			self.damage.extend(damage.iter().map(|r| (now, *r)));
		}
	}

	/// A frame was drawn.
	pub fn frame(&mut self, render: Duration, queued: bool) {
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
		self.event(
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

	/// If the overlays need a new frame all the time.
	pub fn animating(&self) -> bool {
		self.hud || !self.damage.is_empty()
	}

	fn stats(&self) -> Value {
		let last = |n: usize| self.frames.iter().rev().take(n);
		let frames = self.frames.len();
		let span = match (self.frames.front(), self.frames.back()) {
			(Some(a), Some(b)) if frames > 1 => b.at - a.at,
			_ => Duration::ZERO,
		};
		let render: Vec<_> = last(frames).map(|f| ms(f.render)).collect();
		let max = render.iter().cloned().fold(0.0, f64::max);
		let avg = render.iter().sum::<f64>() / render.len().max(1) as f64;
		let commits: u32 = last(frames).map(|f| f.commits).sum();
		let secs = span.as_secs_f64();

		json!({
			"frames": frames,
			"seconds": (secs * 100.0).round() / 100.0,
			"fps": if secs > 0.0 { (frames - 1) as f64 / secs } else { 0.0 },
			"commits_per_s": if secs > 0.0 { commits as f64 / secs } else { 0.0 },
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
			let fade =
				1.0 - (now - *at).as_secs_f32() / DAMAGE_SHOWN.as_secs_f32();
			elements.push(solid(
				rect.to_physical(1),
				[0.4 * fade, 0.0, 0.4 * fade, 0.4 * fade],
			));
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
				elements
					.push(solid(rect(x, base - h, W, h), [0.4, 0.4, 0.4, 0.8]));
			}
			let r = ((ms(f.render) * SCALE * 4.0) as i32).clamp(1, H);
			elements.push(solid(rect(x, base - r, W, r), [0.0, 0.7, 0.0, 0.9]));
			if f.commits > 0 {
				elements
					.push(solid(rect(x, base + 2, W, 3), [0.1, 0.3, 1.0, 1.0]));
			}
			prev = Some(f.at);
		}

		elements.push(solid(
			rect(x0 - 4, y0 - 4, width + 8, H + 12),
			[0.0, 0.0, 0.0, 0.6],
		));
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

/// Listens on the debug socket in the same directory as the wayland socket.
pub fn listen(
	state: &mut State,
	loop_handle: &LoopHandle<'static, State>,
) -> io::Result<()> {
	let dir = env::var_os("XDG_RUNTIME_DIR")
		.ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR not set"))?;
	let path = Path::new(&dir).join(SOCKET_NAME);
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

	let mut buf = Vec::new();
	let res = state.loop_handle.insert_source(
		Generic::new(stream, Interest::READ, Mode::Level),
		move |_, stream, state| {
			let mut chunk = [0; 256];
			// safety: the stream is not dropped
			let stream = unsafe { stream.get_mut() };
			match stream.read(&mut chunk) {
				Ok(0) => return Ok(PostAction::Remove),
				Ok(n) => buf.extend_from_slice(&chunk[..n]),
				Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
					return Ok(PostAction::Continue);
				}
				Err(_) => return Ok(PostAction::Remove),
			}

			let Some(end) = buf.iter().position(|b| *b == b'\n') else {
				if buf.len() > 1024 {
					return Ok(PostAction::Remove);
				}
				return Ok(PostAction::Continue);
			};
			let line = String::from_utf8_lossy(&buf[..end]).to_string();
			command(state, stream, line.trim());
			Ok(PostAction::Remove)
		},
	);
	if let Err(e) = res {
		warn!("debug socket {e}");
	}
}

fn command(state: &mut State, stream: &mut UnixStream, cmd: &str) {
	// the answers are small, write them blocking
	let _ = stream.set_nonblocking(false);
	fn reply(stream: &mut UnixStream, v: Value) {
		let _ = writeln!(stream, "{v}");
	}

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
			for line in &state.debug.history {
				if writeln!(stream, "{line}").is_err() {
					break;
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
		Some("hud") => {
			state.debug.hud = on(parts.next());
			reply(stream, json!({ "hud": state.debug.hud }));
			state.schedule_render();
		}
		_ => reply(
			stream,
			json!({
				"commands": ["events", "dump", "stats", "damage on|off", "hud on|off"]
			}),
		),
	}
}

impl Drop for Debug {
	fn drop(&mut self) {
		if let Some(path) = &self.socket_path {
			let _ = fs::remove_file(path);
		}
	}
}

/// `kiosk-compositor debug <command>`: sends the command to the running
/// compositor and prints its answer.
pub fn client(command: &[String]) -> Result<(), String> {
	let dir =
		env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/14".into());
	let path = Path::new(&dir).join(SOCKET_NAME);
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
