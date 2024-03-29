use crate::core::popup::Popup;
use bevy_egui::egui::Ui;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Error {
    pub code: String,
    pub details: Option<String>,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} -> {}",
            self.code,
            self.details.as_ref().unwrap_or(&"/".to_string())
        )
    }
}

pub type EResult<T> = Result<T, Error>;

#[macro_export]
macro_rules! bail {
    ($a:expr) => {{
        return Err(Error::new($a));
    }};

    ($a:expr,$b:expr) => {{
        return Err(Error::new_with_details($a, $b));
    }};
}

impl Error {
    pub fn new(code: impl ToString) -> Error {
        Error {
            code: code.to_string(),
            details: None,
        }
    }

    pub fn new_with_details(code: impl ToString, details: impl ToString) -> Error {
        Error {
            code: code.to_string(),
            details: Some(details.to_string()),
        }
    }
}

impl Popup for Error {
    fn title(&self) -> &'static str {
        "Error"
    }

    fn ui(&self, ui: &mut Ui) -> bool {
        ui.horizontal(|ui| {
            ui.label("ERR"); // TODO error icon
            ui.label(self.code.as_str());
        });
        if let Some(details) = &self.details {
            ui.separator();
            ui.label(details);
        }
        ui.button("Close").clicked()
    }
}
