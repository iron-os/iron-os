//! Shows the output on a real display with drm/kms, input comes from
//! libinput and devices are opened through the logind session.
//!
//! Rendering never panics: any failure is logged and the frame is retried a
//! refresh later, a kiosk must keep running.

use crate::render;
use crate::state::{
	create_output, refresh_duration, remove_output_global, Backend, State,
};
use crate::stats::LimitedWarn;

use std::time::{Duration, Instant};

use smithay::backend::allocator::gbm::{
	GbmAllocator, GbmBufferFlags, GbmDevice,
};
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::compositor::{DrmCompositor, FrameFlags};
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::{
	DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType,
};
use smithay::backend::egl::{EGLContext, EGLDevice, EGLDisplay};
use smithay::backend::libinput::{
	LibinputInputBackend, LibinputSessionInterface,
};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::drm::control::{connector, crtc, Device as _};
use smithay::reexports::input::Libinput;
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::utils::DeviceFd;
use tracing::{error, info, warn};

type GbmDrmCompositor = DrmCompositor<
	GbmAllocator<DrmDeviceFd>,
	GbmFramebufferExporter<DrmDeviceFd>,
	(),
	DrmDeviceFd,
>;

/// if a page flip did not complete after this time we stop waiting for it
const FLIP_TIMEOUT: Duration = Duration::from_secs(1);

pub struct DrmBackend {
	pub renderer: GlesRenderer,
	// keeps the devices open
	#[allow(dead_code)]
	session: LibSeatSession,
	libinput: Libinput,
	node: DrmNode,
	render_node: DrmNode,
	device: DrmDevice,
	gbm: GbmDevice<DrmDeviceFd>,
	surface: Option<Surface>,
	/// false while our session is not in the foreground
	active: bool,
	render_warn: LimitedWarn,
	drm_warn: LimitedWarn,
}

struct Surface {
	connector: connector::Handle,
	crtc: crtc::Handle,
	compositor: GbmDrmCompositor,
	global: GlobalId,
	refresh: Duration,
	/// a page flip was queued and its vblank did not arrive yet
	flip_pending: Option<Instant>,
	/// a timer checks that the pending flip completes
	flip_timer: bool,
	/// flips lost in a row
	flip_timeouts: u32,
	/// waiting a refresh after a frame without damage or an error
	timer_pending: bool,
	render_queued: bool,
	redraw: bool,
}

pub fn init(
	loop_handle: &LoopHandle<'static, State>,
) -> Result<DrmBackend, String> {
	let (mut session, notifier) = LibSeatSession::new()
		.map_err(|e| format!("could not create a session: {e}"))?;
	let seat = session.seat();
	info!("session on seat {seat}");

	let path = primary_gpu(&seat)
		.ok()
		.flatten()
		.or_else(|| all_gpus(&seat).ok()?.into_iter().next())
		.ok_or("no gpu found")?;
	info!("using gpu {}", path.display());

	let fd = session
		.open(
			&path,
			OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
		)
		.map_err(|e| format!("could not open {}: {e}", path.display()))?;
	let fd = DrmDeviceFd::new(DeviceFd::from(fd));
	let node = DrmNode::from_file(&fd).map_err(|e| format!("drm node {e}"))?;

	let (device, drm_notifier) =
		DrmDevice::new(fd.clone(), true).map_err(|e| format!("drm {e}"))?;
	let gbm = GbmDevice::new(fd).map_err(|e| format!("gbm {e}"))?;

	let display = unsafe { EGLDisplay::new(gbm.clone()) }
		.map_err(|e| format!("egl {e}"))?;
	let render_node = EGLDevice::device_for_display(&display)
		.ok()
		.and_then(|d| d.try_get_render_node().ok().flatten())
		.or_else(|| node.node_with_type(NodeType::Render)?.ok())
		.unwrap_or(node);
	let context =
		EGLContext::new(&display).map_err(|e| format!("egl context {e}"))?;
	let renderer = unsafe { GlesRenderer::new(context) }
		.map_err(|e| format!("gles {e}"))?;

	loop_handle
		.insert_source(drm_notifier, |event, _meta, state| match event {
			DrmEvent::VBlank(crtc) => vblank(state, crtc),
			DrmEvent::Error(e) => {
				if let Some(b) = backend(state) {
					b.drm_warn.warn(format!("drm error {e}"));
				}
			}
		})
		.map_err(|e| format!("drm source {e}"))?;

	let mut libinput = Libinput::new_with_udev::<
		LibinputSessionInterface<LibSeatSession>,
	>(session.clone().into());
	libinput
		.udev_assign_seat(&seat)
		.map_err(|_| "libinput could not assign seat")?;
	loop_handle
		.insert_source(
			LibinputInputBackend::new(libinput.clone()),
			|event, _, state| state.process_input_event(event),
		)
		.map_err(|e| format!("libinput source {e}"))?;

	loop_handle
		.insert_source(notifier, |event, _, state| session_event(state, event))
		.map_err(|e| format!("session source {e}"))?;

	let udev = UdevBackend::new(&seat).map_err(|e| format!("udev {e}"))?;
	loop_handle
		.insert_source(udev, |event, _, state| {
			if let UdevEvent::Changed { device_id } = event {
				let ours = match &state.backend {
					Backend::Drm(b) => b.node.dev_id() == device_id,
					_ => false,
				};
				if ours {
					info!("drm device changed, scanning connectors");
					scan_connectors(state);
				}
			}
		})
		.map_err(|e| format!("udev source {e}"))?;

	Ok(DrmBackend {
		renderer,
		session,
		libinput,
		node,
		render_node,
		device,
		gbm,
		surface: None,
		active: true,
		render_warn: LimitedWarn::default(),
		drm_warn: LimitedWarn::default(),
	})
}

