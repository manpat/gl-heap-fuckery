use super::{ResourceManager, ShaderHandle, shader::BlockDescription};
use std::collections::HashMap;


#[derive(Hash, Clone, Default, Debug, Eq, PartialEq)]
pub struct PipelineDef {
	pub vertex: Option<ShaderHandle>,
	pub fragment: Option<ShaderHandle>,
	pub compute: Option<ShaderHandle>,
}

#[derive(Debug)]
pub struct PipelineObject {
	pub name: u32,
	pub composite_blocks: HashMap<String, BlockDescription>,
	pub composite_image_bindings: HashMap<String, u32>,
}

impl PipelineObject {
	pub fn block_by_name(&self, name: &str) -> Option<&BlockDescription> {
		self.composite_blocks.get(name)
	}

	pub fn block_by_binding_location(&self, loc: super::shader::BlockBindingLocation) -> Option<&BlockDescription> {
		self.composite_blocks.values().find(move |desc| desc.binding_location == loc)
	}

	pub fn image_binding_by_name(&self, name: &str) -> Option<u32> {
		self.composite_image_bindings.get(name).cloned()
	}
}


pub fn create_pipeline(resource_manager: &ResourceManager, def: &PipelineDef) -> anyhow::Result<PipelineObject> {
	let mut pipeline_name = 0;
	let mut composite_blocks = HashMap::new();
	let mut composite_image_bindings = HashMap::new();

	unsafe {
		gl::CreateProgramPipelines(1, &mut pipeline_name);
		if pipeline_name == 0 {
			anyhow::bail!("Failed to create pipeline pipeline");
		}

		bind_shader_to_pipeline(resource_manager, pipeline_name, def.vertex, gl::VERTEX_SHADER_BIT, &mut composite_blocks, &mut composite_image_bindings);
		bind_shader_to_pipeline(resource_manager, pipeline_name, def.fragment, gl::FRAGMENT_SHADER_BIT, &mut composite_blocks, &mut composite_image_bindings);
		bind_shader_to_pipeline(resource_manager, pipeline_name, def.compute, gl::COMPUTE_SHADER_BIT, &mut composite_blocks, &mut composite_image_bindings);

		// TODO(pat.m): gl::ObjectLabel

		gl::ValidateProgramPipeline(pipeline_name);
	}

	Ok(PipelineObject {
		name: pipeline_name,
		composite_blocks,
		composite_image_bindings,
	})
}


fn bind_shader_to_pipeline(resource_manager: &ResourceManager, pipeline_name: u32, shader_handle: Option<ShaderHandle>,
	type_bits: u32, composite_blocks: &mut HashMap<String, BlockDescription>, composite_image_bindings: &mut HashMap<String, u32>)
{
	let Some(shader_handle) = shader_handle else {
		return
	};

	let shader_object = resource_manager.resolve_shader(shader_handle).unwrap();
	unsafe {
		gl::UseProgramStages(pipeline_name, type_bits, shader_object.name);
	}

	for (block_name, block) in shader_object.blocks.iter() {
		let prev_block = composite_blocks.insert(block_name.clone(), block.clone());

		if let Some(prev_block) = prev_block
			&& prev_block != *block
		{
			panic!("Pipeline contains multiple incompatible interface blocks with same name '{block_name}'");
		}
	}

	for (uniform_name, index) in shader_object.image_bindings.iter() {
		let prev_index = composite_image_bindings.insert(uniform_name.clone(), *index);

		if let Some(prev_index) = prev_index
			&& prev_index != *index
		{
			panic!("Pipeline contains multiple image binding of same name '{uniform_name}' with different binding indices");
		}
	}
}