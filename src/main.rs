mod tree;

use bevy::prelude::*;
use bevy_egui::egui::Slider;
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
    mut commands: Commands
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
        if let tree::Action::Selected(selectedEntity) = action {
            for entity in query_selected_entity.iter() {
                commands.entity(entity).remove::<SelectedEntity>();
            }
            commands.entity(selectedEntity).insert(SelectedEntity);
        }
    });
}

fn setup_ui_inspector(
    mut egui_context: ResMut<EguiContext>,
    mut query_selected_entity: Query<&mut Transform, With<SelectedEntity>>
) {
    if let Ok(mut transform) = query_selected_entity.get_single_mut() {
        egui::Window::new("Inspector").show(egui_context.ctx_mut(), |ui| {
            ui.add(Slider::new(&mut transform.translation.x, -10.0..=10.0).text("X"));
            ui.add(Slider::new(&mut transform.translation.y, -10.0..=10.0).text("Y"));
            ui.add(Slider::new(&mut transform.translation.z, -10.0..=10.0).text("Z"));
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