pub fn start(state: &mut State) {
	scan_connectors(state);
}

fn backend(state: &mut State) -> Option<&mut DrmBackend> {
	match &mut state.backend {
		Backend::Drm(b) => Some(b),
		_ => None,
	}
}

fn surface(state: &mut State) -> Option<&mut Surface> {
	backend(state)?.surface.as_mut()
}

/// Sets up the first connected display or removes ours if it was unplugged.
fn scan_connectors(state: &mut State) {
	let dh = state.display_handle.clone();
	let Some(b) = backend(state) else {
		return;
	};

	if let Some(s) = &b.surface {
		if is_connected(&b.device, s.connector) {
			return;
		}

		info!("display disconnected");
		if let Some(s) = b.surface.take() {
			remove_output_global(state, s.global);
		}
		state.set_output(None);
	}

	// use another display which is still connected
	let Some(b) = backend(state) else {
		return;
	};
	let Some((conn, crtc)) = find_connector(&b.device) else {
		info!("no display connected");
		return;
	};

	match create_surface(b, &dh, conn, crtc) {
		Ok((surface, output)) => {
			b.surface = Some(surface);
			state.set_output(Some(output));
			if !state.awake {
				set_awake(state, false);
			}
			state.schedule_render();
		}
		Err(e) => error!("could not set up the display {e}"),
	}
}

fn is_connected(device: &DrmDevice, conn: connector::Handle) -> bool {
	device
		.get_connector(conn, false)
		.map(|i| i.state() == connector::State::Connected)
		.unwrap_or(false)
}

fn find_connector(
	device: &DrmDevice,
) -> Option<(connector::Info, crtc::Handle)> {
	let res = device.resource_handles().ok()?;

	for conn in res.connectors() {
		let Ok(info) = device.get_connector(*conn, true) else {
			continue;
		};
		if info.state() != connector::State::Connected
			|| info.modes().is_empty()
		{
			continue;
		}

		let crtc = info
			.encoders()
			.iter()
			.filter_map(|enc| device.get_encoder(*enc).ok())
			.flat_map(|enc| res.filter_crtcs(enc.possible_crtcs()))
			.next();
		if let Some(crtc) = crtc {
			return Some((info, crtc));
		}
	}

	None
}

