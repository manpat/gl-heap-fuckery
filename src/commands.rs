use std::mem::ManuallyDrop;

use crate::resource_manager::{ShaderHandle, BindingLocation};
use crate::upload_heap::{UploadHeap, BufferAllocation, UPLOAD_BUFFER_SIZE};


#[derive(Debug)]
pub enum Command {
	Draw(DrawCmd),
	Dispatch(DispatchCmd),

	MemoryBarrier,
}



#[derive(Debug)]
pub struct FrameState {
	transient_data: bumpalo::Bump,
	pub commands: Vec<Command>,

	reserved_buffers: Vec<ReservedBuffer>,
	streamed_buffers: Vec<StreamedBuffer>,

	uniform_buffer_offset_alignment: usize,
}

impl FrameState {
	pub fn new() -> Self {
		let mut uniform_buffer_offset_alignment = 0;

		unsafe {
			gl::GetIntegerv(gl::UNIFORM_BUFFER_OFFSET_ALIGNMENT, &mut uniform_buffer_offset_alignment)
		}

		FrameState {
			transient_data: bumpalo::Bump::with_capacity(UPLOAD_BUFFER_SIZE),
			commands: Vec::new(),

			reserved_buffers: Vec::new(),
			streamed_buffers: Vec::new(),

			uniform_buffer_offset_alignment: uniform_buffer_offset_alignment as usize,
		}
	}

	pub fn reset(&mut self) {
		self.commands.clear();
		self.reserved_buffers.clear();
		self.streamed_buffers.clear();
		self.transient_data.reset();
	}

	pub fn push_cmd(&mut self, cmd: impl Into<Command>) {
		self.commands.push(cmd.into());
	}


	pub fn stream_buffer<T>(&mut self, data: &[T]) -> BufferHandle
		where T: Copy
	{
		let data_copy = self.transient_data.alloc_slice_copy(data);

		let index = self.streamed_buffers.len();
		self.streamed_buffers.push(StreamedBuffer::Pending {
			data: data_copy.as_ptr().cast(),
			size: data_copy.len() * std::mem::size_of::<T>(),

			alignment_type: None,
		});

		BufferHandle::Streamed(index)
	}

	pub fn reserve_buffer(&mut self, size: usize) -> BufferHandle
	{
		let index = self.reserved_buffers.len();
		self.reserved_buffers.push(ReservedBuffer::Pending{size, alignment_type: None});
		BufferHandle::Reserved(index)
	}

	pub fn draw(&mut self, vertex_shader: ShaderHandle, fragment_shader: ShaderHandle) -> DrawCmdBuilder<'_> {
		DrawCmdBuilder {
			frame_state: self,
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

	pub fn dispatch(&mut self, compute_shader: ShaderHandle) -> DispatchCmdBuilder<'_> {
		DispatchCmdBuilder {
			frame_state: self,
			cmd: ManuallyDrop::new(DispatchCmd {
				compute_shader,
				num_groups: DispatchSizeSource::Explicit([1; 3]),
				block_bindings: Vec::new(),
			})
		}
	}

	pub fn memory_barrier(&mut self) {
		self.push_cmd(Command::MemoryBarrier);
	}
}


/////////////////// internal
impl FrameState {
	pub fn mark_ubo(&mut self, buffer_handle: BufferHandle) {
		self.mark_alignment(buffer_handle, AlignmentType::Ubo);
	}

	pub fn mark_ssbo(&mut self, buffer_handle: BufferHandle) {
		self.mark_alignment(buffer_handle, AlignmentType::Ssbo);
	}

	pub fn mark_index_buffer(&mut self, buffer_handle: BufferHandle) {
		// TODO(pat.m): alignment?
		self.mark_alignment(buffer_handle, AlignmentType::Other);
	}

	fn mark_alignment(&mut self, buffer_handle: BufferHandle, new_alignment_type: AlignmentType) {
		match buffer_handle {
			BufferHandle::Streamed(index) => {
				match &mut self.streamed_buffers[index] {
					StreamedBuffer::Pending{alignment_type, ..} => {
						alignment_type.get_or_insert(new_alignment_type);
					}

					_ => {}
				}
			}

			BufferHandle::Reserved(index) => {
				match &mut self.reserved_buffers[index] {
					ReservedBuffer::Pending{alignment_type, ..} => {
						alignment_type.get_or_insert(new_alignment_type);
					}

					_ => {}
				}
			}

			BufferHandle::Committed => todo!(),
		}
	}

