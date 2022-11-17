mod plugin;
mod service;
#[macro_use]
mod error;
mod ui;

use std::any::Any;
use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;
use bevy::asset::HandleId;
use bevy::prelude::*;
use bevy::reflect::{FromType, TypeData, TypeInfo, TypeRegistration, TypeRegistry, TypeRegistryArc};
use bevy::render::camera::{CameraProjection, DepthCalculation, Projection, ScalingMode, Viewport, WindowOrigin};
use crate::plugin::EditorPlugin;
use serde::{Serialize, Deserialize, Serializer};
use serde::ser::SerializeStruct;
// use bevytor_spy::plugin::SpyPlugin;

use systems_hot::*;

#[hot_lib_reloader::hot_module(dylib = "scripts")]
mod systems_hot {
    use bevy::prelude::*;
    hot_functions_from_file!("scripts/src/lib.rs");
}

fn main() {
    App::new()
        .register_type::<SkipSerialization>()
        .add_plugins(DefaultPlugins)
        .add_plugin(EditorPlugin::default())
        // .add_system(test_hot_system)
        // .add_startup_system(setup_scene) // TEST
        // .add_system(bonk.exclusive_system())
        // .add_system(honk.exclusive_system())
        .run();
}

#[derive(Component, Reflect)]
struct SkipSerialization;

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 5.0 })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..Default::default()
    });
    // cube
    let cube = commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
            material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
            transform: Transform::from_xyz(0.0, 0.5, 0.0),
            ..Default::default()
        })
        .id();
    // child cube
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
            material: materials.add(Color::rgb(0.6, 0.7, 0.8).into()),
            transform: Transform::from_xyz(0.0, 1.0, 0.0),
            ..Default::default()
        })
        .add_child(cube);
    // light
    commands.spawn_bundle(PointLightBundle {
        transform: Transform::from_xyz(3.0, 8.0, 5.0),
        ..Default::default()
    });

    // camera
    /*commands.spawn_bundle(Camera3dBundle {
        projection: Projection::Orthographic(OrthographicProjection {
            // Why so small scale?
            scale: 0.01,
            ..default()
        }),
        transform: Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    })
        .insert(SkipSerialization);*/
}

fn bonk(world: &mut World) {
    println!("before");
    sleep(Duration::from_secs(2));
    println!("sleep");
    let type_registry_arc = world.resource::<TypeRegistry>();
    type_registry_arc.write().register::<std::borrow::Cow<str>>();
    // type_registry_arc.write().register::<Projection>();
    type_registry_arc.write().register::<Option<Viewport>>();
    type_registry_arc.write().register_type_data::<Option<Viewport>, ReflectSerialize>();
    type_registry_arc.write().register_type_data::<Option<Viewport>, ReflectDeserialize>();

    let mut scene = DynamicScene::from_world(world, type_registry_arc);

    // TODO find by SkipSerialize component, but this component not in the list
    let mut i = 0;
    let mut to_remove = false;
    for entity in &scene.entities {
        println!("{}", entity.entity);
        for component in &entity.components {
            println!("{}", component.type_name());
            if component.type_name() == "bevy_render::camera::camera::Camera" {
                to_remove = true;
                break;
            }
        }
        if to_remove {
            break;
        }
        i += 1;
    }

    if to_remove {
        scene.entities.remove(i);
    }

    let ser = scene.serialize_ron(type_registry_arc).unwrap();
    fs::write(Path::new("/home/grabn/projects/bevytor/banana.ron"), ser).unwrap();
    println!("saved");
}


fn honk(meshes_res: Res<Assets<Mesh>>) {
    println!("before");
    // sleep(Duration::from_secs(2));
    println!("sleep");

    let meshes = meshes_res.deref();
    for (handle, mesh) in meshes.iter() {
        println!("{:?} {:?}", handle, mesh);
    }

    println!("saved");
}