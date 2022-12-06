use bevy::prelude::Entity;
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
pub struct Node(Option<Entity>, pub(crate) Vec<Node>);

impl Debug for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}", self.0.map(|e| e.id()).unwrap_or(0)).as_str())?;
        for child in self.1.as_slice() {
            f.write_str(format!("({:?})", child).as_str())?;
        }
        std::fmt::Result::Ok(())
    }
}

impl Node {
    pub fn new(entity: Option<Entity>, children: Vec<Node>) -> Self {
        Self(entity, children)
    }

    fn ui(&self, ui: &mut Ui) -> Action {
        let name = match self.0 {
            Some(entity) => entity.id().to_string(),
            None => "Scene".to_string(),
        };

        let collapsible = CollapsingHeader::new(name)
            .id_source(
                self.0
                    .map(|entity| format!("{}", entity.id()))
                    .unwrap_or_else(|| "0".to_string()),
            )
            .selectable(self.0.is_some());

        let response = collapsible.show(ui, |ui| {
            let button_response = match self.0 {
                Some(_entity) => Some(ui.button("Inspect")),
                None => None,
            };
            let children_response = self.1.iter().fold(Action::NoAction, |curr_action, child| {
                let action = child.ui(ui);
                if let Action::Selected(_entity) = action {
                    return action;
                }
                curr_action
            });

            if let Action::Selected(_entity) = children_response {
                return children_response;
            }

            if let Some(response) = button_response {
                if response.clicked() {
                    if let Some(entity) = self.0 {
                        return Action::Selected(entity);
                    }
                }
            }

            return Action::NoAction;
        });

        if let Some(action) = response.body_returned {
            return action;
        }

        Action::NoAction
    }
}
