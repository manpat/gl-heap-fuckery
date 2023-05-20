mod draw_cmd;
mod dispatch_cmd;

use std::mem::ManuallyDrop;

use crate::resource_manager::{ShaderHandle, BindingLocation};
use crate::upload_heap::{UploadHeap, BufferAllocation, UPLOAD_BUFFER_SIZE};

pub use draw_cmd::*;
pub use dispatch_cmd::*;

pub const DEFAULT_BUFFER_ALIGNMENT: usize = 4;



#[derive(Debug)]
pub enum Command {
	Draw(DrawCmd),
	Dispatch(DispatchCmd),
}



#[derive(Debug)]
pub struct FrameState {
	transient_data: bumpalo::Bump,
	pub commands: Vec<Command>,

	reserved_buffers: Vec<ReservedBuffer>,
	streamed_buffers: Vec<StreamedBuffer>,
}

impl FrameState {
	pub fn new() -> Self {
		FrameState {
			transient_data: bumpalo::Bump::with_capacity(UPLOAD_BUFFER_SIZE),
			commands: Vec::new(),

			reserved_buffers: Vec::new(),
			streamed_buffers: Vec::new(),
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
			alignment: DEFAULT_BUFFER_ALIGNMENT,
		});

		BufferHandle::Streamed(index)
	}

	// TODO(pat.m): maybe reserved buffers shouldn't use the upload heap?
	// upload heap is mapped, host visible, and that might not be ideal for GPU-only visible stuff
	pub fn reserve_buffer(&mut self, size: usize) -> BufferHandle
	{
		let index = self.reserved_buffers.len();
		self.reserved_buffers.push(ReservedBuffer::Pending{size, alignment: DEFAULT_BUFFER_ALIGNMENT});
		BufferHandle::Reserved(index)
	}

	pub fn draw(&mut self, vertex_shader: ShaderHandle, fragment_shader: ShaderHandle) -> DrawCmdBuilder<'_> {
		DrawCmdBuilder::new(self, vertex_shader, fragment_shader)
	}

	pub fn dispatch(&mut self, compute_shader: ShaderHandle) -> DispatchCmdBuilder<'_> {
		DispatchCmdBuilder::new(self, compute_shader)
	}
}


/////////////////// internal
impl FrameState {
	pub fn imbue_buffer_alignment(&mut self, buffer_handle: BufferHandle, requested_alignment: usize) {
		match buffer_handle {
			BufferHandle::Streamed(index) => {
				match &mut self.streamed_buffers[index] {
					StreamedBuffer::Pending{alignment, ..} => {
						*alignment = (*alignment).max(requested_alignment);
					}

					_ => {}
				}
			}

			BufferHandle::Reserved(index) => {
				match &mut self.reserved_buffers[index] {
					ReservedBuffer::Pending{alignment, ..} => {
						*alignment = (*alignment).max(requested_alignment);
					}

					_ => {}
				}
			}

			BufferHandle::Committed => todo!(),
		}
	}

	pub fn upload_buffers(&mut self, upload_heap: &mut UploadHeap) {
		// TODO(pat.m): these could be sorted for better heap usage.
		// alternatively use gap filling?

		for buffer in self.reserved_buffers.iter_mut() {
			let ReservedBuffer::Pending{size, alignment} = *buffer else {
				continue
			};

			*buffer = ReservedBuffer::Allocated(upload_heap.reserve_space(size, alignment));
		}

		for buffer in self.streamed_buffers.iter_mut() {
			let StreamedBuffer::Pending{data, size, alignment} = *buffer else {
				continue
			};

			let slice = unsafe{std::slice::from_raw_parts(data, size)};
			*buffer = StreamedBuffer::Uploaded(upload_heap.push_data(slice, alignment));
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



#[derive(Debug)]
enum StreamedBuffer {
	Pending {
		data: *const u8,
		size: usize,
		alignment: usize,
	},

	Uploaded(BufferAllocation),
}


#[derive(Debug)]
enum ReservedBuffer {
	Pending {
		size: usize,
		alignment: usize,
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

