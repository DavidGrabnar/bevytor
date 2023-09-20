use bevy::app::{App, Plugin};
use bevy::prelude::{Event, EventReader, ResMut, Resource, Update};
use bevy_egui::egui::{ScrollArea, Ui};
use std::collections::linked_list::Iter;
use std::collections::LinkedList;
use std::fmt;

#[derive(Copy, Clone, PartialEq)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Level::Trace => write!(f, "TRACE"),
            Level::Debug => write!(f, "DEBUG"),
            Level::Info => write!(f, "INFO"),
            Level::Warn => write!(f, "WARN"),
            Level::Error => write!(f, "ERROR"),
            Level::Fatal => write!(f, "FATAL"),
        }
    }
}

pub struct Log {
    level: Level,
    message: String,
}

impl fmt::Display for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.level, self.message)
    }
}

#[derive(Resource, Default)]
pub struct LogBuffer(LinkedList<Log>);

const LOG_SIZE: usize = 100;

impl LogBuffer {
    pub fn write(&mut self, message: String, level: Level) {
        if self.0.len() == LOG_SIZE {
            self.0.pop_front();
        }
        self.0.push_back(Log { message, level });
    }

    pub fn iter(&self) -> Iter<Log> {
        self.0.iter()
    }
}

macro_rules! impl_write_level {
    ($($level: ident),*) => {
        paste::item! {
            impl LogBuffer {
                $(
                    pub fn [< write_ $level:lower >](&mut self, message: String) {
                        self.write(message, Level::$level);
                    }
                )*
            }
        }
    }
}

impl_write_level!(Trace, Debug, Info, Warn, Error, Fatal);

pub fn logs_ui(ui: &mut Ui, log_buffer: &LogBuffer) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.set_height(300.);
        ui.set_width(ui.available_width());
        for entry in log_buffer.iter() {
            ui.label(entry.to_string());
        }
    });
}

fn store_logs(mut reader: EventReader<PushLog>, mut buffer: ResMut<LogBuffer>) {
    for event in reader.iter() {
        buffer.write(event.0.clone(), event.1);
    }
}

#[derive(Event)]
pub struct PushLog(pub String, pub Level);

pub struct LogPlugin;

impl Plugin for LogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LogBuffer>()
            .add_event::<PushLog>()
            .add_systems(Update, store_logs);
    }
}
