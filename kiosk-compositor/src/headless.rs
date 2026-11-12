//! Renders offscreen on a render node without any display, for testing the
//! compositor on a machine without a screen (screenshots still work).

use crate::render;
use crate::state::{Backend, State};

use std::fs::OpenOptions;
use std::time::Duration;

use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::DrmDeviceFd;
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, Offscreen};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{DeviceFd, Transform};
use tracing::{info, warn};

const RENDER_NODE: &str = "/dev/dri/renderD128";
const REFRESH: u32 = 60_000;

pub struct HeadlessBackend {
	pub renderer: GlesRenderer,
	size: (i32, i32),
	frame_pending: bool,
	damage_tracker: Option<OutputDamageTracker>,
	target: Option<GlesTexture>,
}

pub fn init(
	_dh: &DisplayHandle,
	size: (i32, i32),
) -> Result<HeadlessBackend, String> {
	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.open(RENDER_NODE)
		.map_err(|e| format!("could not open {RENDER_NODE}: {e}"))?;
	let fd = DrmDeviceFd::new(DeviceFd::from(std::os::fd::OwnedFd::from(file)));
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
		damage_tracker: None,
		target: None,
	})
}

pub fn start(state: &mut State) {
	let Backend::Headless(backend) = &state.backend else {
		return;
	};
	let size = backend.size;

	let output = Output::new(
		"HEADLESS-1".into(),
		PhysicalProperties {
			size: (0, 0).into(),
			subpixel: Subpixel::Unknown,
			make: "Iron OS".into(),
			model: "headless".into(),
		},
	);
	let _global = output.create_global::<State>(&state.display_handle);
	let mode = Mode {
		size: size.into(),
		refresh: REFRESH as i32,
	};
	output.change_current_state(
		Some(mode),
		Some(Transform::Normal),
		None,
		Some((0, 0).into()),
	);
	output.set_preferred(mode);

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

	let res = state.loop_handle.insert_source(
		Timer::from_duration(Duration::from_micros(
			1_000_000_000 / REFRESH as u64,
		)),
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

	let size = backend.size;
	let renderer = &mut backend.renderer;
	if backend.target.is_none() {
		let buffer_size = (size.0, size.1).into();
		match renderer.create_buffer(Fourcc::Xrgb8888, buffer_size) {
			Ok(t) => backend.target = Some(t),
			Err(e) => {
				warn!("create buffer {e}");
				return;
			}
		}
		backend.damage_tracker =
			Some(OutputDamageTracker::from_output(&output));
	}

	let elements = render::elements(renderer, &state.space, &output);
	let target = backend.target.as_mut().unwrap();
	let damage_tracker = backend.damage_tracker.as_mut().unwrap();
	match renderer.bind(target) {
		Ok(mut fb) => {
			let res = damage_tracker.render_output(
				renderer,
				&mut fb,
				1,
				&elements,
				render::CLEAR_COLOR,
			);
			if let Err(e) = res {
				warn!("render {e:?}");
			}
		}
		Err(e) => warn!("bind {e}"),
	}

	state.send_frames();
}
