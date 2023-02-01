pub mod tree;

#[derive(Component)]
pub struct SelectedEntity;

use crate::tree::{Node, Tree};
use bevy::ecs::entity::Entities;
use bevy::prelude::*;
use bevy_egui::egui::{DragValue, Grid};
use bevy_egui::{egui, EguiContext};
use std::collections::HashMap;

pub fn setup_ui_hierarchy(
    mut egui_context: ResMut<EguiContext>,
    query_hierarchy: Query<(Entity, Option<&Parent>, Option<&Children>, Option<&Name>)>,
    query_selected_entity: Query<Entity, With<SelectedEntity>>,
    mut commands: Commands,
    entities: &Entities,
) {
    let tree = update_state_hierarchy(query_hierarchy, entities);
    bevy_egui::egui::Window::new("Hierarchy").show(egui_context.ctx_mut(), |ui| {
        let action = show_ui_hierarchy(ui, &tree);
        if let tree::Action::Selected(selected_entity) = action {
            for entity in query_selected_entity.iter() {
                commands.entity(entity).remove::<SelectedEntity>();
            }
            commands.entity(selected_entity).insert(SelectedEntity);
        }
    });
}

pub fn update_state_hierarchy(
    hierarchy: Query<(Entity, Option<&Parent>, Option<&Children>, Option<&Name>)>,
    entities: &Entities,
) -> Tree {
    let mut entity_name_map: HashMap<Entity, String> = HashMap::new();
    for (entity, _parent, _children, name) in hierarchy.iter() {
        let label = name
            .map(|n| n.as_str().to_string())
            .unwrap_or(format!("Entity {}", entity.index()));
        entity_name_map.insert(entity, label);
    }

    let mut entity_children: HashMap<Entity, Vec<(&Entity, &String)>> = HashMap::new();
    for (entity, _parent, children, _name) in hierarchy.iter() {
        if let Some(some_children) = children {
            let mut existing_children = some_children
                .iter()
                .filter(|entity| entities.contains(**entity))
                .map(|entity| (entity, entity_name_map.get(entity).unwrap()))
                .collect::<Vec<_>>();
            existing_children.sort_by_key(|entity| entity.0.index()); // TODO remove???
            entity_children.insert(entity, existing_children);
        }
    }
    let mut parents = vec![];
    for (entity, parent, _children, name) in hierarchy.iter() {
        if parent.is_none() {
            let x = build_node(
                (entity, entity_name_map.get(&entity).unwrap().to_string()),
                &entity_children,
            );
            parents.push(x);
        }
    }

    let root = Node::new(None, parents);
    tree::Tree::new(root)
}

pub fn show_ui_hierarchy(ui: &mut egui::Ui, tree: &tree::Tree) -> tree::Action {
    tree.ui(ui)
}

pub fn setup_ui_inspector(
    mut egui_context: ResMut<EguiContext>,
    mut query_selected_entity: Query<
        (Option<&mut Transform>, Option<&Handle<StandardMaterial>>),
        With<SelectedEntity>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Ok((transform_option, standard_material_handle_option)) =
        query_selected_entity.get_single_mut()
    {
        egui::Window::new("Inspector").show(egui_context.ctx_mut(), |ui| {
            if let Some(mut transform) = transform_option {
                Grid::new("transformation").num_columns(2).show(ui, |ui| {
                    ui.label("Translation");
                    ui.horizontal(|ui| {
                        ui.add(
                            DragValue::new(&mut transform.translation.x)
                                .fixed_decimals(2)
                                .speed(0.1),
                        );
                        ui.add(
                            DragValue::new(&mut transform.translation.y)
                                .fixed_decimals(2)
                                .speed(0.1),
                        );
                        ui.add(
                            DragValue::new(&mut transform.translation.z)
                                .fixed_decimals(2)
                                .speed(0.1),
                        );
                    });
                    ui.end_row();
                    ui.label("Rotation");
                    ui.horizontal(|ui| {
                        ui.add(
                            build_rotation_drag_value_input(&mut transform, &EulerRot::XYZ)
                                .fixed_decimals(2)
                                .speed(0.01),
                        );
                        ui.add(
                            build_rotation_drag_value_input(&mut transform, &EulerRot::YZX)
                                .fixed_decimals(2)
                                .speed(0.01),
                        );
                        ui.add(
                            build_rotation_drag_value_input(&mut transform, &EulerRot::ZXY)
                                .fixed_decimals(2)
                                .speed(0.01),
                        );
                    });
                    ui.end_row();
                    ui.label("Scale");
                    ui.horizontal(|ui| {
                        ui.add(
                            DragValue::new(&mut transform.scale.x)
                                .fixed_decimals(2)
                                .speed(0.1),
                        );
                        ui.add(
                            DragValue::new(&mut transform.scale.y)
                                .fixed_decimals(2)
                                .speed(0.1),
                        );
                        ui.add(
                            DragValue::new(&mut transform.scale.z)
                                .fixed_decimals(2)
                                .speed(0.1),
                        );
                    });
                    ui.end_row();
                });

                ui.separator();
            }

            if let Some(standard_material_handle) = standard_material_handle_option {
                if let Some(standard_material) = materials.get_mut(standard_material_handle) {
                    Grid::new("standard-material")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Base color");
                            let mut base_color = standard_material.base_color.as_rgba_f32();
                            ui.color_edit_button_rgba_unmultiplied(&mut base_color);
                            standard_material.base_color.set_r(base_color[0]);
                            standard_material.base_color.set_g(base_color[1]);
                            standard_material.base_color.set_b(base_color[2]);
                            standard_material.base_color.set_a(base_color[3]);
                            ui.end_row();
                        });
                } else {
                    eprintln!("Got handle, but material not found");
                }
            }
        });
    } else {
        // no selected entity
    }
}

