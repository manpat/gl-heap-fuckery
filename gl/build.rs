
use gl_generator::{Registry, Api, Profile, Fallbacks, GlobalGenerator};
use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    let dest = env::var("OUT_DIR").unwrap();
    let mut file = File::create(&Path::new(&dest).join("gl_bindings.rs")).unwrap();

    // https://registry.khronos.org/OpenGL/extensions/ARB/ARB_bindless_texture.txt
    
	Registry::new(Api::Gl, (4, 6), Profile::Core, Fallbacks::All, ["GL_ARB_bindless_texture"])
	    .write_bindings(GlobalGenerator, &mut file)
	    .unwrap();
}