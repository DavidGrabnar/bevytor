/*
General TODOs:
- handle unwraps as errors
*/

use crate::bail;
use crate::error::{EResult, Error};
use crate::plugin::AssetSourceType::AsFile;
use crate::service::existing_projects::ExistingProjects;
use crate::service::project::{Project, ProjectDescription};
use crate::ui::project::{project_list, ProjectListAction};
use bevy::asset::HandleId::Id;
use bevy::asset::{Asset, HandleId};
use bevy::ecs::entity::{Entities, EntityMap};
use bevy::ecs::event::Event;
use bevy::prelude::*;
use bevy::reflect::{ReflectMut, TypeUuid};
use bevy::scene::serialize_ron;
use bevy::utils::Uuid;
use bevy_egui::egui::{Checkbox, Grid, Ui};
use bevy_egui::{egui, EguiContext, EguiPlugin};
use bevytor_core::tree::{Action, Tree};
use bevytor_core::{get_label, show_ui_hierarchy, update_state_hierarchy, SelectedEntity};
use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet, LinkedList};
use std::ffi::OsString;
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use sysinfo::{RefreshKind, SystemExt};

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

pub trait Widget {
    fn show_ui(&self, ui: &mut Ui);
    fn update_state(&self);
}

#[derive(Resource)]
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
        new.register::<Name>();
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

impl Inspectable for Name {
    fn ui(&mut self, ui: &mut Ui, _context: &mut Context) {
        self.mutate(|name| {
            ui.text_edit_singleline(name);
        });
    }
}

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

#[derive(Deref)]
struct RemoveEntity(Entity);

#[derive(Default, Resource)]
struct EventProxy(LinkedList<LoadAsset>);

#[derive(Deref, Debug, Clone)]
struct LoadAsset(AssetSource);

#[derive(Default, Resource)]
struct AssetSourceList(Vec<AssetSource>);

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Clone)]
enum AssetSourceType {
    AsString(String),
    AsFile(String),
}

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Clone)]
struct AssetSource {
    source_type: AssetSourceType,
    type_uuid: String,
    uid: u64,
}

trait AssetSourceable {
    fn from_string(raw: String) -> EResult<Self>
    where
        Self: Sized + Asset;

    fn to_string(&self, prev_raw: String) -> EResult<String>
    where
        Self: Asset;
}

impl AssetSourceable for Mesh {
    fn from_string(raw: String) -> EResult<Self> {
        Ok(raw.parse::<SimpleObject>().unwrap().to_mesh())
    }

    fn to_string(&self, prev_raw: String) -> EResult<String> {
        // Do nothing as all information is in the string itself
        Ok(prev_raw)
    }
}

impl AssetSourceable for StandardMaterial {
    fn from_string(raw: String) -> EResult<Self> {
        let color = ron::from_str::<Color>(raw.as_str()).unwrap();
        Ok(StandardMaterial::from(color))
    }

    fn to_string(&self, _: String) -> EResult<String> {
        let serialized = ron::to_string(&self.base_color).unwrap();
        Ok(serialized)
    }
}

#[derive(Resource)]
struct AssetRegistry {
    impls: HashMap<
        Uuid,
        (
            Box<fn(&AssetSource, &mut World) -> HandleUntyped>, // create
            Box<fn(&mut AssetSource, &World) -> ()>,            // update
            Box<fn(&mut AssetEntry, &mut World) -> ()>,         // attach
        ),
    >,
}

impl Default for AssetRegistry {
    fn default() -> Self {
        let mut instance = Self {
            impls: Default::default(),
        };
        instance.register::<Mesh>();
        instance.register::<StandardMaterial>();
        instance
    }
}

impl AssetRegistry {
    pub fn register<T: AssetSourceable + Asset + Clone + TypeUuid + 'static>(&mut self) {
        self.impls.insert(
            T::TYPE_UUID,
            (
                Box::new(
                    |source: &AssetSource, world: &mut World| match &source.source_type {
                        AssetSourceType::AsString(raw) => {
                            world.resource_scope(|world, mut assets: Mut<Assets<T>>| {
                                let asset = T::from_string(raw.clone()).unwrap();
                                assets.add(asset).clone_untyped()
                            })
                        }
                        AssetSourceType::AsFile(filepath) => {
                            /*let asset_path =
                                Path::new(project.project_description.path.as_os_str())
                                    .join("scenes")
                                    .join(project.project_state.assets_folder.clone())
                                    .join(filepath.clone());

                            asset_server.load_untyped(asset_path.to_str().unwrap())*/
                            todo!("WIP")
                        }
                    },
                ),
                Box::new(|source: &mut AssetSource, world: &World| {
                    source.source_type = match &source.source_type {
                        AssetSourceType::AsString(raw) => {
                            let uuid = Uuid::from_str(&source.type_uuid).unwrap();
                            let handle = Handle::weak(Id(uuid, source.uid));
                            let assets = world.resource::<Assets<T>>();
                            let asset = assets.get(&handle).unwrap();
                            let new_raw = asset.to_string(raw.clone()).unwrap();
                            AssetSourceType::AsString(new_raw)
                        }
                        AssetSourceType::AsFile(filepath) => {
                            // Do nothing as handle does not need to be updated at all
                            AsFile(filepath.to_string())
                        }
                    }
                }),
                Box::new(|entry: &mut AssetEntry, world: &mut World| {
                    world.resource_scope(|world, mut assets: Mut<Assets<T>>| {
                        handle_attach_asset(&mut assets, entry);
                    });
                }),
            ),
        );
    }

    pub fn attach_asset(&self, entry: &mut AssetEntry, world: &mut World) {
        let uuid = Uuid::parse_str(entry.source.type_uuid.as_str()).unwrap();
        let callback = self.impls.get(&uuid).unwrap();
        callback.2(entry, world);
    }

    pub fn update_source(&self, source: &mut AssetSource, world: &World) {
        let uuid = Uuid::parse_str(source.type_uuid.as_str()).unwrap();
        let callback = self.impls.get(&uuid).unwrap();
        callback.1(source, world);
    }
}

