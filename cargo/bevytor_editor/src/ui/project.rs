use crate::error::{EResult, Error};
use crate::service::existing_projects::ExistingProjects;
use crate::service::project::{Project, ProjectDescription};
use bevy_egui::egui::{Id, InnerResponse, Response, Sense, Ui, Window};
use std::ffi::OsString;

pub enum ProjectListAction {
    Create(ProjectDescription),
    NewOpen(ProjectDescription),
    ExistingOpen(ProjectDescription),
    ExistingRemove(ProjectDescription),
}

pub enum ProjectRowAction {
    Select,
    Remove,
}

pub fn project_list(
    ui: &mut Ui,
    projects: &ExistingProjects,
) -> EResult<Option<ProjectListAction>> {
    if project_row(ui, "➕", "Create a new project", None, false).is_some() {
        // ignore action as Remove cannot be returned if removable is false
        println!("CLICK");
        // Window::new("Select new project directory")
        //     .open(open)
        //     .resizable(false)
        //     .show(ctx, |ui| {
        //         use super::View as _;
        //         self.ui(ui);
        //     });

        // TODO show panel with name and location select - file explorer with folder filter, project folder name is slug of project name (can be modified)
        let path = OsString::from("D:\\Projects\\Rust\\bevytor\\das_demo");
        let name = "Das demo".to_string();
        if let Err(e) = Project::verify_new(path.clone()) {
            bail!("PROJECT::LIST::CREATE", e);
        }
        return Ok(Some(ProjectListAction::Create(ProjectDescription {
            name,
            path,
        })));
    }
    ui.separator();

    if project_row(ui, "🗁", "Open an existing project", None, false).is_some() {
        // ignore action as Remove cannot be returned if removable is false
        // TODO show file explorer with folder filter, mark folders with project files (check extension & validate format of main project file)
        let path = OsString::from("~/Documents/sample_project");
        let name = path.to_str().unwrap_or("").to_string();
        if let Err(e) = Project::verify_existing(path.clone()) {
            bail!("PROJECT::LIST::OPEN_EXISTING", e);
        }
        // TODO validate project file in selected dir
        return Ok(Some(ProjectListAction::NewOpen(ProjectDescription {
            name,
            path,
        })));
    }
    if !projects.0.is_empty() {
        ui.separator();
    }

    for project in &projects.0 {
        if let Some(action) = project_row(
            ui,
            "🚀",
            &project.name,
            project.path.as_os_str().to_str(),
            true,
        ) {
            return match action {
                ProjectRowAction::Select => {
                    Ok(Some(ProjectListAction::ExistingOpen(project.clone())))
                }
                ProjectRowAction::Remove => {
                    Ok(Some(ProjectListAction::ExistingRemove(project.clone())))
                }
            };
        }
    }

    Ok(None)
}

fn project_row(
    ui: &mut Ui,
    icon: &str,
    name: &str,
    path: Option<&str>,
    removable: bool,
) -> Option<ProjectRowAction> {
    let response = ui.horizontal(|ui| {
        let response = ui.push_id(name, |ui| {
            ui.label(icon); // TODO avatar kind of icon, centered vertically, 2 lines height
            ui.vertical(|ui| {
                ui.label(name);
                ui.label(path.unwrap_or("---")); // TODO handle no path in a nicer way
            });
        });
        if response.response.interact(Sense::click()).clicked() {
            return Some(ProjectRowAction::Select);
        }
        // TODO move button to left of panel
        if removable && ui.button("❌").clicked() {
            return Some(ProjectRowAction::Remove);
        }
        None
    });

    response.inner
}
