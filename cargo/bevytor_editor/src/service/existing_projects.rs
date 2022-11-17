use std::collections::HashSet;
use std::fs;
use std::path::Path;
use crate::bail;
use crate::error::{EResult, Error};
use crate::service::project::ProjectDescription;

// TODO env config?
const EDITOR_STORAGE_PATH: &str = "/home/grabn/temp/bevytor/storage";
const EXISTING_PROJECTS_FILE: &str = "existing_projects.json";

#[derive(Default)]
pub struct ExistingProjects(pub(crate) HashSet<ProjectDescription>);

impl ExistingProjects {
    pub fn load() -> EResult<Self> {
        let project_path = Path::new(EDITOR_STORAGE_PATH).join(EXISTING_PROJECTS_FILE);

        if !project_path.exists() {
            return Ok(Self::default())
        }
        let serialized = match fs::read_to_string(project_path) {
            Ok(result) => result,
            Err(e) => bail!("PROJECT::LOAD_EXISTING::CANNOT_READ_FILE", e)
        };

        let parsed = match serde_json::from_str(serialized.as_str()) {
            Ok(result) => result,
            Err(e) => bail!("PROJECT::LOAD_EXISTING::CANNOT_PARSE", e)
        };

        Ok(ExistingProjects(parsed))
    }

    pub fn add(&mut self, project: ProjectDescription) -> EResult<()> {
        self.0.insert(project);
        self.save()
    }

    pub fn remove(&mut self, project: &ProjectDescription) -> EResult<()> {
        self.0.remove(project);
        self.save()
    }

    fn save(&self) -> EResult<()> {
        let project_path = Path::new(EDITOR_STORAGE_PATH);
        if project_path.exists() {
            if !project_path.is_dir() {
                bail!("PROJECT::SAVE_EXISTING::PATH_NOT_DIR");
            }
            let dir_contents = project_path.read_dir();
            if dir_contents.is_err() {
                bail!("PROJECT::SAVE_EXISTING::CANNOT_READ_DIR");
            }
        } else if let Err(e) = fs::create_dir_all(project_path) {
            bail!("PROJECT::SAVE_EXISTING::CANNOT_CREATE_DIR", e);
        }
        // TODO multiple file formats: readable serialization, json, binary etc. ???

        match serde_json::to_string(&self.0) {
            Ok(serialized) => match fs::write(project_path.join(EXISTING_PROJECTS_FILE), serialized) {
                Err(e) => bail!("PROJECT::SAVE_EXISTING::CANNOT_WRITE_TO_PROJECT_FILE", e),
                Ok(_) => Ok(())
            },
            Err(e) => bail!("PROJECT::SAVE_EXISTING::CANNOT_SERIALIZE", e)
        }
    }
}

