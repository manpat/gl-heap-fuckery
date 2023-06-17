use crate::resource_manager::*;
use crate::commands::{self, Command, FrameState, BufferHandle};
use crate::upload_heap::UploadHeap;
use common::math::{Vec2i, Vec3i};



pub const SSBO_ALIGNMENT: usize = 32;


#[derive(Debug)]
pub struct Context {
	pub resource_manager: ResourceManager,
	pub upload_heap: UploadHeap,

	vao_name: u32,

	uniform_buffer_offset_alignment: usize,

	in_flight_queries: Vec<InFlightQuery>,
	query_pool: Vec<u32>,
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

			in_flight_queries: Vec::new(),
			query_pool: Vec::new(),
		})
	}

	pub fn start_frame(&mut self) {
		self.upload_heap.reset();
	}

	pub fn end_frame(&mut self, frame_state: &mut FrameState) {
		use crate::commands::{BlockBinding, DispatchSizeSource, ImageBindingLocation};

		// TODO(pat.m): non-ubo data could be interleaved with ubo data to save space

		// let mut commands = std::mem::replace(&mut frame_state.commands, Vec::new());

		let FrameState { passes, allocator } = frame_state;


		// Resolve named buffer block bindings
		for cmd in passes.iter_mut().flat_map(|pass| pass.commands.iter_mut()) {
			let Some(pipeline_def) = cmd.pipeline_def() else { continue };
			let pipeline = self.resource_manager.get_pipeline(&pipeline_def).unwrap();

			if let Some(block_bindings) = cmd.block_bindings_mut() {
				for (binding, _) in block_bindings {
					if let BlockBinding::Named(name) = binding {
						let block = pipeline.block_by_name(*name)
							.expect(&format!("Couldn't find block binding with name '{name}'"));
						*binding = BlockBinding::Explicit(block.binding_location);
					}
				}
			};

			if let Some(image_bindings) = cmd.image_bindings_mut() {
				for binding in image_bindings.iter_mut() {
					let ImageBindingLocation::Named(name) = binding.location() else { continue };
					let unit = pipeline.image_binding_by_name(name)
						.expect(&format!("Couldn't find image binding with name '{name}'"));

					binding.set_location(ImageBindingLocation::Explicit(unit));
				}
			}
		}

		// Determine required alignment for bound buffer
		for cmd in passes.iter().flat_map(|pass| pass.commands.iter()) {
			if let Some(block_bindings) = cmd.block_bindings() {
				for (binding, buffer) in block_bindings {
					let BlockBinding::Explicit(location) = binding else { continue };

					let requested_alignment = match location {
						BlockBindingLocation::Ubo(_) => self.uniform_buffer_offset_alignment,
						BlockBindingLocation::Ssbo(_) => SSBO_ALIGNMENT,
					};

					allocator.imbue_buffer_alignment(*buffer, requested_alignment);
				}
			}

			match cmd {
				Command::Draw(commands::DrawCmd{ index_buffer: Some(buffer), .. }) 
					| Command::Dispatch(commands::DispatchCmd{ num_groups: DispatchSizeSource::Indirect(buffer), .. }) =>
				{
					allocator.imbue_buffer_alignment(*buffer, 4);
				}

				_ => {}
			}
		}



		allocator.upload_buffers(&mut self.upload_heap);

		let upload_buffer_name = self.upload_heap.buffer_name();

		let mut barrier_tracker = ResourceBarrierTracker::new();

		for pass in passes.iter() {
			unsafe {
				let msg = format!("pass: {}", pass.name);
				gl::PushDebugGroup(gl::DEBUG_SOURCE_APPLICATION, 0, msg.len() as i32, msg.as_ptr() as *const _);
			}

			let fbo = self.resource_manager.get_fbo(&pass.fbo_def).unwrap();

			// TODO(pat.m): insert barriers for any dirty images used as fbo attachments
			unsafe {
				gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, fbo.name);

				let Vec2i{x, y} = fbo.viewport_size;
				gl::Viewport(0, 0, x, y);

				// TODO(pat.m): should be option! some passes may want to preserve contents!
				if fbo.name != 0 {
					gl::Clear(gl::COLOR_BUFFER_BIT|gl::DEPTH_BUFFER_BIT|gl::STENCIL_BUFFER_BIT);
				}
			}

			let mut timer_query = None;

			if pass.wants_timer_query {
				let mut gl_name = 0;

				if let Some(pooled_name) = self.query_pool.pop() {
					gl_name = pooled_name;
				} else {
					unsafe {
						gl::GenQueries(1, &mut gl_name);
					}
				}

				unsafe {
					gl::BeginQuery(gl::TIME_ELAPSED, gl_name);
				}

				timer_query = Some(InFlightQuery {
					gl_name,
					pass_name: pass.name.clone(),
				});
			}

			for cmd in pass.commands.iter() {
				use crate::upload_heap::BufferAllocation;

				// Lookup and bind pipeline
				let pipeline = cmd.pipeline_def()
					.map(|def| self.resource_manager.get_pipeline(&def).unwrap());

				if let Some(pipeline) = pipeline {
					unsafe {
						gl::BindProgramPipeline(pipeline.name);
					}

					// Bind buffers
					if let Some(bindings) = cmd.block_bindings() {
						for &(block_binding, buffer) in bindings {
							let binding_location = match block_binding {
								BlockBinding::Explicit(location) => location,
								BlockBinding::Named(name) => {
									panic!("Unresolved named binding '{name}'");
								}
							};

							let (index, ty, barrier_bit) = match binding_location {
								BlockBindingLocation::Ubo(index) => (index, gl::UNIFORM_BUFFER, gl::UNIFORM_BARRIER_BIT),
								BlockBindingLocation::Ssbo(index) => (index, gl::SHADER_STORAGE_BUFFER, gl::SHADER_STORAGE_BARRIER_BIT),
							};

							barrier_tracker.insert_barrier(buffer, barrier_bit);

							let block = pipeline.block_by_binding_location(binding_location).unwrap();
							if block.is_read_write {
								barrier_tracker.mark_resource(buffer);
							}

							let BufferAllocation{offset, size} = allocator.resolve_buffer_allocation(buffer);

							unsafe {
								gl::BindBufferRange(ty, index, upload_buffer_name, offset as isize, size as isize);
							}
						}
					}

					// Bind textures and images
					if let Some(bindings) = cmd.image_bindings() {
						use crate::commands::ImageBinding;

						for binding in bindings {
							let image_handle = binding.image_handle();
							let image = self.resource_manager.resolve_image(image_handle)
								.expect("Failed to resolve image handle - probably use after delete");

							let image_name = image.name;
							let image_format = image.format;

							match binding {
								ImageBinding::Texture{sampler, location: ImageBindingLocation::Explicit(unit), ..} => {
									let sampler_name = self.resource_manager.get_sampler(sampler).name;

									barrier_tracker.insert_barrier(image_handle, gl::TEXTURE_FETCH_BARRIER_BIT);

									unsafe {
										gl::BindTextureUnit(*unit, image_name);
										gl::BindSampler(*unit, sampler_name);
									}
								}

								ImageBinding::Image{read_write, location: ImageBindingLocation::Explicit(unit), ..} => {
									barrier_tracker.insert_barrier(image_handle, gl::SHADER_IMAGE_ACCESS_BARRIER_BIT);

									if *read_write {
										barrier_tracker.mark_resource(image_handle);
									}

									let (level, layered, layer) = (0, 0, 0);
									let access_flags = match read_write {
										true => gl::READ_WRITE,
										false => gl::READ_ONLY,
									};

									unsafe {
										gl::BindImageTexture(*unit, image_name, level, layered, layer, access_flags, image_format);
									}
								}

								_ => unimplemented!(),
							}
						}
					}
				}



				// Bind command specific state and execute
				match cmd {
					Command::Draw(cmd) => {
						if let Some(buffer) = cmd.index_buffer {
							let BufferAllocation{offset, ..} = allocator.resolve_buffer_allocation(buffer);
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
								let BufferAllocation{offset, ..} = allocator.resolve_buffer_allocation(buffer);

								barrier_tracker.insert_barrier(buffer, gl::COMMAND_BARRIER_BIT);

								unsafe {
									gl::BindBuffer(gl::DISPATCH_INDIRECT_BUFFER, upload_buffer_name);
									gl::DispatchComputeIndirect(offset as isize);
								}
							}

							DispatchSizeSource::Explicit(Vec3i{x, y, z}) => unsafe {
								gl::DispatchCompute(x as u32, y as u32, z as u32);
							}
						}
					}
				}
			}

			if let Some(query) = timer_query {
				unsafe {
					gl::EndQuery(gl::TIME_ELAPSED);
				}

				self.in_flight_queries.push(query);
			}

			unsafe {
				gl::PopDebugGroup();
			}
		}

		self.upload_heap.notify_finished();
		frame_state.reset();

		self.process_queries();
	}


	fn process_queries(&mut self) {
		for query in &mut self.in_flight_queries {
			let mut ready = 0;
			let mut nanos = 0;
			unsafe {
				gl::GetQueryObjectiv(query.gl_name, gl::QUERY_RESULT_AVAILABLE, &mut ready);

				if ready != 0 {
					gl::GetQueryObjectui64v(query.gl_name, gl::QUERY_RESULT, &mut nanos);

					self.query_pool.push(query.gl_name);
					query.gl_name = 0;
				}
			}

			if ready != 0 {
				let micros = std::time::Duration::from_nanos(nanos).as_micros();
				println!("pass '{}' took {:4.0}us", query.pass_name, micros);
			}
		}

		self.in_flight_queries.retain(|query| query.gl_name != 0);
	}
}



use std::collections::HashMap;

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
enum TrackerHandle {
	Buffer(BufferHandle),
	Image(ImageHandle),
}

impl From<BufferHandle> for TrackerHandle {
	fn from(o: BufferHandle) -> Self {
		TrackerHandle::Buffer(o)
	}
}

impl From<ImageHandle> for TrackerHandle {
	fn from(o: ImageHandle) -> Self {
		TrackerHandle::Image(o)
	}
}


#[derive(Debug, Default)]
struct ResourceBarrierTracker {
	buffers: HashMap<TrackerHandle, bool>,
}

impl ResourceBarrierTracker {
	fn new() -> Self {
		Self::default()
	}

	fn mark_resource(&mut self, handle: impl Into<TrackerHandle>) {
		self.buffers.insert(handle.into(), true);
	}

	fn insert_barrier(&mut self, handle: impl Into<TrackerHandle>, barrier_bits: u32) {
		let should_insert = self.buffers.insert(handle.into(), false)
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



#[derive(Debug)]
struct InFlightQuery {
	pass_name: String,
	gl_name: u32,
}