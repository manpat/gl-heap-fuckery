pub mod shader;
pub mod pipeline;

use std::collections::HashMap;

pub type ResourcePath = std::path::PathBuf;
pub type ResourcePathRef = std::path::Path;

pub use self::shader::{ShaderType, ShaderDef, BindingLocation};
pub use self::pipeline::{PipelineDef, PipelineObject};

use self::shader::ShaderObject;




#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ShaderHandle(pub u32);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct PipelineHandle(pub u32);



#[derive(Debug)]
pub struct ResourceManager {
	resource_root_path: ResourcePath,

	shader_defs: HashMap<ShaderDef, ShaderHandle>,
	shader_objects: HashMap<ShaderHandle, ShaderObject>,
	shader_counter: u32,

	pipeline_objects: HashMap<PipelineDef, PipelineObject>,
}

impl ResourceManager {
	pub fn new() -> anyhow::Result<Self> {
		let resource_root_path = ResourcePath::from("resource");

		anyhow::ensure!(resource_root_path.exists(), "Couldn't find resource path");

		Ok(Self{
			resource_root_path,

			shader_defs: HashMap::default(),
			shader_objects: HashMap::default(),
			shader_counter: 0,

			pipeline_objects: HashMap::default(),
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

		let object = self::shader::compile_shader(self, def)?;

		dbg!(&object);

		let handle = ShaderHandle(self.shader_counter);
		self.shader_counter += 1;

		self.shader_defs.insert(def.clone(), handle);
		self.shader_objects.insert(handle, object);

		Ok(handle)
	}

	// TODO(pat.m): maybe I want to do away with fixed pipelines and just bind PipelineDefs instead
	pub fn get_pipeline<'s>(&'s mut self, def: &'_ PipelineDef) -> anyhow::Result<&'s PipelineObject> {
		// HACK: I can't figure out the lifetimes for this - something goes weird if I try to use if let = get here
		// see: https://users.rust-lang.org/t/lifetime-is-not-dropped-after-if-let-x-return-x/42892
		if self.pipeline_objects.contains_key(def) {
			return Ok(self.pipeline_objects.get(def).unwrap())
		}

		let object = self::pipeline::create_pipeline(self, def)?;
		let object = self.pipeline_objects.entry(def.clone()).or_insert(object);
		Ok(object)
	}

	pub fn resolve_shader(&self, handle: ShaderHandle) -> Option<&ShaderObject> {
		self.shader_objects.get(&handle)
	}
}