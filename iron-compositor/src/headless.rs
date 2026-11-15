//! Renders offscreen on a render node without any display, for testing the
//! compositor on a machine without a screen (screenshots still work).

use crate::render;
use crate::state::{create_output, refresh_duration, Backend, State};

use std::fs::OpenOptions;
use std::os::fd::OwnedFd;
use std::time::Instant;

use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::DrmDeviceFd;
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::Offscreen;
use smithay::output::{Mode, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{DeviceFd, Transform};
use tracing::{info, warn};

const RENDER_NODE: &str = "/dev/dri/renderD128";
const REFRESH: u32 = 60_000;

pub struct HeadlessBackend {
	pub renderer: GlesRenderer,
	size: (i32, i32),
	frame_pending: bool,
	target: Option<(GlesTexture, OutputDamageTracker)>,
}

pub fn init(size: (i32, i32)) -> Result<HeadlessBackend, String> {
	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.open(RENDER_NODE)
		.map_err(|e| format!("could not open {RENDER_NODE}: {e}"))?;
	let fd = DrmDeviceFd::new(DeviceFd::from(OwnedFd::from(file)));
	let gbm = GbmDevice::new(fd).map_err(|e| format!("gbm {e}"))?;
	let display =
		unsafe { EGLDisplay::new(gbm) }.map_err(|e| format!("egl {e}"))?;
	let context =
		EGLContext::new(&display).map_err(|e| format!("egl context {e}"))?;
	let renderer = unsafe { GlesRenderer::new(context) }
		.map_err(|e| format!("gles {e}"))?;

	info!("headless {}x{} on {RENDER_NODE}", size.0, size.1);

	Ok(HeadlessBackend {
		renderer,
		size,
		frame_pending: false,
		target: None,
	})
}

pub fn start(state: &mut State) {
	let Backend::Headless(backend) = &state.backend else {
		return;
	};

	let properties = PhysicalProperties {
		size: (0, 0).into(),
		subpixel: Subpixel::Unknown,
		make: "Iron OS".into(),
		model: "headless".into(),
	};
	let mode = Mode {
		size: backend.size.into(),
		refresh: REFRESH as i32,
	};
	// the output lives as long as the compositor, the global too
	let (output, _global) = create_output(
		&state.display_handle,
		"HEADLESS-1".into(),
		properties,
		mode,
	);

	state.set_output(Some(output));
}

pub fn schedule_render(state: &mut State) {
	let Backend::Headless(backend) = &mut state.backend else {
		return;
	};
	if backend.frame_pending || !state.awake {
		return;
	}
	backend.frame_pending = true;

	let refresh = refresh_duration(REFRESH as i32);
	let res = state.loop_handle.insert_source(
		Timer::from_duration(refresh),
		|_, _, state| {
			render(state);
			TimeoutAction::Drop
		},
	);
	if let Err(e) = res {
		warn!("could not schedule frame {e}");
	}
}

fn render(state: &mut State) {
	let Some(output) = state.output.clone() else {
		return;
	};
	let Backend::Headless(backend) = &mut state.backend else {
		return;
	};
	backend.frame_pending = false;

	let renderer = &mut backend.renderer;
	if backend.target.is_none() {
		let size = (backend.size.0, backend.size.1).into();
		match renderer.create_buffer(Fourcc::Xrgb8888, size) {
			Ok(t) => {
				let damage_tracker = OutputDamageTracker::new(
					backend.size,
					1.0,
					Transform::Normal,
				);
				backend.target = Some((t, damage_tracker));
			}
			Err(e) => {
				warn!("create buffer {e}");
				return;
			}
		}
	}
	let Some((texture, damage_tracker)) = &mut backend.target else {
		return;
	};

	let start = Instant::now();
	let elements =
		render::elements(renderer, &state.space, &output, &mut state.debug);
	let res =
		render::draw_offscreen(renderer, texture, damage_tracker, 1, &elements);
	if let Err(e) = &res {
		warn!("{e}");
	}

	state
		.debug
		.frame(start.elapsed(), res.is_ok(), res.is_err());
	state.send_frames();
	if state.debug.animating() {
		schedule_render(state);
	}
}
