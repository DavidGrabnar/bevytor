/*
General TODOs:
- handle unwraps as errors
*/

use crate::error::{EResult, Error};
use crate::service::existing_projects::ExistingProjects;
use crate::service::project::Project;
use crate::ui::file_explorer::show_ui_file_editor;
use crate::ui::project::{project_list, ProjectListAction};
use crate::{bail, TypeRegistry};
use bevy::asset::{Asset, HandleId, SourceMeta};
use bevy::ecs::archetype::Archetypes;
use bevy::ecs::component::Components;
use bevy::ecs::entity::Entities;
use bevy::ecs::world::EntityRef;
use bevy::gltf::{Gltf, GltfMesh, GltfPrimitive};
use bevy::prelude::*;
use bevy::reflect::erased_serde::deserialize;
use bevy::reflect::{ReflectMut, TypeRegistryArc, TypeUuid};
use bevy::render::camera::{CameraProjection, Projection};
use bevy::render::mesh::{Indices, MeshVertexAttributeId, VertexAttributeValues};
use bevy::utils::Uuid;
use bevy_egui::egui::{Checkbox, Grid, Ui};
use bevy_egui::{egui, EguiContext, EguiPlugin};
use bevytor_core::tree::{Action, Tree};
use bevytor_core::{
    setup_ui_hierarchy, setup_ui_inspector, show_ui_hierarchy, update_state_hierarchy,
    SelectedEntity,
};
use serde::ser::{SerializeStruct, SerializeTuple};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::any;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub struct EditorPlugin {
    widgets: Vec<Box<dyn Widget + Sync + Send>>,
}

impl Default for EditorPlugin {
    fn default() -> Self {
        Self {
            widgets: vec![Box::new(Hierarchy::default())],
        }
    }
}

#[derive(Default)]
pub struct EditorState {
    // TODO re/load existing projects only when needed: on start, window opened, new project opened/created
    existing_projects: ExistingProjects,
    current_file_explorer_path: PathBuf,
    current_project: Option<Project>,
    tree: Tree,
}

pub trait Widget {
    fn show_ui(&self, ui: &mut Ui);
    fn update_state(&self);
}

#[derive(Default)]
struct Test(u8);

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Clone)]
struct AssetEntry {
    filename: String,
    type_uuid: String,
    uid: u64,
    // #[serde(skip_serializing, skip_deserializing)]
    // gltf_handle: Option<Handle<Gltf>>,
    // #[serde(skip_serializing, skip_deserializing)]
    // asset_handle: Option<Handle<Mesh>>, // TODO make dynamic
}

struct InspectRegistry {
    impls: HashMap<TypeId, Box<fn(&mut dyn Any, &mut egui::Ui, &mut Context) -> ()>>,
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
        new.register::<bool>();
        new.register::<Vec3>();
        new.register::<Quat>();
        new.register::<Handle<StandardMaterial>>();
        // new.register::<Handle<Mesh>>();
        // new.register::<Transform>();
        new.register::<StandardMaterial>();
        new
    }
}

impl InspectRegistry {
    pub fn register<T: Inspectable + 'static>(&mut self) {
        // println!("Register {:?}", TypeId::of::<T>());
        self.impls.insert(
            TypeId::of::<T>(),
            Box::new(
                |value: &mut dyn Any, ui: &mut egui::Ui, context: &mut Context| {
                    // TODO can this usafe be avoided or elaborate on why it can be left here
                    let casted: &mut T = unsafe { &mut *(value as *mut dyn Any as *mut T) };
                    casted.ui(ui, context);
                },
            ),
        );
    }

    pub fn exec_reflect(
        &self,
        value: &mut dyn Reflect,
        ui: &mut egui::Ui,
        mut context: &mut Context,
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
                self.exec_reflect(value, ui, &mut context);
            });
            Ok(())
        } else if let Some(callback) = self.impls.get(&type_id) {
            callback(value.as_any_mut(), ui, context);
            Ok(())
        } else {
            match value.reflect_mut() {
                ReflectMut::Struct(val) => self.exec_reflect_struct(val, ui, context),
                ReflectMut::TupleStruct(val) => self.exec_reflect_tuple_struct(val, ui, context),
                ReflectMut::Tuple(_) => {
                    todo!("WIP {}", value.type_name())
                }
                ReflectMut::List(_) => {
                    todo!("WIP {}", value.type_name())
                }
                ReflectMut::Array(_) => {
                    todo!("WIP {}", value.type_name())
                }
                ReflectMut::Map(_) => {
                    todo!("WIP {}", value.type_name())
                }
                ReflectMut::Value(val) => {
                    todo!("WIP {}", value.type_name());
                    bail!("INSPECT_REGISTRY::EXEC::IMPL_NOT_FOUND");
                }
            }
            // println!("NOTFOUND {:?}", type_id);
        }
    }

    pub fn exec_reflect_struct(
        &self,
        value: &mut dyn Struct,
        ui: &mut egui::Ui,
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
        ui: &mut egui::Ui,
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
}

