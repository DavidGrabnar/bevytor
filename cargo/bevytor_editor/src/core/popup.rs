use bevy_egui::egui;
use bevy_egui::egui::{Context, Ui, Window};

pub type BoxedPopup = Box<dyn Popup + Send + Sync>;

pub trait Popup {
    fn title(&self) -> &'static str;
    fn ui(&self, ui: &mut Ui) -> bool;
}

pub fn show_popup(context: &Context, popup: &BoxedPopup) -> bool {
    let response = Window::new(popup.title())
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(context, |ui| popup.ui(ui));

    if let Some(response) = response {
        if let Some(response) = response.inner {
            return response;
        }
    }
    false
}
