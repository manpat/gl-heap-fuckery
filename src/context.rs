use crate::resource_manager::*;
use crate::commands::{Command, FrameState, BufferHandle};
use crate::upload_heap::UploadHeap;


pub const SSBO_ALIGNMENT: usize = 32;


#[derive(Debug)]
pub struct Context {
	pub resource_manager: ResourceManager,
	pub upload_heap: UploadHeap,

	vao_name: u32,

	uniform_buffer_offset_alignment: usize,
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

		let mut uniform_buffer_offset_alignment = 0;

		unsafe {
			gl::GetIntegerv(gl::UNIFORM_BUFFER_OFFSET_ALIGNMENT, &mut uniform_buffer_offset_alignment)
		}


		Ok(Self{
			resource_manager,
			upload_heap,

			vao_name,

			uniform_buffer_offset_alignment: uniform_buffer_offset_alignment as usize,
		})
	}

	pub fn start_frame(&mut self) {
		self.upload_heap.reset();
	}

	pub fn end_frame(&mut self, frame_state: &mut FrameState) {
		use crate::commands::{BlockBinding, DispatchSizeSource};

		// TODO(pat.m): non-ubo data could be interleaved with ubo data to save space

		let mut commands = std::mem::replace(&mut frame_state.commands, Vec::new());

		// Resolve named buffer block bindings
		for cmd in commands.iter_mut() {
			let (block_bindings, pipeline_def) = match cmd {
				Command::Draw(cmd) => {
					let pipeline_def = PipelineDef {
						vertex: Some(cmd.vertex_shader),
						fragment: cmd.fragment_shader,
						.. PipelineDef::default()
					};

					(&mut cmd.block_bindings, pipeline_def)
				},

				Command::Dispatch(cmd) => {
					let pipeline_def = PipelineDef {
						compute: Some(cmd.compute_shader),
						.. PipelineDef::default()
					};

					(&mut cmd.block_bindings, pipeline_def)
				},
			};

			let pipeline = self.resource_manager.get_pipeline(&pipeline_def).unwrap();

			for (binding, _) in block_bindings.iter_mut() {
				if let BlockBinding::Named(name) = binding {
					let block = pipeline.block_by_name(*name).unwrap();
					*binding = BlockBinding::Explicit(block.binding_location);
				}
			}
		}

		// Upload UBOs first since they have the greatest alignment requirements
		for cmd in commands.iter() {
			let block_bindings = match cmd {
				Command::Draw(cmd) => &cmd.block_bindings,
				Command::Dispatch(cmd) => &cmd.block_bindings,
			};

			for (binding, buffer) in block_bindings.iter() {
				let BlockBinding::Explicit(location) = binding else { continue };

				let requested_alignment = match location {
					BindingLocation::Ubo(_) => self.uniform_buffer_offset_alignment,
					BindingLocation::Ssbo(_) => SSBO_ALIGNMENT,
				};

				frame_state.imbue_buffer_alignment(*buffer, requested_alignment);
			}
		}

		for cmd in commands.iter() {
			match cmd {
				Command::Draw(cmd) => {
					if let Some(buffer) = cmd.index_buffer {
						frame_state.imbue_buffer_alignment(buffer, 4);
					}
				}

				Command::Dispatch(cmd) => {
					if let DispatchSizeSource::Indirect(buffer) = cmd.num_groups {
						frame_state.imbue_buffer_alignment(buffer, 4);
					}
				}
			}
		}

		// unsafe {
		// 	let msg = "Frame Evaluate";
		// 	gl::PushDebugGroup(gl::DEBUG_SOURCE_APPLICATION, 0, msg.len() as i32, msg.as_ptr() as *const _);
		// }

		frame_state.upload_buffers(&mut self.upload_heap);

		let upload_buffer_name = self.upload_heap.buffer_name();

		let mut barrier_tracker = ResourceBarrierTracker::new();

		for cmd in commands {
			use crate::upload_heap::BufferAllocation;

			// Lookup and bind pipeline
			let pipeline_def = match &cmd {
				Command::Draw(cmd) => PipelineDef {
					vertex: Some(cmd.vertex_shader),
					fragment: cmd.fragment_shader,
					.. PipelineDef::default()
				},

				Command::Dispatch(cmd) => PipelineDef {
					compute: Some(cmd.compute_shader),
					.. PipelineDef::default()
				},
			};

			let pipeline = self.resource_manager.get_pipeline(&pipeline_def).unwrap();

			unsafe {
				gl::BindProgramPipeline(pipeline.name);
			}


			// Bind buffers
			let block_bindings = match &cmd {
				Command::Draw(cmd) => &cmd.block_bindings,
				Command::Dispatch(cmd) => &cmd.block_bindings,
			};

			for &(block_binding, buffer) in block_bindings {
				let binding_location = match block_binding {
					BlockBinding::Explicit(location) => location,
					BlockBinding::Named(name) => {
						panic!("Unresolved named binding '{name}'");
					}
				};

				let (index, ty, barrier_bit) = match binding_location {
					BindingLocation::Ubo(index) => (index, gl::UNIFORM_BUFFER, gl::UNIFORM_BARRIER_BIT),
					BindingLocation::Ssbo(index) => (index, gl::SHADER_STORAGE_BUFFER, gl::SHADER_STORAGE_BARRIER_BIT),
				};

				barrier_tracker.insert_barrier(buffer, barrier_bit);

				let block = pipeline.block_by_binding_location(binding_location).unwrap();
				if block.is_read_write {
					barrier_tracker.mark_buffer(buffer);
				}

				let BufferAllocation{offset, size} = frame_state.resolve_buffer_allocation(buffer);

				unsafe {
					gl::BindBufferRange(ty, index, upload_buffer_name, offset as isize, size as isize);
				}
			}


			// Bind command specific state and execute
			match cmd {
				Command::Draw(cmd) => {
					if let Some(buffer) = cmd.index_buffer {
						let BufferAllocation{offset, ..} = frame_state.resolve_buffer_allocation(buffer);
						let offset_ptr = offset as *const _;

						barrier_tracker.insert_barrier(buffer, gl::ELEMENT_ARRAY_BARRIER_BIT);

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

				Command::Dispatch(cmd) => {
					match cmd.num_groups {
						DispatchSizeSource::Indirect(buffer) => {
							let BufferAllocation{offset, ..} = frame_state.resolve_buffer_allocation(buffer);

							barrier_tracker.insert_barrier(buffer, gl::COMMAND_BARRIER_BIT);

							unsafe {
								gl::BindBuffer(gl::DISPATCH_INDIRECT_BUFFER, upload_buffer_name);
								gl::DispatchComputeIndirect(offset as isize);
							}
						}

						DispatchSizeSource::Explicit([x, y, z]) => unsafe {
							gl::DispatchCompute(x, y, z);
						}
					}
				}
			}
		}

		self.upload_heap.notify_finished();
		frame_state.reset();

		// unsafe {
		// 	gl::PopDebugGroup();
		// }
	}
}



use std::collections::HashMap;

#[derive(Debug, Default)]
struct ResourceBarrierTracker {
	buffers: HashMap<BufferHandle, bool>,
}

impl ResourceBarrierTracker {
	fn new() -> Self {
		Self::default()
	}

	fn mark_buffer(&mut self, handle: BufferHandle) {
		self.buffers.insert(handle, true);
	}

	fn insert_barrier(&mut self, handle: BufferHandle, barrier_bits: u32) {
		let should_insert = self.buffers.insert(handle, false)
			.unwrap_or(false);

		if !should_insert {
			return;
		}

		const BY_REGION_FLAGS: u32 = gl::SHADER_STORAGE_BARRIER_BIT | gl::UNIFORM_BARRIER_BIT
			| gl::FRAMEBUFFER_BARRIER_BIT | gl::ATOMIC_COUNTER_BARRIER_BIT
			| gl::SHADER_IMAGE_ACCESS_BARRIER_BIT | gl::TEXTURE_FETCH_BARRIER_BIT;

		if barrier_bits & BY_REGION_FLAGS == barrier_bits {
			unsafe {
				gl::MemoryBarrierByRegion(barrier_bits);
			}
		} else {
			unsafe {
				gl::MemoryBarrier(barrier_bits);
			}
		}
	}
}