trait Inspectable {
    fn ui(&mut self, ui: &mut egui::Ui, context: &mut Context);
}

impl Inspectable for Transform {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        self.translation.ui(ui, context);
        self.scale.ui(ui, context);
        // self.rotation.ui(ui, context);
    }
}

impl Inspectable for Vec2 {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        ui.horizontal(|ui| {
            UiRegistry::ui_num(&mut self.x, ui);
            UiRegistry::ui_num(&mut self.y, ui);
        });
    }
}

impl Inspectable for Vec3 {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        ui.horizontal(|ui| {
            UiRegistry::ui_num(&mut self.x, ui);
            UiRegistry::ui_num(&mut self.y, ui);
            UiRegistry::ui_num(&mut self.z, ui);
        });
    }
}

impl Inspectable for Quat {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        ui.horizontal(|ui| {
            ui.label("TODO");
        });
    }
}

impl Inspectable for f32 {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        UiRegistry::ui_num(self, ui);
    }
}

impl Inspectable for bool {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        ui.add(Checkbox::new(self, "TODO"));
    }
}

impl<T: Asset + Inspectable> Inspectable for Handle<T> {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
        // UNSAFE try to narrow the scope of unsafe - EXCLUSIVELY FOR RESOURCE MODIFICATION OR READ-ONLY OTHERWISE THIS CAN GO KABOOM
        unsafe {
            let world = &mut *context.world;
            world.resource_scope(|world, mut res: Mut<Assets<T>>| {
                let value = res.get_mut(self);
                value.unwrap().ui(ui, context);
            });
        }
    }
}

impl Inspectable for StandardMaterial {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context) {
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

struct SerializeRegistry {
    impls: HashMap<TypeId, Box<fn(&mut dyn Any) -> String>>,
}

impl Default for SerializeRegistry {
    fn default() -> Self {
        let mut new = Self {
            impls: Default::default(),
        };

        new.register::<f32>();
        new.register::<bool>();
        new.register::<Vec3>();
        new.register::<Quat>();
        // new.register::<Handle<StandardMaterial>>();
        // new.register::<Handle<Mesh>>();
        // new.register::<Transform>();
        // new.register::<StandardMaterial>();
        new
    }
}

impl SerializeRegistry {
    pub fn register<T: Serializable + 'static>(&mut self) {
        self.impls.insert(
            TypeId::of::<T>(),
            Box::new(|value: &mut dyn Any| -> String {
                // TODO can this usafe be avoided or elaborate on why it can be left here
                let casted: &mut T = unsafe { &mut *(value as *mut dyn Any as *mut T) };
                casted.serialize()
            }),
        );
    }