fn create_surface(
	b: &mut DrmBackend,
	dh: &smithay::reexports::wayland_server::DisplayHandle,
	conn: connector::Info,
	crtc: crtc::Handle,
) -> Result<(Surface, Output), String> {
	let drm_mode = conn
		.modes()
		.iter()
		.find(|m| {
			m.mode_type().contains(
				smithay::reexports::drm::control::ModeTypeFlags::PREFERRED,
			)
		})
		.copied()
		.unwrap_or(conn.modes()[0]);
	let mode = Mode::from(drm_mode);

	let name = format!("{}-{}", conn.interface().as_str(), conn.interface_id());
	info!(
		"display {name} {}x{}@{}",
		mode.size.w,
		mode.size.h,
		mode.refresh as f32 / 1000.0
	);

	let drm_surface = b
		.device
		.create_surface(crtc, drm_mode, &[conn.handle()])
		.map_err(|e| format!("drm surface {e}"))?;

	let (phys_w, phys_h) = conn.size().unwrap_or((0, 0));
	let properties = PhysicalProperties {
		size: (phys_w as i32, phys_h as i32).into(),
		subpixel: Subpixel::Unknown,
		make: "Unknown".into(),
		model: "Unknown".into(),
	};
	let (output, global) = create_output(dh, name, properties, mode);

	let allocator = GbmAllocator::new(
		b.gbm.clone(),
		GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
	);
	let exporter =
		GbmFramebufferExporter::new(b.gbm.clone(), Some(b.render_node));
	let render_formats =
		b.renderer.egl_context().dmabuf_render_formats().clone();

	let compositor = DrmCompositor::new(
		&output,
		drm_surface,
		None,
		allocator,
		exporter,
		[Fourcc::Xrgb8888, Fourcc::Argb8888],
		render_formats,
		b.device.cursor_size(),
		Some(b.gbm.clone()),
	)
	.map_err(|e| {
		dh.remove_global::<State>(global.clone());
		format!("drm compositor {e}")
	})?;

	let refresh = refresh_duration(mode.refresh);

	let surface = Surface {
		connector: conn.handle(),
		crtc,
		compositor,
		global,
		refresh,
		flip_pending: None,
		flip_timer: false,
		flip_timeouts: 0,
		timer_pending: false,
		render_queued: false,
		redraw: true,
	};

	Ok((surface, output))
}

pub fn schedule_render(state: &mut State) {
	let Some(s) = surface(state) else {
		return;
	};
	s.redraw = true;
	if s.render_queued {
		return;
	}
	s.render_queued = true;

	state.loop_handle.insert_idle(render);
}

fn render(state: &mut State) {
	let awake = state.awake;
	let Some(output) = state.output.clone() else {
		return;
	};
	let Backend::Drm(b) = &mut state.backend else {
		return;
	};
	let Some(s) = &mut b.surface else {
		return;
	};
	s.render_queued = false;

	if !awake || !b.active || s.timer_pending {
		return;
	}

	if let Some(since) = s.flip_pending {
		// the vblank renders again, if it never comes this timer does
		if !s.flip_timer {
			s.flip_timer = true;
			let wait = FLIP_TIMEOUT.saturating_sub(since.elapsed());
			let res = state.loop_handle.insert_source(
				Timer::from_duration(wait),
				|_, _, state| {
					flip_timeout(state);
					TimeoutAction::Drop
				},
			);
			if let Err(e) = res {
				error!("could not insert flip timer {e}");
			}
		}
		return;
	}

	s.redraw = false;
	// always draw complete frames: with one fullscreen window this costs
	// almost nothing and partial updates showed stale areas (virtio-gpu only
	// copies the damaged rects, panels with psr2 do the same)
	s.compositor.reset_buffer_ages();

	let start = Instant::now();
	let elements = render::elements(
		&mut b.renderer,
		&state.space,
		&output,
		&mut state.debug,
	);
	let res = s.compositor.render_frame(
		&mut b.renderer,
		&elements,
		render::CLEAR_COLOR,
		FrameFlags::empty(),
	);

	let (queued, failed) = match res {
		Ok(res) if !res.is_empty => match s.compositor.queue_frame(()) {
			Ok(()) => (true, false),
			Err(e) => {
				b.render_warn.warn(format!("queue frame failed {e}"));
				(false, true)
			}
		},
		// nothing changed
		Ok(_) => (false, false),
		Err(e) => {
			b.render_warn.warn(format!("render frame failed {e:?}"));
			(false, true)
		}
	};

	state.debug.frame(start.elapsed(), queued, failed);
	// the debug overlays change every frame
	if state.debug.animating() {
		s.redraw = true;
	}

	if queued {
		s.flip_pending = Some(Instant::now());
	} else {
		// nothing to show or it failed, wait a refresh before the next try
		// so clients asking for frames don't make us spin
		s.timer_pending = true;
		let refresh = s.refresh;
		let res = state.loop_handle.insert_source(
			Timer::from_duration(refresh),
			|_, _, state| {
				if let Some(s) = surface(state) {
					s.timer_pending = false;
				}
				frame_done(state);
				TimeoutAction::Drop
			},
		);
		if let Err(e) = res {
			error!("could not insert frame timer {e}");
		}
	}

	state.send_frames();
}

