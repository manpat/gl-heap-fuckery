

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
#[repr(u32)]
pub enum AddressingMode {
	/// This is the default
	Repeat = gl::REPEAT,
	ClampToEdge = gl::CLAMP_TO_EDGE,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
#[repr(u32)]
pub enum FilterMode {
	Nearest = gl::NEAREST,
	Linear = gl::LINEAR,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct SamplerDef {
	pub addressing_mode: AddressingMode,
	pub minify_filter: FilterMode,
	pub magnify_filter: FilterMode,
}

impl SamplerDef {
	pub fn linear_clamped() -> SamplerDef {
		SamplerDef {
			addressing_mode: AddressingMode::ClampToEdge,
			minify_filter: FilterMode::Linear,
			magnify_filter: FilterMode::Linear,
		}
	}

	pub fn nearest_clamped() -> SamplerDef {
		SamplerDef {
			addressing_mode: AddressingMode::ClampToEdge,
			minify_filter: FilterMode::Nearest,
			magnify_filter: FilterMode::Nearest,
		}
	}
}


#[derive(Debug)]
pub struct SamplerObject {
	pub name: u32,
}


pub fn create_sampler(def: &SamplerDef) -> SamplerObject {
	let mut sampler_name = 0;

	let debug_label = format!("min: {:?}, mag: {:?}, address: {:?}",
		def.minify_filter, def.magnify_filter, def.addressing_mode);

	unsafe {
		gl::CreateSamplers(1, &mut sampler_name);
		gl::SamplerParameteri(sampler_name, gl::TEXTURE_MIN_FILTER, def.minify_filter as i32);
		gl::SamplerParameteri(sampler_name, gl::TEXTURE_MAG_FILTER, def.magnify_filter as i32);

		gl::SamplerParameteri(sampler_name, gl::TEXTURE_WRAP_S, def.addressing_mode as i32);
		gl::SamplerParameteri(sampler_name, gl::TEXTURE_WRAP_T, def.addressing_mode as i32);
		gl::SamplerParameteri(sampler_name, gl::TEXTURE_WRAP_R, def.addressing_mode as i32);

		gl::ObjectLabel(gl::SAMPLER, sampler_name, debug_label.len() as i32, debug_label.as_ptr() as *const _);
	}

	SamplerObject {
		name: sampler_name
	}
}