    // pub fn exec_reflect(&self, value: &mut dyn Reflect) -> EResult<String> {
    //     // If type is registered, use UI impl, else use reflect to break it down
    //     let type_id = (*value).type_id();
    //     if let Some(callback) = self.impls.get(&type_id) {
    //         Ok(callback(value.as_any_mut()))
    //     } else {
    //         match value.reflect_mut() {
    //             ReflectMut::Struct(val) => {
    //                 self.exec_reflect_struct(val)
    //             }
    //             ReflectMut::TupleStruct(val) => {
    //                 self.exec_reflect_tuple_struct(val)
    //             }
    //             ReflectMut::Tuple(_) => {
    //                 todo!("WIP {}", value.type_name())
    //             }
    //             ReflectMut::List(_) => {
    //                 todo!("WIP {}", value.type_name())
    //             }
    //             ReflectMut::Array(_) => {
    //                 todo!("WIP {}", value.type_name())
    //             }
    //             ReflectMut::Map(_) => {
    //                 todo!("WIP {}", value.type_name())
    //             }
    //             ReflectMut::Value(val) => {
    //                 todo!("WIP {}", value.type_name());
    //                 bail!("INSPECT_REGISTRY::EXEC::IMPL_NOT_FOUND");
    //             }
    //         }
    //         // println!("NOTFOUND {:?}", type_id);
    //     }
    // }
    //
    // pub fn exec_reflect_struct(&self, value: &mut dyn Struct) -> EResult<String> {
    //     let mut out = String::from("{");
    //
    //     for i in 0..value.field_len() {
    //         let name = value.name_at(i).unwrap_or("<missing>");
    //
    //         let value = if let Some(field) = value.field_at_mut(i) {
    //             self.exec_reflect(field).unwrap().as_str()
    //         } else {
    //             "<missing>"
    //         };
    //
    //         out.push_str(format!("\t\"{}\": {}", name, value).as_str())
    //     }
    //
    //     out.push_str("}");
    //     Ok(out)
    // }
    //
    // pub fn exec_reflect_tuple_struct(&self, value: &mut dyn TupleStruct) -> EResult<String> {
    //     let mut out = String::from("[");
    //
    //     for i in 0..value.field_len() {
    //         let value = if let Some(field) = value.field_mut(i) {
    //             let x = self.exec_reflect(field).unwrap();
    //             let y = x.clone();
    //             y.as_str()
    //         } else {
    //             "<missing>"
    //         };
    //
    //         out.push_str(format!("\t{}", value).as_str())
    //     }
    //
    //     out.push_str("]");
    //     Ok(out)
    // }
}

trait Serializable {
    fn serialize(&mut self) -> String;
}

// impl<T: ToString> Serializable for T {
//     fn serialize(&mut self) -> String {
//         self.to_string()
//     }
// }

impl<T: Serialize> Serializable for T {
    fn serialize(&mut self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

struct Serde<T> {
    internal: T,
}

/*impl Serialize for Serde<Mesh> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {

        let mut state = serializer.serialize_struct("Serde<Mesh>", 3)?;
        state.serialize_field("primitive_topology", &(self.internal.primitive_topology() as u8))?;
        state.serialize_field("attributes", &self.internal.attributes())?;
        state.serialize_field("indices", &self.internal.indices().map(|indices| Serde { internal: indices }))?;
        state.end()
    }
}

impl Serialize for Serde<&Indices> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match self.internal {
            Indices::U16(vec) => serializer.serialize_newtype_variant("Indices", 0, "U16", &vec),
            Indices::U32(vec) => serializer.serialize_newtype_variant("Indices", 0, "U32", &vec)
        }
    }
}*/

/*impl Serialize for Serde<Box<dyn Iterator<Item = (MeshVertexAttributeId, &VertexAttributeValues)>>> {
    fn serialize<S>(&mut self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.collect_seq(self.internal.by_ref())
    }
}

impl Serialize for Serde<(MeshVertexAttributeId, &VertexAttributeValues)> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_tuple(2)?;
        seq.serialize_element(&(self.internal.0.))?;
        seq.serialize_element(self.internal.1)?;
        seq.end()
    }
}

impl Serialize for Serde<MeshVertexAttributeId> {

}*/

struct Context {
    world: *mut World,
    collapsible: Option<String>,
}

#[derive(Default)]
pub struct Hierarchy {
    tree: Tree,
}

impl Widget for Hierarchy {
    fn show_ui(&self, ui: &mut Ui) {}

    fn update_state(&self) {
        println!("Update Widget Hierarchy")
    }
}

struct LoadProject(Project);

struct SelectEntity(Entity);

#[derive(Deref, Debug)]
struct LoadAsset(AssetEntry);

#[derive(Debug)]
struct AttachAsset {
    entry: AssetEntry,
    gltf_handle: Handle<Gltf>,
}

enum GltfAsset {
    Mesh(Handle<Mesh>),
    Material(Handle<StandardMaterial>)
}

#[derive(Default, Deref, DerefMut)]
struct AssetManagement(Vec<(AssetEntry, Handle<Gltf>, GltfAsset)>);

#[derive(Eq, PartialEq, Hash)]
enum UiReference {
    Hierarchy,
    Inspector,
    FileExplorer,
}

#[derive(Default)]
struct UiRegistry {
    registry: HashMap<UiReference, &'static mut egui::Ui>,
}

impl UiRegistry {
    fn ui_num<T: egui::emath::Numeric>(value: &mut T, ui: &mut egui::Ui) {
        ui.add(egui::DragValue::new(value).fixed_decimals(2).speed(0.1));
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetManagement>()
            .init_resource::<InspectRegistry>()
            .init_resource::<EditorState>()
            .init_resource::<Vec<Handle<Mesh>>>()
            .init_resource::<Vec<Handle<StandardMaterial>>>()
            .init_resource::<UiRegistry>()
            .init_resource::<Test>()
            .add_event::<LoadProject>()
            .add_event::<SelectEntity>()
            .add_event::<LoadAsset>()
            .add_event::<AttachAsset>()
            .add_plugin(EguiPlugin)
            .add_startup_system(get_editor_state)
            // Ensure order of UI systems execution!
            // .add_system_set(SystemSet::new()
            //     .with_system(ui_menu)
            //     .with_system(ui_hierarchy.after(ui_menu))
            //     .with_system(ui_inspect.after(ui_hierarchy))
            //     .with_system(ui_file_explorer.after(ui_inspect))
            //     .with_system(ui_project_management.after(ui_file_explorer))
            // )
            .add_system(ui_inspect.exclusive_system())
            .add_system(load_project)
            .add_system(select_entity)
            .add_system(load_assets)
            .add_system(attach_assets)
            // .add_system(update_ui_registry)
            .add_system(system_update_state_hierarchy);

        // for widget in self.widgets {
        //     app.add_system(widget.update_state);
        // }
    }
}

fn ui_menu(
    mut egui_context: ResMut<EguiContext>,
    mut ev_load_asset: EventWriter<LoadAsset>,
) {
    egui::TopBottomPanel::top("menu_bar").show(egui_context.ctx_mut(), |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Organize windows").clicked() {
                    ui.ctx().memory().reset_areas();
                    ui.close_menu();
                }
                if ui
                    .button("Reset egui memory")
                    .on_hover_text("Forget scroll, positions, sizes etc")
                    .clicked()
                {
                    *ui.ctx().memory() = Default::default();
                    ui.close_menu();
                }
            });
            ui.menu_button("Objects", |ui| {
                if ui.button("Add cube").clicked() {
                    // ev_load_asset.send(LoadAsset(entry));
                }
            });
        });
    });
}

