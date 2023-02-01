/*
General TODOs:
- handle unwraps as errors
*/

use crate::error::{EResult, Error};
use crate::service::existing_projects::ExistingProjects;
use crate::service::project::{Project, ProjectDescription};
use crate::ui::file_explorer::{explorer_row, show_ui_file_editor};
use crate::ui::project::{project_list, ProjectListAction};
use crate::{bail, TypeRegistry};
use bevy::asset::{Asset, HandleId, SourceMeta};
use bevy::ecs::archetype::Archetypes;
use bevy::ecs::component::Components;
use bevy::ecs::entity::{Entities, EntityMap};
use bevy::ecs::schedule::IntoRunCriteria;
use bevy::ecs::world::EntityRef;
use bevy::gltf::{Gltf, GltfMesh, GltfPrimitive};
use bevy::prelude::*;
use bevy::reflect::erased_serde::deserialize;
use bevy::reflect::{List, ReflectMut, TypeRegistryArc, TypeUuid};
use bevy::render::camera::{CameraProjection, Projection};
use bevy::render::mesh::{Indices, MeshVertexAttributeId, VertexAttributeValues};
use bevy::utils::Uuid;
use bevy_egui::egui::{Checkbox, CollapsingHeader, Grid, TextBuffer, Ui};
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
use std::env::home_dir;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use sysinfo::{DiskExt, RefreshKind, SystemExt};

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

#[derive(Resource)]
pub struct EditorState {
    // TODO re/load existing projects only when needed: on start, window opened, new project opened/created
    existing_projects: ExistingProjects,
    current_file_explorer_path: PathBuf,
    current_project: Option<Project>,
    tree: Tree,
    new_project_popup_shown: bool,
    new_project_name: String,
    new_project_path: String,
    system_info: sysinfo::System,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            existing_projects: Default::default(),
            current_file_explorer_path: Default::default(),
            current_project: None,
            tree: Default::default(),
            new_project_popup_shown: false,
            new_project_name: "".to_string(),
            new_project_path: "".to_string(),
            system_info: sysinfo::System::new_with_specifics(RefreshKind::new().with_disks_list()),
        }
    }
}

#[derive(Component)]
struct SceneRoot {}

pub trait Widget {
    fn show_ui(&self, ui: &mut Ui);
    fn update_state(&self);
}

#[derive(Resource)]
struct InspectRegistry {
    impls: HashMap<TypeId, Box<fn(&mut dyn Any, &mut egui::Ui, &mut Context) -> ()>>,
    skipped: HashSet<TypeId>,
}

#[derive(Resource, Default)]
struct ForceKeep {
    standard_materials: Vec<Handle<StandardMaterial>>,
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
                ReflectMut::Enum(_) => {
                    todo!("WIP {}", value.type_name())
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
struct SaveProject();
struct LoadScene(Handle<DynamicScene>);

#[derive(Resource, Default)]
struct LoadSceneFlag(Option<Handle<DynamicScene>>);

struct SelectEntity(Entity);

#[derive(Deref, Debug)]
struct LoadAsset(AssetSource);

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Clone)]
struct AssetSource {
    filename: String,
    type_uuid: String,
    uid: u64,
    // #[serde(skip_serializing, skip_deserializing)]
    // gltf_handle: Option<Handle<Gltf>>,
    // #[serde(skip_serializing, skip_deserializing)]
    // asset_handle: Option<Handle<Mesh>>, // TODO make dynamic
}

#[derive(Debug, Clone)]
struct AssetEntry {
    source: AssetSource,
    original: Handle<Mesh>,
    attached: Option<Handle<Mesh>>,
}

enum GltfAssetHandle {
    Mesh(Handle<Mesh>),
    Material(Handle<StandardMaterial>),
}

#[derive(Default, Deref, DerefMut, Resource)]
struct AssetManagement(Vec<AssetEntry>);

#[derive(Eq, PartialEq, Hash)]
enum UiReference {
    Hierarchy,
    Inspector,
    FileExplorer,
}

