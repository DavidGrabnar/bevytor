/*
General TODOs:
- handle unwraps as errors
*/

use crate::error::EResult;
use crate::logs::{logs_ui, Level, LogBuffer, LogPlugin, PushLog};
use crate::scripts::{handle_tasks, ScriptableRegistry};
use crate::service::existing_projects::ExistingProjects;
use crate::service::project::{Project, ProjectDescription};
use crate::ui::project::{project_list, ProjectListAction};
use bevy::app::AppLabel;
use bevy::asset::{Asset, HandleId};
use bevy::ecs::entity::EntityMap;
use bevy::ecs::system::{Command, SystemState};
use bevy::pbr::wireframe::{Wireframe, WireframePlugin};
use bevy::prelude::*;
use bevy::reflect::{Array, List, Tuple, TypeUuid};
use bevy::scene::serialize_ron;
use bevy::utils::Uuid;
use bevy::window::PrimaryWindow;
use bevy_egui::egui::Ui;
use bevy_egui::{egui, EguiContext, EguiPlugin};
//use bevy_mod_picking::{PickableBundle, PickingCamera, PickingCameraBundle};
//use bevy_transform_gizmo::{GizmoPickSource, GizmoSettings};
use crate::core::events::{SelectEntity, StartPlaying};
use crate::core::popup::{show_popup, BoxedPopup};
use crate::core::OriginalEntityId;
use crate::modules::controls::{ControlState, Controls, EditorCamera, ResetWorldEvent};
use crate::modules::hierarchy::*;
use crate::modules::inspector::registry::InspectRegistry;
use crate::modules::inspector::Inspector;
use bevy::core_pipeline::core_3d::Camera3dDepthTextureUsage;
use bevy::render::camera;
use bevy_mod_picking::debug::print;
use bevytor_core::SelectedEntity;
use bevytor_script::ComponentRegistry;
use regex::Regex;
use serde::{Deserialize, Serialize};
use smooth_bevy_cameras::controllers::orbit::{
    OrbitCameraBundle, OrbitCameraController, OrbitCameraPlugin,
};
use smooth_bevy_cameras::LookTransformPlugin;
use std::any::TypeId;
use std::collections::HashMap;
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
        Self { widgets: vec![] }
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

    existing_project_popup_shown: bool,
    existing_project_path: String,

    system_info: sysinfo::System,

    current_popup: Option<BoxedPopup>,
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
            current_popup: None,
        }
    }
}

