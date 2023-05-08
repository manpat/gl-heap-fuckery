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
		use crate::commands::BlockBinding;

		// TODO(pat.m): non-ubo data could be interleaved with ubo data to save space

		let mut commands = std::mem::replace(&mut frame_state.commands, Vec::new());

		// Resolve named buffer block bindings
		for cmd in commands.iter_mut() {
			match cmd {
				Command::Draw(cmd) => {
					for (binding, _) in cmd.block_bindings.iter_mut() {
						if let BlockBinding::Named(name) = binding {
							let pipeline_def = PipelineDef {
								vertex: Some(cmd.vertex_shader),
								fragment: cmd.fragment_shader,
								compute: None,
							};

							let pipeline = self.resource_manager.get_pipeline(&pipeline_def).unwrap();
							let block = pipeline.composite_blocks.get(*name).unwrap();

							*binding = BlockBinding::Explicit(block.binding_location);
						}
					}
				}
			}
		}

		// Upload UBOs first since they have the greatest alignment requirements
		for cmd in commands.iter() {
			match cmd {
				Command::Draw(cmd) => {
					for (binding, buffer) in cmd.block_bindings.iter() {
						if let BlockBinding::Explicit(BindingLocation::Ubo(_)) = binding {
							frame_state.mark_ubo(*buffer);
						}
					}
				}
			}
		}

		for cmd in commands.iter() {
			match cmd {
				Command::Draw(cmd) => {
					for (binding, buffer) in cmd.block_bindings.iter() {
						if let BlockBinding::Explicit(BindingLocation::Ssbo(_)) = binding {
							frame_state.mark_ssbo(*buffer);
						}
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
					let pipeline = self.resource_manager.get_pipeline(&pipeline_def).unwrap();

					unsafe {
						gl::BindProgramPipeline(pipeline.name);
					}


					for &(block_binding, buffer) in cmd.block_bindings.iter() {
						let binding_location = match block_binding {
							BlockBinding::Explicit(location) => location,
							BlockBinding::Named(name) => {
								panic!("Unresolved named binding '{name}'");
							}
						};

						let (index, ty) = match binding_location {
							BindingLocation::Ubo(index) => (index, gl::UNIFORM_BUFFER),
							BindingLocation::Ssbo(index) => (index, gl::SHADER_STORAGE_BUFFER),
						};

						let BufferAllocation{offset, size} = frame_state.resolve_buffer_allocation(buffer);

						unsafe {
							gl::BindBufferRange(ty, index, upload_buffer_name, offset as isize, size as isize);
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
}