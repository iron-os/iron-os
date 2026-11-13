use crate::capture;
use crate::debug::Debug;
use crate::handlers::ClientState;
use crate::kiosk::KioskState;
use crate::touch::Touches;
use crate::touch_stats::TouchStats;
use crate::{drm, headless};

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{ImportDma, ImportMemWl};
use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType};
use smithay::input::keyboard::XkbConfig;
use smithay::input::{Seat, SeatState};
use smithay::output::{Mode as OutputMode, Output, PhysicalProperties};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point, Rectangle, Transform, SERIAL_COUNTER};
use smithay::wayland::compositor::{get_parent, CompositorState};
use smithay::wayland::dmabuf::DmabufState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::viewporter::ViewporterState;
use tracing::{info, warn};

pub enum Backend {
	Drm(Box<drm::DrmBackend>),
	Headless(Box<headless::HeadlessBackend>),
}

impl Backend {
	pub fn renderer(&mut self) -> &mut GlesRenderer {
		match self {
			Self::Drm(b) => &mut b.renderer,
			Self::Headless(b) => &mut b.renderer,
		}
	}
}

/// Creates an output with one mode at 0,0 and its global.
pub fn create_output(
	dh: &DisplayHandle,
	name: String,
	properties: PhysicalProperties,
	mode: OutputMode,
) -> (Output, GlobalId) {
	let output = Output::new(name, properties);
	output.change_current_state(
		Some(mode),
		Some(Transform::Normal),
		None,
		Some((0, 0).into()),
	);
	output.set_preferred(mode);
	let global = output.create_global::<State>(dh);
	(output, global)
}

/// The surface at the root of a subsurface tree.
pub fn root_surface(surface: &WlSurface) -> WlSurface {
	let mut root = surface.clone();
	while let Some(parent) = get_parent(&root) {
		root = parent;
	}
	root
}

/// Time between two frames at this refresh rate in mHz.
pub fn refresh_duration(refresh: i32) -> Duration {
	if refresh > 0 {
		Duration::from_micros(1_000_000_000 / refresh as u64)
	} else {
		Duration::from_millis(16)
	}
}

/// Removes an output global, delayed so a client which binds it at the same
/// time doesn't get killed.
pub fn remove_output_global(state: &State, global: GlobalId) {
	state.display_handle.disable_global::<State>(global.clone());
	let res = state.loop_handle.insert_source(
		Timer::from_duration(Duration::from_secs(5)),
		move |_, _, state| {
			state.display_handle.remove_global::<State>(global.clone());
			TimeoutAction::Drop
		},
	);
	if let Err(e) = res {
		warn!("could not remove the output global {e}");
	}
}

pub struct State {
	pub display_handle: DisplayHandle,
	pub loop_handle: LoopHandle<'static, State>,
	pub start_time: Instant,
	pub backend: Backend,

	pub space: Space<Window>,
	pub popups: PopupManager,
	pub output: Option<Output>,

	pub compositor_state: CompositorState,
	pub xdg_shell_state: XdgShellState,
	pub shm_state: ShmState,
	pub seat_state: SeatState<State>,
	pub data_device_state: DataDeviceState,
	pub dmabuf_state: DmabufState,
	pub kiosk: KioskState,

	pub seat: Seat<State>,
	/// physical pointer devices (mice), without them we hide the pointer
	/// capability so chromium behaves like on a touch device
	pub pointer_devices: usize,

	/// if the display is on
	pub awake: bool,
	pub touches: Touches,
	/// mouse buttons pressed on a dark display, their release is not
	/// forwarded either
	pub swallowed_buttons: Vec<u32>,

	pub debug: Debug,
	pub touch_stats: TouchStats,
}

impl State {
	pub fn new(
		display: Display<State>,
		loop_handle: &LoopHandle<'static, State>,
		mut backend: Backend,
		socket_name: &str,
	) -> Result<Self, String> {
		let dh = display.handle();

		let compositor_state = CompositorState::new::<Self>(&dh);
		let xdg_shell_state = XdgShellState::new::<Self>(&dh);
		let shm_formats: Vec<_> = backend.renderer().shm_formats().collect();
		let shm_state = ShmState::new::<Self>(&dh, shm_formats);
		// these only create globals, their state is not needed
		OutputManagerState::new_with_xdg_output::<Self>(&dh);
		ViewporterState::new::<Self>(&dh);
		capture::init(&dh);
		let mut seat_state = SeatState::new();
		let data_device_state = DataDeviceState::new::<Self>(&dh);
		let kiosk = KioskState::new(&dh);

		let mut dmabuf_state = DmabufState::new();
		let formats = backend.renderer().dmabuf_formats();
		dmabuf_state.create_global::<Self>(&dh, formats);

		let mut seat = seat_state.new_wl_seat(&dh, "seat0");
		seat.add_keyboard(XkbConfig::default(), 200, 25)
			.map_err(|e| format!("keyboard {e}"))?;
		seat.add_touch();

		listen(display, loop_handle, socket_name)?;

		Ok(Self {
			display_handle: dh,
			loop_handle: loop_handle.clone(),
			start_time: Instant::now(),
			backend,
			space: Space::default(),
			popups: PopupManager::default(),
			output: None,
			compositor_state,
			xdg_shell_state,
			shm_state,
			seat_state,
			data_device_state,
			dmabuf_state,
			kiosk,
			seat,
			pointer_devices: 0,
			awake: true,
			touches: Touches::default(),
			swallowed_buttons: vec![],
			debug: Debug::new(),
			touch_stats: TouchStats::new(),
		})
	}

