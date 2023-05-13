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



#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BindingLocation {
	Ubo(u32),
	Ssbo(u32),
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockDescription {
	pub binding_location: BindingLocation,
	pub total_size: u32,
}


#[derive(Debug)]
pub struct ShaderObject {
	pub name: u32,
	pub blocks: HashMap<String, BlockDescription>,
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

	let blocks = reflect_blocks(program_name)?;

	Ok(ShaderObject {
		name: program_name,
		blocks,
		workgroup_size: match def.shader_type {
			ShaderType::Compute => Some(reflect_workgroup_size(program_name)),
			_ => None,
		},
	})
}



fn reflect_blocks(program_name: u32) -> anyhow::Result<HashMap<String, BlockDescription>> {
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
			binding_location: BindingLocation::Ubo(buffer_binding as u32),
			total_size: buffer_data_size as u32,
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

		blocks.insert(name, BlockDescription {
			binding_location: BindingLocation::Ssbo(buffer_binding as u32),
			total_size: buffer_data_size as u32,
		});
	}

	Ok(blocks)
}


fn reflect_workgroup_size(program_name: u32) -> [u32; 3] {
	let mut workgroup_size = [0u32; 3];

	unsafe {
		gl::GetProgramiv(program_name, gl::COMPUTE_WORK_GROUP_SIZE, workgroup_size.as_mut_ptr() as *mut i32);
	}

	workgroup_size
}