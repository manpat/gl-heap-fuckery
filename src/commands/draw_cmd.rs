use super::{BufferHandle, IntoBufferHandle, BlockBinding, Command, FrameState};
use crate::resource_manager::{ShaderHandle, BindingLocation};

use std::mem::ManuallyDrop;


#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum PrimitiveType {
	Points = gl::POINTS,
	Lines = gl::LINES,
	Triangles = gl::TRIANGLES,
}


#[derive(Debug)]
pub struct DrawCmd {
	pub vertex_shader: ShaderHandle,
	pub fragment_shader: Option<ShaderHandle>,

	pub primitive_type: PrimitiveType,

	pub num_elements: u32,
	pub num_instances: u32,

	// If set, use indexed rendering
	// TODO(pat.m): how to determine element type
	pub index_buffer: Option<BufferHandle>,

	pub block_bindings: Vec<(BlockBinding, BufferHandle)>,
}


impl From<DrawCmd> for Command {
	fn from(cmd: DrawCmd) -> Command {
		Command::Draw(cmd)
	}
}

pub struct DrawCmdBuilder<'fs> {
	frame_state: &'fs mut FrameState,
	cmd: ManuallyDrop<DrawCmd>,
}

impl<'fs> DrawCmdBuilder<'fs> {
	pub fn elements(&mut self, num_elements: u32) -> &mut Self {
		self.cmd.num_elements = num_elements;
		self
	}

	pub fn instances(&mut self, num_instances: u32) -> &mut Self {
		self.cmd.num_instances = num_instances;
		self
	}

	pub fn primitive(&mut self, ty: PrimitiveType) -> &mut Self {
		self.cmd.primitive_type = ty;
		self
	}

	pub fn indexed(&mut self, buffer: impl IntoBufferHandle) -> &mut Self {
		let buffer_handle = buffer.into_buffer_handle(self.frame_state);
		self.cmd.index_buffer = Some(buffer_handle);
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

impl<'fs> DrawCmdBuilder<'fs> {
	pub(super) fn new(frame_state: &'fs mut FrameState, vertex_shader: ShaderHandle, fragment_shader: ShaderHandle) -> Self {
		DrawCmdBuilder {
			frame_state,
			cmd: ManuallyDrop::new(DrawCmd {
				vertex_shader,
				fragment_shader: Some(fragment_shader),

				primitive_type: PrimitiveType::Triangles,

				num_elements: 3,
				num_instances: 1,

				index_buffer: None,
				block_bindings: Vec::new(),
			})
		}
	}
}

impl<'fs> Drop for DrawCmdBuilder<'fs> {
	fn drop(&mut self) {
		let cmd = unsafe { ManuallyDrop::take(&mut self.cmd) };
		self.frame_state.push_cmd(cmd);
	}
}