// TODO fix to immutable state & handle mut via events
fn ui_project_management(
    mut egui_context: ResMut<EguiContext>,
    mut editor_state: ResMut<EditorState>,
    mut ev_load_project: EventWriter<LoadProject>,
) {
    if editor_state.current_project.is_none() {
        egui::Window::new("Select project")
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(egui_context.ctx_mut(), |ui| {
                let res = project_list(ui, &editor_state.existing_projects).unwrap();
                if let Some(action) = res {
                    match action {
                        ProjectListAction::Create(description) => {
                            Project::generate(description.clone()).unwrap();
                            editor_state
                                .existing_projects
                                .add(description.clone())
                                .unwrap();
                            ev_load_project.send(LoadProject(Project::load(description).unwrap()));
                        }
                        ProjectListAction::NewOpen(description) => {
                            editor_state
                                .existing_projects
                                .add(description.clone())
                                .unwrap();
                            ev_load_project.send(LoadProject(Project::load(description).unwrap()));
                        }
                        ProjectListAction::ExistingOpen(description) => {
                            ev_load_project.send(LoadProject(Project::load(description).unwrap()));
                        }
                        ProjectListAction::ExistingRemove(description) => {
                            editor_state.existing_projects.remove(&description).unwrap();
                        }
                    }
                };
            });
    }
}

fn ui_hierarchy(
    mut egui_context: ResMut<EguiContext>,
    editor_state: Res<EditorState>,
    mut ev_select_entity: EventWriter<SelectEntity>,
) {
    egui::SidePanel::left("hierarchy").show(egui_context.ctx_mut(), |ui| {
        let response = show_ui_hierarchy(ui, &editor_state.tree);
        if let Action::Selected(entity) = response {
            ev_select_entity.send(SelectEntity(entity));
        }

        ui.separator();
    });
}

