use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde::{Deserialize, Serialize};

pub mod events;
pub mod popup;

pub fn to_dynamic_scene(world: &World) -> DynamicScene {
    let mut builder = DynamicSceneBuilder::from_world(world);
    builder.deny_all_resources().extract_resources();
    builder
        .allow_all()
        .deny::<ComputedVisibility>()
        .deny::<Wireframe>()
        .deny::<Window>()
        .deny::<PrimaryWindow>()
        .deny::<OriginalEntityId>()
        .extract_entities(world.iter_entities().map(|r| r.id()))
        .remove_empty_entities();

    let scene = builder.build();
    scene
}

#[derive(Component, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct OriginalEntityId(pub u32);
