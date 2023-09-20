use crate::error::EResult;
use bevy::asset::{Handle, HandleId};
use bevy::prelude::*;
use bevy::reflect::{Array, List, ReflectMut, Tuple};
use bevy_egui::egui::{Grid, Ui};
use bevytor_core::SelectedEntity;
use inspectable::Inspectable;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};

mod inspectable;

pub struct Context<'a> {
    pub world: *mut World,
    pub registry: &'a InspectRegistry,
    pub collapsible: Option<String>,
    pub from_val: bool,
}

#[derive(Resource)]
pub struct InspectRegistry {
    impls: HashMap<TypeId, Box<fn(&mut dyn Any, &mut Ui, &mut Context) -> ()>>,
    skipped: HashSet<TypeId>,
}

impl Default for InspectRegistry {
    fn default() -> Self {
        let mut new = Self {
            impls: Default::default(),
            skipped: Default::default(),
        };
        new.skipped.insert(TypeId::of::<Parent>());
        new.skipped.insert(TypeId::of::<Children>());
        new.skipped.insert(TypeId::of::<HandleId>()); // temporary
        new.skipped.insert(TypeId::of::<SelectedEntity>());

        new.register::<f32>();
        new.register::<f64>();
        new.register::<usize>();
        new.register::<u8>();
        new.register::<u16>();
        new.register::<u32>();
        new.register::<u64>();
        new.register::<isize>();
        new.register::<i8>();
        new.register::<i16>();
        new.register::<i32>();
        new.register::<i64>();
        new.register::<bool>();
        new.register::<Vec3>();
        new.register::<Quat>();
        new.register::<Handle<StandardMaterial>>();
        // new.register::<Handle<Mesh>>();
        // new.register::<Transform>();
        new.register::<StandardMaterial>();
        new.register::<Name>();
        new
    }
}

impl InspectRegistry {
    pub fn register<T: Inspectable + 'static>(&mut self) {
        // println!("Register {:?}", TypeId::of::<T>());
        self.impls.insert(
            TypeId::of::<T>(),
            Box::new(|value: &mut dyn Any, ui: &mut Ui, context: &mut Context| {
                // TODO can this unsafe be avoided or elaborate on why it can be left here
                let casted: &mut T = unsafe { &mut *(value as *mut dyn Any as *mut T) };
                casted.ui(ui, context);
            }),
        );
    }

    pub fn exec_reflect(
        &self,
        value: &mut dyn Reflect,
        ui: &mut Ui,
        context: &mut Context,
    ) -> EResult<()> {
        // If type is registered, use UI impl, else use reflect to break it down
        let type_id = (*value).type_id();
        if self.skipped.contains(&type_id) {
            // println!("Skipped: {}", value.type_name());
            Ok(())
        } else if context.collapsible.is_some() {
            // if root -> collapsible & repeat
            ui.separator();
            ui.collapsing(context.collapsible.as_ref().unwrap().clone(), |ui| {
                context.collapsible = None;
                self.exec_reflect(value, ui, context)
            })
            .body_returned
            .unwrap_or(Ok(()))
        } else if let Some(callback) = self.impls.get(&type_id) {
            callback(value.as_any_mut(), ui, context);
            Ok(())
        } else {
            if context.from_val {
                ui.label(format!("WIP VALUE {}", value.type_name()));
                return Ok(());
            }
            match value.reflect_mut() {
                ReflectMut::Struct(val) => self.exec_reflect_struct(val, ui, context),
                ReflectMut::TupleStruct(val) => self.exec_reflect_tuple_struct(val, ui, context),
                ReflectMut::Tuple(val) => self.exec_reflect_tuple(val, ui, context),
                ReflectMut::List(val) => self.exec_reflect_list(val, ui, context),
                ReflectMut::Array(val) => self.exec_reflect_array(val, ui, context),
                ReflectMut::Map(_) => {
                    // TODO
                    ui.label(format!("WIP MAP {}", value.type_name()));
                    Ok(())
                }
                ReflectMut::Value(val) => {
                    context.from_val = true;
                    self.exec_reflect(val, ui, context)
                }
                ReflectMut::Enum(_) => {
                    // TODO
                    ui.label(format!("WIP ENUM {}", value.type_name()));
                    Ok(())
                }
            }
        }
    }

    pub fn exec_reflect_struct(
        &self,
        value: &mut dyn Struct,
        ui: &mut Ui,
        params: &mut Context,
    ) -> EResult<()> {
        ui.vertical(|ui| {
            let grid = Grid::new((*value).type_id());
            grid.show(ui, |ui| {
                for i in 0..value.field_len() {
                    match value.name_at(i) {
                        Some(name) => ui.label(name),
                        None => ui.label("<missing>"),
                    };
                    if let Some(field) = value.field_at_mut(i) {
                        self.exec_reflect(field, ui, params);
                    } else {
                        ui.label("<missing>");
                    }
                    ui.end_row();
                }
            });
        });

        Ok(())
    }

    pub fn exec_reflect_tuple_struct(
        &self,
        value: &mut dyn TupleStruct,
        ui: &mut Ui,
        params: &mut Context,
    ) -> EResult<()> {
        let grid = Grid::new((*value).type_id());
        grid.show(ui, |ui| {
            for i in 0..value.field_len() {
                ui.label(i.to_string());
                if let Some(field) = value.field_mut(i) {
                    self.exec_reflect(field, ui, params);
                } else {
                    ui.label("<missing>");
                }
                ui.end_row();
            }
        });

        Ok(())
    }

    pub fn exec_reflect_tuple(
        &self,
        value: &mut dyn Tuple,
        ui: &mut Ui,
        params: &mut Context,
    ) -> EResult<()> {
        let grid = Grid::new((*value).type_id());
        grid.show(ui, |ui| {
            for i in 0..value.field_len() {
                ui.label(i.to_string());
                if let Some(field) = value.field_mut(i) {
                    self.exec_reflect(field, ui, params);
                } else {
                    ui.label("<missing>");
                }
                ui.end_row();
            }
        });

        Ok(())
    }

    pub fn exec_reflect_list(
        &self,
        value: &mut dyn List,
        ui: &mut Ui,
        params: &mut Context,
    ) -> EResult<()> {
        let grid = Grid::new((*value).type_id());
        grid.show(ui, |ui| {
            for i in 0..value.len() {
                ui.label(i.to_string());
                if let Some(field) = value.get_mut(i) {
                    self.exec_reflect(field, ui, params);
                } else {
                    ui.label("<missing>");
                }
                ui.end_row();
            }
        });

        Ok(())
    }

    pub fn exec_reflect_array(
        &self,
        value: &mut dyn Array,
        ui: &mut Ui,
        params: &mut Context,
    ) -> EResult<()> {
        let grid = Grid::new((*value).type_id());
        grid.show(ui, |ui| {
            for i in 0..value.len() {
                ui.label(i.to_string());
                if let Some(field) = value.get_mut(i) {
                    self.exec_reflect(field, ui, params);
                } else {
                    ui.label("<missing>");
                }
                ui.end_row();
            }
        });

        Ok(())
    }
}
