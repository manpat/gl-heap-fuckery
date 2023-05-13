




struct MyGame {
	my_view: MyView,
}

impl MyGame {
	fn new(ctx: &mut Context) -> MyGame {
		MyGame {
			my_view: MyView::new(ctx),
		}
	}

	fn draw(&self, ctx: &mut Context) {
		let prepared_slice_buffer = ctx.stream_buffer(&[1.0f32, 2.0, 3.0]);

		ctx.global_ubo(10, prepared_slice_buffer);
		ctx.global_ubo("camera", &make_camera_struct());

		self.my_view.draw(ctx);
	}
}





struct MyView {
	diffuse_tex: CommitedImageHandle,
	compute_indirect: CommitedBufferHandle,

	vertex_shader: ShaderHandle,
	fragment_shader: ShaderHandle,
	compute_shader: ShaderHandle,
}

impl MyView {
	fn new(ctx: &mut Context) -> MyView {
		MyView {
			diffuse_tex: ctx.load_image("foo.png"),
			compute_indirect: ctx.committed_buffer(4*3),

			vertex_shader: ctx.load_shader("my_vertex.vs.glsl"),
			fragment_shader: ctx.load_shader("my_fragment.fs.glsl"),
			compute_shader: ctx.load_shader("my_compute.cs.glsl"),
		}
	}

	fn draw(&self, ctx: &mut Context) {
		let prepared_slice_buffer = ctx.stream_buffer(&[1.0f32, 2.0, 3.0]);
		let prepared_struct_buffer = ctx.stream_buffer(&SomeStruct{ foo: 10.0, bar: 3.0 });

		let target_image = ctx.transient_image(ImageSize::Backbuffer, ImageFormat::Rgba32F);

		ctx.draw(self.vertex_shader, self.fragment_shader)
			.indexed(&[0u16, 1, 2]) // Infer num elements
			.elements(3) // Explicit num elements
			.instances(3)
			.buffer("some_buffer", &[1u32, 2, 3]) // Determine binding type based on shader reflection
			.buffer(Binding::Ubo(0), prepared_slice_buffer)
			.buffer(Binding::Ssbo(0), &SomeStruct{ foo: 10.0, bar: 3.0 })
			.ubo(1, prepared_struct_buffer) // shorthand for the generic `buffer` call
			.ssbo(3, prepared_slice_buffer)
			.texture("t_diffuse", diffuse_tex, SamplerDef::nearest_clamped())
			.image("t_image", diffuse_tex)
			.image_rw("t_image_out", target_image); // Auto insert barrier for diffuse_tex?


		let compute_result_buffer = ctx.reserve_transient_buffer(128);

		ctx.dispatch(self.compute_shader)
			.indirect(self.compute_indirect) // optionally bind indirect buffer
			.groups(10, 10, 1) // or, explicit direct dispatch args
			.buffer("my_output", compute_result_buffer)
			.image("t_image", target_image)
			.ssbo(1, prepared_struct_buffer); // buffer binding same as before
	}
}





struct RenderTargetDef {
	format: ImageFormat,
	size: ImageSize,
}

struct PipelineStageDef {
	name: String,

}