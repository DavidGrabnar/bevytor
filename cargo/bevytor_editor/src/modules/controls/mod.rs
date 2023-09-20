use crate::core::events::StartPlaying;
use crate::core::{to_dynamic_scene, OriginalEntityId};
use crate::modules::hierarchy::get_label;
use bevy::diagnostic::{Diagnostics, DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::entity::EntityMap;
use bevy::ecs::system::Command;
use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

#[derive(Event)]
struct ChangeCamera(Entity);

#[derive(Resource)]
pub struct ControlState {
    pub playing: bool,
    pub initial: bool,
    dynamic_scene_handle: Option<Handle<DynamicScene>>,
}

impl Default for ControlState {
    fn default() -> Self {
        Self {
            playing: false,
            initial: true,
            dynamic_scene_handle: None,
        }
    }
}

pub struct Controls;

impl Plugin for Controls {
    fn build(&self, app: &mut App) {
        app.register_type::<EditorCamera>()
            .add_event::<ChangeCamera>()
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .init_resource::<ControlState>()
            .add_systems(Update, (reset_world, change_camera));
    }
}

impl Controls {
    pub fn ui(ui: &mut egui::Ui, world: &mut World) {
        world.resource_scope(|world, mut state: Mut<ControlState>| {
            ui.columns(3, |cols| {
                cols[0].horizontal(|ui| {
                    let mut editor_camera = {
                        let camera = world
                            .query_filtered::<(Entity, &Camera), With<EditorCamera>>()
                            .get_single(world);

                        if camera.is_err() {
                            return
                        }

                        let camera = camera.unwrap();
                        (camera.0, camera.1.is_active)
                    };

                    if ui
                        .selectable_label(editor_camera.1, "Editor")
                        .clicked()
                    {
                        world.send_event(ChangeCamera(editor_camera.0));
                    }

                    let mut gameplay_cameras = {
                        world
                            .query_filtered::<(Entity, &Camera), Without<EditorCamera>>()
                            .iter(world)
                            .map(|c| (c.0, c.1.is_active))
                            .collect::<Vec<_>>()
                    };
                    if let Some(camera) = gameplay_cameras.get(0) {
                        if ui
                            .selectable_label(camera.1, "Gameplay")
                            .clicked()
                        {
                            world.send_event(ChangeCamera(camera.0));
                        }
                        if !editor_camera.1 {
                            let mut cameras_query = world
                                .query_filtered::<(Entity, &Camera, Option<&Name>), Without<EditorCamera>>();

                            let mut active = cameras_query.iter(world).filter(|r| r.1.is_active).next().unwrap();
                            let mut result = active.0;
                            egui::ComboBox::new("gameplay-camera", "")
                                .selected_text(get_label(active.0, active.2))
                                .show_ui(ui, |ui| {
                                    for (entity, camera, name) in cameras_query.iter(world) {
                                        ui.selectable_value(&mut result, entity, get_label(entity, name));
                                    }
                                });
                        }
                    } else {
                        ui.label("Add gameplay camera: Insert -> Camera");
                    }
                });
                cols[1].vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        if !state.playing && ui.button("▶").clicked() {
                            if state.initial {
                                let mut entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
                                for entity in entities {
                                    world
                                        .entity_mut(entity)
                                        .insert(OriginalEntityId(entity.index()));
                                }
                                let scene = to_dynamic_scene(world);
                                let scene_handle = world.resource_mut::<Assets<DynamicScene>>().add(scene);
                                state.dynamic_scene_handle = Some(scene_handle);
                                println!("set scene handle");
                            }
                            world.send_event(StartPlaying);
                        }
                        if state.playing && ui.button("⏸").clicked() {
                            // TODO when paused, all modifications of scene should be disabled (read-only access)
                            // TODO since stop will reset the scene to a state before clicking play !
                            state.playing = false;
                        }
                        if !state.initial && ui.button("■").clicked() {
                            state.playing = false;
                            world.send_event(ResetWorldEvent);
                        }
                    });
                });
                cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let diagnostics = world.resource::<DiagnosticsStore>();
                    ui.label(format!("{:.2}", diagnostics
                        .get(FrameTimeDiagnosticsPlugin::FPS)
                        .and_then(|fps| fps.average())
                        .unwrap())
                    );
                    ui.label("FPS (avg.): ");
                });
            });
        });
    }
}

#[derive(Component, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EditorCamera;

#[derive(Event)]
pub struct ResetWorldEvent;

pub struct ResetWorld;

impl Command for ResetWorld {
    fn apply(self, world: &mut World) {
        world.resource_scope(|world, mut state: Mut<ControlState>| {
            if let Some(handle) = &state.dynamic_scene_handle {
                world.resource_scope(|world, dynamic_scenes: Mut<Assets<DynamicScene>>| {
                    if let Some(scene) = dynamic_scenes.get(handle) {
                        let mut entity_map = EntityMap::default();
                        let mut query_state = world.query::<(Entity, &OriginalEntityId)>();
                        for (entity, original_id) in query_state.iter(world) {
                            entity_map.insert(Entity::from_raw(original_id.0), entity);
                        }
                        println!("{:?}", entity_map);
                        scene.write_to_world(world, &mut entity_map).unwrap();

                        let mut query_from_script =
                            world.query_filtered::<Entity, Without<OriginalEntityId>>();
                        for entity in query_from_script.iter(world).collect::<Vec<_>>() {
                            world.entity_mut(entity).despawn();
                        }
                    } else {
                        error!("Dynamic scene not found!")
                    }
                });
            } else {
                error!("Dynamic scene handle is None!")
            }
            state.initial = true;
        });
    }
}

pub fn reset_world(mut ev_reset_world: EventReader<ResetWorldEvent>, mut commands: Commands) {
    if ev_reset_world.iter().next().is_some() {
        commands.add(ResetWorld);
    }

    if ev_reset_world.iter().next().is_some() {
        warn!("Multiple ResetWorldEvent events found in listener! Should not happen");
    }
}

fn change_camera(mut ev: EventReader<ChangeCamera>, mut q: Query<(Entity, &mut Camera)>) {
    for e in ev.iter() {
        for (entity, mut camera) in q.iter_mut() {
            if e.0 == entity && !camera.is_active {
                camera.is_active = true;
            } else if e.0 != entity && camera.is_active {
                camera.is_active = false;
            }
        }
    }
}
