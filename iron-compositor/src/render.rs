use crate::debug::Debug;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{
	Bind, Color32F, ExportMem, Offscreen, TextureMapping,
};
use smithay::desktop::space::{space_render_elements, SpaceRenderElements};
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::utils::{Physical, Rectangle, Size, Transform};

smithay::backend::renderer::element::render_elements! {
	pub Element<=GlesRenderer>;
	Space=SpaceRenderElements<
		GlesRenderer,
		WaylandSurfaceRenderElement<GlesRenderer>,
	>,
	Debug=SolidColorRenderElement,
}

/// same as the weston background-color in weston.ini
pub const CLEAR_COLOR: Color32F = Color32F::new(1.0, 1.0, 1.0, 1.0);

/// The elements of a frame front to back, the debug overlays first.
pub fn elements(
	renderer: &mut GlesRenderer,
	space: &Space<Window>,
	output: &Output,
	debug: &mut Debug,
) -> Vec<Element> {
	let mut elements: Vec<Element> = match space.output_geometry(output) {
		Some(geo) => debug.elements(geo).into_iter().map(Into::into).collect(),
		None => vec![],
	};
	let space = space_render_elements(renderer, [space], output, 1.0)
		.unwrap_or_default();
	elements.extend(space.into_iter().map(Element::from));
	elements
}

/// Draws the elements into a texture, age 0 draws everything.
pub fn draw_offscreen(
	renderer: &mut GlesRenderer,
	texture: &mut GlesTexture,
	damage_tracker: &mut OutputDamageTracker,
	age: usize,
	elements: &[Element],
) -> Result<(), String> {
	let mut framebuffer =
		renderer.bind(texture).map_err(|e| format!("bind {e}"))?;
	damage_tracker
		.render_output(renderer, &mut framebuffer, age, elements, CLEAR_COLOR)
		.map_err(|e| format!("render {e:?}"))?;
	Ok(())
}

/// Renders the output into memory as XRGB8888 (little endian, so the bytes
/// are b, g, r, x), top row first.
pub fn render_to_memory(
	renderer: &mut GlesRenderer,
	space: &Space<Window>,
	output: &Output,
	debug: &mut Debug,
) -> Result<(Size<i32, Physical>, Vec<u8>), String> {
	let mode = output.current_mode().ok_or("output has no mode")?;
	let size = mode.size;
	let buffer_size = size.to_logical(1).to_buffer(1, Transform::Normal);

	let elements = elements(renderer, space, output, debug);

	let mut texture: GlesTexture = renderer
		.create_buffer(Fourcc::Xrgb8888, buffer_size)
		.map_err(|e| format!("create buffer {e}"))?;
	let mut damage_tracker =
		OutputDamageTracker::new(size, 1.0, Transform::Normal);
	draw_offscreen(renderer, &mut texture, &mut damage_tracker, 0, &elements)?;

	let framebuffer = renderer
		.bind(&mut texture)
		.map_err(|e| format!("bind {e}"))?;
	let region = Rectangle::from_size(buffer_size);
	// not every gles driver can read bgra, so fall back to rgba
	let (mapping, swap_rb) =
		match renderer.copy_framebuffer(&framebuffer, region, Fourcc::Xrgb8888)
		{
			Ok(m) => (m, false),
			Err(_) => (
				renderer
					.copy_framebuffer(&framebuffer, region, Fourcc::Xbgr8888)
					.map_err(|e| format!("copy framebuffer {e}"))?,
				true,
			),
		};
	// gl reads bottom up, the mapping says if smithay flipped it already
	let bottom_up = !mapping.flipped();
	let data = renderer
		.map_texture(&mapping)
		.map_err(|e| format!("map texture {e}"))?;

	let stride = size.w as usize * 4;
	let rows = size.h as usize;
	if data.len() < stride * rows {
		return Err(format!("mapping too short {}", data.len()));
	}
	let mut out = Vec::with_capacity(stride * rows);
	for y in 0..rows {
		let row = if bottom_up { rows - 1 - y } else { y };
		out.extend_from_slice(&data[row * stride..(row + 1) * stride]);
	}

	if swap_rb {
		for px in out.as_chunks_mut::<4>().0 {
			px.swap(0, 2);
		}
	}

	Ok((size, out))
}
