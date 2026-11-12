//! Frame statistics, logged once a minute while frames are drawn so the
//! journal of a device shows how the compositor performs.

use std::time::{Duration, Instant};

use tracing::info;

const INTERVAL: Duration = Duration::from_secs(60);

pub struct Stats {
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

impl Stats {
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

	pub fn frame(&mut self, took: Duration, ok: bool) {
		self.frames += 1;
		if !ok {
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