#[derive(Debug, Clone)]
struct AssetEntry {
    source: AssetSource,
    original: HandleUntyped,
    attached: Option<HandleUntyped>,
}

#[derive(Default, Deref, DerefMut, Resource)]
struct AssetManagement(Vec<AssetEntry>);

#[derive(PartialEq)]
enum SimpleObject {
    Cube,
    Plane,
    Sphere,
}

impl FromStr for SimpleObject {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Cube" => Ok(SimpleObject::Cube),
            "Plane" => Ok(SimpleObject::Plane),
            "Sphere" => Ok(SimpleObject::Sphere),
            _ => Err(()),
        }
    }
}

impl ToString for SimpleObject {
    fn to_string(&self) -> String {
        match self {
            SimpleObject::Cube => "Cube",
            SimpleObject::Plane => "Plane",
            SimpleObject::Sphere => "Sphere",
        }
        .to_string()
    }
}

impl SimpleObject {
    fn to_mesh(&self) -> Mesh {
        match self {
            SimpleObject::Cube => Mesh::from(shape::Cube { size: 1.0 }),
            SimpleObject::Plane => Mesh::from(shape::Plane { size: 1.0 }),
            SimpleObject::Sphere => Mesh::from(shape::UVSphere {
                radius: 1.0,
                sectors: 10,
                stacks: 10,
            }),
        }
    }
}

