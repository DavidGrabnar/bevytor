use bevy::prelude::Entity;
use bevy_egui::egui::collapsing_header::CollapsingState;
use bevy_egui::egui::{CollapsingHeader, Ui};
use std::fmt::{Debug, Formatter, Write};

pub enum Action {
    NoAction,
    Selected(Entity),
}

#[derive(Default)]
pub struct Tree(Node);

impl Tree {
    pub fn new(root: Node) -> Self {
        Self(root)
    }

    pub fn ui(&self, ui: &mut Ui) -> Action {
        self.0.ui(ui)
    }
}

impl Debug for Tree {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{:?}", self.0).as_str())
    }
}

#[derive(Clone, Default)]
pub struct Node(Option<(Entity, String)>, pub(crate) Vec<Node>);

impl Debug for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}", self.0.as_ref().map(|e| e.0.index()).unwrap_or(0)).as_str())?;
        for child in self.1.as_slice() {
            f.write_str(format!("({:?})", child).as_str())?;
        }
        std::fmt::Result::Ok(())
    }
}

impl Node {
    pub fn new(entity: Option<(Entity, String)>, mut children: Vec<Node>) -> Self {
        children.sort_by_key(|c| c.0.as_ref().map(|v| v.0.index()).unwrap_or_default());
        Self(entity, children)
    }

    fn ui(&self, ui: &mut Ui) -> Action {
        let name = match self.0.as_ref() {
            Some(entity) => entity.1.as_str(),
            None => "Scene",
        };

        if self.1.is_empty() {
            let response = ui
                .horizontal(|ui| {
                    ui.label("  - ");
                    ui.label(name);
                    match self.0.as_ref() {
                        Some(_entity) => Some(ui.button("👁")),
                        None => None,
                    }
                })
                .inner;
            if let Some(response) = response {
                if response.clicked() {
                    return Action::Selected(self.0.as_ref().unwrap().0);
                }
            }
        } else {
            let id = self
                .0
                .as_ref()
                .map(|entity| format!("{}", entity.0.index()))
                .unwrap_or_else(|| "0".to_string());

            let (response, header_response, body_response) =
                CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(id), false)
                    .show_header(ui, |ui| {
                        ui.label(name);
                        match self.0.as_ref() {
                            Some(_entity) => Some(ui.button("👁")),
                            None => None,
                        }
                    })
                    .body(|ui| {
                        self.1.iter().fold(Action::NoAction, |curr_action, child| {
                            let action = child.ui(ui);
                            if let Action::Selected(_entity) = action {
                                return action;
                            }
                            curr_action
                        })
                    });

            if let Some(response) = body_response {
                if let Action::Selected(_entity) = response.inner {
                    return response.inner;
                }
            }

            if let Some(response) = header_response.inner {
                if response.clicked() {
                    if let Some(entity) = self.0.as_ref() {
                        return Action::Selected(entity.0);
                    }
                }
            }
        }
        Action::NoAction
    }
}
