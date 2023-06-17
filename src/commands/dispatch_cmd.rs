use super::{BufferHandle, IntoBufferHandle, BlockBinding, Command, FrameState, ImageBinding, ImageBindingLocation, PassHandle};
use crate::resource_manager::{ShaderHandle, BlockBindingLocation, ImageHandle, SamplerDef};

use std::mem::ManuallyDrop;
use common::Vec3i;


#[derive(Debug)]
pub enum DispatchSizeSource {
	Explicit(Vec3i),
	Indirect(BufferHandle),
}


#[derive(Debug)]
pub struct DispatchCmd {
	pub compute_shader: ShaderHandle,

	pub num_groups: DispatchSizeSource,

	pub block_bindings: Vec<(BlockBinding, BufferHandle)>,
	pub image_bindings: Vec<ImageBinding>,
}


impl From<DispatchCmd> for Command {
	fn from(cmd: DispatchCmd) -> Command {
		Command::Dispatch(cmd)
	}
}

pub struct DispatchCmdBuilder<'fs> {
	frame_state: &'fs mut FrameState,
	cmd: ManuallyDrop<DispatchCmd>,
	pass: PassHandle,
}

impl<'fs> DispatchCmdBuilder<'fs> {
	pub fn indirect(&mut self, buffer: impl IntoBufferHandle) -> &mut Self {
		let buffer_handle = buffer.into_buffer_handle(self.frame_state);
		self.cmd.num_groups = DispatchSizeSource::Indirect(buffer_handle);
		self
	}

	pub fn groups(&mut self, num_groups: impl Into<Vec3i>) -> &mut Self {
		self.cmd.num_groups = DispatchSizeSource::Explicit(num_groups.into());
		self
	}

	pub fn buffer(&mut self, binding: impl Into<BlockBinding>, buffer: impl IntoBufferHandle) -> &mut Self {
		let buffer_handle = buffer.into_buffer_handle(self.frame_state);
		let binding = binding.into();
		self.cmd.block_bindings.push((binding, buffer_handle));
		self
	}

	pub fn ubo(&mut self, index: u32, buffer: impl IntoBufferHandle) -> &mut Self {
		self.buffer(BlockBindingLocation::Ubo(index), buffer)
	}

	pub fn ssbo(&mut self, index: u32, buffer: impl IntoBufferHandle) -> &mut Self {
		self.buffer(BlockBindingLocation::Ssbo(index), buffer)
	}

	pub fn texture(&mut self, location: impl Into<ImageBindingLocation>, image: ImageHandle, sampler: SamplerDef) -> &mut Self {
		self.cmd.image_bindings.push(ImageBinding::texture(image, sampler, location));
		self
	}

	pub fn image(&mut self, location: impl Into<ImageBindingLocation>, image: ImageHandle) -> &mut Self {
		self.cmd.image_bindings.push(ImageBinding::image(image, location));
		self
	}

	pub fn image_rw(&mut self, location: impl Into<ImageBindingLocation>, image: ImageHandle) -> &mut Self {
		self.cmd.image_bindings.push(ImageBinding::image_rw(image, location));
		self
	}
}


impl<'fs> DispatchCmdBuilder<'fs> {
	pub(super) fn new(frame_state: &'fs mut FrameState, pass: PassHandle, compute_shader: ShaderHandle) -> Self {
		DispatchCmdBuilder {
			frame_state,
			cmd: ManuallyDrop::new(DispatchCmd {
				compute_shader,
				num_groups: DispatchSizeSource::Explicit(Vec3i::splat(1)),
				block_bindings: Vec::new(),
				image_bindings: Vec::new(),
			}),
			pass,
		}
	}
}

impl<'fs> Drop for DispatchCmdBuilder<'fs> {
	fn drop(&mut self) {
		let cmd = unsafe { ManuallyDrop::take(&mut self.cmd) };
		self.frame_state.push_cmd(self.pass, cmd);
	}
}