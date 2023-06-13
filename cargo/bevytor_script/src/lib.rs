use bevy::ecs::component::TableStorage;
use bevy::ecs::query::{QueryState, ReadOnlyWorldQuery, WorldQuery};
use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use std::any::{Any, TypeId};
use std::collections::HashMap;

pub trait Script: Any + Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, world: &mut World) -> Option<TypeId>;
    // return is a temporary test for dynamically loaded components, will be moved to a separate function
    fn init(&self, world: &mut World);
}

pub type CreateScript = unsafe fn() -> *mut dyn Script;

#[macro_export]
macro_rules! declare_script {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn create_script() -> *mut dyn $crate::Script {
            let constructor: fn() -> $plugin_type = $constructor;

            let object = constructor();
            let boxed: Box<dyn $crate::Script> = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

#[derive(Resource, Default)]
pub struct ComponentRegistry {
    pub reg: HashMap<TypeId, (String, Box<fn(&mut EntityCommands) -> ()>)>,
    // pub defs2: HashMap<TypeId, Box<dyn Component<Storage = dyn Any>>>,
    // pub defs: HashMap<TypeId, Box<dyn Reflect>>,
}

impl ComponentRegistry {
    pub fn register<T: Component<Storage = TableStorage> + Default + Reflect>(&mut self) {
        self.reg.insert(
            TypeId::of::<T>(),
            (
                T::default().type_name().to_string(),
                Box::new(|cmd: &mut EntityCommands| {
                    cmd.insert(T::default());
                }),
            ),
        );
        //self.defs.insert(TypeId::of::<T>(), Box::<T>::default());
        // self.defs2.insert(TypeId::of::<T>(), Box::<T>::default());
    }

    //pub fn get<T: Component + Reflect + Clone>(&self, id: TypeId) -> T {
    //    let def = self.defs.get(&id).unwrap();
    //    let any = def.as_any();
    //    any.downcast_ref::<T>().unwrap().clone()
    //}
}
