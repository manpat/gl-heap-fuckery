use super::{ResourcePath, ResourcePathRef};


#[derive(Hash, Clone, Debug, Eq, PartialEq)]
pub struct ImageDef {
	path: ResourcePath,
}

#[derive(Debug)]
pub struct ImageObject {
	texture_name: u32,
}


pub fn load_raw(def: &ImageDef)
	-> anyhow::Result<ImageObject>
{
	let image = image::open(&def.path)?.flipv().into_rgba8().into_flat_samples();
	// let image_size = Vec2i::new(image.layout.width as i32, image.layout.height as i32);

	let mut texture_name = 0;

	unsafe {
		gl::CreateTextures(gl::TEXTURE_2D, 1, &mut texture_name);
	}

	Ok(ImageObject {
		texture_name
	})
}


