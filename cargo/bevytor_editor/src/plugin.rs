/*
General TODOs:
- handle unwraps as errors
*/

use crate::error::EResult;
use crate::logs::LogBuffer;
use crate::scripts::{handle_tasks, ScriptableRegistry};
use crate::service::existing_projects::ExistingProjects;
use crate::service::project::{Project, ProjectDescription};
use crate::ui::project::{project_list, ProjectListAction};
use bevy::app::AppLabel;
use bevy::asset::{Asset, HandleId};
use bevy::ecs::entity::{Entities, EntityMap};
use bevy::ecs::system::Command;
use bevy::pbr::wireframe::{Wireframe, WireframePlugin};
use bevy::prelude::*;
use bevy::reflect::{Array, List, ReflectMut, Tuple, TypeUuid};
use bevy::scene::serialize_ron;
use bevy::utils::Uuid;
use bevy::window::PrimaryWindow;
use bevy_egui::egui::{Checkbox, Grid, Ui};
use bevy_egui::{egui, EguiContext, EguiPlugin};
//use bevy_mod_picking::{PickableBundle, PickingCamera, PickingCameraBundle};
//use bevy_transform_gizmo::{GizmoPickSource, GizmoSettings};
use bevytor_core::tree::{Action, HoverEntity, Tree};
use bevytor_core::{get_label, show_ui_hierarchy, update_state_hierarchy, SelectedEntity};
use bevytor_script::ComponentRegistry;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use sysinfo::{RefreshKind, SystemExt};

pub struct EditorPlugin {
    widgets: Vec<Box<dyn Widget + Sync + Send>>,
    //app: std::sync::Arc<App>,
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
    playing: bool,
    initial: bool,
    dynamic_scene_handle: Option<Handle<DynamicScene>>,

    existing_project_popup_shown: bool,
    existing_project_path: String,

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
            existing_project_popup_shown: false,
            existing_project_path: "".to_string(),
            system_info: sysinfo::System::new_with_specifics(RefreshKind::new().with_disks_list()),
            playing: false,
            initial: true,
            dynamic_scene_handle: None,
        }
    }
}

pub trait Widget {
    fn show_ui(&self, ui: &mut Ui);
    fn update_state(&self);
}

#[derive(Resource)]
struct InspectRegistry {
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
        new.register::<u64>();
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
                self.exec_reflect(value, ui, &mut context)
            })
            .body_returned
            .unwrap_or(Ok(()))
        } else if let Some(callback) = self.impls.get(&type_id) {
            callback(value.as_any_mut(), ui, context);
            Ok(())
        } else {
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
                ReflectMut::Value(val) => self.exec_reflect(val, ui, context),
                ReflectMut::Enum(_) => {
                    // TODO
                    ui.label(format!("WIP ENUM {}", value.type_name()));
                    Ok(())
                }
            }
            // println!("NOTFOUND {:?}", type_id);
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

trait Inspectable {
    fn ui(&mut self, ui: &mut Ui, context: &mut Context);
}

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
            UiRegistry::ui_num(&mut self.x, ui);
            UiRegistry::ui_num(&mut self.y, ui);
        });
    }
}

