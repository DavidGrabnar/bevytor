use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use bevy::reflect::{GetTypeRegistration, TypeRegistration};
use std::any::{Any, TypeId};
use std::collections::HashMap;

pub type Definition = (
    TypeRegistration,
    ReflectComponent,
    ReflectSerialize,
    ReflectDeserialize,
);

pub trait Script: Any + Send + Sync {
    fn name(&self) -> &'static str;
    fn start(&self, world: &mut World);
    fn run(&self, world: &mut World, input: &Input<KeyCode>);
    fn init(&self, world: &mut World) -> Vec<Definition>;
}

pub type CreateScript = unsafe fn() -> *mut dyn Script;

#[macro_export]
macro_rules! declare_script {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub fn _create_script() -> *mut dyn $crate::Script {
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
}

impl ComponentRegistry {
    pub fn register<T>(&mut self)
    where
        T: Component + Default + Reflect,
    {
        self.reg.insert(
            TypeId::of::<T>(),
            (
                T::default().type_name().to_string(),
                Box::new(|cmd: &mut EntityCommands| {
                    cmd.insert(T::default());
                }),
            ),
        );
    }

    //pub fn get<T: Component + Reflect + Clone>(&self, id: TypeId) -> T {
    //    let def = self.defs.get(&id).unwrap();
    //    let any = def.as_any();
    //    any.downcast_ref::<T>().unwrap().clone()
    //}
}

pub fn register_component<T>(world: &mut World) -> Definition
where
    T: Reflect + Default + Component + GetTypeRegistration,
{
    world.resource_mut::<ComponentRegistry>().register::<T>();

    let registry = world.resource_mut::<AppTypeRegistry>();
    registry.write().register::<T>();

    let registry = registry.read();
    let type_id = TypeId::of::<T>();
    let registration = registry.get(type_id).unwrap().clone();
    let component = registry
        .get_type_data::<ReflectComponent>(type_id)
        .unwrap()
        .clone();
    let serialize = registry
        .get_type_data::<ReflectSerialize>(type_id)
        .unwrap()
        .clone();
    let deserialize = registry
        .get_type_data::<ReflectDeserialize>(type_id)
        .unwrap()
        .clone();

    (registration, component, serialize, deserialize)
}
