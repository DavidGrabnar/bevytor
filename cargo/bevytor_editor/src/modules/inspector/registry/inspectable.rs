use bevy::math::{EulerRot, Quat, Vec2, Vec3};
use bevy_egui::egui::{Checkbox, Ui};
use bevy::pbr::StandardMaterial;
use bevy::asset::{Asset, Assets, Handle};
use bevy::prelude::{Mut, Reflect, Transform};
use bevy::core::Name;
use bevy_egui::egui;
use crate::modules::inspector::registry::Context;

pub trait Inspectable {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context);
}

macro_rules! impl_inspectable_numeric {
    ($($uint_type: ty),*) => {
        $(
            impl Inspectable for $uint_type {
                fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
                    ui_num(self, ui);
                }
            }
        )*
    }
}

fn ui_num<T: egui::emath::Numeric>(value: &mut T, ui: &mut Ui) {
    ui.add(egui::DragValue::new(value).fixed_decimals(2).speed(0.1));
}

impl_inspectable_numeric!(usize, u8, u16, u32, u64, isize, i8, i16, i32, i64, f32, f64);

impl Inspectable for Transform {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        self.translation.ui(ui, context);
        self.scale.ui(ui, context);
        // self.rotation.ui(ui, context);
    }
}

impl Inspectable for Vec2 {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        ui.horizontal(|ui| {
            ui_num(&mut self.x, ui);
            ui_num(&mut self.y, ui);
        });
    }
}

impl Inspectable for Vec3 {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        ui.horizontal(|ui| {
            ui_num(&mut self.x, ui);
            ui_num(&mut self.y, ui);
            ui_num(&mut self.z, ui);
        });
    }
}

impl Inspectable for Quat {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        ui.horizontal(|ui| {
            let (x, y, z) = self.to_euler(EulerRot::XYZ);
            let mut new_x = x.to_degrees();
            let mut new_y = y.to_degrees();
            let mut new_z = z.to_degrees();
            ui_num(&mut new_x, ui);
            ui_num(&mut new_y, ui);
            ui_num(&mut new_z, ui);

            let (old_x, old_y, old_z) = self.to_euler(EulerRot::XYZ);
            if new_x != old_x || new_y != old_y || new_z != old_z {
                let new = Quat::from_euler(
                    EulerRot::XYZ,
                    new_x.to_radians(),
                    new_y.to_radians(),
                    new_z.to_radians(),
                );
                self.x = new.x;
                self.y = new.y;
                self.z = new.z;
                self.w = new.w;
            }
        });
    }
}

impl Inspectable for bool {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        ui.add(Checkbox::new(self, "TODO"));
    }
}

impl<T: Asset + Reflect> Inspectable for Handle<T> {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        // UNSAFE try to narrow the scope of unsafe - EXCLUSIVELY FOR RESOURCE MODIFICATION OR READ-ONLY OTHERWISE THIS CAN GO KABOOM
        unsafe {
            let world = &mut *context.world;
            world.resource_scope(|world, mut res: Mut<Assets<T>>| {
                let value = res.get_mut(self);
                context
                    .registry
                    .exec_reflect(value.unwrap(), ui, context)
                    .unwrap();
            });
        }
    }
}

impl Inspectable for StandardMaterial {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        ui.horizontal(|ui| {
            ui.label("base_color");
            let mut color: [f32; 4] = self.base_color.into();
            ui.color_edit_button_rgba_unmultiplied(&mut color);
            self.base_color.set_r(color[0]);
            self.base_color.set_g(color[1]);
            self.base_color.set_b(color[2]);
            self.base_color.set_a(color[3]);
        });
    }
}

impl Inspectable for Name {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        self.mutate(|name| {
            ui.text_edit_singleline(name);
        });
    }
}
