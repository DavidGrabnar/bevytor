//! Bevy Version: 0.10, 0.11
//! Source: https://gist.github.com/nwtnni/85d6b87ae75337a522166c500c9a8418
//! Original: https://gist.github.com/GianpaoloBranca/17e5bd6ada9bdb04cca58182db8505d4
//! See also: https://github.com/bevyengine/bevy/issues/1515

use bevy::ecs::system::Command;
use bevy::prelude::*;

pub struct CloneEntity {
    pub source: Entity,
    pub destination: Entity,
}

impl Command for CloneEntity {
    // Copy all components from an entity to another.
    // Using an entity with no components as the destination creates a copy of the source entity.
    // Panics if:
    // - the components are not registered in the type registry,
    // - the world does not have a type registry
    // - the source or destination entity do not exist
    fn apply(self, world: &mut World) {
        let components = {
            let registry = world.get_resource::<AppTypeRegistry>().unwrap().read();

            world
                .get_entity(self.source)
                .unwrap()
                .archetype()
                .components()
                .map(|component_id| {
                    println!(
                        "{}",
                        world.components().get_info(component_id).unwrap().name()
                    );
                    world
                        .components()
                        .get_info(component_id)
                        .unwrap()
                        .type_id()
                        .unwrap()
                })
                .map(|type_id| {
                    println!("{:?}", type_id);
                    registry
                        .get(type_id)
                        .unwrap()
                        .data::<ReflectComponent>()
                        .unwrap()
                        .clone()
                })
                .collect::<Vec<_>>()
        };

        for component in components {
            let source = component
                .reflect(world.get_entity(self.source).unwrap())
                .unwrap()
                .clone_value();

            let mut destination = world.get_entity_mut(self.destination).unwrap();

            component.apply_or_insert(&mut destination, &*source);
        }
    }
}
