

pub const UPLOAD_BUFFER_SIZE: usize = 1<<15;
// pub const UPLOAD_BUFFER_SIZE: usize = 580;

#[derive(Debug)]
pub struct UploadHeap {
	buffer_name: u32,
	buffer_cursor: usize,
	data_pushed_counter: usize,
	buffer_usage_counter: usize,

	buffer_ptr: *mut u8,

	frame_start_cursor: usize,
	locked_ranges: Vec<LockedRange>,
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

			let create_flags = gl::MAP_PERSISTENT_BIT | gl::MAP_COHERENT_BIT | gl::MAP_WRITE_BIT;
			gl::NamedBufferStorage(buffer_name, UPLOAD_BUFFER_SIZE as isize, std::ptr::null(), create_flags);

			let map_flags = gl::MAP_PERSISTENT_BIT | gl::MAP_COHERENT_BIT | gl::MAP_WRITE_BIT;
			buffer_ptr = gl::MapNamedBufferRange(buffer_name, 0, UPLOAD_BUFFER_SIZE as isize, map_flags) as *mut u8;

			let debug_label = "Upload Heap";
			gl::ObjectLabel(gl::BUFFER, buffer_name, debug_label.len() as i32, debug_label.as_ptr() as *const _);
		}

		UploadHeap {
			buffer_name,
			buffer_cursor: 0,
			data_pushed_counter: 0,
			buffer_usage_counter: 0,

			buffer_ptr,

			frame_start_cursor: 0,
			locked_ranges: Vec::new(),
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

		let allocation = BufferAllocation {
			offset,
			size,
		};

		// Check if we need to wait for the earliest range to be used.
		while let Some(locked_range) = self.locked_ranges.first()
			&& locked_range.contains_allocation(&allocation)
		{
			let range = self.locked_ranges.remove(0);

			unsafe {
				// Eager check to see if the fence has already been signaled
				let result = gl::ClientWaitSync(range.fence, gl::SYNC_FLUSH_COMMANDS_BIT, 0);
				if result != gl::ALREADY_SIGNALED && result != gl::CONDITION_SATISFIED {
					print!("Upload heap sync");

					// wait in blocks of 0.1ms
					let timeout_ns = 100_000;

					while let result = gl::ClientWaitSync(range.fence, gl::SYNC_FLUSH_COMMANDS_BIT, timeout_ns)
						&& result != gl::ALREADY_SIGNALED && result != gl::CONDITION_SATISFIED
					{
						print!(".");
					}

					println!("!");
				}

				gl::DeleteSync(range.fence);
			}
		}

		allocation
	}

	pub fn push_data<T>(&mut self, data: &[T], alignment: usize) -> BufferAllocation
		where T: Copy
	{
		let byte_size = data.len() * std::mem::size_of::<T>();
		let allocation = self.reserve_space(byte_size, alignment);

		unsafe {
			let dest_ptr = self.buffer_ptr.offset(allocation.offset as isize);
			std::ptr::copy(data.as_ptr(), dest_ptr.cast(), data.len());
		}

		self.data_pushed_counter += byte_size;

		allocation
	}

	pub fn notify_finished(&mut self) {
		let fence = unsafe {
			gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0)
		};

		let range_size = self.buffer_cursor.checked_sub(self.frame_start_cursor)
			.unwrap_or(UPLOAD_BUFFER_SIZE - self.frame_start_cursor + self.buffer_cursor);

		self.locked_ranges.push(LockedRange {
			fence,
			start: self.frame_start_cursor,
			size: range_size,
		});

		self.frame_start_cursor = self.buffer_cursor;
	}
}





#[derive(Debug)]
struct LockedRange {
	fence: gl::types::GLsync,

	start: usize,
	size: usize, // NOTE: may wrap
}

impl LockedRange {
	fn contains_allocation(&self, allocation: &BufferAllocation) -> bool {
		let allocation_end = allocation.offset + allocation.size;
		let range_end = self.start + self.size;

		if range_end <= UPLOAD_BUFFER_SIZE {
			allocation.offset < range_end && allocation_end >= self.start
		} else {
			allocation.offset >= self.start || allocation_end < (range_end - UPLOAD_BUFFER_SIZE)
		}
	}
}