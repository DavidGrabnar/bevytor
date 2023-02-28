use crate::tree::Action::NoAction;
use bevy::prelude::{CursorIcon, Entity};
use bevy_egui::egui;
use bevy_egui::egui::collapsing_header::CollapsingState;
use bevy_egui::egui::emath::RectTransform;
use bevy_egui::egui::Shape::Rect;
use bevy_egui::egui::{
    emath, epaint, CollapsingHeader, Id, InnerResponse, LayerId, Order, Response, Sense, Shape, Ui,
    Vec2,
};
use std::fmt::{Debug, Formatter, Write};

pub enum Action {
    NoAction,
    Selected(Entity),
    DragDrop(Entity, HoverEntity),
}

#[derive(Debug, Clone)]
pub enum HoverEntity {
    Root,
    Node(Entity),
}

#[derive(Default)]
pub struct Context {
    drag_entity: Option<Entity>,
    hover_entity: Option<HoverEntity>,
}

#[derive(Default)]
pub struct Tree(Node);

impl Tree {
    pub fn new(root: Node) -> Self {
        Self(root)
    }

    pub fn ui(&self, ui: &mut Ui) -> Action {
        let mut context = Context::default();
        let action = self.0.ui(ui, &mut context);

        if let Some(dragged) = context.drag_entity {
            if let Some(hovered) = context.hover_entity {
                if ui.input().pointer.any_released() {
                    if let NoAction = action {
                        if let HoverEntity::Node(entity) = hovered {
                            if entity != dragged {
                                // only return if not equal
                                return Action::DragDrop(dragged, hovered);
                            }
                        } else {
                            return Action::DragDrop(dragged, hovered);
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

    fn ui(&self, ui: &mut Ui, context: &mut Context) -> Action {
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
                    if let Some(response) = response {
                        if response.clicked() {
                            return Action::Selected(self.0.as_ref().unwrap().0);
                        }
                    }
                    Action::NoAction
                })
            });
            let is_being_dragged = ui.memory().is_anything_being_dragged();
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
                        self.1.iter().fold(Action::NoAction, |curr_action, child| {
                            let action = child.ui(ui, context);
                            if let Action::Selected(_entity) = action {
                                return action;
                            }
                            curr_action
                        })
                    })
                });
            if let Some(response) = body_response {
                if let Action::Selected(_entity) = response.inner {
                    return response.inner;
                }
            }
            if header_response.inner.0 {
                // root cannot be dragged, None can be ignored
                if let Some(entity) = &self.0 {
                    context.drag_entity = Some(entity.0);
                }
            }
            if let Some(response) = header_response.inner.1 {
                if response.clicked() {
                    if let Some(entity) = self.0.as_ref() {
                        return Action::Selected(entity.0);
                    }
                }
            }
            let is_being_dragged = ui.memory().is_anything_being_dragged();
            if is_being_dragged && can_accept_what_is_being_dragged && response.hovered() {
                let hover_entity = self
                    .0
                    .as_ref()
                    .map(|e| HoverEntity::Node(e.0.clone()))
                    .unwrap_or(HoverEntity::Root);
                context.hover_entity = Some(hover_entity.clone());
            }
        }
        Action::NoAction
    }

    fn ui_element(&self, name: &str, id_source: String, ui: &mut Ui) -> (bool, Option<Response>) {
        let (dragged, _) = drag_source(ui, Id::new(id_source), |ui| {
            ui.label(name);
        });
        (
            dragged,
            match self.0.as_ref() {
                Some(_entity) => Some(ui.button("👁")),
                None => None,
            },
        )
    }
}

pub fn drag_source<R>(ui: &mut Ui, id: Id, body: impl FnOnce(&mut Ui) -> R) -> (bool, R) {
    let is_being_dragged = ui.memory().is_being_dragged(id);

    if !is_being_dragged {
        let inner_response = ui.scope(body);

        // Check for drags:
        let response = ui.interact(inner_response.response.rect, id, Sense::drag());
        if response.hovered() {
            ui.ctx().output().cursor_icon = egui::CursorIcon::Grab;
        }
        (false, inner_response.inner)
    } else {
        ui.ctx().output().cursor_icon = egui::CursorIcon::Grabbing;

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
    let is_being_dragged = ui.memory().is_anything_being_dragged();

    let response = body(ui);
    response
}
