mod main_loop;
use common::math::*;


fn main() -> anyhow::Result<()> {
	std::env::set_var("RUST_BACKTRACE", "1");

	main_loop::run(Game::new)
}




struct Game {
	context: Context,

	pipeline: PipelineHandle,
	indexed_pipeline: PipelineHandle,

	vert_shader: ShaderHandle,
	vert_indexed_shader: ShaderHandle,
	frag_shader: ShaderHandle,

	time: f32,
}

impl Game {
	fn new() -> anyhow::Result<Self> {
		let mut context = Context::new()?;

		let vert_shader_def = ShaderDef {
			path: ResourcePath::from("shaders/test.vert.glsl"),
			shader_type: ShaderType::Vertex,
		};

		let vert_indexed_shader_def = ShaderDef {
			path: ResourcePath::from("shaders/test_indexed.vert.glsl"),
			shader_type: ShaderType::Vertex,
		};

		let frag_shader_def = ShaderDef {
			path: ResourcePath::from("shaders/test.frag.glsl"),
			shader_type: ShaderType::Fragment,
		};

		let vert_shader = context.load_shader(&vert_shader_def)?;
		let vert_indexed_shader = context.load_shader(&vert_indexed_shader_def)?;
		let frag_shader = context.load_shader(&frag_shader_def)?;

		let pipeline_def = PipelineDef {
			vertex: Some(vert_shader),
			fragment: Some(frag_shader),
			compute: None,
		};

		let indexed_pipeline_def = PipelineDef {
			vertex: Some(vert_indexed_shader),
			fragment: Some(frag_shader),
			compute: None,
		};

		let pipeline = context.create_pipeline(&pipeline_def)?;
		let indexed_pipeline = context.create_pipeline(&indexed_pipeline_def)?;

		unsafe {
			gl::Enable(gl::DEPTH_TEST);
		}

		dbg!(&context);

		Ok(Game {
			context,
			pipeline,
			indexed_pipeline,

			vert_shader,
			vert_indexed_shader,
			frag_shader,

			time: 0.0
		})
	}
}

impl main_loop::MainLoop for Game {
	fn present(&mut self) {
		self.time += 1.0/60.0;

		self.context.start_frame();

		unsafe {
			gl::ClearColor(1.0, 0.5, 1.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT|gl::DEPTH_BUFFER_BIT);
		}


		let projection_view = Mat4::perspective(PI/3.0, 1.0, 0.01, 100.0)
			* Mat4::translate(Vec3::from_z(-2.0))
			* Mat4::rotate_y(self.time);

		self.context.bind_pipeline(self.pipeline);

		let proj_view_buffer = self.context.stream_buffer(&[projection_view]);


		{
			let vertex_buffer = self.context.stream_buffer(&[
				[-0.5, -0.5, 0.0, 1.0f32],
				[-0.5,  0.5, 0.0, 1.0],
				[ 0.5,  0.5, 0.0, 1.0],
				[ 0.5, -0.5, 0.0, 1.0],
			]);

			let index_buffer = self.context.stream_buffer(&[0u32, 1, 2, 0, 2, 3]);
			let colour_buffer = self.context.stream_buffer(&[0.5f32, 0.5, 1.0, 1.0]);

			// self.context.draw(6, 1);

			self.context.commands.push(Command::Draw(DrawCmd {
				vertex_shader: self.vert_shader,
				fragment_shader: Some(self.frag_shader),

				num_elements: 6,
				num_instances: 1,

				index_buffer: std::ptr::null_mut(),

				ubo_bindings: vec![
					(0, proj_view_buffer),
					(1, colour_buffer)
				],

				ssbo_bindings: vec![
					(0, vertex_buffer),
					(1, index_buffer)
				],
			}));
		}



		self.context.push_ssbo(0, &[
			[-0.2, -0.2, 0.5, 1.0f32],
			[ 0.0,  0.2, 0.5, 1.0],
			[ 0.2, -0.2, 0.5, 1.0],
		]);

		self.context.push_ssbo(1, &[0u32, 1, 2]);
		self.context.push_ubo(1, &[0.5f32, 1.0, 0.5, 1.0]);

		self.context.draw(3, 1);


		self.context.bind_pipeline(self.indexed_pipeline);
		self.context.push_ssbo(0, &[
			[-0.2, -0.2, -0.4, 1.0f32],
			[-0.2,  0.2, -0.4, 1.0],
			[ 0.2,  0.2, -0.4, 1.0],
			[ 0.2, -0.2, -0.4, 1.0],
		]);

		self.context.push_ssbo(1, &[1.0, 1.0, 0.5, 1.0f32]);
		self.context.draw_indexed(&[0, 1, 2], 1);

		self.context.push_ssbo(1, &[
			[0.5, 0.2, 0.5, 1.0f32],
			[0.5, 0.4, 0.2, 1.0f32],
			[0.2, 0.5, 0.5, 1.0f32],
		]);
		self.context.draw_indexed(&[0, 2, 3], 3);

		self.context.end_frame();
	}
}





