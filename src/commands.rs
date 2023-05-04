use crate::context::UPLOAD_BUFFER_SIZE;
use crate::resource_manager::{ShaderHandle};




#[derive(Debug)]
pub enum StreamedBuffer {
	Pending {
		data: *const u8,
		size: usize,
	},

	Uploaded {
		offset: isize,
		size: usize,
	}
}

#[derive(Debug)]
pub struct DrawCmd {
	pub vertex_shader: ShaderHandle,
	pub fragment_shader: Option<ShaderHandle>,

	pub num_elements: u32,
	pub num_instances: u32,

	// If set, use indexed rendering
	pub index_buffer: *mut StreamedBuffer,

	pub ssbo_bindings: Vec<(u32, *mut StreamedBuffer)>,
	pub ubo_bindings: Vec<(u32, *mut StreamedBuffer)>,
}

#[derive(Debug)]
pub enum Command {
	Draw(DrawCmd),
}

impl From<DrawCmd> for Command {
	fn from(cmd: DrawCmd) -> Command {
		Command::Draw(cmd)
	}
}



pub struct FrameState {
	transient_data: bumpalo::Bump,
	pub commands: Vec<Command>,
}

impl FrameState {
	pub fn new() -> Self {
		FrameState {
			transient_data: bumpalo::Bump::with_capacity(UPLOAD_BUFFER_SIZE as usize),
			commands: Vec::new(),
		}
	}

	pub fn reset(&mut self) {
		self.commands.clear();
		self.transient_data.reset();
	}

	pub fn stream_buffer<T>(&mut self, data: &[T]) -> *mut StreamedBuffer
		where T: Copy
	{
		let data_copy = self.transient_data.alloc_slice_copy(data);
		self.transient_data.alloc(StreamedBuffer::Pending {
			data: data_copy.as_ptr().cast(),
			size: data_copy.len() * std::mem::size_of::<T>(),
		})
	}

	pub fn push_cmd(&mut self, cmd: impl Into<Command>) {
		self.commands.push(cmd.into());
	}
}