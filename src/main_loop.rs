use winit::{
	event::{Event, WindowEvent, DeviceEvent, KeyboardInput, VirtualKeyCode},
	event_loop::EventLoop,
	window::WindowBuilder,
};

use glutin_winit::DisplayBuilder;

use glutin::config::{ConfigTemplateBuilder, Api};
use glutin::context::{GlProfile, ContextApi, Version, ContextAttributesBuilder, NotCurrentGlContextSurfaceAccessor};
use glutin::display::{GlDisplay, GetGlDisplay};
use glutin::surface::{SurfaceAttributesBuilder, WindowSurface, SwapInterval};
use glutin::prelude::GlSurface;

use raw_window_handle::HasRawWindowHandle;

use std::num::NonZeroU32;

use common::math::Vec2i;



pub trait MainLoop {
	fn present(&mut self);
	fn resize(&mut self, _new_size: Vec2i) {}
}

pub fn run<F, M>(start_main_loop: F) -> anyhow::Result<()>
	where M: MainLoop + 'static
		, F: FnOnce() -> anyhow::Result<M>
{
	let event_loop = EventLoop::new();

	let config_template = ConfigTemplateBuilder::new()
		.with_api(Api::OPENGL)
		.with_stencil_size(8);

	let (window, config) = DisplayBuilder::new()
		.with_window_builder(Some(WindowBuilder::new()))
		.build(&event_loop, config_template, |configs| {
			use glutin::config::GlConfig;

			for config in configs {
				if !config.srgb_capable() { continue }
				return config;
			}

			panic!("No suitable config");
		})
		.unwrap();

	let window = window.unwrap();

	let display = config.display();

	let ctx_attributes = ContextAttributesBuilder::new()
		.with_debug(true)
		.with_profile(GlProfile::Core)
		.with_context_api(ContextApi::OpenGl(Some(Version::new(4, 6))))
		.build(Some(window.raw_window_handle()));

	let context = unsafe {
		display.create_context(&config, &ctx_attributes).unwrap()
	};


	let (width, height): (u32, u32) = window.inner_size().into();
	let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().with_srgb(Some(true)).build(
		window.raw_window_handle(),
		NonZeroU32::new(width).unwrap(),
		NonZeroU32::new(height).unwrap(),
	);

	let surface = unsafe { display.create_window_surface(&config, &attrs).unwrap() };


	let context = context.make_current(&surface).unwrap();
	surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::new(1).unwrap())).unwrap();

	gl::load_with(|symbol| {
		let symbol = std::ffi::CString::new(symbol).unwrap();
		display.get_proc_address(symbol.as_c_str()).cast()
	});


	// Set up srgb backbuffer
	unsafe {
		gl::Enable(gl::FRAMEBUFFER_SRGB);
	}


	// Set up debug callbacks
	unsafe {
		gl::DebugMessageCallback(Some(gl_message_callback), std::ptr::null());
		gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);

		// Disable performance messages
		// gl::DebugMessageControl(
		// 	gl::DONT_CARE,
		// 	gl::DEBUG_TYPE_PERFORMANCE,
		// 	gl::DONT_CARE,
		// 	0, std::ptr::null(),
		// 	0 // false
		// );

		// Disable notification messages
		gl::DebugMessageControl(
			gl::DONT_CARE,
			gl::DONT_CARE,
			gl::DEBUG_SEVERITY_NOTIFICATION,
			0, std::ptr::null(),
			0 // false
		);
	}


	let mut main_loop = start_main_loop()?;

	event_loop.run(move |event, _, control_flow| {
		control_flow.set_poll();

		match event {
			Event::WindowEvent { event: WindowEvent::CloseRequested, .. }
			| Event::DeviceEvent {
				event: DeviceEvent::Key(KeyboardInput{ virtual_keycode: Some(VirtualKeyCode::Escape), .. }), .. } => {
				control_flow.set_exit();
			}

			Event::MainEventsCleared => {
				main_loop.present();
				surface.swap_buffers(&context).unwrap();
			}

			Event::WindowEvent { event: WindowEvent::Resized(physical_size), .. } => {
				main_loop.resize(Vec2i::new(physical_size.width as i32, physical_size.height as i32));
			}

			_ => {}
		}
	});
}





extern "system" fn gl_message_callback(source: u32, ty: u32, _id: u32, severity: u32,
	_length: i32, msg: *const i8, _ud: *mut std::ffi::c_void)
{
	let severity_str = match severity {
		gl::DEBUG_SEVERITY_HIGH => "high",
		gl::DEBUG_SEVERITY_MEDIUM => "medium",
		gl::DEBUG_SEVERITY_LOW => "low",
		gl::DEBUG_SEVERITY_NOTIFICATION => return,
		_ => panic!("Unknown severity {}", severity),
	};

	let ty = match ty {
		gl::DEBUG_TYPE_ERROR => "error",
		gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "deprecated behaviour",
		gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "undefined behaviour",
		gl::DEBUG_TYPE_PORTABILITY => "portability",
		gl::DEBUG_TYPE_PERFORMANCE => "performance",
		gl::DEBUG_TYPE_OTHER => "other",
		_ => panic!("Unknown type {}", ty),
	};

	let source = match source {
		gl::DEBUG_SOURCE_API => "api",
		gl::DEBUG_SOURCE_WINDOW_SYSTEM => "window system",
		gl::DEBUG_SOURCE_SHADER_COMPILER => "shader compiler",
		gl::DEBUG_SOURCE_THIRD_PARTY => "third party",
		gl::DEBUG_SOURCE_APPLICATION => "application",
		gl::DEBUG_SOURCE_OTHER => "other",
		_ => panic!("Unknown source {}", source),
	};

	eprintln!("GL ERROR!");
	eprintln!("Source:   {}", source);
	eprintln!("Severity: {}", severity_str);
	eprintln!("Type:     {}", ty);

	unsafe {
		let msg = std::ffi::CStr::from_ptr(msg as _).to_str().unwrap();
		eprintln!("Message: {}", msg);
	}

	match severity {
		gl::DEBUG_SEVERITY_HIGH | gl::DEBUG_SEVERITY_MEDIUM => panic!("GL ERROR!"),
		_ => {}
	}
}