#![feature(let_chains)]

mod main_loop;
mod resource_manager;
mod commands;
mod context;
mod upload_heap;

use common::math::*;
use resource_manager::*;
use commands::*;
use context::*;


fn main() -> anyhow::Result<()> {
	std::env::set_var("RUST_BACKTRACE", "1");

	main_loop::run(Game::new)
}




struct Game {
	context: Context,
	frame_state: FrameState,

	vert_shader: ShaderHandle,
	vert_indexed_shader: ShaderHandle,
	frag_shader: ShaderHandle,

	gen_args_compute_shader: ShaderHandle,
	gen_color_compute_shader: ShaderHandle,

	time: f32,
}

impl Game {
	fn new() -> anyhow::Result<Self> {
		let mut context = Context::new()?;
		let frame_state = FrameState::new();

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

		let gen_args_compute_shader_def = ShaderDef {
			path: ResourcePath::from("shaders/gen_args.cs.glsl"),
			shader_type: ShaderType::Compute,
		};

		let gen_color_compute_shader_def = ShaderDef {
			path: ResourcePath::from("shaders/gen_color.cs.glsl"),
			shader_type: ShaderType::Compute,
		};

		let vert_shader = context.resource_manager.load_shader(&vert_shader_def)?;
		let vert_indexed_shader = context.resource_manager.load_shader(&vert_indexed_shader_def)?;
		let frag_shader = context.resource_manager.load_shader(&frag_shader_def)?;

		let gen_args_compute_shader = context.resource_manager.load_shader(&gen_args_compute_shader_def)?;
		let gen_color_compute_shader = context.resource_manager.load_shader(&gen_color_compute_shader_def)?;

		unsafe {
			gl::Enable(gl::DEPTH_TEST);
		}

		Ok(Game {
			context,
			frame_state,

			vert_shader,
			vert_indexed_shader,
			frag_shader,

			gen_args_compute_shader,
			gen_color_compute_shader,

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

		let proj_view_buffer = self.frame_state.stream_buffer(&[projection_view]);
		let quad_index_buffer = self.frame_state.stream_buffer(&[0u32, 1, 2, 0, 2, 3]);

		let args_buffer = self.frame_state.reserve_buffer(std::mem::size_of::<[u32; 3]>());
		let colour_buffer = self.frame_state.reserve_buffer(std::mem::size_of::<[f32; 4]>());

		self.frame_state.dispatch(self.gen_args_compute_shader)
			.groups(1, 1, 1)
			.buffer("ArgsBuffer", args_buffer);

		self.frame_state.memory_barrier();

		self.frame_state.dispatch(self.gen_color_compute_shader)
			.indirect(args_buffer)
			.buffer("ColorBuffer", colour_buffer);

		self.frame_state.memory_barrier();

		{
			let vertex_buffer = [
				[-0.5, -0.5, 0.0, 1.0f32],
				[-0.5,  0.5, 0.0, 1.0],
				[ 0.0,  0.5, 0.0, 1.0],
				[ 0.0, -0.5, 0.0, 1.0],
			];


			self.frame_state.draw(self.vert_shader, self.frag_shader)
				.elements(6)
				.instances(1)
				.ubo(0, proj_view_buffer)
				.buffer("PerDrawUniforms", colour_buffer) 
				.buffer("Positions", &vertex_buffer)
				.buffer(BindingLocation::Ssbo(1), quad_index_buffer);
		}

		{
			let vertex_buffer = [
				[-0.2, -0.2, -0.1, 1.0f32],
				[-0.2,  0.2, -0.1, 1.0],
				[ 0.2,  0.2, -0.1, 1.0],
				[ 0.2, -0.2, -0.1, 1.0],
			];

			let colour_data = [
				[1.0, 1.0, 0.5, 1.0f32],
				[1.0, 0.7, 1.0, 1.0f32],
			];

			self.frame_state.draw(self.vert_indexed_shader, self.frag_shader)
				.indexed(quad_index_buffer)
				.elements(6)
				.instances(2)
				.ubo(0, proj_view_buffer)
				.ssbo(0, &vertex_buffer)
				.ssbo(1, &colour_data);
		}

		self.context.end_frame(&mut self.frame_state);
	}
}



