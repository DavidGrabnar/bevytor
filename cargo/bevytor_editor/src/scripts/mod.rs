use bevy::prelude::{Resource, World};
use bevytor_script::{CreateScript, Script};
use libloading::{Library, Symbol};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

struct ScriptEntry {
    library: Library,
    script: Box<dyn Script>,
    path: String,
}

#[derive(Resource, Default)]
pub struct ScriptableRegistry {
    impls: Vec<ScriptEntry>,
}

impl ScriptableRegistry {
    pub fn load(&mut self, world: &mut World, folder_path: String) {
        let base_path = Path::new(&folder_path);
        let lib_path = base_path.join("target/debug/scripts.dll");
        if !lib_path.exists() {
            let output = Command::new("cargo")
                .arg("build")
                .current_dir(base_path)
                .output()
                .expect("failed to build script");

            println!(
                "DONE {}",
                output.status //String::from_utf8(output.stdout).expect("failed to parse command output")
            )
        }
        let (library, script) = unsafe { ScriptableRegistry::load_script(lib_path) };
        script.init(world);
        self.register(ScriptEntry {
            library,
            script,
            path: folder_path,
        });
    }

    pub fn exec(&mut self, world: &mut World) {
        for entry in &mut self.impls {
            entry.script.run(world);
        }
    }

    fn register(&mut self, entry: ScriptEntry) {
        self.impls.push(entry);
    }

    unsafe fn load_script<P: AsRef<OsStr>>(path: P) -> (Library, Box<dyn Script>) {
        let lib = Library::new(path).unwrap();
        let func: Symbol<CreateScript> = lib.get(b"_create_script").unwrap();
        let res = func();
        let script = Box::from_raw(res);
        (lib, script)
    }
}