	pub fn upload_buffers(&mut self, upload_heap: &mut UploadHeap) {
		self.upload_buffer_of_type(upload_heap, AlignmentType::Ubo, self.uniform_buffer_offset_alignment);
		self.upload_buffer_of_type(upload_heap, AlignmentType::Ssbo, 32);
		self.upload_buffer_of_type(upload_heap, AlignmentType::Other, 4);
	}

	fn upload_buffer_of_type(&mut self, upload_heap: &mut UploadHeap, requested_alignment_type: AlignmentType, alignment: usize) {
		// TODO(pat.m): this should maybe be handled separately - come back to this once tracked resource graphs are a thing,
		// since space could be reused
		for buffer in self.reserved_buffers.iter_mut() {
			let ReservedBuffer::Pending{size, alignment_type: Some(alignment_type)} = *buffer else {
				continue
			};

			if alignment_type == requested_alignment_type {
				*buffer = ReservedBuffer::Allocated(upload_heap.reserve_space(size, alignment));
			}
		}

		for buffer in self.streamed_buffers.iter_mut() {
			let StreamedBuffer::Pending{data, size, alignment_type: Some(alignment_type)} = *buffer else {
				continue
			};

			if alignment_type == requested_alignment_type {
				let slice = unsafe{std::slice::from_raw_parts(data, size)};
				*buffer = StreamedBuffer::Uploaded(upload_heap.push_data(slice, alignment));
			}
		}
	}

	pub fn resolve_buffer_allocation(&self, buffer_handle: BufferHandle) -> BufferAllocation {
		match buffer_handle {
			BufferHandle::Streamed(index) => {
				if let Some(StreamedBuffer::Uploaded(allocation)) = self.streamed_buffers.get(index) {
					*allocation
				} else {
					panic!("Unallocated buffer")
				}
			}

			BufferHandle::Reserved(index) => {
				if let Some(ReservedBuffer::Allocated(allocation)) = self.reserved_buffers.get(index) {
					*allocation
				} else {
					panic!("Unallocated buffer")
				}
			}

			BufferHandle::Committed => todo!()
		}
	}
}



#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum BufferHandle {
	Streamed(usize),
	Reserved(usize),
	Committed,
}

pub trait IntoBufferHandle {
	fn into_buffer_handle(self, frame_state: &mut FrameState) -> BufferHandle;
}

impl IntoBufferHandle for BufferHandle {
	fn into_buffer_handle(self, _: &mut FrameState) -> BufferHandle { self }
}

impl<'t, T> IntoBufferHandle for &'t [T]
	where T: Copy
{
	fn into_buffer_handle(self, frame_state: &mut FrameState) -> BufferHandle {
		frame_state.stream_buffer(self)
	}
}

impl<'t, T> IntoBufferHandle for &'t T
	where T: Copy + Sized
{
	fn into_buffer_handle(self, frame_state: &mut FrameState) -> BufferHandle {
		frame_state.stream_buffer(std::slice::from_ref(self))
	}
}


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum AlignmentType {
	Ubo,
	Ssbo,
	Other,
}


#[derive(Debug)]
enum StreamedBuffer {
	Pending {
		data: *const u8,
		size: usize,

		alignment_type: Option<AlignmentType>,
	},

	Uploaded(BufferAllocation),
}


#[derive(Debug)]
enum ReservedBuffer {
	Pending {
		size: usize,

		alignment_type: Option<AlignmentType>,
	},

	Allocated(BufferAllocation),
}


#[derive(Debug, Copy, Clone)]
pub enum BlockBinding {
	Explicit(BindingLocation),
	Named(&'static str),
}

impl From<BindingLocation> for BlockBinding {
	fn from(o: BindingLocation) -> BlockBinding {
		BlockBinding::Explicit(o)
	}
}

impl From<&'static str> for BlockBinding {
	fn from(o: &'static str) -> BlockBinding {
		BlockBinding::Named(o)
	}
}


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

impl<'fs> Drop for DrawCmdBuilder<'fs> {
	fn drop(&mut self) {
		let cmd = unsafe { ManuallyDrop::take(&mut self.cmd) };
		self.frame_state.push_cmd(cmd);
	}
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

impl<'fs> Drop for DispatchCmdBuilder<'fs> {
	fn drop(&mut self) {
		let cmd = unsafe { ManuallyDrop::take(&mut self.cmd) };
		self.frame_state.push_cmd(cmd);
	}
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