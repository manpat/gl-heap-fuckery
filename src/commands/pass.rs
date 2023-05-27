use super::{FrameState, Command};


#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct PassHandle(pub usize);


#[derive(Debug)]
pub struct Pass {
	pub name: String,
	pub commands: Vec<Command>,
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
		});

		PassBuilder {
			pass: frame_state.passes.last_mut().unwrap(),
			handle: PassHandle(index),
		}
	}

	pub fn handle(&self) -> PassHandle {
		self.handle
	}
}
