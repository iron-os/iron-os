//! The wayland protocol handlers smithay needs.

use crate::debug::{self};
use crate::state::{root_surface, State};

use serde_json::json;
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::backend::renderer::ImportDma;
use smithay::desktop::{
	find_popup_root_surface, get_popup_toplevel_coords, PopupKind, Window,
};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::backend::ClientData;
use smithay::reexports::wayland_server::protocol::{
	wl_buffer, wl_seat, wl_surface::WlSurface,
};
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::Serial;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
	is_sync_subsurface, CompositorClientState, CompositorHandler,
	CompositorState,
};
use smithay::wayland::dmabuf::{
	DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier,
};
use smithay::wayland::output::OutputHandler;
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
use smithay::{
	delegate_compositor, delegate_data_device, delegate_dmabuf,
	delegate_output, delegate_seat, delegate_shm, delegate_viewporter,
	delegate_xdg_shell,
};
use tracing::info;

#[derive(Default)]
pub struct ClientState {
	pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {}

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
			if let Some(window) = self.window_for(&root_surface(surface)) {
				window.on_commit();
			}
		}

		// send the initial configure of toplevels
		if let Some(window) = self.window_for(surface) {
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
		let Some(window) = self.window_for(&root) else {
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
