pub mod registry;

use crate::modules::hierarchy::get_label;
use crate::modules::inspector::registry::{Context, InspectRegistry};
use bevy::prelude::*;
use bevy_egui::egui;
use bevytor_core::SelectedEntity;
use bevytor_script::ComponentRegistry;

pub struct Inspector;

impl Plugin for Inspector {
    fn build(&self, app: &mut App) {
        app.init_resource::<InspectRegistry>();
    }
}

impl Inspector {
    pub fn ui(ui: &mut egui::Ui, world: &mut World) {
        if let Ok((entity, name)) = world
            .query_filtered::<(Entity, Option<&Name>), With<SelectedEntity>>()
            .get_single_mut(world)
        {
            let label = get_label(entity, name);
            ui.label(label);

            let mut component_type_ids = Vec::new();
            for archetype in world.archetypes().iter() {
                let mut found = false;
                for archetype_entity in archetype.entities() {
                    if archetype_entity.entity() == entity {
                        found = true;
                    }
                }
                if found {
                    for component_id in archetype.components() {
                        let comp_info = world.components().get_info(component_id).unwrap();
                        component_type_ids
                            .push((comp_info.type_id().unwrap(), comp_info.name().to_string()));
                    }
                    break;
                }
            }

            world.resource_scope(|world, inspect_registry: Mut<InspectRegistry>| {
                world.resource_scope(|world, type_registry_arc: Mut<AppTypeRegistry>| {
                    for (component_type_id, component_name) in component_type_ids {
                        let type_registry = type_registry_arc.read();
                        if let Some(registration) = type_registry.get(component_type_id) {
                            world.resource_scope(|world, comp_registry: Mut<ComponentRegistry>| {
                                let reflect_component =
                                        //if let Some((_, _, reflect_component)) =
                                        //    comp_registry.reg.get(&component_type_id)
                                        //{
                                        //    reflect_component
                                        //} else {
                                        registration.data::<ReflectComponent>().unwrap();
                                //};

                                let context = &mut Context {
                                    world,
                                    registry: &inspect_registry,
                                    collapsible: Some(
                                        component_name.rsplit_once(':').unwrap().1.to_string(),
                                    ),
                                    from_val: false,
                                };
                                let mut entity_mut = world.get_entity_mut(entity).unwrap();
                                let reflect =
                                    reflect_component.reflect_mut(&mut entity_mut).unwrap();

                                inspect_registry
                                    .exec_reflect(reflect.into_inner(), ui, context)
                                    .unwrap();
                            });
                        } else {
                            //println!(
                            //    "NOT IN TYPE REGISTRY {:?}: {}",
                            //    component_type_id, component_name
                            //);
                        }

                        // callback(reflect.as_any_mut(), ui);
                        // }
                    }
                });
            });
        }
    }
}
