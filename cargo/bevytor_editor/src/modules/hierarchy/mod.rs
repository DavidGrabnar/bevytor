use crate::core::events::SelectEntity;
use crate::modules::hierarchy::tree::NodeAction;
use crate::third_party::clone_entity::CloneEntity;
use bevy::ecs::entity::Entities;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy_egui::egui;
use bevytor_core::SelectedEntity;
use std::collections::HashMap;
pub use tree::Tree;
pub use tree::TreeAction;

mod tree;

#[derive(Event)]
pub struct MoveEntity(Entity, Option<Entity>);

#[derive(Event)]
struct DuplicateEntity(Entity);

#[derive(Event)]
struct RemoveEntity(Entity);

pub struct Hierarchy;

impl Plugin for Hierarchy {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tree>()
            .add_event::<MoveEntity>()
            .add_event::<DuplicateEntity>()
            .add_event::<RemoveEntity>()
            .add_systems(Update, update_state_hierarchy)
            .add_systems(Update, move_entity)
            .add_systems(Update, duplicate_entity)
            .add_systems(Update, remove_entity);
    }
}

impl Hierarchy {
    pub fn ui(ui: &mut egui::Ui, world: &mut World) {
        let tree = world.resource::<Tree>();
        ui.label("Hierarchy");
        ui.separator();
        match tree.ui(ui) {
            TreeAction::Node(action) => match action {
                NodeAction::Select(e) => world.send_event(SelectEntity(e)),
                NodeAction::Clone(e) => world.send_event(DuplicateEntity(e)),
                NodeAction::Remove(e) => world.send_event(RemoveEntity(e)),
            },
            TreeAction::Move(dragged, dropped) => {
                world.send_event(MoveEntity(dragged, dropped.entity()))
            }
            TreeAction::NoAction => {}
        }
    }
}

fn move_entity(mut commands: Commands, mut ev_move_entity: EventReader<MoveEntity>) {
    for entity in ev_move_entity.iter() {
        match entity.1 {
            Some(parent) => commands.entity(entity.0).set_parent(parent),
            None => commands.entity(entity.0).remove_parent(),
        };
    }
}

fn duplicate_entity(
    mut commands: Commands,
    mut reader: EventReader<DuplicateEntity>,
    mut writer: EventWriter<SelectEntity>,
) {
    for entity in reader.iter() {
        commands
            .entity(entity.0)
            .remove::<(SelectedEntity, Wireframe)>();

        let destination = commands.spawn_empty().id();
        let clone = CloneEntity {
            source: entity.0,
            destination,
        };

        commands.add(clone);
        writer.send(SelectEntity(destination));
    }
}

fn remove_entity(mut commands: Commands, mut reader: EventReader<RemoveEntity>) {
    for entity in reader.iter() {
        commands.entity(entity.0).despawn_recursive();
    }
}

// consider using HierarchyEvents to keep it updated
// not hierarchy data won't be handled by them (ex.: Name label, etc.?)
fn update_state_hierarchy(
    hierarchy: Query<(Entity, Option<&Parent>, Option<&Children>, Option<&Name>)>,
    entities: &Entities,
    mut commands: Commands,
) {
    let mut entity_name_map: HashMap<Entity, String> = HashMap::new();
    for (entity, _parent, _children, name) in hierarchy.iter() {
        entity_name_map.insert(entity, get_label(entity, name));
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
    for (entity, parent, _children, _name) in hierarchy.iter() {
        if parent.is_none() {
            let x = build_node(
                (entity, entity_name_map.get(&entity).unwrap().to_string()),
                &entity_children,
            );
            parents.push(x);
        }
    }

    let root = tree::Node::new(None, parents);
    commands.insert_resource(Tree::new(root));
}

pub fn get_label(entity: Entity, name: Option<&Name>) -> String {
    let label = name.map(|n| n.as_str()).unwrap_or("/");

    format!("#{} - {}", entity.index(), label)
}

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
