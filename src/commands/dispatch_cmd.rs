use super::{BufferHandle, IntoBufferHandle, BlockBinding, Command, FrameState};
use crate::resource_manager::{ShaderHandle, BindingLocation};

use std::mem::ManuallyDrop;



#[derive(Debug)]
pub enum DispatchSizeSource {
	Explicit([u32; 3]),
	Indirect(BufferHandle),
}


#[derive(Debug)]
pub struct DispatchCmd {
	pub compute_shader: ShaderHandle,

	pub num_groups: DispatchSizeSource,

	pub block_bindings: Vec<(BlockBinding, BufferHandle)>,
}


impl From<DispatchCmd> for Command {
	fn from(cmd: DispatchCmd) -> Command {
		Command::Dispatch(cmd)
	}
}

pub struct DispatchCmdBuilder<'fs> {
	frame_state: &'fs mut FrameState,
	cmd: ManuallyDrop<DispatchCmd>,
}

impl<'fs> DispatchCmdBuilder<'fs> {
	pub fn indirect(&mut self, buffer: impl IntoBufferHandle) -> &mut Self {
		let buffer_handle = buffer.into_buffer_handle(self.frame_state);
		self.cmd.num_groups = DispatchSizeSource::Indirect(buffer_handle);
		self
	}

	pub fn groups(&mut self, x: u32, y: u32, z: u32) -> &mut Self {
		self.cmd.num_groups = DispatchSizeSource::Explicit([x, y, z]);
		self
	}

	pub fn buffer(&mut self, binding: impl Into<BlockBinding>, buffer: impl IntoBufferHandle) -> &mut Self {
		let buffer_handle = buffer.into_buffer_handle(self.frame_state);
		let binding = binding.into();
		self.cmd.block_bindings.push((binding, buffer_handle));
		self
	}

	pub fn ubo(&mut self, index: u32, buffer: impl IntoBufferHandle) -> &mut Self {
		self.buffer(BindingLocation::Ubo(index), buffer)
	}

	pub fn ssbo(&mut self, index: u32, buffer: impl IntoBufferHandle) -> &mut Self {
		self.buffer(BindingLocation::Ssbo(index), buffer)
	}
}


impl<'fs> DispatchCmdBuilder<'fs> {
	pub(super) fn new(frame_state: &'fs mut FrameState, compute_shader: ShaderHandle) -> Self {
		DispatchCmdBuilder {
			frame_state,
			cmd: ManuallyDrop::new(DispatchCmd {
				compute_shader,
				num_groups: DispatchSizeSource::Explicit([1; 3]),
				block_bindings: Vec::new(),
			})
		}
	}
}

impl<'fs> Drop for DispatchCmdBuilder<'fs> {
	fn drop(&mut self) {
		let cmd = unsafe { ManuallyDrop::take(&mut self.cmd) };
		self.frame_state.push_cmd(cmd);
	}
}