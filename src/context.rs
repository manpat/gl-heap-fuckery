use crate::resource_manager::*;
use crate::commands::{Command, FrameState};
use crate::upload_heap::UploadHeap;


#[derive(Debug)]
pub struct Context {
	pub resource_manager: ResourceManager,
	pub upload_heap: UploadHeap,

	vao_name: u32,
}

impl Context {
	pub fn new() -> anyhow::Result<Self> {
		let resource_manager = ResourceManager::new()?;
		let upload_heap = UploadHeap::new();

		let mut vao_name = 0;

		unsafe {
			gl::CreateVertexArrays(1, &mut vao_name);
			gl::BindVertexArray(vao_name);
		}

		Ok(Self{
			resource_manager,
			upload_heap,

			vao_name,
		})
	}

	pub fn start_frame(&mut self) {
		self.upload_heap.reset();
	}

	pub fn end_frame(&mut self, frame_state: &mut FrameState) {
		// TODO(pat.m): non-ubo data could be interleaved with ubo data to save space

		let commands = std::mem::replace(&mut frame_state.commands, Vec::new());

		// Upload UBOs first since they have the greatest alignment requirements
		for cmd in commands.iter() {
			match cmd {
				Command::Draw(cmd) => {
					for (_, buffer) in cmd.ubo_bindings.iter() {
						frame_state.mark_ubo(*buffer);
					}
				}
			}
		}

		for cmd in commands.iter() {
			match cmd {
				Command::Draw(cmd) => {
					for (_, buffer) in cmd.ssbo_bindings.iter() {
						frame_state.mark_ssbo(*buffer);
					}
				}
			}
		}

		for cmd in commands.iter() {
			match cmd {
				Command::Draw(cmd) => {
					if let Some(buffer) = cmd.index_buffer {
						frame_state.mark_index_buffer(buffer);
					}
				}
			}
		}

		frame_state.upload_buffers(&mut self.upload_heap);

		let upload_buffer_name = self.upload_heap.buffer_name();

		for cmd in commands {
			match cmd {
				Command::Draw(cmd) => {
					use crate::upload_heap::BufferAllocation;

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
						let BufferAllocation{offset, size} = frame_state.resolve_buffer_allocation(buffer);

						unsafe {
							gl::BindBufferRange(gl::UNIFORM_BUFFER, index, upload_buffer_name, offset as isize, size as isize);
						}
					}

					for &(index, buffer) in cmd.ssbo_bindings.iter() {
						let BufferAllocation{offset, size} = frame_state.resolve_buffer_allocation(buffer);

						unsafe {
							gl::BindBufferRange(gl::SHADER_STORAGE_BUFFER, index, upload_buffer_name, offset as isize, size as isize);
						}
					}

					if let Some(buffer) = cmd.index_buffer {
						let BufferAllocation{offset, ..} = frame_state.resolve_buffer_allocation(buffer);
						let offset_ptr = offset as *const _;

						unsafe {
							gl::VertexArrayElementBuffer(self.vao_name, upload_buffer_name);
							gl::DrawElementsInstanced(gl::TRIANGLES, cmd.num_elements as i32, gl::UNSIGNED_INT,
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