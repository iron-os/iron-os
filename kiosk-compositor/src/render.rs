use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{
	Bind, Color32F, ExportMem, Offscreen, TextureMapping,
};
use smithay::desktop::space::{space_render_elements, SpaceRenderElements};
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::utils::{Physical, Rectangle, Size, Transform};

pub type Element = SpaceRenderElements<
	GlesRenderer,
	WaylandSurfaceRenderElement<GlesRenderer>,
>;

/// same as the weston background-color in weston.ini
pub const CLEAR_COLOR: Color32F = Color32F::new(1.0, 1.0, 1.0, 1.0);

pub fn elements(
	renderer: &mut GlesRenderer,
	space: &Space<Window>,
	output: &Output,
) -> Vec<Element> {
	space_render_elements(renderer, [space], output, 1.0).unwrap_or_default()
}

/// Renders the output into memory as XRGB8888 (little endian, so the bytes
/// are b, g, r, x), top row first.
pub fn render_to_memory(
	renderer: &mut GlesRenderer,
	space: &Space<Window>,
	output: &Output,
) -> Result<(Size<i32, Physical>, Vec<u8>), String> {
	let mode = output.current_mode().ok_or("output has no mode")?;
	let size = mode.size;
	let buffer_size = size.to_logical(1).to_buffer(1, Transform::Normal);

	let elements = elements(renderer, space, output);

	let mut texture: GlesTexture = renderer
		.create_buffer(Fourcc::Xrgb8888, buffer_size)
		.map_err(|e| format!("create buffer {e}"))?;
	let mut framebuffer = renderer
		.bind(&mut texture)
		.map_err(|e| format!("bind {e}"))?;

	let mut damage_tracker =
		OutputDamageTracker::new(size, 1.0, Transform::Normal);
	damage_tracker
		.render_output(renderer, &mut framebuffer, 0, &elements, CLEAR_COLOR)
		.map_err(|e| format!("render {e:?}"))?;

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
	let mut out = Vec::with_capacity(data.len());
	for y in 0..size.h as usize {
		let row = if bottom_up {
			size.h as usize - 1 - y
		} else {
			y
		};
		out.extend_from_slice(&data[row * stride..(row + 1) * stride]);
	}

	if swap_rb {
		for px in out.chunks_exact_mut(4) {
			px.swap(0, 2);
		}
	}

	Ok((size, out))
}
