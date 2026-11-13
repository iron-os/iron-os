use crate::capture;
use crate::debug::{self, Debug};
use crate::kiosk::KioskState;
use crate::touch_stats::TouchStats;
use crate::{drm, headless};

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use smithay::backend::input::TouchSlot;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::backend::renderer::{ImportDma, ImportMemWl};
use smithay::desktop::{
	find_popup_root_surface, get_popup_toplevel_coords, PopupKind,
	PopupManager, Space, Window, WindowSurfaceType,
};
use smithay::input::keyboard::XkbConfig;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode as OutputMode, Output, PhysicalProperties};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::backend::{
	ClientData, ClientId, DisconnectReason, GlobalId,
};
use smithay::reexports::wayland_server::protocol::{
	wl_buffer, wl_seat, wl_surface::WlSurface,
};
use smithay::reexports::wayland_server::{
	Client, Display, DisplayHandle, Resource,
};
use smithay::utils::{
	Logical, Point, Rectangle, Serial, Transform, SERIAL_COUNTER,
};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
	get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler,
	CompositorState,
};
use smithay::wayland::dmabuf::{
	DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier,
};
use smithay::wayland::output::{OutputHandler, OutputManagerState};
use smithay::wayland::selection::data_device::{
	set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler,
	DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::shell::xdg::{
	PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler,
	XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::viewporter::ViewporterState;
use smithay::{
	delegate_compositor, delegate_data_device, delegate_dmabuf,
	delegate_output, delegate_seat, delegate_shm, delegate_viewporter,
	delegate_xdg_shell,
};
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
	/// touch points that woke the display, these are not forwarded
	pub swallowed_touches: Vec<TouchSlot>,
	/// touch points the client got a down for and no up yet
	pub forwarded_touches: Vec<TouchSlot>,

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
			swallowed_touches: vec![],
			forwarded_touches: vec![],
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

	fn configure_fullscreen(&self, window: &Window) {
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
	fn focus_top(&mut self) {
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

	/// Tells the client that all its touch points are gone.
	pub fn cancel_touches(&mut self) {
		self.forwarded_touches.clear();
		if let Some(touch) = self.seat.get_touch() {
			touch.cancel(self);
		}
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

#[derive(Default)]
pub struct ClientState {
	pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
	fn initialized(&self, _client_id: ClientId) {}
	fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl CompositorHandler for State {
	fn compositor_state(&mut self) -> &mut CompositorState {
		&mut self.compositor_state
	}

	fn client_compositor_state<'a>(
		&self,
		client: &'a Client,
	) -> &'a CompositorClientState {
		&client.get_data::<ClientState>().unwrap().compositor_state
	}

	fn commit(&mut self, surface: &WlSurface) {
		// before the buffer handler, it takes the buffer and the damage
		debug::commit(self, surface);
		on_commit_buffer_handler::<Self>(surface);

		if !is_sync_subsurface(surface) {
			let mut root = surface.clone();
			while let Some(parent) = get_parent(&root) {
				root = parent;
			}
			if let Some(window) = self
				.space
				.elements()
				.find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(&root))
			{
				window.on_commit();
			}
		}

		// send the initial configure of toplevels
		if let Some(window) = self
			.space
			.elements()
			.find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(surface))
		{
			if let Some(toplevel) = window.toplevel() {
				if !toplevel.is_initial_configure_sent() {
					toplevel.send_configure();
				}
			}
		}

		// and the one of popups
		self.popups.commit(surface);
		if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface) {
			if !popup.is_initial_configure_sent() {
				let _ = popup.send_configure();
			}
		}

		self.schedule_render();
	}
}

impl BufferHandler for State {
	fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for State {
	fn shm_state(&self) -> &ShmState {
		&self.shm_state
	}
}

impl DmabufHandler for State {
	fn dmabuf_state(&mut self) -> &mut DmabufState {
		&mut self.dmabuf_state
	}

	fn dmabuf_imported(
		&mut self,
		_global: &DmabufGlobal,
		dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
		notifier: ImportNotifier,
	) {
		if self.backend.renderer().import_dmabuf(&dmabuf, None).is_ok() {
			let _ = notifier.successful::<State>();
		} else {
			notifier.failed();
		}
	}
}

impl XdgShellHandler for State {
	fn xdg_shell_state(&mut self) -> &mut XdgShellState {
		&mut self.xdg_shell_state
	}

	fn new_toplevel(&mut self, surface: ToplevelSurface) {
		info!("new window");
		let surface_id = surface.wl_surface().id().protocol_id();
		self.debug
			.event("window_new", json!({ "surface": surface_id }));
		let window = Window::new_wayland_window(surface);
		self.configure_fullscreen(&window);
		// the newest window is always on top
		self.space.map_element(window, (0, 0), true);
		self.focus_top();
	}

	fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
		info!("window closed");
		let surface_id = surface.wl_surface().id().protocol_id();
		self.debug
			.event("window_closed", json!({ "surface": surface_id }));
		let window = self
			.space
			.elements()
			.find(|w| w.toplevel() == Some(&surface))
			.cloned();
		if let Some(window) = window {
			self.space.unmap_elem(&window);
		}
		self.focus_top();
		self.schedule_render();
	}

	fn new_popup(&mut self, surface: PopupSurface, _pos: PositionerState) {
		self.unconstrain_popup(&surface);
		let _ = self.popups.track_popup(PopupKind::Xdg(surface));
	}

	fn reposition_request(
		&mut self,
		surface: PopupSurface,
		positioner: PositionerState,
		token: u32,
	) {
		surface.with_pending_state(|state| {
			state.geometry = positioner.get_geometry();
			state.positioner = positioner;
		});
		self.unconstrain_popup(&surface);
		surface.send_repositioned(token);
	}

	fn grab(
		&mut self,
		_surface: PopupSurface,
		_seat: wl_seat::WlSeat,
		_: Serial,
	) {
		// popups close themselves when the user taps elsewhere,
		// chromium does that on its own
	}

	// windows are always fullscreen, all requests to change that are
	// answered with the unchanged state
	fn fullscreen_request(
		&mut self,
		surface: ToplevelSurface,
		_output: Option<
			smithay::reexports::wayland_server::protocol::wl_output::WlOutput,
		>,
	) {
		answer_configure(&surface);
	}

	fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
		answer_configure(&surface);
	}

	fn maximize_request(&mut self, surface: ToplevelSurface) {
		answer_configure(&surface);
	}

	fn unmaximize_request(&mut self, surface: ToplevelSurface) {
		answer_configure(&surface);
	}
}