pub trait Widget {
    fn show_ui(&self, ui: &mut Ui);
    fn update_state(&self);
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

#[derive(Event)]
struct LoadProject(Project);

#[derive(Event)]
struct PreSaveProject();

#[derive(Event)]
struct SaveProject();

#[derive(Event)]
struct LoadScene(Handle<DynamicScene>);

impl Command for LoadScene {
    fn apply(self, world: &mut World) {
        println!("loaded scene");
        world.resource_scope(|world, dynamic_scenes: Mut<Assets<DynamicScene>>| {
            if let Some(scene) = dynamic_scenes.get(&self.0) {
                println!("Will attach scene to world");
                scene
                    .write_to_world(world, &mut EntityMap::default())
                    .unwrap();
                println!("Attached scene to world");

                /*world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
                    editor_state.dynamic_scene_handle = Some(self.0.clone());
                });*/

                let mut state =
                    world.query_filtered::<Entity, (With<camera::Camera>, With<EditorCamera>)>();
                if let Ok(entity) = state.get_single(world) {
                    if let Some(mut entity_mut) = world.get_entity_mut(entity) {
                        entity_mut.insert(OrbitCameraBundle::new(
                            OrbitCameraController::default(),
                            Vec3::new(5.0, 5.0, 5.0),
                            Vec3::new(0., 0., 0.),
                            Vec3::Y,
                        ));
                    } else {
                        println!("No entity for camera");
                    }
                } else {
                    println!("No editor camera. Must be added manually!");
                }
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
                //gizmo_settings.enabled =

                // Attach ComputedVisibility to all entities with Mesh
                // since Component fails to serialize and is temporary skipped from serialization
                let mut visibility_query = world.query_filtered::<Entity, With<Handle<Mesh>>>();
                let mut entities = vec![];
                for entity in visibility_query.iter(world) {
                    entities.push(entity);
                }

                for entity in entities {
                    world
                        .entity_mut(entity)
                        .insert(ComputedVisibility::default());
                }

                let mut wireframe_query = world.query_filtered::<Entity, With<FixedWireframe>>();
                let mut entities = vec![];
                for entity in wireframe_query.iter(world) {
                    entities.push(entity);
                }

                for entity in entities {
                    world.entity_mut(entity).insert(Wireframe);
                }
            }
        });
    }
}

#[derive(Deref, Debug, Clone, Event)]
struct LoadAsset(AssetSource);

impl Command for LoadAsset {
    fn apply(self, world: &mut World) {
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

#[derive(Component, Default, Reflect, Serialize, Deserialize)]
struct FixedWireframe;

#[derive(PartialEq)]
enum SimpleObject {
    MeshMaterial(MeshMaterial),
    Light(Light),
    Camera(Camera),
}

impl ToString for SimpleObject {
    fn to_string(&self) -> String {
        match self {
            SimpleObject::MeshMaterial(object) => object.to_string(),
            SimpleObject::Light(light) => light.to_string(),
            SimpleObject::Camera(camera) => camera.to_string(),
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

#[derive(PartialEq)]
enum Camera {
    Perspective,
    Orthographic,
}

impl ToString for Camera {
    fn to_string(&self) -> String {
        match self {
            Camera::Perspective => "Perspective",
            Camera::Orthographic => "Orthographic",
        }
        .to_string()
    }
}

#[derive(Event)]
struct AddSimpleObject(SimpleObject);

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct TestSubApp;

static mut LOAD_FLAG: bool = false;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetManagement>()
            .init_resource::<EditorState>()
            //.init_resource::<UiRegistry>()
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
            .add_event::<AddSimpleObject>()
            .add_event::<ResetWorldEvent>()
            .add_event::<StartPlaying>()
            .add_plugins((EguiPlugin, WireframePlugin, LogPlugin))
            .add_plugins((Hierarchy, Inspector, Controls))
            //.add_plugins(bevy_mod_picking::DefaultPickingPlugins)
            //.add_plugin(bevy_transform_gizmo::TransformGizmoPlugin::default())
            .add_plugins((LookTransformPlugin, OrbitCameraPlugin::default()))
            .add_systems(Startup, get_editor_state)
            .add_systems(Update, ui_inspect)
            .add_systems(Update, load_project)
            .add_systems(Update, load_project_step)
            .add_systems(Update, pre_save_project)
            .add_systems(Update, save_project)
            .add_systems(Update, select_entity)
            .add_systems(Update, attach_assets)
            .add_systems(Update, add_simple_object)
            // .add_systems(Update, update_ui_registry)
            //.add_systems(Update, || {})
            .add_systems(Update, load_scripts)
            .add_systems(Update, process_scripts)
            .add_systems(Update, add_components)
            .add_systems(Update, handle_tasks)
            .add_systems(Update, show_popup_on_error)
            .add_systems(Update, handle_start_playing)
            .register_type::<Rect>()
            .register_type::<FixedWireframe>()
            .register_type_data::<FixedWireframe, ReflectSerialize>()
            .register_type_data::<FixedWireframe, ReflectDeserialize>()
            .register_type_data::<FixedWireframe, ReflectComponent>()
            .register_type_data::<Rect, ReflectSerialize>()
            .register_type_data::<Rect, ReflectDeserialize>()
            .register_type::<Camera3dDepthTextureUsage>();
        //.insert_sub_app(TestSubApp, sub_app);

        // for widget in self.widgets {
        //     app.add_systems(Update, widget.update_state);
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
                ui.menu_button("Camera", |ui| {
                    if ui.button("Perspective").clicked() {
                        world
                            .send_event(AddSimpleObject(SimpleObject::Camera(Camera::Perspective)));
                    }
                    if ui.button("Orthographic").clicked() {
                        world.send_event(AddSimpleObject(SimpleObject::Camera(
                            Camera::Orthographic,
                        )));
                    }
                });
            });
        });
    });

    egui::SidePanel::left("hierarchy").show(egui_context, |ui| {
        {
            Hierarchy::ui(ui, world);
        }
        world.resource_scope(|world, editor_state: Mut<EditorState>| {
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
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Ok((entity, name)) = world
                .query_filtered::<(Entity, Option<&Name>), With<SelectedEntity>>()
                .get_single_mut(world)
            {
                Inspector::ui(ui, world);
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
                ui.separator();
            }
        });
    });

    egui::TopBottomPanel::top("controls").show(egui_context, |ui| {
        world.resource_scope(|world, editor_state: Mut<EditorState>| {
            if editor_state.current_project.is_some() {
                Controls::ui(ui, world);
            }
        });
    });

    egui::TopBottomPanel::bottom("logs").show(egui_context, |ui| {
        let log_buffer = world.resource::<LogBuffer>();
        logs_ui(ui, log_buffer);
    });

    world.resource_scope(|world, mut editor_state: Mut<EditorState>| {
        if let Some(popup) = &editor_state.current_popup {
            if show_popup(egui_context, popup) {
                editor_state.current_popup = None;
            }
        } else if editor_state.current_project.is_none() {
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

#[derive(Event)]
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

        if event.0.project_state.script_enabled {
            load_project_progress.0 = LoadProjectStep::Scripts(false);

            let path = Path::new(&event.0.project_description.path).join("scripts");
            commands.add(AttachScript(path.display().to_string(), false));
        } else {
            load_project_progress.0 = LoadProjectStep::Scripts(true);
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
    fn apply(self, world: &mut World) {
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

            let scene = crate::core::to_dynamic_scene(world);
            let mut scene_serialized = scene.serialize_ron(&type_registry).unwrap();

            // TODO weird hack for transforming Rect type for area in OrthographicProjection
            loop {
                let re = Regex::new(r"area: \(\n +min: \(\n +x: ([+-]?\d+\.?\d*),\n +y: ([+-]?\d+\.?\d*),\n +\),\n +max: \(\n +x: ([+-]?\d+\.?\d*),\n +y: ([+-]?\d+\.?\d*),\n +\),\n +\),").unwrap();
                if let Some(caps) = re.captures(&scene_serialized) {
                    println!("reformat rect");
                    let replacement = r"area: (
            min: (X1, Y1),
            max: (X2, Y2),
          ),";
                    let new_val = replacement
                        .replace("X1", caps.get(1).unwrap().into())
                        .replace("Y1", caps.get(2).unwrap().into())
                        .replace("X2", caps.get(3).unwrap().into())
                        .replace("Y2", caps.get(4).unwrap().into());

                    scene_serialized = re.replace(&scene_serialized, new_val).to_string();
                } else {
                    break;
                }
            }

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
    mut ev_select_entity: EventWriter<SelectEntity>,
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
                let entity = commands
                    .spawn(PbrBundle {
                        mesh: mesh_handle,
                        material: material_handle,
                        transform: Transform::from_xyz(0.0, 0.0, 0.0),
                        ..default()
                    })
                    .insert(Name::new(event.0.to_string()))
                    .id();

                ev_select_entity.send(SelectEntity(entity));
            }
            SimpleObject::Light(light) => {
                let wireframe_mesh = Mesh::from(shape::Cube { size: 1.0 });
                let wireframe_mesh_handle = meshes.add(wireframe_mesh);
                if let HandleId::Id(_, id) = wireframe_mesh_handle.id() {
                    asset_source_list.0.push(AssetSource {
                        source_type: AssetSourceType::AsString("Cube".to_string()),
                        type_uuid: Mesh::TYPE_UUID.to_string(),
                        uid: id,
                    });
                } else {
                    error!("AssetPathId handle is not supported yet");
                }
                let name = Name::new(light.to_string());
                match &light {
                    // WIP - spotlight has no effect - broken ???
                    Light::Spot => {
                        let entity = commands
                            .spawn((
                                SpotLightBundle::default(),
                                name,
                                wireframe_mesh_handle,
                                Wireframe,
                                FixedWireframe,
                            ))
                            .id();

                        ev_select_entity.send(SelectEntity(entity));
                    }
                    Light::Point => {
                        let entity = commands
                            .spawn((
                                PointLightBundle::default(),
                                name,
                                wireframe_mesh_handle,
                                Wireframe,
                                FixedWireframe,
                            ))
                            .id();

                        ev_select_entity.send(SelectEntity(entity));
                    }
                    Light::Directional => {
                        let entity = commands
                            .spawn((
                                DirectionalLightBundle::default(),
                                name,
                                wireframe_mesh_handle,
                                Wireframe,
                                FixedWireframe,
                            ))
                            .id();

                        ev_select_entity.send(SelectEntity(entity));
                    }
                    Light::Ambient => error!("WIP Ambient light"),
                }
            }
            SimpleObject::Camera(camera) => {
                let wireframe_mesh = Mesh::from(shape::Cube { size: 1.0 });
                let wireframe_mesh_handle = meshes.add(wireframe_mesh);
                if let HandleId::Id(_, id) = wireframe_mesh_handle.id() {
                    asset_source_list.0.push(AssetSource {
                        source_type: AssetSourceType::AsString("Cube".to_string()),
                        type_uuid: Mesh::TYPE_UUID.to_string(),
                        uid: id,
                    });
                } else {
                    error!("AssetPathId handle is not supported yet");
                }
                let name = Name::new(camera.to_string());
                let projection = match camera {
                    Camera::Perspective => {
                        Projection::Perspective(PerspectiveProjection::default())
                    }
                    Camera::Orthographic => Projection::Orthographic(OrthographicProjection {
                        scale: 0.01,
                        ..default()
                    }),
                };
                let entity = commands
                    .spawn((
                        Camera3dBundle {
                            projection,
                            camera: camera::Camera {
                                is_active: false,
                                ..default()
                            },
                            ..default()
                        },
                        Visibility::default(),
                        ComputedVisibility::default(),
                        name,
                        wireframe_mesh_handle,
                        Wireframe,
                        FixedWireframe,
                    ))
                    .id();

                ev_select_entity.send(SelectEntity(entity));
            }
        }
    }
}

