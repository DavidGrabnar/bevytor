mod tree;

use std::borrow::Borrow;
use bevy::prelude::*;
use bevy_egui::egui::Slider;
use bevy_egui::{egui, EguiContext, EguiPlugin};
use std::collections::HashMap;

#[derive(Default)]
struct InspectorState {
    selected_entity: Box<Option<Entity>>,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .init_resource::<InspectorState>()
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
    query: Query<(Entity, Option<&Parent>, Option<&Children>)>,
    mut inspector_state: ResMut<InspectorState>,
) {
    let mut entity_children: HashMap<Entity, &Children> = HashMap::new();
    for (entity, _parent, children) in query.iter() {
        if let Some(some_children) = children {
            entity_children.insert(entity, some_children);
        }
    }
    let mut parents = vec![];
    for (entity, parent, _children) in query.iter() {
        if parent.is_none() {
            parents.push(build_node(entity, &entity_children));
        }
    }
    egui::Window::new("Hierarchy").show(egui_context.ctx_mut(), |ui| {
        let root = tree::Node::new(None, parents);
        let tree = tree::Tree::new(root);

        let action = tree.ui(ui);
        if let tree::Action::Selected(id) = action {
            inspector_state.selected_entity = Box::new(Some(id));
        }
    });
}

fn setup_ui_inspector(
    mut egui_context: ResMut<EguiContext>,
    mut query: Query<(Entity, &mut Transform)>,
    inspector_state: Res<InspectorState>,
) {
    if let Some(open_id) = inspector_state.selected_entity.borrow() {
        for (entity, mut transform) in query.iter_mut() {
            if entity.id() == open_id.id() {
                egui::Window::new("Inspector").show(egui_context.ctx_mut(), |ui| {
                    ui.add(Slider::new(&mut transform.translation.x, -10.0..=10.0).text("X"));
                    ui.add(Slider::new(&mut transform.translation.y, -10.0..=10.0).text("Y"));
                    ui.add(Slider::new(&mut transform.translation.z, -10.0..=10.0).text("Z"));
                });
            }
        }
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