impl Inspectable for Vec3 {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        ui.horizontal(|ui| {
            UiRegistry::ui_num(&mut self.x, ui);
            UiRegistry::ui_num(&mut self.y, ui);
            UiRegistry::ui_num(&mut self.z, ui);
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
            UiRegistry::ui_num(&mut new_x, ui);
            UiRegistry::ui_num(&mut new_y, ui);
            UiRegistry::ui_num(&mut new_z, ui);

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

impl Inspectable for u64 {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        UiRegistry::ui_num(self, ui);
    }
}

impl Inspectable for f32 {
    fn ui(&mut self, ui: &mut Ui, _: &mut Context) {
        UiRegistry::ui_num(self, ui);
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

struct Context<'a> {
    world: *mut World,
    registry: &'a InspectRegistry,
    collapsible: Option<String>,
}

#[derive(Default)]
pub struct Hierarchy {
    tree: Tree,
}

impl Widget for Hierarchy {
    fn show_ui(&self, _: &mut Ui) {}

    fn update_state(&self) {
        println!("Update Widget Hierarchy")
    }
}

#[derive(Default)]
pub enum LoadProjectStep {
    #[default]
    None,
    Scripts(bool),
    Assets(usize),
    Scene(Handle<DynamicScene>, bool),
    Done,
}

#[derive(Default, Resource)]
pub struct LoadProjectProgress(pub(crate) LoadProjectStep);

struct LoadProject(Project);
struct PreSaveProject();
struct SaveProject();
struct LoadScene(Handle<DynamicScene>);

impl Command for LoadScene {
    fn write(self, world: &mut World) {
        println!("loaded scene");
        world.resource_scope(|world, dynamic_scenes: Mut<Assets<DynamicScene>>| {
            if let Some(scene) = dynamic_scenes.get(&self.0) {
                println!("Will attach scene to world");
                scene
                    .write_to_world(world, &mut EntityMap::default())
                    .unwrap();
                println!("Attached scene to world");

                world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
                    editor_state.dynamic_scene_handle = Some(self.0.clone());
                });

                /*let mut state1 =
                    world.query_filtered::<(&GlobalTransform, &Camera), With<PickingCamera>>();
                for result in state1.iter(world) {
                    info!("found camera with pick");
                }
                let mut state = world.query_filtered::<Entity, With<Camera>>();
                info!("before set check");
                let camera = state.iter(world).next().unwrap();
                if let Some(mut entity) = world.get_entity_mut(camera) {
                    entity
                        .insert(PickingCameraBundle::default())
                        .insert(GizmoPickSource::default());
                    info!("Added pick camera components")
                }*/

                //let mut gizmo_settings = world.resource_mut::<GizmoSettings>();
                //gizmo_settings.enabled = true;
            }
        });
    }
}

#[derive(Component, Default, Reflect, Serialize, Deserialize)]
struct OriginalEntityId(u32);

struct SelectEntity(Entity);

#[derive(Deref)]
struct RemoveEntity(Entity);

struct MoveEntity(Entity, Option<Entity>);

#[derive(Deref, Debug, Clone)]
struct LoadAsset(AssetSource);

impl Command for LoadAsset {
    fn write(self, world: &mut World) {
        info!("new asset requested {:?} - will load", self.0);

        let untyped_handle = world.resource_scope(|world, asset_registry: Mut<AssetRegistry>| {
            let uuid = Uuid::parse_str(self.0.type_uuid.as_str()).unwrap();
            let asset_impl = asset_registry.impls.get(&uuid).unwrap();
            asset_impl.0(&self.0, world)
        });

        world.resource_scope(|world, mut asset_management: Mut<AssetManagement>| {
            info!("new asset pushed to mgmt {:?}", self.0);
            asset_management.push(AssetEntry {
                source: self.0.clone(),
                original: untyped_handle,
                attached: None,
            });
        });

        world.resource_scope(
            |world, mut load_project_progress: Mut<LoadProjectProgress>| {
                if let LoadProjectStep::Assets(asset_count) = &load_project_progress.0 {
                    load_project_progress.0 = LoadProjectStep::Assets(asset_count - 1);
                } else {
                    error!("Progress on LoadAsset is not STEP::ASSET! Should not happen")
                }
            },
        );
    }
}

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
        Ok(raw.parse::<MeshMaterial>().unwrap().to_mesh())
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
                        AssetSourceType::AsFile(_) => {
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
                            let handle = Handle::weak(HandleId::Id(uuid, source.uid));
                            let assets = world.resource::<Assets<T>>();
                            let asset = assets.get(&handle).unwrap();
                            let new_raw = asset.to_string(raw.clone()).unwrap();
                            AssetSourceType::AsString(new_raw)
                        }
                        AssetSourceType::AsFile(filepath) => {
                            // Do nothing as handle does not need to be updated at all
                            AssetSourceType::AsFile(filepath.to_string())
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

#[derive(Component)]
struct FixedWireframe;

#[derive(PartialEq)]
enum SimpleObject {
    MeshMaterial(MeshMaterial),
    Light(Light),
}

impl ToString for SimpleObject {
    fn to_string(&self) -> String {
        match self {
            SimpleObject::MeshMaterial(object) => object.to_string(),
            SimpleObject::Light(light) => light.to_string(),
        }
    }
}

#[derive(PartialEq)]
enum MeshMaterial {
    Cube,
    Plane,
    Sphere,
}

impl ToString for MeshMaterial {
    fn to_string(&self) -> String {
        match self {
            MeshMaterial::Cube => "Cube".to_string(),
            MeshMaterial::Plane => "Plane".to_string(),
            MeshMaterial::Sphere => "Sphere".to_string(),
        }
    }
}

impl FromStr for MeshMaterial {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Cube" => Ok(MeshMaterial::Cube),
            "Plane" => Ok(MeshMaterial::Plane),
            "Sphere" => Ok(MeshMaterial::Sphere),
            _ => Err(()),
        }
    }
}