#[derive(Resource, Default)]
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
            .init_resource::<ForceKeep>()
            .init_resource::<UiRegistry>()
            .init_resource::<LoadSceneFlag>()
            .add_event::<LoadProject>()
            .add_event::<LoadScene>()
            .add_event::<LoadAsset>()
            .add_event::<SaveProject>()
            .add_event::<SelectEntity>()
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
            .add_system(ui_inspect)
            .add_system(load_project)
            .add_system(load_scene_proxy)
            .add_system(load_scene)
            .add_system(load_assets)
            .add_system(save_project)
            .add_system(select_entity)
            .add_system(attach_assets)
            // .add_system(update_ui_registry)
            .add_system(system_update_state_hierarchy);

        // for widget in self.widgets {
        //     app.add_system(widget.update_state);
        // }
    }
}

fn ui_menu(mut egui_context: ResMut<EguiContext>, mut ev_load_asset: EventWriter<LoadAsset>) {
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
/*fn ui_project_management(
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
                        ProjectListAction::Create => {
                            /*Window::new("Select new project directory")
                            .open(open)
                            .resizable(false)
                            .show(ctx, |ui| {
                                use super::View as _;
                                self.ui(ui);
                            });*/

                            // TODO show panel with name and location select - file explorer with folder filter, project folder name is slug of project name (can be modified)
                            let path = OsString::from(
                                "C:\\Users\\grabn\\Documents\\Faks\\bevytor\\das_demo",
                            );
                            let name = "Das demo".to_string();
                            Project::verify_new(path.clone()).unwrap();
                            let description = ProjectDescription { name, path };
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
}*/
/*
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
}*/

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
                    if ui.button("Save project").clicked() {
                        world.send_event(SaveProject());
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
                    let mut found = false;
                    for archetype_entity in archetype.entities() {
                        if archetype_entity.entity() == entity {
                            found = true;
                        }
                    }
                    if found {
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
                    world.resource_scope(|world, type_registry_arc: Mut<AppTypeRegistry>| {
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
                                ProjectListAction::Create => {
                                    editor_state.new_project_popup_shown = true;
                                    editor_state.new_project_name = "Project".to_string();
                                    let home_dir = dirs::home_dir().unwrap();
                                    editor_state.new_project_path =
                                        home_dir.to_str().unwrap().to_string();
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

                if editor_state.new_project_popup_shown {
                    // TODO default path
                    let home_dir = dirs::home_dir().unwrap();
                    let desktop_dir = dirs::desktop_dir().unwrap();
                    egui::Window::new("Create new project")
                        .collapsible(false)
                        .show(egui_context.ctx_mut(), |ui| {
                            // TODO as grid
                            // TODO set location final folder to name as slug, if location not yet modified!
                            ui.horizontal(|ui| {
                                ui.label("Name");
                                // TODO fix text edit
                                ui.add(egui::TextEdit::singleline(
                                    &mut editor_state.new_project_name,
                                ));
                            });
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("Location");
                                // TODO must sync with file explorer
                                ui.add_enabled(
                                    true,
                                    egui::TextEdit::singleline(&mut editor_state.new_project_path),
                                );
                                //ui.text_edit_singleline(&mut path);
                            });
                            /*ui.horizontal(|ui| {
                                // TODO - fix icons
                                if ui.button("➕").clicked() {
                                    // TODO update tree
                                    editor_state.new_project_path =
                                        home_dir.to_str().unwrap().to_string();
                                }
                                if ui.button("➕").clicked() {
                                    // TODO update tree
                                    editor_state.new_project_path =
                                        desktop_dir.to_str().unwrap().to_string();
                                }
                                if ui.button("➕").clicked() {
                                    // TODO new folder
                                }
                                if ui.button("➕").clicked() {
                                    // TODO delete folder
                                }
                                if ui.button("➕").clicked() {
                                    // TODO refresh
                                }
                                if ui.button("➕").clicked() {
                                    // TODO show hidden folders
                                }
                            });
                            ui.separator();
                            // TODO background for file explorer
                            egui::ScrollArea::vertical()
                                .max_height(300.0)
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        for disk in editor_state.system_info.disks() {
                                            let root = disk.mount_point().to_str().unwrap();
                                            explorer_row(
                                                ui,
                                                root,
                                                editor_state.new_project_path.as_str(),
                                            );
                                        }
                                    });
                                }); */
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui.button("Cancel").clicked() {
                                    editor_state.new_project_popup_shown = false;
                                }
                                if ui.button("Create").clicked() {
                                    let path =
                                        OsString::from(editor_state.new_project_path.as_str());
                                    let name = editor_state.new_project_name.clone();
                                    Project::verify_new(path.clone()).unwrap();
                                    let description = ProjectDescription { name, path };
                                    Project::generate(description.clone()).unwrap();
                                    editor_state
                                        .existing_projects
                                        .add(description.clone())
                                        .unwrap();
                                    world.send_event(LoadProject(
                                        Project::load(description).unwrap(),
                                    ));
                                }
                            });
                        });
                }
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
    mut ev_load_scene: EventWriter<LoadScene>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut force_keep: ResMut<ForceKeep>,
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

        let asset_entries: Vec<AssetSource> = ron::from_str(
            std::fs::read_to_string(project_asset_path)
                .unwrap()
                .as_str(),
        )
        .unwrap();

        println!("{:?}", asset_entries);

        for entry in asset_entries {
            ev_load_asset.send(LoadAsset(entry));
        }

        ev_load_scene.send(LoadScene(asset_server.load(project_scene_path)));

        /*commands.spawn(DynamicSceneBundle {
            scene: asset_server.load(project_scene_path),
            ..default()
        });*/
        // TODO serialize camera
        // Manually add a camera as it cannot be serialized at the moment ... No idea why, try when serialization update is released
        /*commands.spawn_bundle(Camera3dBundle {
            projection: Projection::Orthographic(OrthographicProjection {
                // Why so small scale?
                scale: 0.01,
                ..default()
            }),
            transform: Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        });*/

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

        force_keep.standard_materials.push(_handle_material1);
        force_keep.standard_materials.push(_handle_material2);
        force_keep.standard_materials.push(_handle_material3);

        let project: Project = event.0.clone();
        editor_state.current_file_explorer_path =
            PathBuf::from(project.project_description.path.clone());
        editor_state.current_project = Some(project);
    }

    if ev_load_project.iter().next().is_some() {
        warn!("Multiple LoadProjects events found in listener! Should not happen");
    }
}

