use bevy::prelude::Entity;
use crate::egui::{CollapsingHeader, Ui};

pub enum Action {
    NoAction,
    Selected(Entity),
}

pub struct Tree(Node);

impl Tree {
    pub fn new(root: Node) -> Self {
        Self(root)
    }

    pub fn ui(self, ui: &mut Ui) -> Action {
        self.0.ui(ui)
    }
}

#[derive(Clone)]
pub struct Node(Option<Entity>, Vec<Node>);

impl Node {
    pub fn new(entity: Option<Entity>, children: Vec<Node>) -> Self {
        Self(entity, children)
    }

    fn ui(self, ui: &mut Ui) -> Action {
        let name = match self.0 {
            Some(entity) => entity.id().to_string(),
            None => "Scene".to_string()
        };
        let collapsible = CollapsingHeader::new(name).selectable(self.0.is_some());

        let response = collapsible.show(ui, |ui| {
            let button_response = match self.0 {
                Some(_entity) => Some(ui.button("Inspect")),
                None => None
            };
            let children_response = self.1
                .into_iter()
                .fold(Action::NoAction, |curr_action, child| {
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