impl MeshMaterial {
    fn to_mesh(&self) -> Mesh {
        match self {
            MeshMaterial::Cube => Mesh::from(shape::Cube { size: 1.0 }),
            MeshMaterial::Plane => Mesh::from(shape::Plane {
                size: 1.0,
                subdivisions: 0,
            }),
            MeshMaterial::Sphere => Mesh::from(shape::UVSphere {
                radius: 1.0,
                sectors: 10,
                stacks: 10,
            }),
        }
    }
}

#[derive(PartialEq)]
enum Light {
    Spot,
    Point,
    Directional,
    Ambient,
}

impl ToString for Light {
    fn to_string(&self) -> String {
        match self {
            Light::Spot => "Spot",
            Light::Point => "Point",
            Light::Directional => "Directional",
            Light::Ambient => "Ambient",
        }
        .to_string()
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
    registry: HashMap<UiReference, &'static mut Ui>,
}

impl UiRegistry {
    fn ui_num<T: egui::emath::Numeric>(value: &mut T, ui: &mut Ui) {
        ui.add(egui::DragValue::new(value).fixed_decimals(2).speed(0.1));
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct TestSubApp;

static mut LOAD_FLAG: bool = false;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetManagement>()
            .init_resource::<InspectRegistry>()
            .init_resource::<EditorState>()
            .init_resource::<UiRegistry>()
            .init_resource::<AssetRegistry>()
            .init_resource::<AssetSourceList>()
            .init_resource::<ScriptableRegistry>()
            .init_resource::<ComponentRegistry>()
            .init_resource::<LogBuffer>()
            .init_resource::<LoadProjectProgress>()
            .add_event::<LoadProject>()
            .add_event::<AddComponent>()
            .add_event::<LoadScript>()
            .add_event::<PreSaveProject>()
            .add_event::<SaveProject>()
            .add_event::<SelectEntity>()
            .add_event::<RemoveEntity>()
            .add_event::<MoveEntity>()
            .add_event::<AddSimpleObject>()
            .add_event::<ResetWorldEvent>()
            .add_plugin(EguiPlugin)
            .add_plugin(WireframePlugin)
            //.add_plugins(bevy_mod_picking::DefaultPickingPlugins)
            //.add_plugin(bevy_transform_gizmo::TransformGizmoPlugin::default())
            .add_startup_system(get_editor_state)
            //.add_startup_system(disable_gizmo)
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
            .add_system(load_project_step)
            .add_system(pre_save_project)
            .add_system(save_project)
            .add_system(select_entity)
            .add_system(attach_assets)
            .add_system(add_simple_object)
            .add_system(remove_entity)
            .add_system(move_entity)
            .add_system(reset_world)
            // .add_system(update_ui_registry)
            //.add_system(|| {})
            .add_system(load_scripts)
            .add_system(process_scripts)
            .add_system(system_update_state_hierarchy)
            .add_system(add_components)
            .add_system(handle_tasks)
            //.register_type::<CursorIcon>()
            //.register_type::<bevy::window::CursorGrabMode>()
            //.register_type::<bevy::window::CompositeAlphaMode>()
            //.register_type::<Option<bevy::math::DVec2>>()
            //.register_type::<Option<bool>>()
            //.register_type::<Option<f64>>()
            //.register_type::<bevy::window::WindowLevel>()
            .register_type::<Rect>()
            .register_type::<OriginalEntityId>()
            .register_type_data::<OriginalEntityId, ReflectSerialize>()
            .register_type_data::<OriginalEntityId, ReflectDeserialize>()
            .register_type_data::<OriginalEntityId, ReflectComponent>()
            .register_type_data::<Rect, ReflectSerialize>()
            .register_type_data::<Rect, ReflectDeserialize>();
        //.insert_sub_app(TestSubApp, sub_app);

        // for widget in self.widgets {
        //     app.add_system(widget.update_state);
        // }
    }
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
    let mut egui_context_query = world.query_filtered::<&mut EguiContext, With<PrimaryWindow>>();
    let mut egui_context_mut = egui_context_query.get_single_mut(world).unwrap();

