mod tree;

use bevy::prelude::*;
use bevy_egui::egui::{DragValue, Grid};
use bevy_egui::{egui, EguiContext, EguiPlugin};
use std::collections::HashMap;

#[derive(Component)]
struct SelectedEntity;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_startup_system(setup_scene)
        .add_system(setup_ui_hierarchy)
        .add_system(setup_ui_inspector)
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // set up the camera
    let mut camera = OrthographicCameraBundle::new_3d();
    camera.orthographic_projection.scale = 3.0;
    camera.transform = Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);

    // camera
    commands.spawn_bundle(camera);

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
        .insert(Parent(cube));
    // light
    commands.spawn_bundle(PointLightBundle {
        transform: Transform::from_xyz(3.0, 8.0, 5.0),
        ..Default::default()
    });
}

fn setup_ui_hierarchy(
    mut egui_context: ResMut<EguiContext>,
    query_hierarchy: Query<(Entity, Option<&Parent>, Option<&Children>)>,
    query_selected_entity: Query<Entity, With<SelectedEntity>>,
    mut commands: Commands,
) {
    let mut entity_children: HashMap<Entity, &Children> = HashMap::new();
    for (entity, _parent, children) in query_hierarchy.iter() {
        if let Some(some_children) = children {
            entity_children.insert(entity, some_children);
        }
    }
    let mut parents = vec![];
    for (entity, parent, _children) in query_hierarchy.iter() {
        if parent.is_none() {
            parents.push(build_node(entity, &entity_children));
        }
    }
    egui::Window::new("Hierarchy").show(egui_context.ctx_mut(), |ui| {
        let root = tree::Node::new(None, parents);
        let tree = tree::Tree::new(root);

        let action = tree.ui(ui);
        if let tree::Action::Selected(selected_entity) = action {
            for entity in query_selected_entity.iter() {
                commands.entity(entity).remove::<SelectedEntity>();
            }
            commands.entity(selected_entity).insert(SelectedEntity);
        }
    });
}

fn setup_ui_inspector(
    mut egui_context: ResMut<EguiContext>,
    mut query_selected_entity: Query<(Option<&mut Transform>, Option<&Handle<StandardMaterial>>), With<SelectedEntity>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Ok((transform_option, standard_material_handle_option)) = query_selected_entity.get_single_mut() {
        egui::Window::new("Inspector").show(egui_context.ctx_mut(), |ui| {
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
        });
    }
}

fn build_node(entity: Entity, entity_children: &HashMap<Entity, &Children>) -> tree::Node {
    let mut child_nodes = vec![];

    if let Some(children) = entity_children.get(&entity) {
        for child in children.iter() {
            child_nodes.push(build_node(*child, entity_children));
        }
    }

    tree::Node::new(Some(entity), child_nodes)
}

fn build_rotation_drag_value_input<'a>(transform: &'a mut Transform, euler_rot: &'a EulerRot) -> DragValue<'a> {
    DragValue::from_get_set(|input| {
        if let Some(value) = input {
            let euler = transform.rotation.to_euler(*euler_rot);
            transform.rotate(Quat::from_euler(*euler_rot, value as f32 - euler.0, 0.0, 0.0));
        }
        transform.rotation.to_euler(*euler_rot).0 as f64
    })
}
