

pub const UPLOAD_BUFFER_SIZE: usize = 1<<15;
// pub const UPLOAD_BUFFER_SIZE: usize = 580;

#[derive(Debug)]
pub struct UploadHeap {
	buffer_name: u32,
	buffer_cursor: usize,
	data_pushed_counter: usize,
	buffer_usage_counter: usize,

	buffer_ptr: *mut u8,
	// buffer_invalidate_fence: Option<gl::types::GLsync>,
	// needs_fence: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct BufferAllocation {
	pub offset: usize,
	pub size: usize,
}

impl UploadHeap {
	pub fn new() -> UploadHeap {
		let mut buffer_name = 0;
		let buffer_ptr;

		unsafe {
			gl::CreateBuffers(1, &mut buffer_name);

			// Specifically not using  gl::DYNAMIC_STORAGE_BIT
			let flags = gl::MAP_PERSISTENT_BIT | gl::MAP_COHERENT_BIT | gl::MAP_WRITE_BIT;
			gl::NamedBufferStorage(buffer_name, UPLOAD_BUFFER_SIZE as isize, std::ptr::null(), flags);

			let map_flags = gl::MAP_PERSISTENT_BIT | gl::MAP_COHERENT_BIT | gl::MAP_WRITE_BIT;
			buffer_ptr = gl::MapNamedBufferRange(buffer_name, 0, UPLOAD_BUFFER_SIZE as isize, map_flags) as *mut u8;

			// TODO(pat.m): SYNCHRONISATION
			// This is a bit useless but will ensure buffer_invalidate_fence is always valid
			// buffer_invalidate_fence = gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);

			let debug_label = "Upload Heap";
			gl::ObjectLabel(gl::BUFFER, buffer_name, debug_label.len() as i32, debug_label.as_ptr() as *const _);
		}

		UploadHeap {
			buffer_name,
			buffer_cursor: 0,
			data_pushed_counter: 0,
			buffer_usage_counter: 0,

			buffer_ptr,
			// buffer_invalidate_fence: None,
			// needs_fence: true,
		}
	}

	pub fn reset(&mut self) {
		if self.buffer_usage_counter > UPLOAD_BUFFER_SIZE {
			dbg!(self.buffer_usage_counter);
			dbg!(self.data_pushed_counter);
			panic!("upload buffer overrun");
		}

		self.data_pushed_counter = 0;
		self.buffer_usage_counter = 0;
	}

	pub fn buffer_name(&self) -> u32 {
		self.buffer_name
	}

	pub fn reserve_space(&mut self, size: usize, alignment: usize) -> BufferAllocation {
		// Move to next alignment boundary
		let pre_alignment_cursor = self.buffer_cursor;
		self.buffer_cursor = (self.buffer_cursor + alignment - 1) & (!alignment + 1);

		let should_invalidate = self.buffer_cursor + size > UPLOAD_BUFFER_SIZE;
		if should_invalidate {
			self.buffer_cursor = 0;
		}

		let offset = self.buffer_cursor;
		self.buffer_cursor += size;

		// Keep track of total buffer usage - including alignment
		self.buffer_usage_counter += self.buffer_cursor.checked_sub(pre_alignment_cursor)
			.unwrap_or(size + UPLOAD_BUFFER_SIZE - pre_alignment_cursor);

		BufferAllocation {
			offset,
			size,
		}
	}

	pub fn push_data<T>(&mut self, data: &[T], alignment: usize) -> BufferAllocation
		where T: Copy
	{
		let byte_size = data.len() * std::mem::size_of::<T>();
		let allocation = self.reserve_space(byte_size, alignment);

		unsafe {
			// let access = gl::MAP_WRITE_BIT
			// 	| gl::MAP_UNSYNCHRONIZED_BIT
			// 	| gl::MAP_INVALIDATE_RANGE_BIT;

			// // TODO(pat.m): map less
			// let ptr = gl::MapNamedBufferRange(self.buffer_name, allocation.offset as isize, allocation.size as isize, access);

			let dest_ptr = self.buffer_ptr.offset(allocation.offset as isize);
			std::ptr::copy(data.as_ptr(), dest_ptr.cast(), data.len());

			// gl::UnmapNamedBuffer(self.buffer_name);
		}

		self.data_pushed_counter += byte_size;

		allocation
	}

	pub fn notify_finished(&mut self) {
		// if !self.needs_fence {
		// 	return;
		// }

		// self.needs_fence = false;

		// if let Some() = self.buffer_invalidate_fence
	}
}