    //let src = EguiContext::default();
    //let mut dst = std::mem::replace(&mut *egui_context_mut, src);
    let egui_context = &egui_context_mut.get_mut().clone();
    //let egui_context = dst.get_mut();
    //let egui_context = egui_context_mut.get_mut();
    //world.resource_scope(|world: &mut World, mut egui_context: EguiContexts| {
    egui::TopBottomPanel::top("menu_bar").show(egui_context, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Organize windows").clicked() {
                    ui.ctx().memory_mut(|ui| ui.reset_areas());
                    ui.close_menu();
                }
                if ui
                    .button("Reset egui memory")
                    .on_hover_text("Forget scroll, positions, sizes etc")
                    .clicked()
                {
                    ui.ctx().memory_mut(|ui| *ui = Default::default());
                    ui.close_menu();
                }
                if ui.button("Save project").clicked() {
                    world.send_event(PreSaveProject());
                    ui.close_menu();
                }
            });
            ui.menu_button("Insert", |ui| {
                ui.menu_button("Object", |ui| {
                    if ui.button("Cube").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::MeshMaterial(
                            MeshMaterial::Cube,
                        )));
                    }
                    if ui.button("Plane").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::MeshMaterial(
                            MeshMaterial::Plane,
                        )));
                    }
                    if ui.button("Sphere").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::MeshMaterial(
                            MeshMaterial::Sphere,
                        )));
                    }
                });
                ui.menu_button("Light", |ui| {
                    if ui.button("Spot").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Light(Light::Spot)));
                    }
                    if ui.button("Point").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Light(Light::Point)));
                    }
                    if ui.button("Directional").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Light(Light::Directional)));
                    }
                    if ui.button("Ambient").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Light(Light::Ambient)));
                    }
                });
            });
        });
    });

    egui::SidePanel::left("hierarchy").show(egui_context, |ui| {
        world.resource_scope(|world, editor_state: Mut<EditorState>| {
            let response = show_ui_hierarchy(ui, &editor_state.tree);
            match response {
                Action::Selected(entity) => world.send_event(SelectEntity(entity)),
                Action::DragDrop(dragged, dropped) => {
                    let parent = match dropped {
                        HoverEntity::Root => None,
                        HoverEntity::Node(entity) => Some(entity),
                    };
                    world.send_event(MoveEntity(dragged, parent))
                }
                Action::NoAction => {}
            }

            ui.separator();
            ui.separator();
            ui.label("Scripts");
            ui.separator();
            if let Some(project) = &editor_state.current_project {
                // currently, only one script per project is available
                // script === dynamic lib from project dir
                let path = Path::new(&project.project_description.path).join("scripts");
                if project.project_state.script_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Script (dylib)");
                        if ui.button("⟲").clicked() {
                            world.send_event(LoadScript(path.display().to_string(), true))
                        }
                        if ui.button("❌").clicked() {
                            world.resource_scope(|world, mut reg: Mut<ScriptableRegistry>| {
                                reg.old_impls.clear();
                            });
                            // TODO remove script
                        }
                    });
                } else if ui.button("Load script ➕").clicked() {
                    world.send_event(LoadScript(path.display().to_string(), false))
                }
            }
        });
    });

    egui::SidePanel::right("inspector").show(egui_context, |ui| {
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
                                registry: &inspect_registry,
                                collapsible: Some(
                                    component_name.rsplit_once(':').unwrap().1.to_string(),
                                ),
                            };
                            let mut entity_mut = world.get_entity_mut(entity).unwrap();
                            let reflect = reflect_component.reflect_mut(&mut entity_mut).unwrap();
                            inspect_registry
                                .exec_reflect(reflect.into_inner(), ui, context)
                                .unwrap();
                        } else {
                            // println!("NOT IN TYPE REGISTRY {:?}: {}", component_type_id, component_name);
                        }

                        // callback(reflect.as_any_mut(), ui);
                        // }
                    }
                });
            });
            ui.separator();
            ui.menu_button("Add component ➕", |ui| {
                // TODO add fixed elements (if not already on entity) (transform, light, etc.) besides script components
                world.resource_scope(|world, registry: Mut<ComponentRegistry>| {
                    for (id, (name, _)) in registry.reg.iter() {
                        if ui.button(name).clicked() {
                            world.send_event(AddComponent(entity, id.clone()));
                        }
                    }
                });
            });
        }
        ui.separator();
    });

    egui::TopBottomPanel::top("controls").show(egui_context, |ui| {
        world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
            ui.horizontal(|ui| {
                if !editor_state.playing && ui.button("▶").clicked() {
                    editor_state.playing = true;
                }
                if editor_state.playing && ui.button("⏸").clicked() {
                    editor_state.playing = false;
                }
                if !editor_state.initial && ui.button("■").clicked() {
                    world.send_event(ResetWorldEvent);
                }
            });
        });
    });

    egui::TopBottomPanel::bottom("logs").show(egui_context, |ui| {
        let log_buffer = world.resource::<LogBuffer>();
        for entry in log_buffer.iter() {
            ui.label(entry);
        }
    });

    world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
        if editor_state.current_project.is_none() {
            egui::Window::new("Select project")
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(egui_context, |ui| {
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
                            ProjectListAction::NewOpen => {
                                editor_state.existing_project_popup_shown = true;
                                let home_dir = dirs::home_dir().unwrap();
                                editor_state.existing_project_path =
                                    home_dir.to_str().unwrap().to_string();
                            }
                            ProjectListAction::ExistingOpen(description) => {
                                world.send_event(LoadProject(Project::load(description).unwrap()));
                            }
                            ProjectListAction::ExistingRemove(description) => {
                                editor_state.existing_projects.remove(&description).unwrap();
                            }
                        }
                    };
                });

            if editor_state.new_project_popup_shown {
                // TODO default path
                //let home_dir = dirs::home_dir().unwrap();
                //let desktop_dir = dirs::desktop_dir().unwrap();
                egui::Window::new("Create new project")
                    .collapsible(false)
                    .show(egui_context, |ui| {
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
                                let path = OsString::from(editor_state.new_project_path.as_str());
                                let name = editor_state.new_project_name.clone();
                                Project::verify_new(path.clone()).unwrap();
                                let description = ProjectDescription { name, path };
                                Project::generate(description.clone()).unwrap();
                                editor_state
                                    .existing_projects
                                    .add(description.clone())
                                    .unwrap();
                                world.send_event(LoadProject(Project::load(description).unwrap()));
                            }
                        });
                    });
            } else if editor_state.existing_project_popup_shown {
                // TODO default path
                egui::Window::new("Open existing project")
                    .collapsible(false)
                    .show(egui_context, |ui| {
                        // TODO as grid
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
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                editor_state.existing_project_popup_shown = false;
                            }
                            if ui.button("Create").clicked() {
                                let path = OsString::from(editor_state.new_project_path.as_str());
                                let name = editor_state.new_project_name.clone();
                                Project::verify_existing(path.clone()).unwrap();
                                let description = ProjectDescription { name, path };
                                editor_state
                                    .existing_projects
                                    .add(description.clone())
                                    .unwrap();
                                world.send_event(LoadProject(Project::load(description).unwrap()));
                            }
                        });
                    });
            }
        }
    });
    //});
    //let mut blank = std::mem::replace(&mut *egui_context_mut, dst);
}

