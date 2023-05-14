use crate::error::{EResult, Error};
use crate::{bail, World};
use bevy::asset::{FileAssetIo, HandleId};
use bevy::prelude::*;
use bevy::render::camera::{CameraProjection, Projection};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ffi::OsString;
use std::path::Path;
use std::{fs, io};

const EDITOR_ROOT_FOLDER_PATH: &str = env!("CARGO_MANIFEST_DIR");
const TEMPLATE_PROJECT_DIR_PATH: &str = "resources/project_template";
const INITIAL_TEMPLATE_SCENE_PATH: &str = "resources/initial.scn.ron";
const INITIAL_TEMPLATE_ASSET_PATH: &str = "resources/initial.asset.ron";
const INITIAL_ASSETS_PATH: &str = "resources/assets";

#[derive(Default, Serialize, Deserialize, Eq, PartialEq, Hash, Clone)]
pub struct ProjectDescription {
    pub name: String,
    pub path: OsString,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProjectState {
    pub scene_file: String,
    pub asset_file: String,
    pub assets_folder: String,
    pub script_enabled: bool,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Project {
    pub project_description: ProjectDescription,
    pub project_state: ProjectState,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            scene_file: "initial.scn.ron".to_string(),
            asset_file: "initial.asset.ron".to_string(),
            assets_folder: "assets".to_string(),
            script_enabled: false,
        }
    }
}

impl Project {
    pub fn verify_new(path: OsString) -> EResult<()> {
        let project_path = Path::new(path.as_os_str());
        if project_path.exists() {
            if !project_path.is_dir() {
                bail!("PROJECT::BUILD::PATH_NOT_DIR");
            }
            let dir_contents = project_path.read_dir();
            if dir_contents.is_err() {
                bail!("PROJECT::BUILD::CANNOT_READ_DIR");
            }
            if dir_contents.unwrap().next().is_some() {
                bail!("PROJECT::BUILD::DIR_NOT_EMPTY");
            }
        } else if let Err(e) = fs::create_dir_all(project_path) {
            bail!("PROJECT::BUILD::CANNOT_CREATE_DIR", e);
        }

        Ok(())
    }

    pub fn verify_existing(path: OsString) -> EResult<()> {
        let project_path = Path::new(path.as_os_str());
        if project_path.exists() {
            if !project_path.is_dir() {
                bail!("PROJECT::BUILD::PATH_NOT_DIR");
            }
            let dir_contents = project_path.read_dir();
            if dir_contents.is_err() {
                bail!("PROJECT::BUILD::CANNOT_READ_DIR");
            }
            // TODO validate project files
        } else {
            bail!("PROJECT::BUILD::PATH_DOES_NOT_EXIST");
        }

        Ok(())
    }

    pub fn generate(description: ProjectDescription) -> EResult<Project> {
        let project = Project {
            project_description: description.clone(),
            project_state: ProjectState::default(),
        };

        if let Err(e) = Self::verify_new(description.path.clone()) {
            bail!("PROJECT::GENERATE::VERIFY_FAILED", e);
        }

        // TODO multiple file formats: readable serialization, json, binary etc. ???

        let project_path = Path::new(description.path.as_os_str());
        let serialized = match serde_json::to_string(&project) {
            Ok(result) => result,
            Err(e) => bail!("PROJECT::BUILD::CANNOT_SERIALIZE_PROJECT", e),
        };

        //if let Err(e) = fs::create_dir(project_path.join("scenes")) {
        //    bail!("PROJECT::BUILD::CANNOT_CREATE_SCENES_FOLDER", e);
        //}

        let template_project_dir_path =
            Path::new(EDITOR_ROOT_FOLDER_PATH).join(TEMPLATE_PROJECT_DIR_PATH);

        if let Err(e) = copy_recursively(template_project_dir_path, project_path) {
            bail!("PROJECT::BUILD::CANNOT_COPY_TEMPLATE", e);
        }

        if let Err(e) = fs::write(project_path.join("project.bv"), serialized) {
            bail!("PROJECT::BUILD::CANNOT_WRITE_TO_PROJECT_FILE", e);
        }

        /*
        let template_scene_path =
            Path::new(EDITOR_ROOT_FOLDER_PATH).join(INITIAL_TEMPLATE_SCENE_PATH);

        let template_scene_content = match fs::read(template_scene_path) {
            Ok(result) => result,
            Err(e) => bail!("PROJECT::BUILD::CANNOT_READ_SCENE_TEMPLATE_FILE", e),
        };

        // fs::copy fails for network mounted disks as it cannot copy permissions, solved by reading template + writing a new file
        if let Err(e) = fs::write(
            project_path
                .join("scenes")
                .join(project.project_state.scene_file.clone()),
            template_scene_content,
        ) {
            bail!("PROJECT::BUILD::CANNOT_WRITE_SCENE_TEMPLATE_FILE", e);
        }

        let template_asset_path =
            Path::new(EDITOR_ROOT_FOLDER_PATH).join(INITIAL_TEMPLATE_ASSET_PATH);

        let template_asset_content = match fs::read(template_asset_path) {
            Ok(result) => result,
            Err(e) => bail!("PROJECT::BUILD::CANNOT_READ_ASSET_TEMPLATE_FILE", e),
        };

        if let Err(e) = fs::write(
            project_path
                .join("scenes")
                .join(project.project_state.asset_file.clone()),
            template_asset_content,
        ) {
            bail!("PROJECT::BUILD::CANNOT_WRITE_ASSET_TEMPLATE_FILE", e);
        }

        if let Err(e) = fs::create_dir_all(
            project_path
                .join("scenes")
                .join(project.project_state.assets_folder.clone()),
        ) {
            bail!("PROJECT::BUILD::CANNOT_CREATE_ASSETS_FOLDER", e);
        }

        match fs::read_dir(Path::new(EDITOR_ROOT_FOLDER_PATH).join(INITIAL_ASSETS_PATH)) {
            Err(e) => bail!("PROJECT::BUILD::CANNOT_READ_ASSETS_FOLDER", e),
            Ok(dir) => {
                for entry in dir {
                    match entry {
                        Err(e) => bail!("PROJECT::BUILD::CANNOT_READ_ASSET", e),
                        Ok(entry) => {
                            if let Err(e) = fs::copy(
                                entry.path(),
                                project_path
                                    .join("scenes")
                                    .join(project.project_state.assets_folder.clone())
                                    .join(entry.file_name()),
                            ) {
                                bail!("PROJECT::BUILD::CANNOT_WRITE_ASSET", e);
                            }
                        }
                    }
                }
            }
        }*/

        // TODO cleanup on error ?

        Ok(project)
    }

