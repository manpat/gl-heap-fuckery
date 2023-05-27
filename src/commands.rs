mod pass;
mod draw_cmd;
mod dispatch_cmd;

use crate::resource_manager::{ShaderHandle, BlockBindingLocation, ImageHandle, SamplerDef, PipelineDef};
use crate::upload_heap::{UploadHeap, BufferAllocation, UPLOAD_BUFFER_SIZE};

pub use pass::*;
pub use draw_cmd::*;
pub use dispatch_cmd::*;

pub const DEFAULT_BUFFER_ALIGNMENT: usize = 4;



#[derive(Debug)]
pub enum Command {
	Draw(DrawCmd),
	Dispatch(DispatchCmd),
}

impl Command {
	pub fn block_bindings(&self) -> Option<&[(BlockBinding, BufferHandle)]> {
		match self {
			Command::Draw(DrawCmd { block_bindings, .. })
			| Command::Dispatch(DispatchCmd { block_bindings, .. })
				=> Some(block_bindings),
		}
	}

	pub fn block_bindings_mut(&mut self) -> Option<&mut [(BlockBinding, BufferHandle)]> {
		match self {
			Command::Draw(DrawCmd { block_bindings, .. })
			| Command::Dispatch(DispatchCmd { block_bindings, .. })
				=> Some(block_bindings),
		}
	}

	pub fn image_bindings(&self) -> Option<&[ImageBinding]> {
		match self {
			Command::Draw(DrawCmd { image_bindings, .. })
			| Command::Dispatch(DispatchCmd { image_bindings, .. })
				=> Some(image_bindings),
		}
	}

	pub fn image_bindings_mut(&mut self) -> Option<&mut [ImageBinding]> {
		match self {
			Command::Draw(DrawCmd { image_bindings, .. })
			| Command::Dispatch(DispatchCmd { image_bindings, .. })
				=> Some(image_bindings),
		}
	}

	pub fn pipeline_def(&self) -> Option<PipelineDef> {
		match self {
			Command::Draw(cmd) => Some(PipelineDef {
				vertex: Some(cmd.vertex_shader),
				fragment: cmd.fragment_shader,
				.. PipelineDef::default()
			}),

			Command::Dispatch(cmd) => Some(PipelineDef {
				compute: Some(cmd.compute_shader),
				.. PipelineDef::default()
			}),
		}
	}
}


#[derive(Debug)]
pub struct TransientAllocator {
	transient_data: bumpalo::Bump,
	reserved_buffers: Vec<ReservedBuffer>,
	streamed_buffers: Vec<StreamedBuffer>,
}

#[derive(Debug)]
pub struct FrameState {
	pub passes: Vec<Pass>,
	pub allocator: TransientAllocator,

}

impl FrameState {
	pub fn new() -> Self {
		FrameState {
			passes: Vec::new(),

			allocator: TransientAllocator {
				transient_data: bumpalo::Bump::with_capacity(UPLOAD_BUFFER_SIZE),
				reserved_buffers: Vec::new(),
				streamed_buffers: Vec::new(),
			}
		}
	}

	pub fn reset(&mut self) {
		// self.commands.clear();
		self.passes.clear();
		self.allocator.reserved_buffers.clear();
		self.allocator.streamed_buffers.clear();
		self.allocator.transient_data.reset();
	}

	pub fn push_cmd(&mut self, pass: PassHandle, cmd: impl Into<Command>) {
		self.passes[pass.0].commands.push(cmd.into());
	}


	pub fn stream_buffer<T>(&mut self, data: &[T]) -> BufferHandle
		where T: Copy
	{
		let data_copy = self.allocator.transient_data.alloc_slice_copy(data);

		let index = self.allocator.streamed_buffers.len();
		self.allocator.streamed_buffers.push(StreamedBuffer::Pending {
			data: data_copy.as_ptr().cast(),
			size: data_copy.len() * std::mem::size_of::<T>(),
			alignment: DEFAULT_BUFFER_ALIGNMENT,
		});

		BufferHandle::Streamed(index)
	}