struct AddComponent(Entity, TypeId);

fn add_components(
    mut events: EventReader<AddComponent>,
    component_registry: Res<ComponentRegistry>,
    mut commands: Commands,
) {
    for event in events.iter() {
        let entity = event.0;
        let component_type_id = event.1;
        println!("Add component {:?} to {:?}", component_type_id, entity);
        if let Some((_, callback)) = &component_registry.reg.get(&component_type_id) {
            let mut cmd = commands.entity(entity);
            callback(&mut cmd);
        } else {
            error!("Component {:?} not found", component_type_id);
        }
    }
}

fn load_project(
    mut editor_state: ResMut<EditorState>,
    mut ev_load_project: EventReader<LoadProject>,
    mut load_project_progress: ResMut<LoadProjectProgress>,
    mut commands: Commands,
) {
    // Only take one instance of LoadProject event - multiple events should not happen
    if let Some(event) = ev_load_project.iter().next() {
        println!("LOAD PROJECT");

        let project: Project = event.0.clone();
        editor_state.current_file_explorer_path =
            PathBuf::from(project.project_description.path.clone());
        editor_state.current_project = Some(project);

        load_project_progress.0 = LoadProjectStep::Scripts(false);

        if event.0.project_state.script_enabled {
            let path = Path::new(&event.0.project_description.path).join("scripts");
            commands.add(AttachScript(path.display().to_string(), false));
        }
    }

    if ev_load_project.iter().next().is_some() {
        warn!("Multiple LoadProjects events found in listener! Should not happen");
    }
}

