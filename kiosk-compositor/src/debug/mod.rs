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

mod commit;
mod overlay;
mod socket;

pub use commit::commit;
pub use socket::{client, listen};

use crate::stats::MinuteStats;

use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use smithay::utils::{Logical, Rectangle};

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
	pub fn frame(&mut self, render: Duration, queued: bool, failed: bool) {
		self.minute.frame(render, failed);

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