    pub fn load(description: ProjectDescription) -> EResult<Project> {
        if let Err(e) = Self::verify_existing(description.path.clone()) {
            bail!("PROJECT::LOAD::VERIFY_FAILED", e);
        }

        let project_path = Path::new(description.path.as_os_str());
        let serialized = match fs::read_to_string(project_path.join("project.bv")) {
            Ok(result) => result,
            Err(e) => bail!("PROJECT::LOAD::CANNOT_READ_PROJECT_FILE", e),
        };

        let project = match serde_json::from_str(&serialized) {
            Ok(result) => result,
            Err(e) => bail!("PROJECT::LOAD::CANNOT_DESERIALIZE_PROJECT", e),
        };

        Ok(project)
    }
}
/*
fn setup_template_scene() -> World {
    let mut world = World::new();

    let settings = world.get_resource_or_insert_with(AssetServerSettings::default);
    let source = FileAssetIo::new(&settings.asset_folder, settings.watch_for_changes);

    let asset_server = AssetServer::with_boxed_io(Box::new(source));
    world.insert_resource(asset_server);
    // set up the camera
    let mut camera = Camera3dBundle::default();
    camera.projection = Projection::Orthographic(OrthographicProjection::default());
    camera.projection.get_projection_matrix().mul_scalar(3.0);
    camera.transform = Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);

    // camera
    world.spawn().insert_bundle(camera);

    // plane
    let bundle = setup_template_pbr_bundle(
        &mut world,
        Mesh::from(shape::Plane { size: 5.0 }),
        Color::rgb(0.3, 0.5, 0.3),
        Transform::default(),
    );
    world.spawn().insert_bundle(bundle);
    // cube
    let bundle = setup_template_pbr_bundle(
        &mut world,
        Mesh::from(shape::Cube { size: 1.0 }),
        Color::rgb(0.8, 0.7, 0.6),
        Transform::from_xyz(0.0, 0.5, 0.0),
    );
    let cube = world.spawn().insert_bundle(bundle).id();
    // child cube
    let bundle = setup_template_pbr_bundle(
        &mut world,
        Mesh::from(shape::Cube { size: 1.0 }),
        Color::rgb(0.6, 0.7, 0.8),
        Transform::from_xyz(0.0, 1.0, 0.0),
    );
    world
        .spawn()
        .insert_bundle(bundle)
        .insert(Children::with(&[cube]));
    // light
    world.spawn().insert_bundle(PointLightBundle {
        transform: Transform::from_xyz(3.0, 8.0, 5.0),
        ..Default::default()
    });

    world
}
fn setup_template_pbr_bundle(
    world: &mut World,
    mesh: Mesh,
    color: Color,
    transform: Transform,
) -> PbrBundle {
    PbrBundle {
        mesh: world
            .get_resource_mut::<ResMut<Assets<Mesh>>>()
            .unwrap()
            .add(mesh),
        material: world
            .get_resource_mut::<ResMut<Assets<StandardMaterial>>>()
            .unwrap()
            .add(color.into()),
        transform,
        ..Default::default()
    }
}
*/

fn copy_recursively(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let filetype = entry.file_type()?;
        if filetype.is_dir() {
            copy_recursively(entry.path(), destination.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), destination.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
