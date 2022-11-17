use std::fs;
use std::path::{Path, PathBuf};
use bevy_egui::egui::Ui;
use crate::error::EResult;
use crate::error::Error;

pub enum FileEditorAction {
    ChangePath(PathBuf),
    Select,
    None
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
        Err(e) => bail!("FILE_EXPLORER::SHOW::CANNOT_READ_DIR", e)
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