fn save_project(
    world: &World,
    mut ev_save_project: EventReader<SaveProject>,
    type_registry: Res<AppTypeRegistry>,
) {
    // Only take one instance of LoadProject event - multiple events should not happen
    if let Some(event) = ev_save_project.iter().next() {
        if let Some(project) = &world.resource::<EditorState>().current_project {
            let project_scene_path = Path::new(project.project_description.path.as_os_str())
                .join("scenes")
                .join(project.project_state.scene_file.clone());

            let project_asset_path = Path::new(project.project_description.path.as_os_str())
                .join("scenes")
                .join(project.project_state.asset_file.clone());
            println!(
                "SAVE PROJECT {:?} - {:?}",
                project_scene_path, project_asset_path
            );

            //let type_registry = type_registry.read();
            let scene = DynamicScene::from_world(world, &type_registry);
            let ser = scene.serialize_ron(&type_registry).unwrap();

            std::fs::write(project_scene_path, ser).unwrap();
        }
    }

    if ev_save_project.iter().next().is_some() {
        warn!("Multiple SaveProject events found in listener! Should not happen");
    }
}

fn select_entity(
    mut commands: Commands,
    mut ev_select_entity: EventReader<SelectEntity>,
    mut existing_selected: Query<Entity, With<SelectedEntity>>,
) {
    // Only take one instance of SelectEntity event - multiple events should not happen
    if let Some(event) = ev_select_entity.iter().next() {
        // Remove old selected
        if let Ok(entity) = existing_selected.get_single_mut() {
            commands.entity(entity).remove::<SelectedEntity>();
        }
        commands.entity(event.0).insert(SelectedEntity);
    }

    if ev_select_entity.iter().next().is_some() {
        warn!("Multiple SelectEntity events found in listener! Should not happen");
    }
}

