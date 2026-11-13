//! What the journal gets: frame statistics once a minute while frames are
//! drawn, and warnings which can't flood it (journald keeps only 40M).

use std::fmt::Display;
use std::time::{Duration, Instant};

use tracing::{info, warn};

const INTERVAL: Duration = Duration::from_secs(60);

pub struct MinuteStats {
	since: Instant,
	frames: u32,
	failed: u32,
	render: Duration,
	render_max: Duration,
	/// time from queueing a frame until it was on the display
	flip: Duration,
	flip_max: Duration,
	flips: u32,
}

impl MinuteStats {
	pub fn new() -> Self {
		Self {
			since: Instant::now(),
			frames: 0,
			failed: 0,
			render: Duration::ZERO,
			render_max: Duration::ZERO,
			flip: Duration::ZERO,
			flip_max: Duration::ZERO,
			flips: 0,
		}
	}

	pub fn frame(&mut self, took: Duration, failed: bool) {
		self.frames += 1;
		if failed {
			self.failed += 1;
		}
		self.render += took;
		self.render_max = self.render_max.max(took);
		self.log();
	}

	pub fn flipped(&mut self, took: Duration) {
		self.flips += 1;
		self.flip += took;
		self.flip_max = self.flip_max.max(took);
	}

	fn log(&mut self) {
		let elapsed = self.since.elapsed();
		if elapsed < INTERVAL {
			return;
		}

		let ms = |d: Duration| d.as_secs_f64() * 1000.0;
		info!(
			"stats: {:.1} fps, render avg {:.2}ms max {:.2}ms, \
			 flip avg {:.1}ms max {:.1}ms, {} failed frames",
			self.frames as f64 / elapsed.as_secs_f64(),
			ms(self.render) / self.frames.max(1) as f64,
			ms(self.render_max),
			ms(self.flip) / self.flips.max(1) as f64,
			ms(self.flip_max),
			self.failed
		);

		*self = Self::new();
	}
}

/// A warning which is logged at most once a minute, with the number of
/// times it was suppressed in between.
#[derive(Default)]
pub struct LimitedWarn {
	last: Option<Instant>,
	suppressed: u32,
}

impl LimitedWarn {
	pub fn warn(&mut self, msg: impl Display) {
		if self.last.is_some_and(|t| t.elapsed() < INTERVAL) {
			self.suppressed += 1;
			return;
		}

		if self.suppressed > 0 {
			warn!("{msg} ({} more times in the last minute)", self.suppressed);
		} else {
			warn!("{msg}");
		}
		self.last = Some(Instant::now());
		self.suppressed = 0;
	}
}
