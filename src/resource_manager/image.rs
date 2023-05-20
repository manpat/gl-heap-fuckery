use super::{ResourceManager, ResourcePath};
use common::math::Vec2i;

#[derive(Hash, Clone, Debug, Eq, PartialEq)]
pub struct ImageDef {
	path: ResourcePath,
}

#[derive(Debug)]
pub struct ImageObject {
	pub name: u32,
	pub size: Vec2i,
}

impl ImageObject {
	// pub fn bindless_handle_with_sampler(&self, sampler_name: u32) -> u64 {
	// 	unsafe {
	// 		gl::GetTextureSamplerHandleARB(self.name, sampler_name)
	// 	}
	// }
}

impl ImageDef {
	pub fn new(path: impl Into<ResourcePath>) -> ImageDef {
		ImageDef {
			path: path.into(),
		}
	}
}


pub fn load_raw(resource_manager: &ResourceManager, def: &ImageDef)
	-> anyhow::Result<ImageObject>
{
	let image = image::open(&resource_manager.resolve_path(&def.path))?.flipv().into_rgba8().into_flat_samples();
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

		if let Some(path_str) = def.path.to_str() {
			gl::ObjectLabel(gl::TEXTURE, name, path_str.len() as i32, path_str.as_ptr() as *const _);
		}
	}

	Ok(ImageObject {
		name,
		size,
	})
}