	// TODO(pat.m): maybe reserved buffers shouldn't use the upload heap?
	// upload heap is mapped, host visible, and that might not be ideal for GPU-only visible stuff
	pub fn reserve_buffer(&mut self, size: usize) -> BufferHandle {
		let index = self.allocator.reserved_buffers.len();
		self.allocator.reserved_buffers.push(ReservedBuffer::Pending{size, alignment: DEFAULT_BUFFER_ALIGNMENT});
		BufferHandle::Reserved(index)
	}

	pub fn pass_builder(&mut self, name: impl Into<String>) -> PassBuilder<'_> {
		PassBuilder::new(self, name.into())
	}

	pub fn pass(&mut self, name: impl Into<String>) -> PassHandle {
		self.pass_builder(name).handle()
	}

	pub fn draw(&mut self, pass: PassHandle, vertex_shader: ShaderHandle, fragment_shader: ShaderHandle) -> DrawCmdBuilder<'_> {
		DrawCmdBuilder::new(self, pass, vertex_shader, fragment_shader)
	}

	pub fn dispatch(&mut self, pass: PassHandle, compute_shader: ShaderHandle) -> DispatchCmdBuilder<'_> {
		DispatchCmdBuilder::new(self, pass, compute_shader)
	}
}


/////////////////// internal
impl TransientAllocator {
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
	Explicit(BlockBindingLocation),
	Named(&'static str),
}

impl From<BlockBindingLocation> for BlockBinding {
	fn from(o: BlockBindingLocation) -> BlockBinding {
		BlockBinding::Explicit(o)
	}
}

impl From<&'static str> for BlockBinding {
	fn from(o: &'static str) -> BlockBinding {
		BlockBinding::Named(o)
	}
}



#[derive(Debug, Copy, Clone)]
pub enum ImageBindingLocation {
	Explicit(u32),
	Named(&'static str),
}

impl From<u32> for ImageBindingLocation {
	fn from(o: u32) -> ImageBindingLocation {
		ImageBindingLocation::Explicit(o)
	}
}

impl From<&'static str> for ImageBindingLocation {
	fn from(o: &'static str) -> ImageBindingLocation {
		ImageBindingLocation::Named(o)
	}
}


#[derive(Debug, Copy, Clone)]
pub enum ImageBinding {
	Texture {
		handle: ImageHandle,
		sampler: SamplerDef,
		location: ImageBindingLocation,
	},

	Image {
		handle: ImageHandle,
		location: ImageBindingLocation,
		read_write: bool,
	}
}


impl ImageBinding {
	pub fn texture(handle: ImageHandle, sampler: SamplerDef, location: impl Into<ImageBindingLocation>) -> Self {
		ImageBinding::Texture {
			handle,
			sampler,
			location: location.into(),
		}
	}

	pub fn image(handle: ImageHandle, location: impl Into<ImageBindingLocation>) -> Self {
		ImageBinding::Image {
			handle,
			location: location.into(),
			read_write: false,
		}
	}

	pub fn image_rw(handle: ImageHandle, location: impl Into<ImageBindingLocation>) -> Self {
		ImageBinding::Image {
			handle,
			location: location.into(),
			read_write: true,
		}
	}

	pub fn image_handle(&self) -> ImageHandle {
		let (ImageBinding::Texture{handle, ..} | ImageBinding::Image{handle, ..}) = self;
		*handle
	}

	pub fn location(&self) -> ImageBindingLocation {
		let (ImageBinding::Texture{location, ..} | ImageBinding::Image{location, ..}) = self;
		*location
	}

	pub fn set_location(&mut self, new_location: ImageBindingLocation) {
		let (ImageBinding::Texture{location, ..} | ImageBinding::Image{location, ..}) = self;
		*location = new_location;
	}
}