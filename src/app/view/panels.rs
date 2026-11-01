use crate::app::settings::GraphSettings;
use crate::app::widgets::{enter_sends, palette};
use crate::map::{Cmd, Layout, NodeId, Text, Turn};
use crate::ai;
use eframe::egui::{self, Key, TextEdit};
use std::collections::HashSet;
use super::keys::toggle;
use super::menu::{node_menu, Pick};
use super::View;

/// One node of the outline, then its children when it is expanded. The arrows share `expanded` with the
/// canvas, so collapsing here hides the branch there. Clicking a name makes it current and expands it.
fn outline(ui: &mut egui::Ui, layout: &Layout, id: NodeId, expanded: &mut HashSet<NodeId>, cmds: &mut Vec<Cmd>, menu: &mut Option<(NodeId, Pick)>) {
    let Some(node) = layout.node(id) else { return };
    let children: Vec<_> = layout.children(id).collect();
    let mut name = |ui: &mut egui::Ui| {
        let response = ui.selectable_label(id == layout.current, node.text.as_str()).interact(egui::Sense::click_and_drag());
        if node.parent.is_some() {
            response.dnd_set_drag_payload(id);
        }
        if let Some(pick) = node_menu(&response, layout, id) {
            *menu = Some((id, pick));
        }
        (response.clicked(), drop_at(ui, layout, &response, id))
    };
    let (clicked, dropped) = if children.is_empty() {
        name(ui)
    } else {
        let key = ui.make_persistent_id(("outline", id));
        let open = expanded.contains(&id);
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), key, open);
        state.set_open(open);
        let (_, header, _) = state.show_header(ui, name).body(|ui| {
            for child in &children {
                outline(ui, layout, child.id, expanded, cmds, menu);
            }
        });
        if egui::collapsing_header::CollapsingState::load(ui.ctx(), key).is_some_and(|s| s.is_open()) != open {
            toggle(layout, id, expanded, cmds);
        }
        header.inner
    };
    if clicked {
        cmds.push(Cmd::Select(id));
        expanded.insert(id);
    }
    if let Some(cmd) = dropped {
        if let Cmd::Move { parent, .. } = cmd {
            expanded.insert(parent);
        }
        cmds.push(cmd);
    }
}

/// Where a node dragged over the outline entry `id` would land, drawn as a line or an outline, and the move
/// once it is released. The top or bottom quarter puts it before or after `id`; the middle puts it under `id`.
fn drop_at(ui: &egui::Ui, layout: &Layout, response: &egui::Response, id: NodeId) -> Option<Cmd> {
    let moving = *response.dnd_hover_payload::<NodeId>()?;
    let pos = ui.input(|i| i.pointer.hover_pos())?;
    let (rect, own) = (response.rect, layout.node(id)?.parent);
    let edge = rect.height() / 4.0;
    let (parent, before, line) = match own {
        Some(parent) if pos.y < rect.top() + edge => (parent, Some(id), Some(rect.top())),
        Some(parent) if pos.y > rect.bottom() - edge => {
            let next = layout.children(parent).map(|n| n.id).skip_while(|n| *n != id).nth(1);
            (parent, next, Some(rect.bottom()))
        }
        _ => (id, None, None),
    };
    let into_itself = parent == moving || layout.ancestors(parent).any(|a| a == moving);
    let onto_own_parent = line.is_none() && layout.node(moving)?.parent == Some(parent);
    if moving == id || into_itself || onto_own_parent {
        return None;
    }
    let trail = palette(ui).trail;
    match line {
        Some(y) => _ = ui.painter().hline(rect.x_range(), y, egui::Stroke::new(2.0, trail)),
        None => _ = ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(2.0, trail), egui::StrokeKind::Outside),
    };
    response.dnd_release_payload::<NodeId>().map(|_| Cmd::Move { node: moving, parent, before })
}

