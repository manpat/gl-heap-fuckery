use crate::resource_manager::*;
use crate::commands::{Command, FrameState, StreamedBuffer};


#[derive(Debug)]
pub struct Context {
	pub resource_manager: ResourceManager,

	// TODO(pat.m): pull these out into an UploadHeap
	upload_buffer_name: u32,
	upload_buffer_cursor: isize,
	data_pushed_counter: usize,

	vao_name: u32,

	uniform_buffer_offset_alignment: isize,
}

pub const UPLOAD_BUFFER_SIZE: isize = 1<<15;

impl Context {
	pub fn new() -> anyhow::Result<Self> {
		let resource_manager = ResourceManager::new()?;

		let mut upload_buffer_name = 0;
		unsafe {
			gl::CreateBuffers(1, &mut upload_buffer_name);

			let flags = /*gl::MAP_PERSISTENT_BIT |*/ gl::MAP_WRITE_BIT;
			gl::NamedBufferStorage(upload_buffer_name, UPLOAD_BUFFER_SIZE, std::ptr::null(), flags);
		}

		let mut uniform_buffer_offset_alignment = 0;

		unsafe {
			gl::GetIntegerv(gl::UNIFORM_BUFFER_OFFSET_ALIGNMENT, &mut uniform_buffer_offset_alignment)
		}

		let mut vao_name = 0;

		unsafe {
			gl::CreateVertexArrays(1, &mut vao_name);
			gl::BindVertexArray(vao_name);
		}

		Ok(Self{
			resource_manager,

			upload_buffer_name,
			upload_buffer_cursor: 0,
			data_pushed_counter: 0,

			vao_name,

			uniform_buffer_offset_alignment: uniform_buffer_offset_alignment as isize,
		})
	}

	pub fn start_frame(&mut self) {
		self.data_pushed_counter = 0;
	}

	pub fn end_frame(&mut self, frame_state: &mut FrameState) {
		// TODO(pat.m): non-ubo data could be interleaved with ubo data to save space

		let mut commands = std::mem::replace(&mut frame_state.commands, Vec::new());

		// Upload UBOs first since they have the greatest alignment requirements
		self.upload_ubo_data(&mut commands);
		self.upload_non_ubo_data(&mut commands);


		for cmd in commands {
			match cmd {
				Command::Draw(cmd) => {
					let pipeline_def = PipelineDef {
						vertex: Some(cmd.vertex_shader),
						fragment: cmd.fragment_shader,
						compute: None,
					};

					// Maybe this should be a LRU pool of pipelines instead of a created resource
					let pipeline_handle = self.resource_manager.create_pipeline(&pipeline_def)
						.unwrap();

					let name = self.resource_manager.resolve_pipeline_name(pipeline_handle).unwrap();

					unsafe {
						gl::BindProgramPipeline(name);
					}


					for &(index, buffer) in cmd.ubo_bindings.iter() {
						let &StreamedBuffer::Uploaded{offset, size, ..} = (unsafe {&*buffer}) else {
							panic!()
						};


						unsafe {
							gl::BindBufferRange(gl::UNIFORM_BUFFER, index, self.upload_buffer_name, offset, size as isize);
						}
					}

					for &(index, buffer) in cmd.ssbo_bindings.iter() {
						let &StreamedBuffer::Uploaded{offset, size, ..} = (unsafe {&*buffer}) else {
							panic!()
						};

						unsafe {
							gl::BindBufferRange(gl::SHADER_STORAGE_BUFFER, index, self.upload_buffer_name, offset, size as isize);
						}
					}

					if let Some(&StreamedBuffer::Uploaded{offset, ..}) = unsafe { cmd.index_buffer.as_ref() } {
						let offset_ptr = offset as usize as *const _;

						unsafe {
							gl::VertexArrayElementBuffer(self.vao_name, self.upload_buffer_name);
							gl::DrawElementsInstanced(gl::TRIANGLES, cmd.num_elements as i32, gl::UNSIGNED_SHORT,
								offset_ptr, cmd.num_instances as i32);
						}
					} else {
						unsafe {
							gl::DrawArraysInstanced(gl::TRIANGLES, 0, cmd.num_elements as i32, cmd.num_instances as i32);
						}
					}
				}
			}
		}

		// dbg!(self.data_pushed_counter);
		if self.data_pushed_counter >= UPLOAD_BUFFER_SIZE as usize {
			panic!("upload buffer overrun");
		}
	}

	fn upload_ubo_data(&mut self, commands: &mut [Command]) {
		for cmd in commands.iter_mut() {
			match cmd {
				Command::Draw(cmd) => {
					for (_, buffer) in cmd.ubo_bindings.iter_mut() {
						if let &StreamedBuffer::Pending{data, size} = unsafe {&**buffer} {
							let slice = unsafe{std::slice::from_raw_parts(data, size)};

							let offset = Self::push_data_inner(slice, self.uniform_buffer_offset_alignment,
								self.upload_buffer_name, &mut self.upload_buffer_cursor,
								&mut self.data_pushed_counter);

							unsafe {
								**buffer = StreamedBuffer::Uploaded {offset, size}
							}
						}
					}
				}
			}
		}
	}

