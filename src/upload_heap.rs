

pub const UPLOAD_BUFFER_SIZE: usize = 1<<15;

#[derive(Debug)]
pub struct UploadHeap {
	buffer_name: u32,
	buffer_cursor: usize,
	data_pushed_counter: usize,
	buffer_usage_counter: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct BufferAllocation {
	pub offset: usize,
	pub size: usize,
}

impl UploadHeap {
	pub fn new() -> UploadHeap {
		let mut buffer_name = 0;
		unsafe {
			gl::CreateBuffers(1, &mut buffer_name);

			let flags = /*gl::MAP_PERSISTENT_BIT |*/ gl::MAP_WRITE_BIT;
			gl::NamedBufferStorage(buffer_name, UPLOAD_BUFFER_SIZE as isize, std::ptr::null(), flags);

			let debug_label = "Upload Heap";
			gl::ObjectLabel(gl::BUFFER, buffer_name, debug_label.len() as i32, debug_label.as_ptr() as *const _);
		}

		UploadHeap {
			buffer_name,
			buffer_cursor: 0,
			data_pushed_counter: 0,
			buffer_usage_counter: 0,
		}
	}

	pub fn reset(&mut self) {
		if self.buffer_usage_counter >= UPLOAD_BUFFER_SIZE {
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
			let access = gl::MAP_WRITE_BIT
				| gl::MAP_UNSYNCHRONIZED_BIT
				| gl::MAP_INVALIDATE_RANGE_BIT;

			// TODO(pat.m): map less
			let ptr = gl::MapNamedBufferRange(self.buffer_name, allocation.offset as isize, allocation.size as isize, access);

			std::ptr::copy(data.as_ptr(), ptr as *mut T, data.len());

			gl::UnmapNamedBuffer(self.buffer_name);
		}

		self.data_pushed_counter += byte_size;

		allocation
	}
}