/*pub fn show_ui_inspector(ui: &mut egui::Ui) {
    if let Some(mut transform) = transform_option {
        Grid::new("transformation")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Translation");
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut transform.translation.x).fixed_decimals(2).speed(0.1));
                    ui.add(DragValue::new(&mut transform.translation.y).fixed_decimals(2).speed(0.1));
                    ui.add(DragValue::new(&mut transform.translation.z).fixed_decimals(2).speed(0.1));
                });
                ui.end_row();
                ui.label("Rotation");
                ui.horizontal(|ui| {
                    ui.add(build_rotation_drag_value_input(&mut transform, &EulerRot::XYZ).fixed_decimals(2).speed(0.01));
                    ui.add(build_rotation_drag_value_input(&mut transform, &EulerRot::YZX).fixed_decimals(2).speed(0.01));
                    ui.add(build_rotation_drag_value_input(&mut transform, &EulerRot::ZXY).fixed_decimals(2).speed(0.01));
                });
                ui.end_row();
                ui.label("Scale");
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut transform.scale.x).fixed_decimals(2).speed(0.1));
                    ui.add(DragValue::new(&mut transform.scale.y).fixed_decimals(2).speed(0.1));
                    ui.add(DragValue::new(&mut transform.scale.z).fixed_decimals(2).speed(0.1));
                });
                ui.end_row();
            });

        ui.separator();
    }

    if let Some(standard_material_handle) = standard_material_handle_option {
        if let Some(standard_material) = materials.get_mut(standard_material_handle) {
            Grid::new("standard-material")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Base color");
                    let mut base_color = standard_material.base_color.as_rgba_f32();
                    ui.color_edit_button_rgba_unmultiplied(&mut base_color);
                    standard_material.base_color.set_r(base_color[0]);
                    standard_material.base_color.set_g(base_color[1]);
                    standard_material.base_color.set_b(base_color[2]);
                    standard_material.base_color.set_a(base_color[3]);
                    ui.end_row();
                });
        } else {
            eprintln!("Got handle, but material not found");
        }
    }
}*/

fn build_node(
    entity: (Entity, String),
    entity_children: &HashMap<Entity, Vec<(&Entity, &String)>>,
) -> tree::Node {
    let mut child_nodes = vec![];

    if let Some(children) = entity_children.get(&entity.0) {
        for child in children.iter() {
            child_nodes.push(build_node(
                (*child.0, (*child.1.clone()).to_string()),
                entity_children,
            ));
        }
    }

    tree::Node::new(Some(entity), child_nodes)
}

fn build_rotation_drag_value_input<'a>(
    transform: &'a mut Transform,
    euler_rot: &'a EulerRot,
) -> DragValue<'a> {
    DragValue::from_get_set(|input| {
        if let Some(value) = input {
            let euler = transform.rotation.to_euler(*euler_rot);
            transform.rotate(Quat::from_euler(
                *euler_rot,
                value as f32 - euler.0,
                0.0,
                0.0,
            ));
        }
        transform.rotation.to_euler(*euler_rot).0 as f64
    })
}
