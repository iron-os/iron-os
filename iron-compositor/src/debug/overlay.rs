//! Overlays drawn over the window: flashing damage and the frame graph.

use super::{ms, Debug, DAMAGE_SHOWN, FRAMES};

use std::time::Instant;

use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::utils::{Logical, Physical, Rectangle};

impl Debug {
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
