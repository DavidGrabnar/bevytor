use bevy::prelude::{Entity, Event};

#[derive(Event)]
pub struct SelectEntity(pub Entity);

#[derive(Event)]
pub struct StartPlaying;
