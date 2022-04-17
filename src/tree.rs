use crate::egui::{CollapsingHeader, Ui};

pub enum Action {
    NoAction,
    Selected(u32),
}

pub struct Tree(Node);

impl Tree {
    pub fn new(root: Node) -> Self {
        Self(root)
    }
}

impl Tree {
    pub fn ui(self, ui: &mut Ui) -> Action {
        self.0.ui(ui)
    }
}

#[derive(Clone)]
pub struct Node(String, Option<u32>, Vec<Node>);

impl Node {
    pub fn new(name: String, id: Option<u32>, children: Vec<Node>) -> Self {
        Self(name, id, children)
    }
}

impl Node {
    fn ui(self, ui: &mut Ui) -> Action {
        let collapsible = CollapsingHeader::new(self.0).selectable(self.1.is_some());

        let response = collapsible.show(ui, |ui| {
            let button_response = match self.1 {
                Some(_id) => Some(ui.button("Inspect")),
                None => None
            };
            let children_response = self.2
                .into_iter()
                .fold(Action::NoAction, |curr_action, child| {
                    let action = child.ui(ui);
                    if let Action::Selected(_x) = action {
                        return action;
                    }
                    curr_action
                });

            if let Action::Selected(_id) = children_response {
                return children_response;
            }

            if let Some(response) = button_response {
                if response.clicked() {
                    if let Some(id) = self.1 {
                        return Action::Selected(id);
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
