mod main_loop;
mod resource_manager;
mod commands;
mod context;

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

	// pipeline: PipelineHandle,
	// indexed_pipeline: PipelineHandle,

	vert_shader: ShaderHandle,
	vert_indexed_shader: ShaderHandle,
	frag_shader: ShaderHandle,

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

		let vert_shader = context.resource_manager.load_shader(&vert_shader_def)?;
		let vert_indexed_shader = context.resource_manager.load_shader(&vert_indexed_shader_def)?;
		let frag_shader = context.resource_manager.load_shader(&frag_shader_def)?;

		// let pipeline_def = PipelineDef {
		// 	vertex: Some(vert_shader),
		// 	fragment: Some(frag_shader),
		// 	compute: None,
		// };

		// let indexed_pipeline_def = PipelineDef {
		// 	vertex: Some(vert_indexed_shader),
		// 	fragment: Some(frag_shader),
		// 	compute: None,
		// };

		// let pipeline = context.create_pipeline(&pipeline_def)?;
		// let indexed_pipeline = context.create_pipeline(&indexed_pipeline_def)?;

		unsafe {
			gl::Enable(gl::DEPTH_TEST);
		}

		dbg!(&context);

		Ok(Game {
			context,
			frame_state,

			// pipeline,
			// indexed_pipeline,

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

		self.frame_state.reset();
		self.context.start_frame();

		unsafe {
			gl::ClearColor(1.0, 0.5, 1.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT|gl::DEPTH_BUFFER_BIT);
		}

		// self.context.bind_pipeline(self.pipeline);

		let projection_view = Mat4::perspective(PI/3.0, 1.0, 0.01, 100.0)
			* Mat4::translate(Vec3::from_z(-2.0))
			* Mat4::rotate_y(self.time);


		let proj_view_buffer = self.frame_state.stream_buffer(&[projection_view]);


		{
			let vertex_buffer = self.frame_state.stream_buffer(&[
				[-0.5, -0.5, 0.0, 1.0f32],
				[-0.5,  0.5, 0.0, 1.0],
				[ 0.5,  0.5, 0.0, 1.0],
				[ 0.5, -0.5, 0.0, 1.0],
			]);

			let index_buffer = self.frame_state.stream_buffer(&[0u32, 1, 2, 0, 2, 3]);
			let colour_buffer = self.frame_state.stream_buffer(&[0.5f32, 0.5, 1.0, 1.0]);

			self.frame_state.push_cmd(DrawCmd {
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
			});
		}



		// self.context.push_ssbo(0, &[
		// 	[-0.2, -0.2, 0.5, 1.0f32],
		// 	[ 0.0,  0.2, 0.5, 1.0],
		// 	[ 0.2, -0.2, 0.5, 1.0],
		// ]);

		// self.context.push_ssbo(1, &[0u32, 1, 2]);
		// self.context.push_ubo(1, &[0.5f32, 1.0, 0.5, 1.0]);

		// self.context.draw(3, 1);


		// self.context.bind_pipeline(self.indexed_pipeline);
		// self.context.push_ssbo(0, &[
		// 	[-0.2, -0.2, -0.4, 1.0f32],
		// 	[-0.2,  0.2, -0.4, 1.0],
		// 	[ 0.2,  0.2, -0.4, 1.0],
		// 	[ 0.2, -0.2, -0.4, 1.0],
		// ]);

		// self.context.push_ssbo(1, &[1.0, 1.0, 0.5, 1.0f32]);
		// self.context.draw_indexed(&[0, 1, 2], 1);

		{
			let vertex_buffer = self.frame_state.stream_buffer(&[
				[-0.2, -0.2, -0.4, 1.0f32],
				[-0.2,  0.2, -0.4, 1.0],
				[ 0.2,  0.2, -0.4, 1.0],
				[ 0.2, -0.2, -0.4, 1.0],
			]);

			let index_buffer = self.frame_state.stream_buffer(&[0u16, 1, 2]);
			let colour_buffer = self.frame_state.stream_buffer(&[1.0, 1.0, 0.5, 1.0f32]);

			// self.context.draw(6, 1);

			self.frame_state.push_cmd(DrawCmd {
				vertex_shader: self.vert_indexed_shader,
				fragment_shader: Some(self.frag_shader),

				num_elements: 3,
				num_instances: 300,

				index_buffer,

				ubo_bindings: vec![
					(0, proj_view_buffer),
				],

				ssbo_bindings: vec![
					(0, vertex_buffer),
					(1, colour_buffer)
				],
			});
		}

		// self.context.bind_pipeline(self.indexed_pipeline);
		// self.context.push_ssbo(1, &[
		// 	[0.5, 0.2, 0.5, 1.0f32],
		// 	[0.5, 0.4, 0.2, 1.0f32],
		// 	[0.2, 0.5, 0.5, 1.0f32],
		// ]);
		// self.context.draw_indexed(&[0, 2, 3], 3);

		self.context.end_frame(&mut self.frame_state);
	}
}