type ResourcePath = std::path::PathBuf;
type ResourcePathRef = std::path::Path;


#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum ShaderType {
	Vertex = gl::VERTEX_SHADER,
	Fragment = gl::FRAGMENT_SHADER,
	Compute = gl::COMPUTE_SHADER,
}

#[derive(Hash, Clone, Debug, Eq, PartialEq)]
pub struct ShaderDef {
	pub path: ResourcePath,
	pub shader_type: ShaderType,
}

#[derive(Hash, Clone, Debug, Eq, PartialEq)]
pub struct PipelineDef {
	pub vertex: Option<ShaderHandle>,
	pub fragment: Option<ShaderHandle>,
	pub compute: Option<ShaderHandle>,
}


#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ShaderHandle(pub u32);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct PipelineHandle(pub u32);


use std::collections::HashMap;


#[derive(Debug)]
enum StreamedBuffer {
	Pending {
		data: *const u8,
		size: usize,
	},

	Uploaded {
		offset: isize,
		size: usize,
		is_ubo: bool,
	}
}

#[derive(Debug)]
struct DrawCmd {
	vertex_shader: ShaderHandle,
	fragment_shader: Option<ShaderHandle>,

	num_elements: u32,
	num_instances: u32,

	// If set, use indexed rendering
	index_buffer: *mut StreamedBuffer,

	ssbo_bindings: Vec<(u32, *mut StreamedBuffer)>,
	ubo_bindings: Vec<(u32, *mut StreamedBuffer)>,
}

#[derive(Debug)]
enum Command {
	Draw(DrawCmd),
}



#[derive(Debug)]
struct Context {
	resource_root_path: ResourcePath,

	shader_defs: HashMap<ShaderDef, ShaderHandle>,
	shader_names: HashMap<ShaderHandle, u32>,
	shader_counter: u32,

	pipeline_defs: HashMap<PipelineDef, PipelineHandle>,
	pipeline_names: HashMap<PipelineHandle, u32>,
	pipeline_counter: u32,

	frame_data: bumpalo::Bump,
	commands: Vec<Command>,

	upload_buffer_name: u32,
	upload_buffer_cursor: isize,
	data_pushed_counter: usize,

	vao_name: u32,

	uniform_buffer_offset_alignment: isize,
}

const UPLOAD_BUFFER_SIZE: isize = 1<<15;