impl View {
    /// The outline's buttons and tree, in the side panel or, in a compact window, over the whole window.
    /// True when a name in the tree was clicked.
    pub(super) fn outline_panel(&mut self, ui: &mut egui::Ui, layout: &Layout, cmds: &mut Vec<Cmd>, menu: &mut Option<(NodeId, Pick)>) -> bool {
        let compact = crate::app::widgets::compact(ui.ctx());
        ui.horizontal(|ui| {
            let all = layout.nodes.iter().all(|n| self.expanded.contains(&n.id) || layout.children(n.id).next().is_none());
            let (icon, does) = if all { ("−", "Collapse all") } else { ("+", "Show all") };
            let button = ui.small_button(icon).on_hover_text(does);
            button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, does));
            if button.clicked() && all {
                self.expanded.clear();
                cmds.push(Cmd::Select(layout.nodes[0].id));
            } else if button.clicked() {
                self.expanded.extend(layout.nodes.iter().map(|n| n.id));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let hide = if compact { ui.button("Map »") } else { ui.small_button("«") };
                if hide.on_hover_text("Hide outline").clicked() {
                    self.hide_outline = true;
                }
            });
        });
        ui.separator();
        let before = cmds.len();
        egui::ScrollArea::vertical().show(ui, |ui| outline(ui, layout, layout.nodes[0].id, &mut self.expanded, cmds, menu));
        cmds[before..].iter().any(|c| matches!(c, Cmd::Select(_)))
    }

    pub(super) fn shell(&mut self, ui: &mut egui::Ui, layout: &Layout, settings: &mut GraphSettings, skills: &mut [ai::Skill], cmds: &mut Vec<Cmd>) {
        let mut menu = None;
        if !self.hide_outline {
            egui::Panel::left("outline").default_size(220.0).show(ui, |ui| _ = self.outline_panel(ui, layout, cmds, &mut menu));
        }
        if !self.hide_inspector {
            egui::Panel::right("inspector").default_size(340.0).show(ui, |ui| self.inspector(ui, layout, settings, skills, cmds));
        }
        // After the inspector, so Rename focuses the name box next frame, as from the canvas.
        if let Some((id, pick)) = menu {
            self.pick(ui.ctx(), layout, id, pick, cmds);
        }
    }

        pub(super) fn chat_box(&mut self, ui: &mut egui::Ui, layout: &Layout, cmds: &mut Vec<Cmd>) {
        let waiting = matches!(layout.chat.last(), Some(Turn::You(_)));
        let id = ui.make_persistent_id("discuss");
        let enter = enter_sends(ui, id);
        ui.add_space(4.0);
        let edit = TextEdit::multiline(&mut self.chat_draft).id(id).hint_text("Ask about this node").desired_rows(3).desired_width(f32::INFINITY);
        ui.add(edit);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Enter sends · Shift+Enter new line").color(palette(ui).hint).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let send = ui.add_enabled(!waiting, egui::Button::new("Send")).clicked();
                if (send || enter) && !waiting && !self.chat_draft.trim().is_empty() {
                    cmds.push(Cmd::Chat(std::mem::take(&mut self.chat_draft)));
                    ui.memory_mut(|m| m.request_focus(id));
                }
            });
        });
    }

        pub(super) fn add_box(&mut self, ui: &mut egui::Ui, current: NodeId, cmds: &mut Vec<Cmd>) {
        let Some(text) = &mut self.adding else { return };
        let response = ui.add(TextEdit::singleline(text).hint_text("New branch").desired_width(f32::INFINITY));
        // Not on the frame of the "+ Add" click, which egui treats as a click away from the box.
        if self.focus_add && !ui.input(|i| i.pointer.any_click()) {
            self.focus_add = false;
            response.request_focus();
        }
        if response.lost_focus() {
            if let Some(text) = Text::new(text).filter(|_| ui.input(|i| i.key_pressed(Key::Enter))) {
                cmds.push(Cmd::Add(text));
                self.expanded.insert(current);
            }
            self.adding = None;
        }
    }

}
