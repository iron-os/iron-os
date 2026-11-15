//! The mouse cursor: the client's cursor surface, or a built in arrow for
//! named cursors (the image has no cursor theme). It is only shown after
//! the mouse was used and hidden again by a touch, so a touchscreen never
//! shows one, like weston.

use crate::render::Element;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::{
	MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::surface::render_elements_from_surface_tree;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::utils::send_frames_surface_tree;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Point, Transform};
use smithay::wayland::compositor::{with_states, SurfaceAttributes};
use std::time::Duration;
use tracing::warn;

/// The default arrow, `X` black, `.` white, the tip is the hotspot.
const ARROW: [&str; 19] = [
	"X           ",
	"XX          ",
	"X.X         ",
	"X..X        ",
	"X...X       ",
	"X....X      ",
	"X.....X     ",
	"X......X    ",
	"X.......X   ",
	"X........X  ",
	"X.........X ",
	"X..........X",
	"X......XXXXX",
	"X...X..X    ",
	"X..XX..X    ",
	"X.X  X..X   ",
	"XX   X..X   ",
	"X     X..X  ",
	"      XXX   ",
];

pub struct Cursor {
	image: CursorImageStatus,
	visible: bool,
	arrow: MemoryRenderBuffer,
}

impl Cursor {
	pub fn new() -> Self {
		let mut pixels = Vec::with_capacity(ARROW.len() * ARROW[0].len() * 4);
		for row in ARROW {
			for c in row.bytes() {
				// argb8888 little endian: b, g, r, a
				let px: [u8; 4] = match c {
					b'X' => [0, 0, 0, 255],
					b'.' => [255, 255, 255, 255],
					_ => [0, 0, 0, 0],
				};
				pixels.extend_from_slice(&px);
			}
		}
		let size = (ARROW[0].len() as i32, ARROW.len() as i32);
		let arrow = MemoryRenderBuffer::from_slice(
			&pixels,
			Fourcc::Argb8888,
			size,
			1,
			Transform::Normal,
			None,
		);

		Self {
			image: CursorImageStatus::default_named(),
			visible: false,
			arrow,
		}
	}

	pub fn set_image(&mut self, image: CursorImageStatus) {
		self.image = image;
	}

	pub fn is_visible(&self) -> bool {
		self.visible
	}

	/// Returns true if the visibility changed.
	pub fn set_visible(&mut self, visible: bool) -> bool {
		let changed = self.visible != visible;
		self.visible = visible;
		changed
	}

	fn surface(&self) -> Option<&WlSurface> {
		match &self.image {
			CursorImageStatus::Surface(surface) => Some(surface),
			_ => None,
		}
	}

	/// A commit moves the hotspot by the offset of the new buffer
	/// (`wl_surface.attach` / `offset`), smithay only sets it in set_cursor.
	pub fn commit(&self, surface: &WlSurface) {
		if self.surface() != Some(surface) {
			return;
		}

		with_states(surface, |states| {
			let Some(data) = states.data_map.get::<CursorImageSurfaceData>()
			else {
				return;
			};
			let delta = states
				.cached_state
				.get::<SurfaceAttributes>()
				.current()
				.buffer_delta
				.take();
			if let Some(delta) = delta {
				data.lock().unwrap().hotspot -= delta;
			}
		});
	}

	/// Frame callbacks for the cursor surface, else animated cursors stall
	/// and the callbacks pile up.
	pub fn send_frames(&self, output: &Output, time: Duration) {
		if let Some(surface) = self.surface() {
			send_frames_surface_tree(
				surface,
				output,
				time,
				Some(Duration::ZERO),
				|_, _| Some(output.clone()),
			);
		}
	}

	/// The elements of the cursor at the pointer location, front to back.
	pub fn elements(
		&self,
		renderer: &mut GlesRenderer,
		location: Point<f64, Logical>,
	) -> Vec<Element> {
		if !self.visible {
			return vec![];
		}

		match &self.image {
			CursorImageStatus::Hidden => vec![],
			// a destroyed surface would draw nothing, so the arrow until
			// the client sets a new one
			CursorImageStatus::Surface(surface) if surface.alive() => {
				let hotspot = with_states(surface, |states| {
					states
						.data_map
						.get::<CursorImageSurfaceData>()
						.map(|d| d.lock().unwrap().hotspot)
						.unwrap_or_default()
				});
				let pos = (location - hotspot.to_f64()).to_physical(1.0);
				render_elements_from_surface_tree(
					renderer,
					surface,
					pos.to_i32_round(),
					1.0,
					1.0,
					Kind::Cursor,
				)
			}
			CursorImageStatus::Surface(_) | CursorImageStatus::Named(_) => {
				let res = MemoryRenderBufferRenderElement::from_buffer(
					renderer,
					location.to_physical(1.0),
					&self.arrow,
					None,
					None,
					None,
					Kind::Cursor,
				);
				match res {
					Ok(e) => vec![Element::Arrow(e)],
					Err(e) => {
						warn!("could not draw the cursor {e}");
						vec![]
					}
				}
			}
		}
	}
}