/// The client waits for a configure after a request, even if nothing changed.
fn answer_configure(surface: &ToplevelSurface) {
	// before the initial commit the initial configure answers
	if surface.is_initial_configure_sent() {
		surface.send_configure();
	}
}

impl State {
	fn unconstrain_popup(&self, popup: &PopupSurface) {
		let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone()))
		else {
			return;
		};
		let Some(window) = self
			.space
			.elements()
			.find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(&root))
		else {
			return;
		};
		let (Some(output_geo), Some(window_geo)) =
			(self.output_size(), self.space.element_geometry(window))
		else {
			return;
		};

		// the target geometry is relative to the parent
		let mut target = output_geo;
		target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
		target.loc -= window_geo.loc;

		popup.with_pending_state(|state| {
			state.geometry =
				state.positioner.get_unconstrained_geometry(target);
		});
	}
}

impl SeatHandler for State {
	type KeyboardFocus = WlSurface;
	type PointerFocus = WlSurface;
	type TouchFocus = WlSurface;

	fn seat_state(&mut self) -> &mut SeatState<State> {
		&mut self.seat_state
	}

	fn focus_changed(
		&mut self,
		seat: &Seat<Self>,
		focused: Option<&WlSurface>,
	) {
		let dh = &self.display_handle;
		let client = focused.and_then(|s| dh.get_client(s.id()).ok());
		set_data_device_focus(dh, seat, client);
	}
}

impl SelectionHandler for State {
	type SelectionUserData = ();
}

impl DataDeviceHandler for State {
	fn data_device_state(&self) -> &DataDeviceState {
		&self.data_device_state
	}
}

impl ClientDndGrabHandler for State {}
impl ServerDndGrabHandler for State {}

impl OutputHandler for State {}

delegate_compositor!(State);
delegate_shm!(State);
delegate_dmabuf!(State);
delegate_xdg_shell!(State);
delegate_seat!(State);
delegate_data_device!(State);
delegate_output!(State);
delegate_viewporter!(State);