fn process_scripts(world: &mut World) {
    world.resource_scope(|world, mut registry: Mut<ScriptableRegistry>| {
        world.resource_scope(|world, mut editor_state: Mut<ControlState>| {
            if editor_state.playing {
                registry.exec(world);
                if editor_state.initial {
                    editor_state.initial = false;
                }
            }
        });
    });
}

#[derive(Event)]
struct LoadScript(String, bool);

struct AttachScript(String, bool);

impl Command for AttachScript {
    fn apply(self, world: &mut World) {
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

fn show_popup_on_error(mut reader: EventReader<PushLog>, mut editor_state: ResMut<EditorState>) {
    for event in reader.iter() {
        if event.1 == Level::Error || event.1 == Level::Fatal {
            editor_state.current_popup = Some(Box::new(crate::error::Error {
                code: event.0.to_string(),
                details: None,
            }))
        }
    }
}

impl Command for StartPlaying {
    fn apply(self, world: &mut World) {
        let mut state = world.resource::<ControlState>();
        if state.initial {
            println!("will apply");
            world.resource_scope(|world, mut registry: Mut<ScriptableRegistry>| {
                registry.start(world);
            });
            println!("start done");
        }
        let mut state = world.resource_mut::<ControlState>();
        state.playing = true;
        println!("updated flag");
    }
}

fn handle_start_playing(mut commands: Commands, mut events: EventReader<StartPlaying>) {
    for _ in events.iter() {
        println!("got event");
        commands.add(StartPlaying);
        break; // only run once
    }
}