	pub fn output_size(&self) -> Option<Rectangle<i32, Logical>> {
		let output = self.output.as_ref()?;
		self.space.output_geometry(output)
	}

	/// Sets (or replaces) the single output and makes every window fill it.
	pub fn set_output(&mut self, output: Option<Output>) {
		if let Some(old) = self.output.take() {
			self.space.unmap_output(&old);
		}

		if let Some(output) = &output {
			self.space.map_output(output, (0, 0));
		}
		let mode = output.as_ref().and_then(|o| o.current_mode());
		self.debug.event(
			"output",
			json!({
				"name": output.as_ref().map(|o| o.name()),
				"w": mode.map(|m| m.size.w),
				"h": mode.map(|m| m.size.h),
				"refresh": mode.map(|m| m.refresh),
			}),
		);
		self.output = output;

		let windows: Vec<_> = self.space.elements().cloned().collect();
		for window in windows {
			self.configure_fullscreen(&window);
		}
	}

	pub fn configure_fullscreen(&self, window: &Window) {
		let Some(toplevel) = window.toplevel() else {
			return;
		};
		let size = self.output_size().map(|g| g.size);

		toplevel.with_pending_state(|state| {
			state.states.set(xdg_toplevel::State::Fullscreen);
			state.states.set(xdg_toplevel::State::Activated);
			state.size = size;
		});

		if toplevel.is_initial_configure_sent() {
			toplevel.send_pending_configure();
		}
	}

	/// The window whose toplevel is this surface.
	pub fn window_for(&self, surface: &WlSurface) -> Option<&Window> {
		self.space
			.elements()
			.find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(surface))
	}

	pub fn surface_under(
		&self,
		pos: Point<f64, Logical>,
	) -> Option<(WlSurface, Point<f64, Logical>)> {
		self.space
			.element_under(pos)
			.and_then(|(window, location)| {
				window
					.surface_under(
						pos - location.to_f64(),
						WindowSurfaceType::ALL,
					)
					.map(|(s, p)| (s, (p + location).to_f64()))
			})
	}

	/// Focus the window on top for keyboard input.
	pub fn focus_top(&mut self) {
		let surface = self
			.space
			.elements()
			.last()
			.and_then(|w| w.toplevel())
			.map(|t| t.wl_surface().clone());

		if let Some(keyboard) = self.seat.get_keyboard() {
			keyboard.set_focus(self, surface, SERIAL_COUNTER.next_serial());
		}
	}

	/// Something changed, a new frame should be drawn.
	pub fn schedule_render(&mut self) {
		match &self.backend {
			Backend::Drm(_) => drm::schedule_render(self),
			Backend::Headless(_) => headless::schedule_render(self),
		}
	}

	/// Sets up the output once everything is ready.
	pub fn start_backend(&mut self) {
		match &self.backend {
			Backend::Drm(_) => drm::start(self),
			Backend::Headless(_) => headless::start(self),
		}
	}

	/// Turns the display on or off, returns true if the state changed.
	pub fn set_awake(&mut self, awake: bool) -> bool {
		if self.awake == awake {
			return false;
		}

		info!("display {}", if awake { "on" } else { "off" });
		self.awake = awake;
		self.debug.event("display", json!({ "on": awake }));

		// fingers still down (or stuck in the touch controller) must not
		// carry over to the next wake up
		if !awake {
			self.cancel_touches();
		}

		match &self.backend {
			Backend::Drm(_) => drm::set_awake(self, awake),
			Backend::Headless(_) => {}
		}

		if awake {
			self.schedule_render();
		}

		true
	}

	/// The display was woken up by input.
	pub fn wake_by_input(&mut self) {
		if self.set_awake(true) {
			self.kiosk.send_state_change(true);
		}
	}

	/// Send frame callbacks to all surfaces on the output.
	pub fn send_frames(&self) {
		let Some(output) = &self.output else {
			return;
		};

		let time = self.start_time.elapsed();
		for window in self.space.elements() {
			window.send_frame(output, time, Some(Duration::ZERO), |_, _| {
				Some(output.clone())
			});
		}
	}
}

fn listen(
	display: Display<State>,
	loop_handle: &LoopHandle<'static, State>,
	socket_name: &str,
) -> Result<(), String> {
	let socket = ListeningSocketSource::with_name(socket_name)
		.map_err(|e| format!("could not create socket {socket_name}: {e}"))?;

	loop_handle
		.insert_source(socket, |stream, _, state| {
			let res = state
				.display_handle
				.insert_client(stream, Arc::new(ClientState::default()));
			if let Err(e) = res {
				warn!("could not insert client {e}");
			}
		})
		.map_err(|e| format!("socket source {e}"))?;

	loop_handle
		.insert_source(
			Generic::new(display, Interest::READ, Mode::Level),
			|_, display, state| {
				// Safety: we don't drop the display
				let res = unsafe { display.get_mut().dispatch_clients(state) };
				if let Err(e) = res {
					warn!("dispatch clients failed {e}");
				}
				Ok(PostAction::Continue)
			},
		)
		.map_err(|e| format!("display source {e}"))?;

	Ok(())
}