// TODO use event listener in load_scene system
// currently I cannot retrieve listener from world
// temporarily replaced with resource option of event contents
fn load_scene_proxy(
    mut ev_load_scene: EventReader<LoadScene>,
    mut load_scene_flag: ResMut<LoadSceneFlag>,
) {
    if let Some(event) = ev_load_scene.iter().next() {
        load_scene_flag.0 = Some(event.0.clone())
    }
    if ev_load_scene.iter().next().is_some() {
        warn!("Multiple LoadScene events found in listener! Should not happen");
    }
}

fn load_scene(mut world: &mut World) {
    // Only take one instance of SelectEntity event - multiple events should not happen
    // TODO EventReader resource does not exist, figure out how to do it manually
    world.resource_scope(|world, mut load_scene_flag: Mut<LoadSceneFlag>| {
        if let Some(event) = &load_scene_flag.0 {
            world.resource_scope(|world, mut dynamic_scenes: Mut<Assets<DynamicScene>>| {
                if let Some(scene) = dynamic_scenes.get(event) {
                    println!("Will load scene to world");
                    scene
                        .write_to_world(world, &mut EntityMap::default())
                        .unwrap();
                    println!("Loaded scene to world");
                }
            });
            load_scene_flag.0 = None
        }
    });
}

fn load_assets(
    mut ev_load_assets: EventReader<LoadAsset>,
    asset_server: Res<AssetServer>,
    editor_state: ResMut<EditorState>,
    mut asset_management: ResMut<AssetManagement>,
) {
    if let Some(project) = editor_state.current_project.as_ref() {
        for source in ev_load_assets.iter() {
            info!("new asset requested {:?} - will load", source,);
            let asset_path = Path::new(project.project_description.path.as_os_str())
                .join("scenes")
                .join(project.project_state.assets_folder.clone())
                .join(source.filename.clone());

            let full = asset_path.to_str().unwrap().to_owned() + "#Mesh0/Primitive0";
            let handle = asset_server.load(&full);
            asset_management.push(AssetEntry {
                source: source.0.clone(),
                original: handle,
                attached: None,
            });
        }
    }
}

fn attach_assets(
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut asset_management: ResMut<AssetManagement>,
) {
    use bevy::asset::LoadState;

    for mut entry in asset_management.0.iter_mut() {
        if entry.attached.is_some() {
            // already attached
            continue;
        }

        match asset_server.get_load_state(&entry.original) {
            LoadState::NotLoaded => { /*do nothing*/ }
            LoadState::Loading => { /*do nothing*/ }
            LoadState::Loaded => {
                let mut clone = None;
                if let Some(mesh) = meshes.get(&entry.original) {
                    clone = Some(mesh.clone());
                }
                if let Some(mesh) = clone {
                    let new_handle = meshes.set(
                        HandleId::Id(
                            Uuid::from_str(entry.source.type_uuid.as_str()).unwrap(),
                            entry.source.uid,
                        ),
                        mesh.clone(),
                    );
                    entry.attached = Some(new_handle);
                    println!("new asset attached {:?} - done processing", entry,);
                } else {
                    error!("No asset for entry {:?}", entry)
                }
            }
            LoadState::Failed => {
                error!("Failed to load asset for entry {:?}", entry)
            }
            LoadState::Unloaded => { /*do nothing*/ }
        }
    }
}

fn system_update_state_hierarchy(
    query_hierarchy: Query<(Entity, Option<&Parent>, Option<&Children>, Option<&Name>)>,
    mut editor_state: ResMut<EditorState>,
    entities: &Entities,
) {
    let tree = update_state_hierarchy(query_hierarchy, entities);
    editor_state.tree = tree;
}

fn get_editor_state(mut editor_state: ResMut<EditorState>) {
    editor_state.existing_projects = ExistingProjects::load().unwrap();
}
