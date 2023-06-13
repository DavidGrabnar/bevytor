use crate::logs::LogBuffer;
use crate::plugin::{LoadProjectProgress, LoadProjectStep};
use bevy::prelude::{info, Mut, Resource, World};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::utils::HashMap;
use bevytor_script::{CreateScript, Script};
use futures_lite::future;
use libloading::{Library, Symbol};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ScriptEntry {
    folder_path: String,
    state: LoadedScript,
}

pub struct LoadedScript {
    library: Library,
    script: Box<dyn Script>,
    lib_path: String,
}

#[derive(Resource, Default)]
pub struct ScriptableRegistry {
    // TODO rewrite in a way that one "script" - key = folder_path - has multiple versions
    // TODO each with state compiling, loaded, old, etc.
    compiling_impls: HashMap<String, Task<((Library, Box<dyn Script>), String, String)>>,
    impls: HashMap<String, ScriptEntry>,
    pub old_impls: Vec<LoadedScript>,
}

impl ScriptableRegistry {
    pub fn load(&mut self, world: &mut World, folder_path: String) {
        self.load_async(world, folder_path, false);
    }

    pub fn reload(&mut self, world: &mut World, folder_path: String) {
        self.load_async(world, folder_path, true);
    }

    fn load_async(&mut self, world: &mut World, folder_path: String, force: bool) {
        let folder_path_clone = folder_path.clone();
        let pool = AsyncComputeTaskPool::get();
        let task = pool.spawn(async move {
            let base_path = Path::new(folder_path_clone.as_str());
            if force || Self::check_exists(base_path) {
                Self::build(base_path);
            }
            let clone_lib_path = Self::clone_lib_file(base_path);

            unsafe {
                (
                    ScriptableRegistry::load_script(&clone_lib_path),
                    clone_lib_path.to_str().unwrap().to_string(),
                    folder_path_clone,
                )
            }
        });

        let old_entry = self.compiling_impls.insert(folder_path, task);
        if let Some(_) = old_entry {
            // TODO halt task?
        }
        let mut logger = world.resource_mut::<LogBuffer>();
        logger.write("Script loading started".to_string());
    }

    fn clone_lib_file(base_path: &Path) -> PathBuf {
        let lib_path = base_path.join("target/debug/scripts.dll");

        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let clone_lib_path = base_path.join(format!(
            "target/debug/scripts-{}.dll",
            since_the_epoch.as_millis()
        ));
        std::fs::copy(lib_path, &clone_lib_path).expect("Cannot copy lib file");
        clone_lib_path
    }

    pub fn exec(&mut self, world: &mut World) {
        for (_, entry) in &mut self.impls {
            entry.state.script.run(world);
        }
    }

    fn register(&mut self, entry: ScriptEntry) -> Option<ScriptEntry> {
        self.impls.insert(entry.folder_path.clone(), entry)
    }

    fn check_exists(base_path: &Path) -> bool {
        let lib_path = base_path.join("target/debug/scripts.dll");
        lib_path.exists()
    }

    fn build(base_path: &Path) {
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

    unsafe fn load_script<P: AsRef<OsStr>>(path: P) -> (Library, Box<dyn Script>) {
        let lib = Library::new(path).unwrap();
        let func: Symbol<CreateScript> = lib.get(b"_create_script").unwrap();
        let res = func();
        let script = Box::from_raw(res);
        (lib, script)
    }
}

pub fn handle_tasks(world: &mut World) {
    world.resource_scope(|world, mut registry: Mut<ScriptableRegistry>| {
        let mut new_impls = vec![];
        for (_, task) in registry.compiling_impls.iter_mut() {
            if let Some(((library, script), lib_path, folder_path)) =
                future::block_on(future::poll_once(&mut *task))
            {
                // Task is complete, init and update
                new_impls.push(ScriptEntry {
                    folder_path,
                    state: LoadedScript {
                        library,
                        script,
                        lib_path,
                    },
                });
            }
        }
        for new_impl in new_impls {
            registry.compiling_impls.remove(&new_impl.folder_path);
            new_impl.state.script.init(world);
            let old_impl = registry
                .impls
                .insert(new_impl.folder_path.clone(), new_impl);
            if let Some(old_impl) = old_impl {
                registry.old_impls.push(old_impl.state);
            }
            let mut logger = world.resource_mut::<LogBuffer>();
            logger.write("Script loading complete".to_string());

            let mut load_project_progress = world.resource_mut::<LoadProjectProgress>();
            if let LoadProjectStep::Scripts(false) = load_project_progress.0 {
                load_project_progress.0 = LoadProjectStep::Scripts(true);
            }
        }
    });
}