fn vblank(state: &mut State, crtc: crtc::Handle) {
	let Some(b) = backend(state) else {
		return;
	};
	let Some(s) = &mut b.surface else {
		return;
	};
	if s.crtc != crtc {
		return;
	}

	let took = s.flip_pending.take().map(|since| since.elapsed());
	s.flip_timeouts = 0;
	if let Err(e) = s.compositor.frame_submitted() {
		b.render_warn.warn(format!("frame submitted {e}"));
	}
	if let Some(took) = took {
		state.debug.flipped(took);
	}

	frame_done(state);
}

/// The vblank of a queued frame did not come. Without it smithay keeps the
/// frame as pending and never submits a new one, so the screen would freeze:
/// mark it as done ourselves.
fn flip_timeout(state: &mut State) {
	let Some(b) = backend(state) else {
		return;
	};
	let Some(s) = &mut b.surface else {
		return;
	};
	s.flip_timer = false;

	// while paused the resume resets everything
	if !b.active {
		return;
	}
	let Some(since) = s.flip_pending else {
		return;
	};
	if since.elapsed() < FLIP_TIMEOUT {
		// a new flip was queued after the timer was set
		frame_done(state);
		return;
	}

	b.render_warn.warn("page flip did not complete");
	s.flip_pending = None;
	s.flip_timeouts += 1;
	// if flips keep getting lost the kernel state is stuck, start over
	if s.compositor.frame_submitted().is_err() || s.flip_timeouts >= 3 {
		s.flip_timeouts = 0;
		if let Err(e) = s.compositor.reset_state() {
			warn!("reset state {e}");
		}
	}

	frame_done(state);
}

fn frame_done(state: &mut State) {
	let Some(s) = surface(state) else {
		return;
	};
	if s.redraw {
		schedule_render(state);
	}
}

pub fn set_awake(state: &mut State, awake: bool) {
	let Some(s) = surface(state) else {
		return;
	};

	if awake {
		// the next frame turns the crtc on again
		s.redraw = true;
	} else {
		// clear drops a pending frame, its vblank must not be waited for
		s.flip_pending = None;
		if let Err(e) = s.compositor.clear() {
			warn!("could not turn the display off {e}, retrying");
			// a display which stays lit would burn in, try again
			let res = state.loop_handle.insert_source(
				Timer::from_duration(Duration::from_secs(1)),
				|_, _, state| {
					if !state.awake {
						set_awake(state, false);
					}
					TimeoutAction::Drop
				},
			);
			if let Err(e) = res {
				error!("could not insert display off timer {e}");
			}
		}
	}
}

fn session_event(state: &mut State, event: SessionEvent) {
	let Some(b) = backend(state) else {
		return;
	};

	match event {
		SessionEvent::PauseSession => {
			info!("session paused");
			b.active = false;
			b.libinput.suspend();
			b.device.pause();
		}
		SessionEvent::ActivateSession => {
			info!("session resumed");
			if b.libinput.resume().is_err() {
				warn!("could not resume libinput");
			}
			if let Err(e) = b.device.activate(false) {
				error!("could not activate drm device {e}");
				return;
			}
			b.active = true;
			if let Some(s) = &mut b.surface {
				s.flip_pending = None;
				if let Err(e) = s.compositor.reset_state() {
					warn!("reset state {e}");
				}
			}
			// a display may have been plugged in or out while paused
			scan_connectors(state);
			if !state.awake {
				set_awake(state, false);
			}
			state.schedule_render();
		}
	}
}
