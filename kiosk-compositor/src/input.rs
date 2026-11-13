use crate::state::State;

use serde_json::json;
use smithay::backend::input::{
	AbsolutePositionEvent, Axis, AxisSource, ButtonState, Device,
	DeviceCapability, Event, InputBackend, InputEvent, KeyState,
	KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
	TouchEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{
	AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent,
};
use smithay::utils::{Logical, Point, Size, SERIAL_COUNTER};
use tracing::info;

impl State {
	pub fn process_input_event<I: InputBackend>(
		&mut self,
		event: InputEvent<I>,
	) {
		match event {
			InputEvent::DeviceAdded { device } => {
				info!("input device added: {}", device.name());
				self.debug.event(
					"input_device_added",
					json!({ "name": device.name() }),
				);
				if device.has_capability(DeviceCapability::Pointer) {
					self.pointer_devices += 1;
					if self.seat.get_pointer().is_none() {
						self.seat.add_pointer();
					}
				}
			}
			InputEvent::DeviceRemoved { device } => {
				info!("input device removed: {}", device.name());
				self.debug.event(
					"input_device_removed",
					json!({ "name": device.name() }),
				);
				// a touchscreen which disappears (e.g. an usb reset)
				// never releases its fingers
				if device.has_capability(DeviceCapability::Touch) {
					self.touch_device_cancel();
				}
				if device.has_capability(DeviceCapability::Pointer) {
					self.pointer_devices =
						self.pointer_devices.saturating_sub(1);
					if self.pointer_devices == 0 {
						self.seat.remove_pointer();
					}
				}
			}
			InputEvent::Keyboard { event } => {
				self.debug.event(
					"key",
					json!({
						"code": u32::from(event.key_code()),
						"pressed": event.state() == KeyState::Pressed,
					}),
				);
				self.wake_by_input();
				let Some(keyboard) = self.seat.get_keyboard() else {
					return;
				};
				keyboard.input::<(), _>(
					self,
					event.key_code(),
					event.state(),
					SERIAL_COUNTER.next_serial(),
					event.time_msec(),
					|_, _, _| FilterResult::Forward,
				);
			}
			InputEvent::PointerMotion { event } => {
				self.wake_by_input();
				let Some(pointer) = self.seat.get_pointer() else {
					return;
				};
				let pos =
					self.clamp(pointer.current_location() + event.delta());
				let under = self.surface_under(pos);
				pointer.motion(
					self,
					under.clone(),
					&MotionEvent {
						location: pos,
						serial: SERIAL_COUNTER.next_serial(),
						time: event.time_msec(),
					},
				);
				pointer.relative_motion(
					self,
					under,
					&RelativeMotionEvent {
						delta: event.delta(),
						delta_unaccel: event.delta_unaccel(),
						utime: event.time(),
					},
				);
				pointer.frame(self);
			}
			InputEvent::PointerMotionAbsolute { event } => {
				self.wake_by_input();
				let pos =
					self.output_pos(|size| event.position_transformed(size));
				let (Some(pointer), Some(pos)) = (self.seat.get_pointer(), pos)
				else {
					return;
				};
				let under = self.surface_under(pos);
				pointer.motion(
					self,
					under,
					&MotionEvent {
						location: pos,
						serial: SERIAL_COUNTER.next_serial(),
						time: event.time_msec(),
					},
				);
				pointer.frame(self);
			}
			InputEvent::PointerButton { event } => {
				self.debug.event(
					"pointer_button",
					json!({
						"button": event.button_code(),
						"pressed": event.state() == ButtonState::Pressed,
					}),
				);
				// like a touch, a click on a dark display only wakes it
				let button = event.button_code();
				if event.state() == ButtonState::Pressed && !self.awake {
					self.wake_by_input();
					self.swallowed_buttons.push(button);
					return;
				}
				if event.state() == ButtonState::Released {
					let swallowed = self
						.swallowed_buttons
						.iter()
						.position(|b| *b == button);
					if let Some(i) = swallowed {
						self.swallowed_buttons.swap_remove(i);
						return;
					}
				}
				let Some(pointer) = self.seat.get_pointer() else {
					return;
				};
				pointer.button(
					self,
					&ButtonEvent {
						button: event.button_code(),
						state: event.state(),
						serial: SERIAL_COUNTER.next_serial(),
						time: event.time_msec(),
					},
				);
				pointer.frame(self);
			}
			InputEvent::PointerAxis { event } => {
				self.wake_by_input();
				let Some(pointer) = self.seat.get_pointer() else {
					return;
				};
				let mut frame =
					AxisFrame::new(event.time_msec()).source(event.source());
				for axis in [Axis::Horizontal, Axis::Vertical] {
					let v120 = event.amount_v120(axis);
					let amount = event
						.amount(axis)
						.unwrap_or_else(|| v120.unwrap_or(0.0) * 15.0 / 120.);
					if amount != 0.0 {
						frame = frame.value(axis, amount);
						if let Some(v120) = v120 {
							frame = frame.v120(axis, v120 as i32);
						}
					}
					if event.source() == AxisSource::Finger
						&& event.amount(axis) == Some(0.0)
					{
						frame = frame.stop(axis);
					}
				}
				pointer.axis(self, frame);
				pointer.frame(self);
			}
			InputEvent::TouchDown { event } => {
				let pos =
					self.output_pos(|size| event.position_transformed(size));
				self.touch_down(event.slot(), pos, event.time_msec());
			}
			InputEvent::TouchMotion { event } => {
				let pos =
					self.output_pos(|size| event.position_transformed(size));
				self.touch_motion(event.slot(), pos, event.time_msec());
			}
			InputEvent::TouchUp { event } => {
				self.touch_up(event.slot(), event.time_msec());
			}
			InputEvent::TouchFrame { .. } => self.touch_frame(),
			InputEvent::TouchCancel { .. } => self.touch_device_cancel(),
			_ => {}
		}
	}

	/// A position from the touchscreen or a tablet in output coordinates.
	fn output_pos(
		&self,
		transformed: impl FnOnce(Size<i32, Logical>) -> Point<f64, Logical>,
	) -> Option<Point<f64, Logical>> {
		let geo = self.output_size()?;
		Some(transformed(geo.size) + geo.loc.to_f64())
	}

	fn clamp(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
		let Some(geo) = self.output_size() else {
			return pos;
		};
		let max_x = (geo.loc.x + geo.size.w - 1) as f64;
		let max_y = (geo.loc.y + geo.size.h - 1) as f64;
		(
			pos.x.clamp(geo.loc.x as f64, max_x),
			pos.y.clamp(geo.loc.y as f64, max_y),
		)
			.into()
	}
}
