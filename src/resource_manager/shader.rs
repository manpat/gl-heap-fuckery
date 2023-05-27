use super::{ResourceManager, ResourcePath};
use std::collections::HashMap;

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

impl ShaderDef {
	pub fn vertex(path: impl Into<ResourcePath>) -> ShaderDef {
		ShaderDef {
			path: path.into(),
			shader_type: ShaderType::Vertex,
		}
	}

	pub fn fragment(path: impl Into<ResourcePath>) -> ShaderDef {
		ShaderDef {
			path: path.into(),
			shader_type: ShaderType::Fragment,
		}
	}

	pub fn compute(path: impl Into<ResourcePath>) -> ShaderDef {
		ShaderDef {
			path: path.into(),
			shader_type: ShaderType::Compute,
		}
	}
}


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BlockBindingLocation {
	Ubo(u32),
	Ssbo(u32),
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockDescription {
	pub binding_location: BlockBindingLocation,
	pub total_size: u32,
	pub is_read_write: bool,
}


#[derive(Debug)]
pub struct ShaderObject {
	pub name: u32,
	pub blocks: HashMap<String, BlockDescription>,
	pub image_bindings: HashMap<String, u32>, // HACK: don't really care about whether they're texture or image units for now
	pub workgroup_size: Option<[u32; 3]>,
}


pub fn compile_shader(resource_manager: &mut ResourceManager, def: &ShaderDef) -> anyhow::Result<ShaderObject> {
	let content = resource_manager.load_text(&def.path)?;

	let program_name;

	unsafe {
		let src_cstring = std::ffi::CString::new(content.as_bytes())?;
		let source_strings = [
			b"#version 450\n\0".as_ptr()  as *const i8,
			src_cstring.as_ptr(),
		];

		program_name = gl::CreateShaderProgramv(def.shader_type as u32, source_strings.len() as _, source_strings.as_ptr());

		if program_name == 0 {
			anyhow::bail!("Failed to create shader '{}'", def.path.display());
		}

		let mut status = 0;
		gl::GetProgramiv(program_name, gl::LINK_STATUS, &mut status);

		if status == 0 {
			let mut buf = [0u8; 1024];
			let mut len = 0;
			gl::GetProgramInfoLog(program_name, buf.len() as _, &mut len, buf.as_mut_ptr() as _);

			gl::DeleteProgram(program_name);

			let error = std::str::from_utf8(&buf[..len as usize])?;
			anyhow::bail!("Failed to create shader '{}':\n{}", def.path.display(), error);
		}

		if let Some(path_str) = def.path.to_str() {
			gl::ObjectLabel(gl::PROGRAM, program_name, path_str.len() as i32, path_str.as_ptr() as *const _);
		}
	}

	let blocks = reflect_blocks(program_name, &content)?;
	let image_bindings = reflect_image_bindings(program_name, &content)?;

	Ok(ShaderObject {
		name: program_name,
		blocks,
		image_bindings,
		workgroup_size: match def.shader_type {
			ShaderType::Compute => Some(reflect_workgroup_size(program_name)),
			_ => None,
		},
	})
}



fn reflect_blocks(program_name: u32, content: &str) -> anyhow::Result<HashMap<String, BlockDescription>> {
	let mut blocks = HashMap::new();

	let mut num_uniform_blocks = 0;
	let mut num_buffer_blocks = 0;

	unsafe {
		gl::GetProgramInterfaceiv(program_name, gl::UNIFORM_BLOCK, gl::ACTIVE_RESOURCES, &mut num_uniform_blocks);
		gl::GetProgramInterfaceiv(program_name, gl::SHADER_STORAGE_BLOCK, gl::ACTIVE_RESOURCES, &mut num_buffer_blocks);
	}

	let block_property_names = [gl::NUM_ACTIVE_VARIABLES, gl::NAME_LENGTH, gl::BUFFER_BINDING, gl::BUFFER_DATA_SIZE];

	for block_idx in 0..num_uniform_blocks {
		let mut result = [0; 4];
		unsafe {
			gl::GetProgramResourceiv(
				program_name, gl::UNIFORM_BLOCK,
				block_idx as u32,
				block_property_names.len() as _, block_property_names.as_ptr(),
				result.len() as _, std::ptr::null_mut(), result.as_mut_ptr());
		}

		// TODO(pat.m): may want to get information about actual structure of block
		let [_num_active_variables, name_length, buffer_binding, buffer_data_size] = result;

		// Name includes null terminator which we don't care about
		let mut str_buf = vec![0u8; name_length as usize];
		unsafe {
			gl::GetProgramResourceName(
				program_name, gl::UNIFORM_BLOCK,
				block_idx as u32,
				name_length, std::ptr::null_mut(), str_buf.as_mut_ptr() as *mut i8);
		}

		str_buf.pop(); // Remove null terminator
		let name = String::from_utf8(str_buf)?;

		blocks.insert(name, BlockDescription {
			binding_location: BlockBindingLocation::Ubo(buffer_binding as u32),
			total_size: buffer_data_size as u32,
			is_read_write: false,
		});
	}

	for block_idx in 0..num_buffer_blocks {
		let mut result = [0; 4];

		unsafe {
			gl::GetProgramResourceiv(
				program_name, gl::SHADER_STORAGE_BLOCK,
				block_idx as u32,
				block_property_names.len() as _, block_property_names.as_ptr(),
				result.len() as _, std::ptr::null_mut(), result.as_mut_ptr());
		}

		// TODO(pat.m): may want to get information about actual structure of block
		let [_num_active_variables, name_length, buffer_binding, buffer_data_size] = result;

		// Name includes null terminator which we don't care about
		let mut str_buf = vec![0u8; name_length as usize];
		unsafe {
			gl::GetProgramResourceName(
				program_name, gl::SHADER_STORAGE_BLOCK,
				block_idx as u32,
				name_length, std::ptr::null_mut(), str_buf.as_mut_ptr() as *mut i8);
		}

		str_buf.pop(); // Remove null terminator
		let name = String::from_utf8(str_buf)?;
		let is_readonly = buffer_block_has_readonly_keyword(&name, content);

		blocks.insert(name, BlockDescription {
			binding_location: BlockBindingLocation::Ssbo(buffer_binding as u32),
			total_size: buffer_data_size as u32,
			is_read_write: !is_readonly,
		});
	}

	Ok(blocks)
}

// HACK: opengl doesn't provide a way to query the storage qualifiers for interface blocks, so we have to parse them out ourselves
fn buffer_block_has_readonly_keyword(name: &str, content: &str) -> bool {
	for (idx, _) in content.match_indices("buffer") {
		// Read forward one token to see if we're looking at the right buffer block
		let Some((parsed_name, _)) = content[idx + "buffer".len() ..].trim_start()
			.split_once(|c: char| c.is_whitespace() || c == '{') else { continue };

		if parsed_name != name {
			continue
		}

		// Find end of previous declaration.
		let scan_begin = content[..idx].rfind(|c: char| c == ';').unwrap_or(0);

		let has_keyword = content[scan_begin..idx].split_whitespace()
			.rfind(|&keyword| keyword == "readonly")
			.is_some();

		return has_keyword;
	}

	false
}

fn reflect_image_bindings(program_name: u32, _content: &str) -> anyhow::Result<HashMap<String, u32>> {
	let mut image_bindings = HashMap::new();

	let mut num_uniforms = 0;

	unsafe {
		gl::GetProgramInterfaceiv(program_name, gl::UNIFORM, gl::ACTIVE_RESOURCES, &mut num_uniforms);
	}

	let uniform_property_names = [gl::NAME_LENGTH, gl::TYPE, gl::LOCATION, gl::BLOCK_INDEX];

	for uniform_index in 0..num_uniforms {
		let mut result = [0; 4];

		unsafe {
			gl::GetProgramResourceiv(
				program_name, gl::UNIFORM,
				uniform_index as u32,
				uniform_property_names.len() as _, uniform_property_names.as_ptr(),
				result.len() as _, std::ptr::null_mut(), result.as_mut_ptr());
		}

		let [name_length, gl_type, location, block_index] = result;

		// Skip uniforms in blocks
		if block_index != -1 {
			continue
		}

		// Skip uniforms that aren't sampler or image types
		// https://registry.khronos.org/OpenGL/specs/gl/glspec46.core.pdf#table.7.3
		let image_binding_types = [
			gl::SAMPLER_1D,
			gl::SAMPLER_2D,
			gl::SAMPLER_3D,
			gl::SAMPLER_CUBE,
			gl::SAMPLER_1D_SHADOW,
			gl::SAMPLER_2D_SHADOW,
			gl::SAMPLER_1D_ARRAY,
			gl::SAMPLER_2D_ARRAY,
			gl::SAMPLER_1D_ARRAY_SHADOW,
			gl::SAMPLER_2D_ARRAY_SHADOW,
			gl::SAMPLER_2D_MULTISAMPLE,
			gl::SAMPLER_2D_MULTISAMPLE_ARRAY,
			gl::SAMPLER_CUBE_SHADOW,
			gl::SAMPLER_BUFFER,
			gl::SAMPLER_2D_RECT,
			gl::SAMPLER_2D_RECT_SHADOW,
			gl::INT_SAMPLER_1D,
			gl::INT_SAMPLER_2D,
			gl::INT_SAMPLER_3D,
			gl::INT_SAMPLER_CUBE,
			gl::INT_SAMPLER_1D_ARRAY,
			gl::INT_SAMPLER_2D_ARRAY,
			gl::INT_SAMPLER_2D_MULTISAMPLE,
			gl::INT_SAMPLER_2D_MULTISAMPLE_ARRAY,
			gl::INT_SAMPLER_BUFFER,
			gl::INT_SAMPLER_2D_RECT,
			gl::UNSIGNED_INT_SAMPLER_1D,
			gl::UNSIGNED_INT_SAMPLER_2D,
			gl::UNSIGNED_INT_SAMPLER_3D,
			gl::UNSIGNED_INT_SAMPLER_CUBE,
			gl::UNSIGNED_INT_SAMPLER_1D_ARRAY,
			gl::UNSIGNED_INT_SAMPLER_2D_ARRAY,
			gl::UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE,
			gl::UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE_ARRAY,
			gl::UNSIGNED_INT_SAMPLER_BUFFER,
			gl::UNSIGNED_INT_SAMPLER_2D_RECT,

			gl::IMAGE_1D,
			gl::IMAGE_2D,
			gl::IMAGE_3D,
			gl::IMAGE_2D_RECT,
			gl::IMAGE_CUBE,
			gl::IMAGE_BUFFER,
			gl::IMAGE_1D_ARRAY,
			gl::IMAGE_2D_ARRAY,
			gl::IMAGE_CUBE_MAP_ARRAY,
			gl::IMAGE_2D_MULTISAMPLE,
			gl::IMAGE_2D_MULTISAMPLE_ARRAY,
			gl::INT_IMAGE_1D,
			gl::INT_IMAGE_2D,
			gl::INT_IMAGE_3D,
			gl::INT_IMAGE_2D_RECT,
			gl::INT_IMAGE_CUBE,
			gl::INT_IMAGE_BUFFER,
			gl::INT_IMAGE_1D_ARRAY,
			gl::INT_IMAGE_2D_ARRAY,
			gl::INT_IMAGE_CUBE_MAP_ARRAY,
			gl::INT_IMAGE_2D_MULTISAMPLE,
			gl::INT_IMAGE_2D_MULTISAMPLE_ARRAY,
			gl::UNSIGNED_INT_IMAGE_1D,
			gl::UNSIGNED_INT_IMAGE_2D,
			gl::UNSIGNED_INT_IMAGE_3D,
			gl::UNSIGNED_INT_IMAGE_2D_RECT,
			gl::UNSIGNED_INT_IMAGE_CUBE,
			gl::UNSIGNED_INT_IMAGE_BUFFER,
			gl::UNSIGNED_INT_IMAGE_1D_ARRAY,
			gl::UNSIGNED_INT_IMAGE_2D_ARRAY,
			gl::UNSIGNED_INT_IMAGE_CUBE_MAP_ARRAY,
			gl::UNSIGNED_INT_IMAGE_2D_MULTISAMPLE,
			gl::UNSIGNED_INT_IMAGE_2D_MULTISAMPLE_ARRAY,
		];

		if !image_binding_types.contains(&(gl_type as u32)) {
			continue;
		}

		let mut str_buf = vec![0u8; name_length as usize];
		unsafe {
			gl::GetProgramResourceName(
				program_name, gl::UNIFORM,
				uniform_index as u32,
				name_length, std::ptr::null_mut(), str_buf.as_mut_ptr() as *mut i8);
		}

		str_buf.pop(); // Remove null terminator
		let name = String::from_utf8(str_buf)?;

		let mut binding_index = 0;
		unsafe {
			gl::GetUniformiv(program_name, location, &mut binding_index);
		}

		image_bindings.insert(name, binding_index as u32);
	}

	Ok(image_bindings)
}

fn reflect_workgroup_size(program_name: u32) -> [u32; 3] {
	let mut workgroup_size = [0u32; 3];

	unsafe {
		gl::GetProgramiv(program_name, gl::COMPUTE_WORK_GROUP_SIZE, workgroup_size.as_mut_ptr() as *mut i32);
	}

	workgroup_size
}
