use std::borrow::Cow;
use bevy::prelude::*;
use bevy::reflect::{FromType, TypeData, TypeRegistration, TypeRegistryArc};
use bevy::render::camera::{Projection, Viewport};

fn main() {
    App::new()
        .register_type::<Option<Viewport>>()
        .register_type_data::<Option<Viewport>, ReflectSerialize>()
        .register_type::<Cow<str>>()
        .register_type::<Projection>()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_scene)
        .add_system(save_scene)
        .run();
}

fn setup_scene(
    mut commands: Commands
) {
    commands.spawn_bundle(Camera3dBundle::default());
}

fn save_scene(world: &World) {
    // just to be sure
    std::thread::sleep(std::time::Duration::from_secs(2));

    let type_registry_arc = world.resource::<TypeRegistryArc>();

    // let mut write = type_registry_arc.write();
    // let mut reg = TypeRegistration::of::<Projection>();
    // reg.insert::<ReflectSerialize>(Viewport::default().clone_type_data());
    // write.add_registration(reg);

    let scene = DynamicScene::from_world(world, type_registry_arc);
    let ser = scene.serialize_ron(type_registry_arc).unwrap();

    std::fs::write(std::path::Path::new("./help-plz.ron"), ser).unwrap();
    println!("saved");
}
