//! The weston_kiosk_shell protocol which the Iron OS service uses to turn the
//! display on and off, set the brightness and click.

use crate::backlight;
use crate::state::State;

use std::time::Duration;

use serde_json::json;
use smithay::backend::input::ButtonState;
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::{
	Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use tracing::{info, warn};

pub mod protocol {
	#![allow(dead_code, non_camel_case_types, unused_unsafe)]
	#![allow(unused_variables, non_upper_case_globals, non_snake_case)]
	#![allow(unused_imports, missing_docs, clippy::all)]

	use wayland_server;
	use wayland_server::protocol::*;

	pub mod __interfaces {
		use wayland_server::protocol::__interfaces::*;
		wayland_scanner::generate_interfaces!(
			"protocols/weston-kiosk-shell.xml"
		);
	}
	use self::__interfaces::*;

	wayland_scanner::generate_server_code!("protocols/weston-kiosk-shell.xml");
}

use protocol::weston_kiosk_shell::{self, WestonKioskShell};

const BTN_LEFT: u32 = 0x110;
/// time a client gets to bind the pointer if we had to create one
const POINTER_BIND_DELAY: Duration = Duration::from_millis(50);

pub struct KioskState {
	resources: Vec<WestonKioskShell>,
	click_pending: bool,
}

impl KioskState {
	pub fn new(dh: &DisplayHandle) -> Self {
		dh.create_global::<State, WestonKioskShell, ()>(1, ());

		Self {
			resources: vec![],
			click_pending: false,
		}
	}

	pub fn send_state_change(&mut self, awake: bool) {
		self.resources.retain(|r| r.is_alive());
		for resource in &self.resources {
			resource.state_change(awake as u32);
		}
	}
}

impl GlobalDispatch<WestonKioskShell, ()> for State {
	fn bind(
		state: &mut Self,
		_dh: &DisplayHandle,
		_client: &Client,
		resource: New<WestonKioskShell>,
		_data: &(),
		data_init: &mut DataInit<'_, Self>,
	) {
		info!("bind kiosk shell");
		let resource = data_init.init(resource, ());
		state.kiosk.resources.push(resource);
	}
}

impl Dispatch<WestonKioskShell, ()> for State {
	fn request(
		state: &mut Self,
		_client: &Client,
		resource: &WestonKioskShell,
		request: weston_kiosk_shell::Request,
		_data: &(),
		_dh: &DisplayHandle,
		_data_init: &mut DataInit<'_, Self>,
	) {
		match request {
			weston_kiosk_shell::Request::SetState { state: s } => {
				let awake = s != 0;
				state.debug.event("kiosk_set_state", json!({ "on": awake }));
				if state.set_awake(awake) {
					resource.state_change(awake as u32);
				}
			}
			weston_kiosk_shell::Request::SetBrightness { brightness } => {
				// same limits as weston, never fully dark
				let brightness = brightness.clamp(5, 255);
				state.debug.event(
					"kiosk_brightness",
					json!({ "brightness": brightness }),
				);
				info!("set brightness {brightness}");
				if let Err(e) = backlight::set(brightness) {
					warn!("could not set brightness {e}");
				}
			}
			weston_kiosk_shell::Request::MouseClick { x, y } => {
				state.debug.event("kiosk_click", json!({ "x": x, "y": y }));
				mouse_click(state, x, y);
			}
		}
	}
}

fn mouse_click(state: &mut State, x: f64, y: f64) {
	if state.kiosk.click_pending {
		return;
	}

	// weston wakes up on any input, so does a click
	state.wake_by_input();

	if state.seat.get_pointer().is_some() {
		execute_click(state, x, y);
		return;
	}

	// touch only devices have no pointer, create one for this click and give
	// the client some time to bind it
	state.seat.add_pointer();
	state.kiosk.click_pending = true;

	let res = state.loop_handle.insert_source(
		Timer::from_duration(POINTER_BIND_DELAY),
		move |_, _, state| {
			state.kiosk.click_pending = false;
			execute_click(state, x, y);
			if state.pointer_devices == 0 {
				state.seat.remove_pointer();
			}
			TimeoutAction::Drop
		},
	);
	if let Err(e) = res {
		warn!("could not schedule click {e}");
		state.kiosk.click_pending = false;
	}
}

fn execute_click(state: &mut State, x: f64, y: f64) {
	let (Some(pointer), Some(geo)) =
		(state.seat.get_pointer(), state.output_size())
	else {
		warn!("click: no pointer or output");
		return;
	};

	let pos: Point<f64, Logical> = (
		geo.loc.x as f64 + x.clamp(0.0, 1.0) * geo.size.w as f64,
		geo.loc.y as f64 + y.clamp(0.0, 1.0) * geo.size.h as f64,
	)
		.into();
	let time = state.start_time.elapsed().as_millis() as u32;

	let under = state.surface_under(pos);
	if under.is_none() {
		warn!("click: no surface at {pos:?}");
	}

	pointer.motion(
		state,
		under,
		&MotionEvent {
			location: pos,
			serial: SERIAL_COUNTER.next_serial(),
			time,
		},
	);
	for button_state in [ButtonState::Pressed, ButtonState::Released] {
		pointer.button(
			state,
			&ButtonEvent {
				button: BTN_LEFT,
				state: button_state,
				serial: SERIAL_COUNTER.next_serial(),
				time,
			},
		);
	}
	pointer.frame(state);
}
