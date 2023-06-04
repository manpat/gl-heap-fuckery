
use super::{ResourceManager, ImageHandle};
use common::math::Vec2i;


#[derive(Hash, Clone, Default, Debug, Eq, PartialEq)]
pub struct FboDef {
	pub color_attachment_0: Option<ImageHandle>,
	pub color_attachment_1: Option<ImageHandle>,
	pub color_attachment_2: Option<ImageHandle>,
	pub color_attachment_3: Option<ImageHandle>,
	pub depth_stencil_attachment: Option<ImageHandle>,
}

#[derive(Debug)]
pub struct FboObject {
	pub name: u32,
	pub viewport_size: Vec2i,
}


pub(super) fn create(resource_manager: &ResourceManager, def: &FboDef)
	-> anyhow::Result<FboObject>
{
	let mut name = 0;

	unsafe {
		gl::CreateFramebuffers(1, &mut name);
	}

	let mut object = FboObject {name, viewport_size: Vec2i::zero()};

	resolve_and_bind(resource_manager, def, &mut object);

	Ok(object)
}

pub(super) fn resolve_and_bind(resource_manager: &ResourceManager, def: &FboDef,
	fbo: &mut FboObject)
{
	let mut common_size = None;

	if let Some(handle) = def.color_attachment_0 {
		let image = resource_manager.resolve_image(handle).unwrap();

		common_size = Some(image.size);

		unsafe {
			gl::NamedFramebufferTexture(fbo.name, gl::COLOR_ATTACHMENT0, image.name, 0);
		}
	}

	if let Some(handle) = def.color_attachment_1 {
		let image = resource_manager.resolve_image(handle).unwrap();

		assert!(common_size == None || common_size == Some(image.size));
		common_size = Some(image.size);

		unsafe {
			gl::NamedFramebufferTexture(fbo.name, gl::COLOR_ATTACHMENT1, image.name, 0);
		}
	}

	if let Some(handle) = def.color_attachment_2 {
		let image = resource_manager.resolve_image(handle).unwrap();

		assert!(common_size == None || common_size == Some(image.size));
		common_size = Some(image.size);

		unsafe {
			gl::NamedFramebufferTexture(fbo.name, gl::COLOR_ATTACHMENT2, image.name, 0);
		}
	}

	if let Some(handle) = def.color_attachment_3 {
		let image = resource_manager.resolve_image(handle).unwrap();

		assert!(common_size == None || common_size == Some(image.size));
		common_size = Some(image.size);

		unsafe {
			gl::NamedFramebufferTexture(fbo.name, gl::COLOR_ATTACHMENT3, image.name, 0);
		}
	}

	if let Some(handle) = def.depth_stencil_attachment {
		let image = resource_manager.resolve_image(handle).unwrap();

		assert!(common_size == None || common_size == Some(image.size));
		common_size = Some(image.size);

		unsafe {
			gl::NamedFramebufferTexture(fbo.name, gl::DEPTH_STENCIL_ATTACHMENT, image.name, 0);
		}
	}

	let status = unsafe { gl::CheckNamedFramebufferStatus(fbo.name, gl::DRAW_FRAMEBUFFER) };
	assert!(status == gl::FRAMEBUFFER_COMPLETE);

	fbo.viewport_size = common_size
		.map(|s| s.resolve(resource_manager.backbuffer_size))
		.unwrap_or(Vec2i::zero());
}