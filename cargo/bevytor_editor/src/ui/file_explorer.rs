use crate::error::EResult;
use crate::error::Error;
use bevy_egui::egui::{CollapsingHeader, Ui};
use std::fs;
use std::path::{Path, PathBuf};

pub enum FileEditorAction {
    ChangePath(PathBuf),
    Select,
    None,
}

pub fn show_ui_file_editor(ui: &mut Ui, current_path: &Path) -> EResult<FileEditorAction> {
    ui.horizontal(|ui| {
        ui.label("⬅");
        ui.label("➡");
        ui.label("⬆");
        ui.label(current_path.to_str().unwrap()); // TODO change to input and sync with explorer
                                                  // TODO add search
    });

    ui.separator();

    let contents = match current_path.read_dir() {
        Ok(result) => result,
        Err(e) => bail!("FILE_EXPLORER::SHOW::CANNOT_READ_DIR", e),
    };
    // TODO multiple views
    for entry in contents.flatten() {
        ui.horizontal(|ui| {
            ui.label(entry.path().as_os_str().to_str().unwrap());
            // TODO add more info
        });
    }

    Ok(FileEditorAction::None)
}

pub fn explorer_row(ui: &mut Ui, path: &str, current: &str) {
    CollapsingHeader::new(path)
        .default_open(Path::new(current).starts_with(path))
        .show(ui, |ui| {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let entry_type = entry.file_type().unwrap();
                if entry_type.is_dir() {
                    explorer_row(ui, entry.path().to_str().unwrap(), current);
                }
            }
        });
}