struct AddSimpleObject(SimpleObject);

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
            .init_resource::<UiRegistry>()
            .init_resource::<EventProxy>()
            .init_resource::<LoadSceneFlag>()
            .init_resource::<AssetRegistry>()
            .init_resource::<AssetSourceList>()
            .add_event::<LoadProject>()
            .add_event::<LoadScene>()
            .add_event::<LoadAsset>()
            .add_event::<SaveProject>()
            .add_event::<SelectEntity>()
            .add_event::<RemoveEntity>()
            .add_event::<AddSimpleObject>()
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
            .add_system(load_assets_proxy)
            .add_system(load_assets)
            .add_system(save_project)
            .add_system(select_entity)
            .add_system(attach_assets)
            .add_system(add_simple_object)
            .add_system(remove_entity)
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
                ui.menu_button("Objects", |ui| {
                    if ui.button("Cube").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Cube));
                    }
                    if ui.button("Plane").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Plane));
                    }
                    if ui.button("Sphere").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Sphere));
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

            if let Ok((entity, name)) = world
                .query_filtered::<(Entity, Option<&Name>), With<SelectedEntity>>()
                .get_single_mut(world)
            {
                let label = get_label(entity, name);
                ui.horizontal(|ui| {
                    ui.label(label);
                    if ui.button("❌").clicked() {
                        world.send_event(RemoveEntity(entity));
                    }
                });
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
    mut editor_state: ResMut<EditorState>,
    mut ev_load_project: EventReader<LoadProject>,
    asset_server: Res<AssetServer>,
    mut ev_load_asset: EventWriter<LoadAsset>,
    mut ev_load_scene: EventWriter<LoadScene>,
    mut asset_source_list: ResMut<AssetSourceList>,
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

        let c = Color::rgb(0.7, 0.7, 0.7);
        let x = AssetSource {
            source_type: AssetSourceType::AsString(serialize_ron(c).unwrap()),
            type_uuid: "asdasd".to_string(),
            uid: 1234,
        };
        println!("TEST {}", serialize_ron(x).unwrap());

        let asset_entries: Vec<AssetSource> = ron::from_str(
            std::fs::read_to_string(project_asset_path)
                .unwrap()
                .as_str(),
        )
        .unwrap();

        println!("{:?}", asset_entries);

        for entry in asset_entries {
            asset_source_list.0.push(entry.clone());
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
        /*let _handle_material1 = materials.set(
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
        force_keep.standard_materials.push(_handle_material3);*/

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
    asset_registry: Res<AssetRegistry>,
    asset_source_list: Res<AssetSourceList>,
) {
    // Only take one instance of LoadProject event - multiple events should not happen
    if let Some(_) = ev_save_project.iter().next() {
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
            let scene_serialized = scene.serialize_ron(&type_registry).unwrap();

            std::fs::write(project_scene_path, scene_serialized).unwrap();

            let mut source_list_clone = asset_source_list.0.clone();
            for source in source_list_clone.as_mut_slice() {
                asset_registry.update_source(source, world);
            }
            let assets_serialized = serialize_ron(&source_list_clone).unwrap();
            std::fs::write(project_asset_path, assets_serialized).unwrap();
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

fn remove_entity(mut commands: Commands, mut ev_remove_entity: EventReader<RemoveEntity>) {
    for entity in ev_remove_entity.iter() {
        commands.entity(**entity).despawn_recursive();
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
    // Only take one instance of SelectEntity event - multiple events should not happen
    if ev_load_scene.iter().next().is_some() {
        warn!("Multiple LoadScene events found in listener! Should not happen");
    }
}

fn load_scene(mut world: &mut World) {
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

fn load_assets_proxy(
    mut er_load_assets: EventReader<LoadAsset>,
    mut ep_load_assets: ResMut<EventProxy>,
) {
    for event in er_load_assets.iter() {
        ep_load_assets.0.push_front(event.clone());
    }
}

fn load_assets(mut world: &mut World) {
    world.resource_scope(|world, mut ep_load_assets: Mut<EventProxy>| {
        while let Some(source) = ep_load_assets.0.pop_front() {
            info!("new asset requested {:?} - will load", source);

            let untyped_handle =
                world.resource_scope(|world, asset_registry: Mut<AssetRegistry>| {
                    let uuid = Uuid::parse_str(source.type_uuid.as_str()).unwrap();
                    let asset_impl = asset_registry.impls.get(&uuid).unwrap();
                    asset_impl.0(&source, world)
                });

            world.resource_scope(|world, mut asset_management: Mut<AssetManagement>| {
                info!("new asset pushed to mgmt {:?}", source);
                asset_management.push(AssetEntry {
                    source: source.0.clone(),
                    original: untyped_handle,
                    attached: None,
                });
            });
        }
    });
}

fn attach_assets(mut world: &mut World) {
    use bevy::asset::LoadState;

    world.resource_scope(|world, mut asset_management: Mut<AssetManagement>| {
        for mut entry in asset_management.0.iter_mut() {
            if entry.attached.is_some() {
                // already attached
                continue;
            }

            world.resource_scope(|world, asset_server: Mut<AssetServer>| {
                match asset_server.get_load_state(&entry.original) {
                    LoadState::NotLoaded => {
                        println!("attaching simple asset {:?}", entry.source);
                        world.resource_scope(|world, asset_registry: Mut<AssetRegistry>| {
                            asset_registry.attach_asset(entry, world);
                        });
                    }
                    LoadState::Loading => { /*do nothing*/ }
                    LoadState::Loaded => {
                        println!("attaching asset {:?}", entry.source);
                        world.resource_scope(|world, asset_registry: Mut<AssetRegistry>| {
                            asset_registry.attach_asset(entry, world);
                        });
                    }
                    LoadState::Failed => {
                        error!("Failed to load asset for entry {:?}", entry)
                    }
                    LoadState::Unloaded => { /*do nothing*/ }
                }
            });
        }
    });
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

fn handle_attach_asset<T: Asset + Clone>(assets: &mut Assets<T>, entry: &mut AssetEntry) {
    let mut clone = None;
    if let Some(asset) = assets.get(&entry.original.clone().typed()) {
        clone = Some(asset.clone());
    }
    if let Some(asset) = clone {
        let new_handle = assets.set(
            HandleId::Id(
                Uuid::from_str(entry.source.type_uuid.as_str()).unwrap(),
                entry.source.uid,
            ),
            asset.clone(),
        );
        entry.attached = Some(new_handle.clone_untyped());
        println!("new asset attached {:?} - done processing", entry,);
    } else {
        error!("No asset for entry {:?}", entry)
    }
}

fn add_simple_object(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ev_add_simple_object: EventReader<AddSimpleObject>,
    mut asset_source_list: ResMut<AssetSourceList>,
) {
    for event in ev_add_simple_object.iter() {
        let mesh_handle = meshes.add(event.0.to_mesh());
        if let Id(_, id) = mesh_handle.id() {
            asset_source_list.0.push(AssetSource {
                source_type: AssetSourceType::AsString(event.0.to_string()),
                type_uuid: Mesh::TYPE_UUID.to_string(),
                uid: id,
            });
        } else {
            error!("AssetPathId handle is not supported yet");
        }

        let color = Color::rgb(0.7, 0.7, 0.7);
        let material_handle = materials.add(color.into());
        if let Id(_, id) = material_handle.id() {
            asset_source_list.0.push(AssetSource {
                source_type: AssetSourceType::AsString(ron::to_string(&color).unwrap()),
                type_uuid: StandardMaterial::TYPE_UUID.to_string(),
                uid: id,
            });
        } else {
            error!("AssetPathId handle is not supported yet");
        }

        commands
            .spawn(PbrBundle {
                mesh: mesh_handle,
                material: material_handle,
                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                ..default()
            })
            .insert(Name::new(event.0.to_string()));
    }
}
