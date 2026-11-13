//! The weston_capture_v1 protocol, used by weston-screenshooter.
//!
//! Captures always render the current window offscreen, so they also work
//! while the display is off.

use crate::render;
use crate::state::State;

use serde_json::json;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::{
	Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
	WEnum,
};
use smithay::wayland::shm::with_buffer_contents_mut;
use tracing::{debug, warn};

pub mod protocol {
	#![allow(dead_code, non_camel_case_types, unused_unsafe)]
	#![allow(unused_variables, non_upper_case_globals, non_snake_case)]
	#![allow(unused_imports, missing_docs, clippy::all)]

	use wayland_server;
	use wayland_server::protocol::*;

	pub mod __interfaces {
		use wayland_server::protocol::__interfaces::*;
		wayland_scanner::generate_interfaces!(
			"protocols/weston-output-capture.xml"
		);
	}
	use self::__interfaces::*;

	wayland_scanner::generate_server_code!(
		"protocols/weston-output-capture.xml"
	);
}

use protocol::weston_capture_source_v1::{self, WestonCaptureSourceV1};
use protocol::weston_capture_v1::{self, WestonCaptureV1};

/// DRM_FORMAT_XRGB8888
const FORMAT_XRGB8888: u32 = 0x34325258;

pub fn init(dh: &DisplayHandle) {
	dh.create_global::<State, WestonCaptureV1, ()>(2, ());
}

impl GlobalDispatch<WestonCaptureV1, ()> for State {
	fn bind(
		_state: &mut Self,
		_dh: &DisplayHandle,
		_client: &Client,
		resource: New<WestonCaptureV1>,
		_data: &(),
		data_init: &mut DataInit<'_, Self>,
	) {
		data_init.init(resource, ());
	}
}

impl Dispatch<WestonCaptureV1, ()> for State {
	fn request(
		state: &mut Self,
		_client: &Client,
		resource: &WestonCaptureV1,
		request: weston_capture_v1::Request,
		_data: &(),
		_dh: &DisplayHandle,
		data_init: &mut DataInit<'_, Self>,
	) {
		match request {
			weston_capture_v1::Request::Create {
				source,
				capture_source_new_id,
				..
			} => {
				if !matches!(source, WEnum::Value(_)) {
					resource.post_error(
						weston_capture_v1::Error::InvalidSource,
						"invalid source",
					);
					return;
				}

				let src = data_init.init(capture_source_new_id, ());

				// we only have one output, so we ignore which one was asked
				let Some(mode) =
					state.output.as_ref().and_then(|o| o.current_mode())
				else {
					// the client waits for a size, without one it would
					// wait forever
					src.failed(Some("output unavailable".into()));
					return;
				};

				src.format(FORMAT_XRGB8888);
				if src.version() >= 2 {
					src.formats_done();
				}
				src.size(mode.size.w, mode.size.h);
			}
			weston_capture_v1::Request::Destroy => {}
		}
	}
}

impl Dispatch<WestonCaptureSourceV1, ()> for State {
	fn request(
		state: &mut Self,
		_client: &Client,
		resource: &WestonCaptureSourceV1,
		request: weston_capture_source_v1::Request,
		_data: &(),
		_dh: &DisplayHandle,
		_data_init: &mut DataInit<'_, Self>,
	) {
		match request {
			weston_capture_source_v1::Request::Capture { buffer } => {
				capture(state, resource, &buffer);
			}
			weston_capture_source_v1::Request::Destroy => {}
		}
	}
}

fn capture(state: &mut State, src: &WestonCaptureSourceV1, buffer: &WlBuffer) {
	let Some(output) = state.output.clone() else {
		src.failed(Some("output unavailable".into()));
		return;
	};

	let renderer = state.backend.renderer();
	let (size, pixels) = match render::render_to_memory(
		renderer,
		&state.space,
		&output,
		&mut state.debug,
	) {
		Ok(r) => r,
		Err(e) => {
			warn!("capture failed {e}");
			src.failed(Some(e));
			return;
		}
	};

	let res = with_buffer_contents_mut(buffer, |ptr, len, data| {
		let format_ok = matches!(
			data.format,
			wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888
		);
		if !format_ok || data.width != size.w || data.height != size.h {
			return false;
		}

		let row = size.w as usize * 4;
		let stride = data.stride as usize;
		let offset = data.offset as usize;
		if stride < row || offset + stride * size.h as usize > len {
			return false;
		}

		for y in 0..size.h as usize {
			// safety: the bounds are checked above, the client may
			// change the memory concurrently but that only garbles
			// its own image
			unsafe {
				std::ptr::copy_nonoverlapping(
					pixels.as_ptr().add(y * row),
					ptr.add(offset + y * stride),
					row,
				);
			}
		}

		true
	});

	match res {
		Ok(true) => {
			debug!("captured {}x{}", size.w, size.h);
			state
				.debug
				.event("capture", json!({ "w": size.w, "h": size.h }));
			src.complete();
		}
		Ok(false) => {
			if let Some(mode) = output.current_mode() {
				src.size(mode.size.w, mode.size.h);
			}
			src.retry();
		}
		Err(_) => {
			src.post_error(
				weston_capture_source_v1::Error::BadBuffer,
				"the buffer is not a shm buffer",
			);
		}
	}
}
