//! Client commits as debug events and for the damage overlay.

use super::{DAMAGE_SHOWN, MAX_DAMAGE};

use crate::state::{root_surface, State};

use std::time::Instant;

use serde_json::json;
use smithay::desktop::get_popup_toplevel_coords;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::compositor::{with_states, Damage, SurfaceAttributes};

/// A client committed new content: counts it for the stats, remembers its
/// damage for the overlay and sends it to the live subscribers.
///
/// Has to run before the buffer handler, it takes the buffer and the damage.
pub fn commit(state: &mut State, surface: &WlSurface) {
	let debug = &mut state.debug;
	debug.commits += 1;
	if !debug.damage_overlay && !debug.live() {
		return;
	}

	let (damage, buffer) = with_states(surface, |states| {
		let mut attrs = states.cached_state.get::<SurfaceAttributes>();
		let attrs = attrs.current();
		let damage: Vec<Rectangle<i32, Logical>> = attrs
			.damage
			.iter()
			.take(MAX_DAMAGE)
			.map(|d| match d {
				Damage::Surface(r) => *r,
				// buffer scale and transform are always 1 and normal
				Damage::Buffer(r) => Rectangle::new(
					(r.loc.x, r.loc.y).into(),
					(r.size.w, r.size.h).into(),
				),
			})
			.collect();
		(damage, attrs.buffer.is_some())
	});

	// subsurfaces are shown at the position of their parent
	let root = root_surface(surface);
	let (kind, offset): (_, Point<i32, Logical>) =
		if let Some(popup) = state.popups.find_popup(&root) {
			let toplevel = get_popup_toplevel_coords(&popup);
			("popup", toplevel + popup.geometry().loc)
		} else if let Some(window) = state.window_for(&root) {
			let loc = state.space.element_location(window).unwrap_or_default();
			("window", loc - window.geometry().loc)
		} else {
			("other", (0, 0).into())
		};
	let kind = if &root != surface { "subsurface" } else { kind };

	// client values, never overflow
	let damage: Vec<Rectangle<i32, Logical>> = damage
		.into_iter()
		.map(|r| {
			Rectangle::new(
				(
					r.loc.x.saturating_add(offset.x),
					r.loc.y.saturating_add(offset.y),
				)
					.into(),
				r.size,
			)
		})
		.collect();

	let debug = &mut state.debug;
	if debug.damage_overlay {
		let now = Instant::now();
		debug.damage.retain(|(at, _)| now - *at < DAMAGE_SHOWN);
		debug.damage.extend(damage.iter().map(|r| (now, *r)));
		while debug.damage.len() > MAX_DAMAGE {
			debug.damage.pop_front();
		}
	}

	if !debug.live() {
		return;
	}
	let area: i64 = damage
		.iter()
		.map(|r| (r.size.w as i64).saturating_mul(r.size.h as i64))
		.fold(0, i64::saturating_add);
	let pid = state
		.display_handle
		.get_client(surface.id())
		.ok()
		.and_then(|c| c.get_credentials(&state.display_handle).ok())
		.map(|c| c.pid);

	state.debug.frequent(
		"commit",
		json!({
			"surface": surface.id().protocol_id(),
			"pid": pid,
			"kind": kind,
			"buffer": buffer,
			"damage_px": area,
			"damage": damage
				.iter()
				.take(8)
				.map(|r| [r.loc.x, r.loc.y, r.size.w, r.size.h])
				.collect::<Vec<_>>(),
		}),
	);
}
