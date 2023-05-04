use std::collections::HashMap;



pub type ResourcePath = std::path::PathBuf;
pub type ResourcePathRef = std::path::Path;


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



#[derive(Debug)]
pub struct ResourceManager {
	resource_root_path: ResourcePath,

	shader_defs: HashMap<ShaderDef, ShaderHandle>,
	shader_names: HashMap<ShaderHandle, u32>,
	shader_counter: u32,

	pipeline_defs: HashMap<PipelineDef, PipelineHandle>,
	pipeline_names: HashMap<PipelineHandle, u32>,
	pipeline_counter: u32,
}

impl ResourceManager {
	pub fn new() -> anyhow::Result<Self> {
		let resource_root_path = ResourcePath::from("resource");

		anyhow::ensure!(resource_root_path.exists(), "Couldn't find resource path");

		Ok(Self{
			resource_root_path,

			shader_defs: HashMap::default(),
			shader_names: HashMap::default(),
			shader_counter: 0,

			pipeline_defs: HashMap::default(),
			pipeline_names: HashMap::default(),
			pipeline_counter: 0,
		})
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

	pub fn resolve_shader_name(&self, handle: ShaderHandle) -> Option<u32> {
		self.shader_names.get(&handle).cloned()
	}

	pub fn resolve_pipeline_name(&self, handle: PipelineHandle) -> Option<u32> {
		self.pipeline_names.get(&handle).cloned()
	}
}