fn load_project_step(
    mut editor_state: ResMut<EditorState>,
    mut load_project_progress: ResMut<LoadProjectProgress>,
    mut asset_source_list: ResMut<AssetSourceList>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if let Some(project) = &editor_state.current_project {
        match load_project_progress.0 {
            LoadProjectStep::None => {
                // do nothing
            }
            LoadProjectStep::Scripts(done) => {
                if !done {
                    info!("STEP - Progress loading script");
                } else {
                    info!("STEP - Finished loading script");
                    info!("STEP - Starting loading assets");
                    let project_asset_path =
                        Path::new(project.project_description.path.as_os_str())
                            .join("scenes")
                            .join(project.project_state.asset_file.clone());

                    let asset_entries: Vec<AssetSource> = ron::from_str(
                        std::fs::read_to_string(project_asset_path)
                            .unwrap()
                            .as_str(),
                    )
                    .unwrap();

                    println!("{:?}", asset_entries);

                    let asset_count = asset_entries.len();
                    for entry in asset_entries {
                        asset_source_list.0.push(entry.clone());
                        commands.add(LoadAsset(entry));
                    }

                    load_project_progress.0 = LoadProjectStep::Assets(asset_count);
                }
            }
            LoadProjectStep::Assets(left) => {
                if left == 0 {
                    info!("STEP - Finished loading assets");
                    info!("STEP - Started loading scene");
                    let project_scene_path =
                        Path::new(project.project_description.path.as_os_str())
                            .join("scenes")
                            .join(project.project_state.scene_file.clone());

                    println!("loading {}", project_scene_path.to_str().unwrap());
                    let handle = asset_server.load(project_scene_path);
                    load_project_progress.0 = LoadProjectStep::Scene(handle, false);
                } else {
                    info!("STEP - Progress loading assets {} left", left);
                }
            }
            LoadProjectStep::Scene(ref handle, done) => {
                if !done {
                    info!("STEP - Progress loading scene");
                    use bevy::asset::LoadState;

                    match asset_server.get_load_state(handle.id()) {
                        LoadState::Failed => {
                            println!("FAILED to load scene! {:?}", handle);
                            // one of our assets had an error
                            load_project_progress.0 = LoadProjectStep::Scene(handle.clone(), true);
                        }
                        LoadState::Loaded => {
                            // all assets are now ready
                            println!("Success - loaded scene, will attach! {:?}", handle);
                            commands.add(LoadScene(handle.clone()));

                            load_project_progress.0 = LoadProjectStep::Scene(handle.clone(), true);
                        }
                        _ => {
                            // NotLoaded/Loading: not fully ready yet
                        }
                    }
                } else {
                    info!("STEP - Finished loading scene");
                    load_project_progress.0 = LoadProjectStep::Done;
                }
            }
            LoadProjectStep::Done => {
                // do nothing
            }
        }
    }
}

