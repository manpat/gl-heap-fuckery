use super::{ResourceManager, ResourcePath, ResourcePathRef};
use common::math::Vec2i;

#[derive(Hash, Clone, Debug, Eq, PartialEq)]
pub enum ImageDef {
	Path(ResourcePath),
	Runtime {
		format: u32,
		size: ImageSize,
	}
}

impl ImageDef {
	pub fn render_target(format: u32) -> ImageDef {
		ImageDef::Runtime {
			format,
			size: ImageSize::Backbuffer,
		}
	}

	pub fn depth_stencil() -> ImageDef {
		ImageDef::render_target(gl::DEPTH24_STENCIL8)
	}
}


#[derive(Hash, Copy, Clone, Debug, Eq, PartialEq)]
pub enum ImageSize {
	Fixed(Vec2i),
	Backbuffer,
}

impl ImageSize {
	pub fn resolve(self, backbuffer_size: Vec2i) -> Vec2i {
		match self {
			ImageSize::Fixed(size) => size,
			ImageSize::Backbuffer => backbuffer_size,
		}
	}
}

#[derive(Debug)]
pub struct ImageObject {
	pub name: u32,
	pub size: ImageSize,
	pub format: u32,
}

impl ImageDef {
	pub fn new(path: impl Into<ResourcePath>) -> ImageDef {
		ImageDef::Path(path.into())
	}
}


pub(super) fn load(resource_manager: &ResourceManager, def: &ImageDef)
	-> anyhow::Result<ImageObject>
{
	let backbuffer_size = resource_manager.backbuffer_size;

	match def {
		ImageDef::Path(path) => load_from_path(resource_manager, path),
		ImageDef::Runtime{ format, size } => create_rendertarget(*format, size.resolve(backbuffer_size)),
	}
}


fn load_from_path(resource_manager: &ResourceManager, path: &ResourcePathRef)
	-> anyhow::Result<ImageObject>
{
	let image = image::open(&resource_manager.resolve_path(path))?.flipv().into_rgba8().into_flat_samples();
	let size = Vec2i::new(image.layout.width as i32, image.layout.height as i32);
	let Vec2i{x: width, y: height} = size;

	let mut name = 0;

	unsafe {
		gl::CreateTextures(gl::TEXTURE_2D, 1, &mut name);
		gl::TextureStorage2D(name, 1, gl::SRGB8_ALPHA8, width, height);

		let (level, offset_x, offset_y) = (0, 0, 0);
		let data = image.samples.as_ptr();

		gl::TextureSubImage2D(name, level, offset_x, offset_y,
			width, height,
			gl::RGBA,
			gl::UNSIGNED_BYTE,
			data as *const _);

		if let Some(path_str) = path.to_str() {
			gl::ObjectLabel(gl::TEXTURE, name, path_str.len() as i32, path_str.as_ptr() as *const _);
		}
	}

	Ok(ImageObject {
		name,
		size: ImageSize::Fixed(size),
		format: gl::SRGB8_ALPHA8,
	})
}



fn create_rendertarget(format: u32, size: Vec2i)
	-> anyhow::Result<ImageObject>
{
	let mut name = 0;

	unsafe {
		gl::CreateTextures(gl::TEXTURE_2D, 1, &mut name);
		gl::TextureStorage2D(name, 1, format, size.x, size.y);

		let label = "rendertarget";
		gl::ObjectLabel(gl::TEXTURE, name, label.len() as i32, label.as_ptr() as *const _);
	}

	Ok(ImageObject {
		name,
		size: ImageSize::Backbuffer,
		format,
	})
}