	fn upload_non_ubo_data(&mut self, commands: &mut [Command]) {
		// SSBOs first
		for cmd in commands.iter_mut() {
			match cmd {
				Command::Draw(cmd) => {
					for (_, buffer) in cmd.ssbo_bindings.iter_mut() {
						if let &StreamedBuffer::Pending{data, size} = unsafe {&**buffer} {
							let slice = unsafe{std::slice::from_raw_parts(data, size)};

							let offset = Self::push_data_inner(slice, 32,
								self.upload_buffer_name, &mut self.upload_buffer_cursor,
								&mut self.data_pushed_counter);

							unsafe {
								**buffer = StreamedBuffer::Uploaded {offset, size}
							}
						}
					}
				}
			}
		}

		// Then index buffers
		for cmd in commands.iter_mut() {
			match cmd {
				Command::Draw(cmd) => {
					if let Some(buffer) = unsafe { cmd.index_buffer.as_mut() } {
						if let &mut StreamedBuffer::Pending{data, size} = buffer {
							let slice = unsafe{std::slice::from_raw_parts(data, size)};

							let offset = Self::push_data_inner(slice, 2,
								self.upload_buffer_name, &mut self.upload_buffer_cursor,
								&mut self.data_pushed_counter);

							*buffer = StreamedBuffer::Uploaded {offset, size}
						}
					}
				}
			}
		}
	}


	fn push_data_inner<T>(data: &[T], alignment: isize, upload_buffer_name: u32,
		upload_buffer_cursor: &mut isize, data_pushed_counter: &mut usize) -> isize
		where T: Copy
	{
		let byte_size = (data.len() * std::mem::size_of::<T>()) as isize;

		// Move to next alignment boundary
		let pre_alignment_cursor = *upload_buffer_cursor;
		*upload_buffer_cursor = (*upload_buffer_cursor + alignment - 1) & -alignment;

		let should_invalidate = *upload_buffer_cursor + byte_size > UPLOAD_BUFFER_SIZE;
		if should_invalidate {
			*upload_buffer_cursor = 0;
		}

		unsafe {
			let access = gl::MAP_WRITE_BIT
				| gl::MAP_UNSYNCHRONIZED_BIT
				| gl::MAP_INVALIDATE_RANGE_BIT;

			let ptr = gl::MapNamedBufferRange(upload_buffer_name, *upload_buffer_cursor, byte_size as isize, access);

			std::ptr::copy(data.as_ptr(), ptr as *mut T, data.len());

			gl::UnmapNamedBuffer(upload_buffer_name);
		}

		let offset = *upload_buffer_cursor;
		*upload_buffer_cursor += byte_size;
		*data_pushed_counter += (*upload_buffer_cursor as usize).checked_sub(pre_alignment_cursor as usize)
			.unwrap_or((byte_size + UPLOAD_BUFFER_SIZE - pre_alignment_cursor) as usize);

		offset
	}

	// pub fn bind_pipeline(&mut self, handle: PipelineHandle) {
	// 	let name = self.resource_manager.resolve_pipeline_name(handle).unwrap();

	// 	unsafe {
	// 		gl::BindProgramPipeline(name);
	// 	}
	// }

	// fn push_data<T>(&mut self, data: &[T], alignment: isize) -> isize
	// 	where T: Copy
	// {
	// 	Self::push_data_inner(data, alignment, self.upload_buffer_name, &mut self.upload_buffer_cursor,
	// 		&mut self.data_pushed_counter)
	// }

	// pub fn push_ssbo<T>(&mut self, index: u32, data: &[T])
	// 	where T: Copy
	// {
	// 	let offset_bytes = self.push_data(data, 32);
	// 	let size_bytes = (data.len() * std::mem::size_of::<T>()) as isize;

	// 	unsafe {
	// 		gl::BindBufferRange(gl::SHADER_STORAGE_BUFFER, index, self.upload_buffer_name, offset_bytes, size_bytes);
	// 	}
	// }

	// pub fn push_ubo<T>(&mut self, index: u32, data: &[T])
	// 	where T: Copy
	// {
	// 	let offset_bytes = self.push_data(data, self.uniform_buffer_offset_alignment);
	// 	let size_bytes = (data.len() * std::mem::size_of::<T>()) as isize;

	// 	unsafe {
	// 		gl::BindBufferRange(gl::UNIFORM_BUFFER, index, self.upload_buffer_name, offset_bytes, size_bytes);
	// 	}
	// }

	// pub fn draw(&mut self, vertex_count: u32, instance_count: u32) {
	// 	unsafe {
	// 		gl::DrawArraysInstanced(gl::TRIANGLES, 0, vertex_count as i32, instance_count as i32);
	// 	}
	// }

	// pub fn draw_indexed(&mut self, indices: &[u16], instance_count: u32) {
	// 	let offset_bytes = self.push_data(indices, 2);
	// 	let offset_ptr = offset_bytes as usize as *const _;

	// 	unsafe {
	// 		gl::VertexArrayElementBuffer(self.vao_name, self.upload_buffer_name);
	// 		gl::DrawElementsInstanced(gl::TRIANGLES, indices.len() as _, gl::UNSIGNED_SHORT, offset_ptr, instance_count as i32);
	// 	}
	// }
}