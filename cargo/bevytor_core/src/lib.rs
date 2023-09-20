#[derive(Component)]
pub struct SelectedEntity;

use bevy::ecs::entity::Entities;
use bevy::prelude::*;
use bevy_egui::egui::{DragValue, Grid};
use bevy_egui::{egui, EguiContext, EguiContexts};
use std::collections::HashMap;

pub fn setup_ui_inspector(
    mut egui_context: EguiContexts,
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
