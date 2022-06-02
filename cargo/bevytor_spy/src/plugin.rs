use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevytor_core::{setup_ui_hierarchy, setup_ui_inspector};

pub struct SpyPlugin;

impl Plugin for SpyPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugin(EguiPlugin)
            .add_system(setup_ui_hierarchy)
            .add_system(setup_ui_inspector);
    }
}