fn pre_save_project(
    mut ev_pre_save_project: EventReader<PreSaveProject>,
    query: Query<Entity>,
    mut commands: Commands,
) {
    if let Some(_) = ev_pre_save_project.iter().next() {
        for entity in query.iter() {
            commands
                .entity(entity)
                .insert(OriginalEntityId(entity.index()));
        }
        commands.add(SaveProjectCommand);
    }

    if ev_pre_save_project.iter().next().is_some() {
        warn!("Multiple PreSaveProject events found in listener! Should not happen");
    }
}

struct SaveProjectCommand;

impl Command for SaveProjectCommand {
    fn write(self, world: &mut World) {
        world.send_event(SaveProject());
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

            let project_file_path =
                Path::new(project.project_description.path.as_os_str()).join("project.bv");

            println!(
                "SAVE PROJECT {:?} - {:?} - {:?}",
                project_scene_path, project_asset_path, project_file_path
            );

            //let type_registry = type_registry.read();
            let scene = DynamicScene::from_world(world, &type_registry);
            let scene_serialized = scene.serialize_ron(&type_registry).unwrap();

            // TODO weird hack for transforming Rect type for area in OrthographicProjection
            let scene_serialized = {
                let re = Regex::new(r"area: \(\n +min: \(\n +x: ([+-]?\d+\.?\d*),\n +y: ([+-]?\d+\.?\d*),\n +\),\n +max: \(\n +x: ([+-]?\d+\.?\d*),\n +y: ([+-]?\d+\.?\d*),\n +\),\n +\),").unwrap();
                let caps = re.captures(&scene_serialized).unwrap();
                let replacement = r"area: (
            min: (X1, Y1),
            max: (X2, Y2),
          ),";
                let new_val = replacement
                    .replace("X1", caps.get(1).unwrap().into())
                    .replace("Y1", caps.get(2).unwrap().into())
                    .replace("X2", caps.get(3).unwrap().into())
                    .replace("Y2", caps.get(4).unwrap().into());

                re.replace(&scene_serialized, new_val).to_string()
            };
            // TODO: weird hack to remove window entities
            let scene_serialized = {
                let re = Regex::new(
                    r"\d+: \(\n.+\n +.bevy_window::window::Window.: \((\n.+){50}\n {4}\),\n {4}",
                )
                .unwrap();
                println!("Window entities found: {}", re.captures_len());
                re.replace_all(&scene_serialized, "").to_string()
            };

            std::fs::write(project_scene_path, scene_serialized).unwrap();

            let mut source_list_clone = asset_source_list.0.clone();
            for source in source_list_clone.as_mut_slice() {
                asset_registry.update_source(source, world);
            }
            let assets_serialized = serialize_ron(&source_list_clone).unwrap();
            std::fs::write(project_asset_path, assets_serialized).unwrap();

            let file_serialized = serde_json::to_string(&project).unwrap();
            std::fs::write(project_file_path, file_serialized).unwrap();
        }
    }

    if ev_save_project.iter().next().is_some() {
        warn!("Multiple SaveProject events found in listener! Should not happen");
    }
}