impl Context {
	pub fn new() -> anyhow::Result<Self> {
		let resource_root_path = ResourcePath::from("resource");

		anyhow::ensure!(resource_root_path.exists(), "Couldn't find resource path");

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
			resource_root_path,

			shader_defs: HashMap::default(),
			shader_names: HashMap::default(),
			shader_counter: 0,

			pipeline_defs: HashMap::default(),
			pipeline_names: HashMap::default(),
			pipeline_counter: 0,

			frame_data: bumpalo::Bump::with_capacity(UPLOAD_BUFFER_SIZE as usize),
			commands: Vec::new(),

			upload_buffer_name,
			upload_buffer_cursor: 0,
			data_pushed_counter: 0,

			vao_name,

			uniform_buffer_offset_alignment: uniform_buffer_offset_alignment as isize,
		})
	}

	pub fn start_frame(&mut self) {
		self.data_pushed_counter = 0;

		self.commands.clear();
		self.frame_data.reset();
	}

	pub fn end_frame(&mut self) {
		// TODO(pat.m): non-ubo data could be interleaved with ubo data to save space

		// Upload UBOs first since they have the greatest alignment requirements
		self.upload_ubo_data();
		self.upload_non_ubo_data();

		let commands = std::mem::replace(&mut self.commands, Vec::new());

		for cmd in commands {
			match cmd {
				Command::Draw(cmd) => {
					let pipeline_def = PipelineDef {
						vertex: Some(cmd.vertex_shader),
						fragment: cmd.fragment_shader,
						compute: None,
					};

					let pipeline_handle = self.create_pipeline(&pipeline_def)
						.unwrap();

					self.bind_pipeline(pipeline_handle);


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

	pub fn stream_buffer<T>(&mut self, data: &[T]) -> *mut StreamedBuffer
		where T: Copy
	{
		let data_copy = self.frame_data.alloc_slice_copy(data);
		self.frame_data.alloc(StreamedBuffer::Pending {
			data: data_copy.as_ptr().cast(),
			size: data_copy.len() * std::mem::size_of::<T>(),
		})
	}

	fn upload_ubo_data(&mut self) {
		for cmd in self.commands.iter_mut() {
			match cmd {
				Command::Draw(cmd) => {
					for (_, buffer) in cmd.ubo_bindings.iter_mut() {
						if let &StreamedBuffer::Pending{data, size} = unsafe {&**buffer} {
							let slice = unsafe{std::slice::from_raw_parts(data, size)};

							let offset = Self::push_data_inner(slice, self.uniform_buffer_offset_alignment,
								self.upload_buffer_name, &mut self.upload_buffer_cursor,
								&mut self.data_pushed_counter);

							unsafe {
								**buffer = StreamedBuffer::Uploaded {offset, size, is_ubo: true}
							}
						}
					}
				}
			}
		}
	}

	fn upload_non_ubo_data(&mut self) {
		// SSBOs first
		for cmd in self.commands.iter_mut() {
			match cmd {
				Command::Draw(cmd) => {
					for (_, buffer) in cmd.ssbo_bindings.iter_mut() {
						if let &StreamedBuffer::Pending{data, size} = unsafe {&**buffer} {
							let slice = unsafe{std::slice::from_raw_parts(data, size)};

							let offset = Self::push_data_inner(slice, 32,
								self.upload_buffer_name, &mut self.upload_buffer_cursor,
								&mut self.data_pushed_counter);

							unsafe {
								**buffer = StreamedBuffer::Uploaded {offset, size, is_ubo: false}
							}
						}
					}
				}
			}
		}

		// Then index buffers
		for cmd in self.commands.iter_mut() {
			match cmd {
				Command::Draw(cmd) => {
					if let Some(buffer) = unsafe { cmd.index_buffer.as_mut() } {
						if let &mut StreamedBuffer::Pending{data, size} = buffer {
							let slice = unsafe{std::slice::from_raw_parts(data, size)};

							let offset = Self::push_data_inner(slice, 2,
								self.upload_buffer_name, &mut self.upload_buffer_cursor,
								&mut self.data_pushed_counter);

							*buffer = StreamedBuffer::Uploaded {offset, size, is_ubo: false}
						}
					}
				}
			}
		}
	}

	pub fn load_text(&mut self, def: &ResourcePathRef) -> anyhow::Result<String> {
		let string = std::fs::read_to_string(&self.resource_root_path.join(def))?;
		Ok(string)
	}

	pub fn load_shader(&mut self, def: &ShaderDef) -> anyhow::Result<ShaderHandle> {
		if let Some(handle) = self.shader_defs.get(def) {
			return Ok(*handle);
		}

		let content = self.load_text(&def.path)?;

		let raw_handle;

		unsafe {
			let src_cstring = std::ffi::CString::new(content.as_bytes())?;
			let source_strings = [
				b"#version 450\n\0".as_ptr()  as *const i8,
				src_cstring.as_ptr(),
			];

			raw_handle = gl::CreateShaderProgramv(def.shader_type as u32, source_strings.len() as _, source_strings.as_ptr());

			if raw_handle == 0 {
				anyhow::bail!("Failed to create shader '{}'", def.path.display());
			}

			let mut status = 0;
			gl::GetProgramiv(raw_handle, gl::LINK_STATUS, &mut status);

			if status == 0 {
				let mut buf = [0u8; 1024];
				let mut len = 0;
				gl::GetProgramInfoLog(raw_handle, buf.len() as _, &mut len, buf.as_mut_ptr() as _);

				gl::DeleteProgram(raw_handle);

				let error = std::str::from_utf8(&buf[..len as usize])?;
				anyhow::bail!("Failed to create shader '{}':\n{}", def.path.display(), error);
			}
		}

		let handle = ShaderHandle(self.shader_counter);
		self.shader_counter += 1;

		self.shader_defs.insert(def.clone(), handle);
		self.shader_names.insert(handle, raw_handle);

		Ok(handle)
	}

	// TODO(pat.m): maybe I want to do away with fixed pipelines and just bind PipelineDefs instead
	pub fn create_pipeline(&mut self, def: &PipelineDef) -> anyhow::Result<PipelineHandle> {
		if let Some(handle) = self.pipeline_defs.get(def) {
			return Ok(*handle);
		}

		let mut raw_handle = 0;

		unsafe {
			gl::CreateProgramPipelines(1, &mut raw_handle);
			if raw_handle == 0 {
				anyhow::bail!("Failed to create pipeline pipeline");
			}

			if let Some(sh_handle) = def.vertex {
				let sh_name = self.shader_names[&sh_handle];
				gl::UseProgramStages(raw_handle, gl::VERTEX_SHADER_BIT, sh_name);
			}

			if let Some(sh_handle) = def.fragment {
				let sh_name = self.shader_names[&sh_handle];
				gl::UseProgramStages(raw_handle, gl::FRAGMENT_SHADER_BIT, sh_name);
			}

			if let Some(sh_handle) = def.compute {
				let sh_name = self.shader_names[&sh_handle];
				gl::UseProgramStages(raw_handle, gl::COMPUTE_SHADER_BIT, sh_name);
			}

			gl::ValidateProgramPipeline(raw_handle);
		}

		let handle = PipelineHandle(self.pipeline_counter);
		self.pipeline_counter += 1;

		self.pipeline_defs.insert(def.clone(), handle);
		self.pipeline_names.insert(handle, raw_handle);

		Ok(handle)
	}

	pub fn bind_pipeline(&mut self, handle: PipelineHandle) {
		let name = self.pipeline_names[&handle];

		unsafe {
			gl::BindProgramPipeline(name);
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

	fn push_data<T>(&mut self, data: &[T], alignment: isize) -> isize
		where T: Copy
	{
		Self::push_data_inner(data, alignment, self.upload_buffer_name, &mut self.upload_buffer_cursor,
			&mut self.data_pushed_counter)
	}

	pub fn push_ssbo<T>(&mut self, index: u32, data: &[T])
		where T: Copy
	{
		let offset_bytes = self.push_data(data, 32);
		let size_bytes = (data.len() * std::mem::size_of::<T>()) as isize;

		unsafe {
			gl::BindBufferRange(gl::SHADER_STORAGE_BUFFER, index, self.upload_buffer_name, offset_bytes, size_bytes);
		}
	}

	pub fn push_ubo<T>(&mut self, index: u32, data: &[T])
		where T: Copy
	{
		let offset_bytes = self.push_data(data, self.uniform_buffer_offset_alignment);
		let size_bytes = (data.len() * std::mem::size_of::<T>()) as isize;

		unsafe {
			gl::BindBufferRange(gl::UNIFORM_BUFFER, index, self.upload_buffer_name, offset_bytes, size_bytes);
		}
	}

	pub fn draw(&mut self, vertex_count: u32, instance_count: u32) {
		unsafe {
			gl::DrawArraysInstanced(gl::TRIANGLES, 0, vertex_count as i32, instance_count as i32);
		}
	}

	pub fn draw_indexed(&mut self, indices: &[u16], instance_count: u32) {
		let offset_bytes = self.push_data(indices, 2);
		let offset_ptr = offset_bytes as usize as *const _;

		unsafe {
			gl::VertexArrayElementBuffer(self.vao_name, self.upload_buffer_name);
			gl::DrawElementsInstanced(gl::TRIANGLES, indices.len() as _, gl::UNSIGNED_SHORT, offset_ptr, instance_count as i32);
		}
	}
}


