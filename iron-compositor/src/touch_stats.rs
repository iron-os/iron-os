//! Touch statistics for the journal, the only log we get from devices in
//! the field (journald keeps 40M, so at most one line a minute plus
//! warnings).
//!
//! Once a minute with touches a line with: touches, how many only woke the
//! display, how long fingers were down, touches that came again without a
//! release, fingers still down and where on the screen the touches were (a
//! 4x3 grid). A finger held for more than 30s is logged right away, that's
//! what a controller looks like which lost a release.

use crate::state::State;
use crate::stats::LimitedWarn;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{Logical, Point, Size};
use tracing::{info, warn};

const SUMMARY: Duration = Duration::from_secs(60);
const CHECK: Duration = Duration::from_secs(10);
const HELD_TOO_LONG: Duration = Duration::from_secs(30);
const STUCK_REPEAT: Duration = Duration::from_secs(3600);
const SHORT: Duration = Duration::from_millis(50);
const COLS: usize = 4;
const ROWS: usize = 3;

struct Finger {
	since: Instant,
	pos: Option<(i32, i32)>,
	warned: bool,
}

pub struct TouchStats {
	since: Instant,
	downs: u32,
	woke: u32,
	ups: u32,
	cancels: u32,
	again: u32,
	held: Vec<Duration>,
	grid: [u32; COLS * ROWS],
	fingers: HashMap<i32, Finger>,
	/// when only fingers still down were logged
	stuck_logged: Option<Instant>,
	again_warn: LimitedWarn,
}

impl TouchStats {
	pub fn new() -> Self {
		Self {
			since: Instant::now(),
			downs: 0,
			woke: 0,
			ups: 0,
			cancels: 0,
			again: 0,
			held: vec![],
			grid: [0; COLS * ROWS],
			fingers: HashMap::new(),
			stuck_logged: None,
			again_warn: LimitedWarn::default(),
		}
	}

	pub fn down(
		&mut self,
		slot: i32,
		pos: Option<Point<f64, Logical>>,
		size: Option<Size<i32, Logical>>,
		woke: bool,
	) {
		self.downs += 1;
		if woke {
			self.woke += 1;
		}

		if let (Some(pos), Some(size)) = (pos, size) {
			let col = (pos.x / size.w as f64 * COLS as f64) as usize;
			let row = (pos.y / size.h as f64 * ROWS as f64) as usize;
			self.grid[row.min(ROWS - 1) * COLS + col.min(COLS - 1)] += 1;
		}

		let pos = pos.map(|p| (p.x as i32, p.y as i32));
		let old = self.fingers.insert(
			slot,
			Finger {
				since: Instant::now(),
				pos,
				warned: false,
			},
		);
		if let Some(old) = old {
			self.again += 1;
			// a noisy controller can do this all the time
			self.again_warn.warn(format!(
				"touch: slot {slot} down again without a release, it was \
				 down for {:.1}s",
				old.since.elapsed().as_secs_f32()
			));
		}
	}

	pub fn up(&mut self, slot: i32) {
		self.ups += 1;
		if let Some(f) = self.fingers.remove(&slot) {
			self.held.push(f.since.elapsed());
		}
	}

	/// The touch device cancelled all touches or disappeared.
	pub fn cancel(&mut self) {
		self.cancels += 1;
		self.fingers.clear();
	}

	fn check(&mut self) {
		for (slot, f) in &mut self.fingers {
			let held = f.since.elapsed();
			if held > HELD_TOO_LONG && !f.warned {
				f.warned = true;
				warn!(
					"touch: slot {slot} held for {}s at {} without a release",
					held.as_secs(),
					fmt_pos(f.pos)
				);
			}
		}

		if self.since.elapsed() < SUMMARY {
			return;
		}
		// a finger which is stuck for good is repeated only once an hour
		let stuck_logged = self
			.stuck_logged
			.is_some_and(|t| t.elapsed() < STUCK_REPEAT);
		if self.downs > 0 || (!self.fingers.is_empty() && !stuck_logged) {
			info!("{}", self.summary());
			if self.downs == 0 {
				self.stuck_logged = Some(Instant::now());
			}
		}

		let fingers = std::mem::take(&mut self.fingers);
		let stuck_logged = self.stuck_logged;
		let again_warn = std::mem::take(&mut self.again_warn);
		*self = Self::new();
		self.fingers = fingers;
		self.stuck_logged = stuck_logged;
		self.again_warn = again_warn;
	}

	fn summary(&self) -> String {
		let ms = |d: Duration| d.as_millis();
		let held = if self.held.is_empty() {
			"-".to_string()
		} else {
			let total: Duration = self.held.iter().sum();
			format!(
				"avg {}ms max {}ms, {} under {}ms",
				ms(total / self.held.len() as u32),
				ms(*self.held.iter().max().unwrap()),
				self.held.iter().filter(|d| **d < SHORT).count(),
				ms(SHORT)
			)
		};
		let fingers: Vec<_> = self
			.fingers
			.iter()
			.map(|(slot, f)| {
				format!(
					"slot {slot} {}s at {}",
					f.since.elapsed().as_secs(),
					fmt_pos(f.pos)
				)
			})
			.collect();
		let grid: Vec<_> = self
			.grid
			.chunks(COLS)
			.map(|row| {
				row.iter()
					.map(|c| c.to_string())
					.collect::<Vec<_>>()
					.join(",")
			})
			.collect();

		format!(
			"touch: {} down ({} woke the display), {} up, {} cancel, {} down \
			 again without release, held {held}, still down [{}], grid {}",
			self.downs,
			self.woke,
			self.ups,
			self.cancels,
			self.again,
			fingers.join(", "),
			grid.join("/")
		)
	}
}

fn fmt_pos(pos: Option<(i32, i32)>) -> String {
	match pos {
		Some((x, y)) => format!("{x},{y}"),
		None => "?".into(),
	}
}

pub fn start(state: &mut State) {
	let res = state.loop_handle.insert_source(
		Timer::from_duration(CHECK),
		|_, _, state| {
			state.touch_stats.check();
			TimeoutAction::ToDuration(CHECK)
		},
	);
	if let Err(e) = res {
		warn!("could not start touch stats {e}");
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn summary() {
		let size = Some((1200, 900).into());
		let mut s = TouchStats::new();
		// a normal tap top left which woke the display
		s.down(0, Some((10.0, 10.0).into()), size, true);
		s.up(0);
		// a finger which never gets released, then the same slot again
		s.down(1, Some((1190.0, 890.0).into()), size, false);
		s.down(1, Some((1190.0, 890.0).into()), size, false);

		let line = s.summary();
		assert!(line.starts_with("touch: 3 down (1 woke the display), 1 up"));
		assert!(line.contains("1 down again without release"));
		assert!(line.contains("1 under 50ms"));
		assert!(line.contains("still down [slot 1 0s at 1190,890]"));
		assert!(line.ends_with("grid 1,0,0,0/0,0,0,0/0,0,0,2"), "{line}");

		s.cancel();
		assert!(s.summary().contains("still down []"));
	}
}