fn ui_file_explorer(mut egui_context: ResMut<EguiContext>, editor_state: Res<EditorState>) {
    egui::TopBottomPanel::bottom("file_explorer").show(egui_context.ctx_mut(), |ui| {
        if editor_state.current_project.is_some() {
            show_ui_file_editor(ui, editor_state.current_file_explorer_path.as_path()).unwrap();
        }
    });
}

fn ui_inspect(
    world: &mut World,
    // mut egui_context: ResMut<EguiContext>,
    // inspect_registry: ResMut<InspectRegistry>,
    // mut transform_query: Query<&mut Transform>,
    // mut selected_query: Query<Entity, With<SelectedEntity>>,
    // archetypes: &Archetypes,
    // components: &Components,
    // entities: &Entities,
    // mut type_registry_arc: Mut<TypeRegistryArc>
) {
    world.resource_scope(|world, mut egui_context: Mut<EguiContext>| {
        egui::TopBottomPanel::top("menu_bar").show(egui_context.ctx_mut(), |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Organize windows").clicked() {
                        ui.ctx().memory().reset_areas();
                        ui.close_menu();
                    }
                    if ui
                        .button("Reset egui memory")
                        .on_hover_text("Forget scroll, positions, sizes etc")
                        .clicked()
                    {
                        *ui.ctx().memory() = Default::default();
                        ui.close_menu();
                    }
                });
            });
        });

        egui::SidePanel::left("hierarchy").show(egui_context.ctx_mut(), |ui| {
            let editor_state = world.resource::<EditorState>();
            let response = show_ui_hierarchy(ui, &editor_state.tree);
            if let Action::Selected(entity) = response {
                world.send_event(SelectEntity(entity));
            }

            ui.separator();
        });

        egui::SidePanel::right("inspector").show(egui_context.ctx_mut(), |ui| {
            // show_ui_hierarchy(ui, &editor_state.tree);

            if let Ok(entity) = world
                .query_filtered::<Entity, With<SelectedEntity>>()
                .get_single_mut(world)
            {
                //     let type_registry = type_registry_arc.read();
                //
                let mut component_type_ids = Vec::new();
                for archetype in world.archetypes().iter() {
                    if archetype.entities().contains(&entity) {
                        for component_id in archetype.components() {
                            let comp_info = world.components().get_info(component_id).unwrap();
                            component_type_ids
                                .push((comp_info.type_id().unwrap(), comp_info.name().to_string()));
                            // if let Some(comp_info) = world.components().get_info(component_id) {
                            //     println!("ITER {} {}", comp_info.name(), component_id.index());
                            //     let comp_type_id = comp_info.type_id().unwrap();
                            //     if let Some(inspectable) = inspect_registry.inspectables.get(&comp_type_id) {
                            //         let registration = type_registry.get(comp_type_id).unwrap();
                            //         if let Some(reflect_component) = registration.data::<ReflectComponent>() {
                            //             // reflect_component.reflect_mut(world, entity);
                            //         }
                            //         // inspectable(transform.into_inner(), ui);
                            //     }
                            // }
                        }
                        break;
                    }
                }

                world.resource_scope(|world, inspect_registry: Mut<InspectRegistry>| {
                    world.resource_scope(|world, type_registry_arc: Mut<TypeRegistryArc>| {
                        // let mut entity_mut = world.entity_mut(entity);
                        for (component_type_id, component_name) in component_type_ids {
                            // TODO is this even possible ???
                            // let component = entity_mut.get_mut_by_id(component_id).unwrap();
                            // inspect_registry.exec(&mut component.into_inner().as_ptr() as &mut dyn Any, ui);

                            // if let Some(callback) = inspect_registry.impls.get(&component_id.type_id()) {

                            let type_registry = type_registry_arc.read();
                            if let Some(registration) = type_registry.get(component_type_id) {
                                let reflect_component =
                                    registration.data::<ReflectComponent>().unwrap();

                                let context = &mut Context {
                                    world,
                                    collapsible: Some(
                                        component_name.rsplit_once(':').unwrap().1.to_string(),
                                    ),
                                };
                                let reflect = reflect_component.reflect_mut(world, entity).unwrap();
                                inspect_registry.exec_reflect(reflect.into_inner(), ui, context);
                            } else {
                                // println!("NOT IN TYPE REGISTRY {:?}: {}", component_type_id, component_name);
                            }

                            // callback(reflect.as_any_mut(), ui);
                            // }
                        }
                    });
                });
            }

            // world.resource_scope(|world, inspect_registry: Mut<InspectRegistry>| {
            //     // THIS WORKS!!
            //     if let Ok(mut transform) = world.query_filtered::<&mut Transform, With<SelectedEntity>>().get_single_mut(world) {
            //             inspect_registry.exec(transform.as_any_mut(), ui);
            //     }
            // });
            //
            // for transform in transform_query.iter_mut() {
            //     let inspectable = inspect_registry.inspectables.get(&TypeId::of::<Transform>()).unwrap();
            //     inspectable(transform.into_inner(), ui);
            // }
            ui.separator();
        });

        world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
            if editor_state.current_project.is_none() {
                egui::Window::new("Select project")
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(egui_context.ctx_mut(), |ui| {
                        let res = project_list(ui, &editor_state.existing_projects).unwrap();
                        if let Some(action) = res {
                            match action {
                                ProjectListAction::Create(description) => {
                                    Project::generate(description.clone()).unwrap();
                                    editor_state
                                        .existing_projects
                                        .add(description.clone())
                                        .unwrap();
                                    world.send_event(LoadProject(
                                        Project::load(description).unwrap(),
                                    ));
                                }
                                ProjectListAction::NewOpen(description) => {
                                    editor_state
                                        .existing_projects
                                        .add(description.clone())
                                        .unwrap();
                                    world.send_event(LoadProject(
                                        Project::load(description).unwrap(),
                                    ));
                                }
                                ProjectListAction::ExistingOpen(description) => {
                                    world.send_event(LoadProject(
                                        Project::load(description).unwrap(),
                                    ));
                                }
                                ProjectListAction::ExistingRemove(description) => {
                                    editor_state.existing_projects.remove(&description).unwrap();
                                }
                            }
                        };
                    });
            }
        });
    });
}

