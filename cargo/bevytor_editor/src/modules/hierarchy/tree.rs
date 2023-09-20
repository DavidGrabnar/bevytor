use bevy::prelude::{Entity, Resource};
use bevy_egui::egui;
use bevy_egui::egui::collapsing_header::CollapsingState;
use bevy_egui::egui::{Id, LayerId, Order, Sense, Ui};
use std::fmt::{Debug, Formatter};

pub enum NodeAction {
    Select(Entity),
    Clone(Entity),
    Remove(Entity),
}

pub enum TreeAction {
    NoAction,
    Node(NodeAction),
    Move(Entity, HoverEntity),
}

#[derive(Debug, Clone)]
pub enum HoverEntity {
    Root,
    Node(Entity),
}

impl HoverEntity {
    pub fn entity(&self) -> Option<Entity> {
        match self {
            Self::Root => None,
            HoverEntity::Node(entity) => Some(*entity),
        }
    }
}

#[derive(Default)]
pub struct Context {
    drag_entity: Option<Entity>,
    hover_entity: Option<HoverEntity>,
}

#[derive(Resource, Default)]
pub struct Tree(Node);

impl Tree {
    pub fn new(root: Node) -> Self {
        Self(root)
    }

    pub fn ui(&self, ui: &mut Ui) -> TreeAction {
        let mut context = Context::default();
        let action = self.0.ui(ui, &mut context);

        if let Some(dragged) = context.drag_entity {
            if let Some(hovered) = context.hover_entity {
                if ui.input(|ui| ui.pointer.any_released()) {
                    if let TreeAction::NoAction = &action {
                        if let HoverEntity::Node(entity) = hovered {
                            if entity != dragged {
                                // only return if not equal
                                return TreeAction::Move(dragged, hovered);
                            }
                        } else {
                            return TreeAction::Move(dragged, hovered);
                        }
                    }
                }
            }
        }

        action
    }
}

impl Debug for Tree {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{:?}", self.0).as_str())
    }
}

#[derive(Clone, Default)]
pub struct Node(Option<(Entity, String)>, pub Vec<Node>);

impl Debug for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}", self.0.as_ref().map(|e| e.0.index()).unwrap_or(0)).as_str())?;
        for child in self.1.as_slice() {
            f.write_str(format!("({:?})", child).as_str())?;
        }
        Ok(())
    }
}

impl Node {
    pub fn new(entity: Option<(Entity, String)>, mut children: Vec<Node>) -> Self {
        children.sort_by_key(|c| c.0.as_ref().map(|v| v.0.index()).unwrap_or_default());
        Self(entity, children)
    }

    fn ui(&self, ui: &mut Ui, context: &mut Context) -> TreeAction {
        let name = match self.0.as_ref() {
            Some(entity) => entity.1.as_str(),
            None => "Scene",
        };
        let id_source = match self.0 {
            Some((entity, _)) => format!("{}v{}", entity.index(), entity.generation()),
            None => "root".to_string(),
        };

        if self.1.is_empty() {
            // TODO what to accept?? if not already child????? and if not itself?????
            let can_accept_what_is_being_dragged = true;
            let response = drop_target(ui, can_accept_what_is_being_dragged, |ui| {
                ui.horizontal(|ui| {
                    ui.label("  - ");
                    let (dragged, response) = self.ui_element(name, id_source, ui);
                    if dragged {
                        // root cannot be dragged, None can be ignored
                        if let Some(entity) = &self.0 {
                            context.drag_entity = Some(entity.0);
                        }
                    }
                    response.map_or(TreeAction::NoAction, |a| TreeAction::Node(a))
                })
            });
            let is_being_dragged = ui.memory(|ui| ui.is_anything_being_dragged());
            if is_being_dragged && can_accept_what_is_being_dragged && response.response.hovered() {
                let hover_entity = self
                    .0
                    .as_ref()
                    .map(|e| HoverEntity::Node(e.0.clone()))
                    .unwrap_or(HoverEntity::Root);
                context.hover_entity = Some(hover_entity.clone());
            }
            return response.inner;
        } else {
            let can_accept_what_is_being_dragged = true;
            let (response, header_response, body_response) =
                drop_target(ui, can_accept_what_is_being_dragged, |ui| {
                    CollapsingState::load_with_default_open(
                        ui.ctx(),
                        ui.make_persistent_id(id_source.clone()),
                        false,
                    )
                    .show_header(ui, |ui| self.ui_element(name, id_source, ui))
                    .body(|ui| {
                        self.1
                            .iter()
                            .fold(TreeAction::NoAction, |curr_action, child| {
                                let action = child.ui(ui, context);
                                if let TreeAction::Node(_) = action {
                                    return action;
                                }
                                curr_action
                            })
                    })
                });
            if let Some(response) = body_response {
                if let TreeAction::Node(_) = response.inner {
                    return response.inner;
                }
            }
            if header_response.inner.0 {
                // root cannot be dragged, None can be ignored
                if let Some(entity) = &self.0 {
                    context.drag_entity = Some(entity.0);
                }
            }
            if let Some(a) = header_response.inner.1 {
                return TreeAction::Node(a);
            }
            let is_being_dragged = ui.memory(|ui| ui.is_anything_being_dragged());
            if is_being_dragged && can_accept_what_is_being_dragged && response.hovered() {
                let hover_entity = self
                    .0
                    .as_ref()
                    .map(|e| HoverEntity::Node(e.0.clone()))
                    .unwrap_or(HoverEntity::Root);
                context.hover_entity = Some(hover_entity.clone());
            }
        }
        TreeAction::NoAction
    }

    fn ui_element(&self, name: &str, id_source: String, ui: &mut Ui) -> (bool, Option<NodeAction>) {
        let (dragged, _) = drag_source(ui, Id::new(id_source), |ui| {
            ui.label(name);
        });
        let response = ui
            .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(el) = &self.0 {
                    if ui.button("❌").clicked() {
                        Some(NodeAction::Remove(el.0))
                    } else if ui.button("🗐").clicked() {
                        Some(NodeAction::Clone(el.0))
                    } else if ui.button("👁").clicked() {
                        Some(NodeAction::Select(el.0))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .inner;
        (dragged, response)
    }
}

pub fn drag_source<R>(ui: &mut Ui, id: Id, body: impl FnOnce(&mut Ui) -> R) -> (bool, R) {
    let is_being_dragged = ui.memory(|ui| ui.is_being_dragged(id));

    if !is_being_dragged {
        let inner_response = ui.scope(body);

        // Check for drags:
        let response = ui.interact(inner_response.response.rect, id, Sense::drag());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        (false, inner_response.inner)
    } else {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        // Paint the body to a new layer:
        let layer_id = LayerId::new(Order::Tooltip, id);
        let inner_response = ui.with_layer_id(layer_id, body);

        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let delta = pointer_pos - inner_response.response.rect.center();
            ui.ctx().translate_layer(layer_id, delta);
        }
        (true, inner_response.inner)
    }
}

pub fn drop_target<R>(
    ui: &mut Ui,
    can_accept_what_is_being_dragged: bool,
    body: impl FnOnce(&mut Ui) -> R,
) -> R {
    let is_being_dragged = ui.memory(|ui| ui.is_anything_being_dragged());
    body(ui)
}
