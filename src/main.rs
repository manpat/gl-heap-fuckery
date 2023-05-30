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
	backbuffer_size: Vec2i,

	vert_shader: ShaderHandle,
	vert_indexed_shader: ShaderHandle,
	vert_sprite_shader: ShaderHandle,
	frag_shader: ShaderHandle,
	frag_textured_shader: ShaderHandle,

	gen_args_compute_shader: ShaderHandle,
	gen_color_compute_shader: ShaderHandle,

	coolcat_image: ImageHandle,
	render_target: ImageHandle,
	depth_stencil_image: ImageHandle,

	time: f32,
}

impl Game {
	fn new() -> anyhow::Result<Self> {
		let mut context = Context::new()?;
		let frame_state = FrameState::new();

		let vert_shader = context.resource_manager.load_shader(&ShaderDef::vertex("shaders/test.vert.glsl"))?;
		let vert_indexed_shader = context.resource_manager.load_shader(&ShaderDef::vertex("shaders/test_indexed.vert.glsl"))?;
		let vert_sprite_shader = context.resource_manager.load_shader(&ShaderDef::vertex("shaders/sprite.vert.glsl"))?;

		let frag_shader = context.resource_manager.load_shader(&ShaderDef::fragment("shaders/test.frag.glsl"))?;
		let frag_textured_shader = context.resource_manager.load_shader(&ShaderDef::fragment("shaders/textured.frag.glsl"))?;

		let gen_args_compute_shader = context.resource_manager.load_shader(&ShaderDef::compute("shaders/gen_args.cs.glsl"))?;
		let gen_color_compute_shader = context.resource_manager.load_shader(&ShaderDef::compute("shaders/gen_color.cs.glsl"))?;

		let coolcat_image = context.resource_manager.load_image(&ImageDef::new("images/coolcat.png"))?;

		let render_target = context.resource_manager.load_image(&ImageDef::RenderTarget)?;
		let depth_stencil_image = context.resource_manager.load_image(&ImageDef::DepthStencil)?;

		unsafe {
			gl::Enable(gl::DEPTH_TEST);
		}

		Ok(Game {
			context,
			frame_state,
			backbuffer_size: Vec2i::splat(1),

			vert_shader,
			vert_indexed_shader,
			vert_sprite_shader,

			frag_shader,
			frag_textured_shader,

			gen_args_compute_shader,
			gen_color_compute_shader,

			coolcat_image,
			render_target,
			depth_stencil_image,

			time: 0.0
		})
	}
}

impl main_loop::MainLoop for Game {
	fn present(&mut self) {
		self.time += 1.0/60.0;

		self.context.start_frame();

		unsafe {
			gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
			gl::Viewport(0, 0, self.backbuffer_size.x, self.backbuffer_size.y);

			// TODO(pat.m): these should probably be commands, or options on passes
			gl::ClearColor(1.0, 0.5, 1.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT|gl::DEPTH_BUFFER_BIT);

			gl::ClearColor(0.5, 1.0, 1.0, 1.0);
		}

		let aspect = self.backbuffer_size.x as f32 / self.backbuffer_size.y as f32;

		let fake_projection_view = Mat4::perspective(PI/3.0, 1.0, 0.01, 100.0)
			* Mat4::translate(Vec3::from_z(-2.0))
			* Mat4::rotate_y(self.time);

		let projection_view = Mat4::perspective(PI/3.0, aspect, 0.01, 100.0)
			* Mat4::translate(Vec3::from_z(-2.0))
			* Mat4::rotate_y(self.time);

		let proj_view_buffer = self.frame_state.stream_buffer(&[projection_view]);
		let fake_proj_view_buffer = self.frame_state.stream_buffer(&[fake_projection_view]);
		let quad_index_buffer = self.frame_state.stream_buffer(&[0u32, 1, 2, 0, 2, 3]);

		let args_buffer = self.frame_state.reserve_buffer(std::mem::size_of::<[u32; 3]>());
		let colour_buffer = self.frame_state.reserve_buffer(std::mem::size_of::<[f32; 4]>());

		let compute_pass = self.frame_state.pass("compute");
		let draw_pass = self.frame_state.pass_builder("draw")
			.color_attachment(0, self.render_target)
			.depth_stencil_attachment(self.depth_stencil_image)
			.handle();

		let final_draw_pass = self.frame_state.pass("draw");

		self.frame_state.dispatch(compute_pass, self.gen_args_compute_shader)
			.groups(1, 1, 1)
			.buffer("ArgsBuffer", args_buffer)
			.buffer("ColorBuffer", colour_buffer);

		{
			let vertex_buffer = [
				[-0.5, -0.5, 1.0, 1.0f32],
				[-0.5,  0.5, 1.0, 1.0],
				[ 0.0,  0.5, 1.0, 1.0],
				[ 0.0, -0.5, 1.0, 1.0],
			];


			self.frame_state.draw(draw_pass, self.vert_shader, self.frag_shader)
				.elements(6)
				.ubo(0, fake_proj_view_buffer)
				.buffer("PerDrawUniforms", colour_buffer)
				.buffer("Positions", &vertex_buffer)
				.buffer(BlockBindingLocation::Ssbo(1), quad_index_buffer);
		}

		{
			let vertex_buffer = [
				[-0.2, -0.2, 0.1, 1.0f32],
				[-0.2,  0.2, 0.1, 1.0],
				[ 0.2,  0.2, 0.1, 1.0],
				[ 0.2, -0.2, 0.1, 1.0],
			];

			let colour_data = [
				[1.0, 1.0, 0.5, 1.0f32],
				[1.0, 0.7, 1.0, 1.0f32],
				[0.5, 1.0, 0.7, 1.0f32],
				[0.7, 1.0, 1.0, 1.0f32],
			];

			self.frame_state.draw(draw_pass, self.vert_indexed_shader, self.frag_shader)
				.indexed(quad_index_buffer)
				.elements(6)
				.instances(4)
				.ubo(0, fake_proj_view_buffer)
				.ssbo(0, &vertex_buffer)
				.ssbo(1, &colour_data);
		}

		{
			#[derive(Copy, Clone)]
			#[repr(C)]
			struct SpriteData {
				color: [f32; 4],
			}

			let sprite_data = SpriteData {
				color: [1.0, 1.0, 1.0, 1.0],
			};

			self.frame_state.draw(draw_pass, self.vert_sprite_shader, self.frag_textured_shader)
				.elements(6)
				.ubo(0, proj_view_buffer)
				.buffer("SpriteData", &sprite_data)
				.texture("u_texture", self.coolcat_image, SamplerDef::nearest_clamped());

			self.frame_state.draw(final_draw_pass, self.vert_sprite_shader, self.frag_textured_shader)
				.elements(6)
				.ubo(0, proj_view_buffer)
				.buffer("SpriteData", &sprite_data)
				.texture("u_texture", self.render_target, SamplerDef::nearest_clamped());
		}

		self.frame_state.dispatch(compute_pass, self.gen_color_compute_shader)
			.indirect(args_buffer)
			.buffer("ColorBuffer", colour_buffer);

		self.context.end_frame(&mut self.frame_state);
	}

	fn resize(&mut self, size: Vec2i) {
		self.backbuffer_size = size;
		self.context.resource_manager.notify_size_changed(size);
	}
}