// fn update_ui_registry(mut res_context: ResMut<EguiContext>, mut res_ui_registry: ResMut<UiRegistry>) {
//     let egui_context = res_context.ctx_mut();
//
//     egui::SidePanel::left("hierarchy")
//         .show(egui_context, |ui| {
//             res_ui_registry.registry.insert(UiReference::Hierarchy, ui);
//         });
// }

fn load_project(
    mut commands: Commands,
    mut editor_state: ResMut<EditorState>,
    mut ev_load_project: EventReader<LoadProject>,
    asset_server: Res<AssetServer>,
    mut ev_load_asset: EventWriter<LoadAsset>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials_force_keep: ResMut<Vec<Handle<StandardMaterial>>>,
) {
    // Only take one instance of LoadProject event - multiple events should not happen
    if let Some(event) = ev_load_project.iter().next() {
        let project_scene_path = Path::new(event.0.project_description.path.as_os_str())
            .join("scenes")
            .join(event.0.project_state.scene_file.clone());

        let project_asset_path = Path::new(event.0.project_description.path.as_os_str())
            .join("scenes")
            .join(event.0.project_state.asset_file.clone());
        println!(
            "LOAD PROJECT {:?} - {:?}",
            project_scene_path, project_asset_path
        );

        let asset_entries: Vec<AssetEntry> = ron::from_str(
            std::fs::read_to_string(project_asset_path)
                .unwrap()
                .as_str(),
        )
            .unwrap();

        println!("{:?}", asset_entries);

        for entry in asset_entries {
            ev_load_asset.send(LoadAsset(entry));
        }

        commands.spawn_bundle(DynamicSceneBundle {
            scene: asset_server.load(project_scene_path),
            ..default()
        });
        // TODO serialize camera
        // Manually add a camera as it cannot be serialized at the moment ... No idea why, try when serialization update is released
        commands.spawn_bundle(Camera3dBundle {
            projection: Projection::Orthographic(OrthographicProjection {
                // Why so small scale?
                scale: 0.01,
                ..default()
            }),
            transform: Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        });

        // TODO dynamically load resources
        let _handle_material1 = materials.set(
            HandleId::Id(
                Uuid::from_str("7494888b-c082-457b-aacf-517228cc0c22").unwrap(),
                9399192938557672737,
            ),
            Color::rgb(0.3, 0.5, 0.3).into(),
        );
        let _handle_material2 = materials.set(
            HandleId::Id(
                Uuid::from_str("7494888b-c082-457b-aacf-517228cc0c22").unwrap(),
                13487579056845269015,
            ),
            Color::rgb(0.8, 0.7, 0.6).into(),
        );
        let _handle_material3 = materials.set(
            HandleId::Id(
                Uuid::from_str("7494888b-c082-457b-aacf-517228cc0c22").unwrap(),
                2626654359401176236,
            ),
            Color::rgb(0.6, 0.7, 0.8).into(),
        );

        materials_force_keep.push(_handle_material1);
        materials_force_keep.push(_handle_material2);
        materials_force_keep.push(_handle_material3);

        let project: Project = event.0.clone();
        editor_state.current_file_explorer_path =
            PathBuf::from(project.project_description.path.clone());
        editor_state.current_project = Some(project);
    }

    if ev_load_project.iter().next().is_some() {
        warn!("Multiple LoadProjects events found in listener! Should not happen");
    }
}

