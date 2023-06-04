pub mod shader;
pub mod pipeline;
pub mod sampler;
pub mod image;
pub mod fbo;

use std::collections::HashMap;

pub type ResourcePath = std::path::PathBuf;
pub type ResourcePathRef = std::path::Path;

pub use self::shader::{ShaderType, ShaderDef, ShaderObject, BlockBindingLocation};
pub use self::pipeline::{PipelineDef, PipelineObject};
pub use self::sampler::{SamplerDef, AddressingMode, FilterMode, SamplerObject};
pub use self::image::{ImageDef, ImageObject, ImageSize};
pub use self::fbo::{FboDef, FboObject};

use common::math::Vec2i;




#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ShaderHandle(pub u32);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ImageHandle(pub u32);



#[derive(Debug)]
pub struct ResourceManager {
	resource_root_path: ResourcePath,

	shader_defs: HashMap<ShaderDef, ShaderHandle>,
	shader_objects: HashMap<ShaderHandle, ShaderObject>,
	shader_counter: u32,

	pipeline_objects: HashMap<PipelineDef, PipelineObject>,
	sampler_objects: HashMap<SamplerDef, SamplerObject>,
	fbo_objects: HashMap<FboDef, FboObject>,

	backbuffer_size: Vec2i,
	default_fbo: FboObject,

	image_defs: HashMap<ImageDef, ImageHandle>,
	image_objects: HashMap<ImageHandle, ImageObject>,
	image_counter: u32,
}

impl ResourceManager {
	pub fn new() -> anyhow::Result<Self> {
		let resource_root_path = ResourcePath::from("resource");

		anyhow::ensure!(resource_root_path.exists(), "Couldn't find resource path");

		let viewport_size = unsafe {
			let mut viewport = [0; 4];
			gl::GetIntegerv(gl::VIEWPORT, viewport.as_mut_ptr());
			let [_, _, x, y] = viewport;
			Vec2i::new(x, y)
		};


		Ok(Self{
			resource_root_path,

			shader_defs: HashMap::default(),
			shader_objects: HashMap::default(),
			shader_counter: 0,

			pipeline_objects: HashMap::default(),
			sampler_objects: HashMap::default(),
			fbo_objects: HashMap::default(),

			backbuffer_size: viewport_size,
			default_fbo: FboObject {
				name: 0,
				viewport_size,
			},


			image_defs: HashMap::default(),
			image_objects: HashMap::default(),
			image_counter: 0,
		})
	}

	pub fn resolve_path(&self, path: &ResourcePathRef) -> ResourcePath {
		self.resource_root_path.join(path)
	}

	pub fn backbuffer_size(&self) -> Vec2i {
		self.backbuffer_size
	}

	pub fn notify_size_changed(&mut self, new_size: Vec2i) {
		self.backbuffer_size = new_size;
		self.default_fbo.viewport_size = new_size;


		for image in self.image_objects.values_mut() {
			if image.size != ImageSize::Backbuffer {
				continue
			}

			let mut object_label = [0i8; 256];
			let mut label_length = 0;

			unsafe {
				gl::GetObjectLabel(gl::TEXTURE, image.name, object_label.len() as i32,
					&mut label_length, object_label.as_mut_ptr());

				gl::DeleteTextures(1, &image.name);
				gl::CreateTextures(gl::TEXTURE_2D, 1, &mut image.name);
				gl::TextureStorage2D(image.name, 1, image.format, new_size.x, new_size.y);

				gl::ObjectLabel(gl::TEXTURE, image.name, label_length, object_label.as_ptr());
			}
		}


		let mut fbo_objects = std::mem::replace(&mut self.fbo_objects, HashMap::default());

		for (def, fbo) in fbo_objects.iter_mut() {
			fbo::resolve_and_bind(self, def, fbo);
		}

		self.fbo_objects = fbo_objects;
	}

	pub fn load_text(&mut self, def: &ResourcePathRef) -> anyhow::Result<String> {
		let string = std::fs::read_to_string(&self.resolve_path(def))?;
		Ok(string)
	}

	pub fn load_shader(&mut self, def: &ShaderDef) -> anyhow::Result<ShaderHandle> {
		if let Some(handle) = self.shader_defs.get(def) {
			return Ok(*handle);
		}

		let object = self::shader::compile_shader(self, def)?;

		let handle = ShaderHandle(self.shader_counter);
		self.shader_counter += 1;

		self.shader_defs.insert(def.clone(), handle);
		self.shader_objects.insert(handle, object);

		Ok(handle)
	}

	pub fn load_image(&mut self, def: &ImageDef) -> anyhow::Result<ImageHandle> {
		if let Some(handle) = self.image_defs.get(def) {
			return Ok(*handle);
		}

		let object = self::image::load(self, def)?;

		let handle = ImageHandle(self.image_counter);
		self.image_counter += 1;

		self.image_defs.insert(def.clone(), handle);
		self.image_objects.insert(handle, object);

		Ok(handle)
	}

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

	pub fn get_sampler<'s>(&'s mut self, def: &'_ SamplerDef) -> &'s SamplerObject {
		self.sampler_objects.entry(def.clone())
			.or_insert_with(|| self::sampler::create_sampler(def))
	}

	pub fn get_fbo<'s>(&'s mut self, def: &'_ FboDef) -> anyhow::Result<&'s FboObject> {
		if def == &FboDef::default() {
			return Ok(&self.default_fbo);
		}

		// HACK: I can't figure out the lifetimes for this - something goes weird if I try to use if let = get here
		// see: https://users.rust-lang.org/t/lifetime-is-not-dropped-after-if-let-x-return-x/42892
		if self.fbo_objects.contains_key(def) {
			return Ok(self.fbo_objects.get(def).unwrap())
		}

		let object = self::fbo::create(self, def)?;
		let object = self.fbo_objects.entry(def.clone()).or_insert(object);
		Ok(object)
	}


	pub fn resolve_shader(&self, handle: ShaderHandle) -> Option<&ShaderObject> {
		self.shader_objects.get(&handle)
	}

	pub fn resolve_image(&self, handle: ImageHandle) -> Option<&ImageObject> {
		self.image_objects.get(&handle)
	}
}


