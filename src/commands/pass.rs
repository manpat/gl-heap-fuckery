use super::{FrameState, Command};
use crate::resource_manager::{FboDef, ImageHandle};


#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct PassHandle(pub usize);


#[derive(Debug)]
pub struct Pass {
	pub name: String,
	pub commands: Vec<Command>,
	pub fbo_def: FboDef,
	pub wants_timer_query: bool,
}

#[must_use]
pub struct PassBuilder<'fs> {
	pass: &'fs mut Pass,
	handle: PassHandle,
}


impl<'fs> PassBuilder<'fs> {
	pub(super) fn new(frame_state: &'fs mut FrameState, name: String) -> Self {
		let index = frame_state.passes.len();
		frame_state.passes.push(Pass {
			name,
			commands: Vec::new(),
			fbo_def: FboDef::default(),
			wants_timer_query: false,
		});

		PassBuilder {
			pass: frame_state.passes.last_mut().unwrap(),
			handle: PassHandle(index),
		}
	}

	pub fn color_attachment(&mut self, attachment_point: u32, image: ImageHandle) -> &mut Self {
		assert!(attachment_point < 4);

		let def = &mut self.pass.fbo_def;

		match attachment_point {
			0 => def.color_attachment_0 = Some(image),
			1 => def.color_attachment_1 = Some(image),
			2 => def.color_attachment_2 = Some(image),
			3 => def.color_attachment_3 = Some(image),
			_ => unreachable!(),
		}

		self
	}

	pub fn depth_stencil_attachment(&mut self, image: ImageHandle) -> &mut Self {
		self.pass.fbo_def.depth_stencil_attachment = Some(image);
		self
	}

	pub fn time(&mut self) -> &mut Self {
		self.pass.wants_timer_query = true;
		self
	}

	pub fn handle(&self) -> PassHandle {
		self.handle
	}
}
