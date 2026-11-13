//! Touch input between the touchscreen and the client.
//!
//! eGalax controllers lose releases and report phantom touches, so:
//! - a touch on a dark display only wakes it, the client doesn't get it
//! - a new down on a slot which is still down first ends the old touch
//! - all touches are cancelled when the display turns off or the
//!   touchscreen disappears

use crate::state::State;

use serde_json::json;
use smithay::backend::input::TouchSlot;
use smithay::input::touch::{DownEvent, MotionEvent, UpEvent};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

type Pos = Point<f64, Logical>;

#[derive(Default)]
pub struct Touches {
	/// touch points that woke the display, these are not forwarded
	swallowed: Vec<TouchSlot>,
	/// touch points the client got a down for and no up yet, with their
	/// last position
	forwarded: Vec<(TouchSlot, Pos)>,
}

impl State {
	pub fn touch_down(&mut self, slot: TouchSlot, pos: Option<Pos>, time: u32) {
		let size = self.output_size().map(|g| g.size);
		self.touch_stats.down(slot.into(), pos, size, !self.awake);

		// a new down means the controller lost the up of the last touch on
		// this slot, end that one first
		self.touches.swallowed.retain(|s| *s != slot);
		self.end_forwarded(slot, time);

		// a touch on a dark display only wakes it up, the client should not
		// get a tap the user could not see
		if !self.awake {
			self.touch_event("touch_down", slot, pos, false, true);
			self.wake_by_input();
			self.touches.swallowed.push(slot);
			return;
		}

		let (Some(touch), Some(pos)) = (self.seat.get_touch(), pos) else {
			return;
		};
		let under = self.surface_under(pos);
		self.touch_event("touch_down", slot, Some(pos), under.is_some(), false);
		self.touches.forwarded.push((slot, pos));
		touch.down(
			self,
			under,
			&DownEvent {
				slot,
				location: pos,
				serial: SERIAL_COUNTER.next_serial(),
				time,
			},
		);
	}

	pub fn touch_motion(
		&mut self,
		slot: TouchSlot,
		pos: Option<Pos>,
		time: u32,
	) {
		let Some(pos) = pos else {
			return;
		};
		let Some(f) = self.touches.forwarded.iter_mut().find(|f| f.0 == slot)
		else {
			// swallowed or never seen
			return;
		};
		f.1 = pos;

		let Some(touch) = self.seat.get_touch() else {
			return;
		};
		let under = self.surface_under(pos);
		self.touch_event(
			"touch_motion",
			slot,
			Some(pos),
			under.is_some(),
			false,
		);
		touch.motion(
			self,
			under,
			&MotionEvent {
				slot,
				location: pos,
				time,
			},
		);
	}

	pub fn touch_up(&mut self, slot: TouchSlot, time: u32) {
		self.touch_stats.up(slot.into());

		let swallowed = self.touches.swallowed.iter().position(|s| *s == slot);
		if let Some(i) = swallowed {
			self.touch_event("touch_up", slot, None, false, true);
			self.touches.swallowed.swap_remove(i);
			return;
		}

		self.touch_event("touch_up", slot, None, false, false);
		// an up the client never got a down for is not forwarded
		self.end_forwarded(slot, time);
	}

	pub fn touch_frame(&mut self) {
		if let Some(touch) = self.seat.get_touch() {
			touch.frame(self);
		}
	}

	/// The touchscreen cancelled its touches or disappeared.
	pub fn touch_device_cancel(&mut self) {
		self.debug.event("touch_cancel", json!({}));
		self.touch_stats.cancel();
		self.touches.swallowed.clear();
		self.cancel_touches();
	}

	/// Tells the client that all its touch points are gone.
	pub fn cancel_touches(&mut self) {
		let forwarded = std::mem::take(&mut self.touches.forwarded);
		let Some(touch) = self.seat.get_touch() else {
			return;
		};
		if forwarded.is_empty() {
			return;
		}

		// smithay only cancels touch points with an event after their last
		// frame, so give every point one more motion
		for (slot, pos) in forwarded {
			touch.motion(
				self,
				None,
				&MotionEvent {
					slot,
					location: pos,
					time: self.start_time.elapsed().as_millis() as u32,
				},
			);
		}
		touch.cancel(self);
	}

	/// Sends the client an up for a touch it still has down on this slot.
	fn end_forwarded(&mut self, slot: TouchSlot, time: u32) {
		let Some(i) = self.touches.forwarded.iter().position(|f| f.0 == slot)
		else {
			return;
		};
		self.touches.forwarded.swap_remove(i);
		let Some(touch) = self.seat.get_touch() else {
			return;
		};
		touch.up(
			self,
			&UpEvent {
				slot,
				serial: SERIAL_COUNTER.next_serial(),
				time,
			},
		);
	}

	fn touch_event(
		&mut self,
		kind: &str,
		slot: TouchSlot,
		pos: Option<Pos>,
		focused: bool,
		swallowed: bool,
	) {
		let data = json!({
			"slot": i32::from(slot),
			"x": pos.map(|p| p.x.round()),
			"y": pos.map(|p| p.y.round()),
			"focused": focused,
			"swallowed": swallowed,
		});
		if kind == "touch_motion" {
			self.debug.frequent(kind, data);
		} else {
			self.debug.event(kind, data);
		}
	}
}