fn select_entity(
    mut commands: Commands,
    mut ev_select_entity: EventReader<SelectEntity>,
    mut existing_selected: Query<Entity, With<SelectedEntity>>,
) {
    // Only take one instance of SelectEntity event - multiple events should not happen
    if let Some(event) = ev_select_entity.iter().next() {
        println!("Select!!!!!!!");
        // Remove old selected
        if let Ok(entity) = existing_selected.get_single_mut() {
            commands.entity(entity).remove::<SelectedEntity>();
        } else {
            // TODO else is temporary for debug
            commands.entity(event.0).insert(SelectedEntity);
        }
    }

    if ev_select_entity.iter().next().is_some() {
        warn!("Multiple SelectEntity events found in listener! Should not happen");
    }
}

fn load_assets(
    mut ev_load_assets: EventReader<LoadAsset>,
    mut ev_attach_assets: EventWriter<AttachAsset>,
    asset_server: Res<AssetServer>,
    editor_state: ResMut<EditorState>,
) {
    if let Some(project) = editor_state.current_project.as_ref() {
        for entry in ev_load_assets.iter() {
            println!(
                "new asset requested {:?} - will load",
                entry,
            );
            let asset_path = Path::new(project.project_description.path.as_os_str())
                .join("scenes")
                .join(project.project_state.assets_folder.clone())
                .join(entry.filename.clone());

            let master_asset = asset_server.load(asset_path);

            ev_attach_assets.send(AttachAsset { entry: entry.0.clone(), gltf_handle: master_asset });
            println!(
                "new asset loaded {:?} - sent for attach",
                entry,
            );
        }
    }
}

fn attach_assets(
    mut ev_attach_assets: EventReader<AttachAsset>,
    mut gltfs: ResMut<Assets<Gltf>>,
    mut gltf_meshes: ResMut<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut asset_management: ResMut<AssetManagement>,
) {
    for event in ev_attach_assets.iter() {
        let entry = &event.entry;
        if let Some(gltf) = gltfs.get(&event.gltf_handle) {
            // TODO How to determine what to take? Enum for type, which also contains type UUID ?
            if let Some(gltf_mesh_handle) = &gltf.named_meshes.get("Main") {
                if let Some(gltf_mesh) = gltf_meshes.get(gltf_mesh_handle) {
                    if let Some(primitive) = gltf_mesh.primitives.first() {
                        let mut clone = None;
                        if let Some(mesh) = meshes.get(&primitive.mesh) {
                            clone = Some(mesh.clone());
                        }
                        if let Some(mesh) = clone {
                            let new_handle = meshes.set(
                                HandleId::Id(
                                    Uuid::from_str(entry.type_uuid.as_str()).unwrap(),
                                    entry.uid,
                                ),
                                mesh,
                            );
                            asset_management.push((entry.clone(), event.gltf_handle.clone(), GltfAsset::Mesh(new_handle)));
                            println!(
                                "new asset attached {:?} - done processing",
                                entry.clone(),
                            );
                        }
                    }
                }
            }
        }
    }
}

fn system_update_state_hierarchy(
    query_hierarchy: Query<(Entity, Option<&Parent>, Option<&Children>)>,
    mut editor_state: ResMut<EditorState>,
    entities: &Entities,
) {
    let tree = update_state_hierarchy(query_hierarchy, entities);
    editor_state.tree = tree;
}

fn get_editor_state(mut editor_state: ResMut<EditorState>) {
    editor_state.existing_projects = ExistingProjects::load().unwrap();
}
