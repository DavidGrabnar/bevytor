#[macro_use]
extern crate bevytor_script;

use bevy::prelude::*;
use bevytor_derive::DynamicScript;
use bevytor_script::{ComponentRegistry, Script};
use serde::{Deserialize, Serialize};
use std::any::TypeId;

#[derive(Debug, Default, DynamicScript)]
pub struct TestScript;

#[derive(Default, Reflect, Component, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Velocity(f32);

impl Script for TestScript {
    fn name(&self) -> &'static str {
        "Le Demo finally!"
    }

    fn run(&self, world: &mut World) -> Option<TypeId> {
        let mut state =
            world.query_filtered::<(Entity, &mut Transform, &Velocity), Without<Camera>>();
        for (entity, mut transform, velocity) in state.iter_mut(world) {
            transform.translation.x += velocity.0;
        }
        Some(TypeId::of::<Velocity>())
    }

    fn init(&self, world: &mut World) {
        let registry = world.resource_mut::<AppTypeRegistry>();
        registry.write().register::<Velocity>();
        let mut registry2 = world.resource_mut::<ComponentRegistry>();
        registry2.register::<Velocity>();
        println!("registered {}", registry2.reg.len());
    }
}