fn select_entity(
    mut commands: Commands,
    mut ev_select_entity: EventReader<SelectEntity>,
    mut existing_selected: Query<(Entity, Option<&FixedWireframe>), With<SelectedEntity>>,
) {
    // Only take one instance of SelectEntity event - multiple events should not happen
    if let Some(event) = ev_select_entity.iter().next() {
        // Remove old selected
        if let Ok((entity, fixed_wireframe)) = existing_selected.get_single_mut() {
            commands.entity(entity).remove::<SelectedEntity>();

            if fixed_wireframe.is_none() {
                commands.entity(entity).remove::<Wireframe>();
            }
        }
        commands.entity(event.0).insert((SelectedEntity, Wireframe));
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

fn move_entity(mut commands: Commands, mut ev_move_entity: EventReader<MoveEntity>) {
    for entity in ev_move_entity.iter() {
        match entity.1 {
            Some(parent) => commands.entity(entity.0).set_parent(parent),
            None => commands.entity(entity.0).remove_parent(),
        };
    }
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

// consider using HierarchyEvents to keep it updated
// not hierarchy data won't be handled by them (ex.: Name label, etc.?)
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

/*fn disable_gizmo(mut gizmo_settings: ResMut<GizmoSettings>) {
    gizmo_settings.enabled = false;
}*/

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
        match &event.0 {
            SimpleObject::MeshMaterial(mesh_material) => {
                let mesh_handle = meshes.add(mesh_material.to_mesh());
                if let HandleId::Id(_, id) = mesh_handle.id() {
                    asset_source_list.0.push(AssetSource {
                        source_type: AssetSourceType::AsString(mesh_material.to_string()),
                        type_uuid: Mesh::TYPE_UUID.to_string(),
                        uid: id,
                    });
                } else {
                    error!("AssetPathId handle is not supported yet");
                }

                let color = Color::rgb(0.7, 0.7, 0.7);
                let material_handle = materials.add(color.into());
                if let HandleId::Id(_, id) = material_handle.id() {
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
            SimpleObject::Light(light) => {
                let wireframe_mesh = Mesh::from(shape::Cube { size: 1.0 });
                let wireframe_mesh_handle = meshes.add(wireframe_mesh);
                let name = Name::new(light.to_string());
                match &light {
                    Light::Spot => {
                        commands.spawn((
                            SpotLightBundle::default(),
                            name,
                            wireframe_mesh_handle,
                            Wireframe,
                            FixedWireframe,
                        ));
                    }
                    Light::Point => {
                        commands.spawn((
                            PointLightBundle::default(),
                            name,
                            wireframe_mesh_handle,
                            Wireframe,
                            FixedWireframe,
                        ));
                    }
                    Light::Directional => {
                        commands.spawn((
                            DirectionalLightBundle::default(),
                            name,
                            wireframe_mesh_handle,
                            Wireframe,
                            FixedWireframe,
                        ));
                    }
                    Light::Ambient => error!("WIP Ambient light"),
                }
            }
        }
    }
}

fn process_scripts(world: &mut World) {
    world.resource_scope(|world, mut registry: Mut<ScriptableRegistry>| {
        world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
            if editor_state.playing {
                registry.exec(world);
                if editor_state.initial {
                    editor_state.initial = false;
                }
            }
        });
    });
}

struct LoadScript(String, bool);

struct AttachScript(String, bool);

impl Command for AttachScript {
    fn write(self, world: &mut World) {
        world.resource_scope(|world, mut registry: Mut<ScriptableRegistry>| {
            if self.1 {
                registry.reload(world, self.0);
            } else {
                registry.load(world, self.0);

                let mut editor = world.resource_mut::<EditorState>();
                if let Some(ref mut project) = &mut editor.current_project {
                    project.project_state.script_enabled = true;
                }
            }
        });
    }
}

fn load_scripts(mut commands: Commands, mut events: EventReader<LoadScript>) {
    for event in events.iter() {
        commands.add(AttachScript(event.0.clone(), event.1))
    }
}

struct ResetWorldEvent;

struct ResetWorld;

impl Command for ResetWorld {
    fn write(self, world: &mut World) {
        world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
            if let Some(handle) = &editor_state.dynamic_scene_handle {
                world.resource_scope(|world, dynamic_scenes: Mut<Assets<DynamicScene>>| {
                    if let Some(scene) = dynamic_scenes.get(handle) {
                        let mut entity_map = EntityMap::default();
                        let mut query_state = world.query::<(Entity, &OriginalEntityId)>();
                        for (entity, original_id) in query_state.iter(world) {
                            entity_map.insert(Entity::from_raw(original_id.0), entity);
                        }
                        scene.write_to_world(world, &mut entity_map).unwrap();
                    } else {
                        error!("Dynamic scene not found!")
                    }
                });
            } else {
                error!("Dynamic scene handle is None!")
            }
        });
    }
}

fn reset_world(mut ev_reset_world: EventReader<ResetWorldEvent>, mut commands: Commands) {
    if ev_reset_world.iter().next().is_some() {
        commands.add(ResetWorld);
    }

    if ev_reset_world.iter().next().is_some() {
        warn!("Multiple ResetWorldEvent events found in listener! Should not happen");
    }
}
