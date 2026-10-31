use crate::map::{Cmd, Focus, Layout, NodeId, Slot};
use eframe::egui::{self, Key};
use super::View;

#[derive(Clone, Copy)]
pub(super) enum Pick {
    Child,
    Sibling,
    Copy,
    Paste,
    Rename,
    MapOut,
    Delete,
}

pub(super) fn node_menu(response: &egui::Response, layout: &Layout, id: NodeId) -> Option<Pick> {
    let mut picked = None;
    response.context_menu(|ui| {
        let root = layout.node(id).is_some_and(|n| n.parent.is_none());
        let items = [
            (Pick::Child, "Add child", true),
            (Pick::Sibling, "Add sibling", !root),
            (Pick::Copy, "Copy  Ctrl+C", true),
            (Pick::Paste, "Paste  Ctrl+V", true),
            (Pick::Rename, "Rename", true),
            (Pick::MapOut, "Map out  G", true),
            (Pick::Delete, "Delete  Del", !root),
        ];
        for (pick, label, shown) in items {
            if shown && ui.button(label).clicked() {
                picked = Some(pick);
            }
        }
    });
    picked
}

impl View {
    /// Does what the node menu picked for `id`, which first becomes current, or for Add sibling its parent.
    pub(super) fn pick(&mut self, ctx: &egui::Context, layout: &Layout, id: NodeId, pick: Pick, cmds: &mut Vec<Cmd>) {
        match pick {
            Pick::Copy => return self.copy = Some(id),
            Pick::Sibling => cmds.extend(layout.node(id).and_then(|n| n.parent).map(Cmd::Select)),
            _ => cmds.push(Cmd::Select(id)),
        }
        match pick {
            Pick::Child | Pick::Sibling => (self.adding, self.focus_add) = (Some(String::new()), true),
            Pick::Paste => {
                self.expanded.insert(id);
                // eframe reads the clipboard and returns it as `Event::Paste`, which `keys` turns into `Cmd::Paste`.
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            Pick::Rename => (self.focus_name, self.hide_inspector) = (true, false),
            Pick::MapOut => {
                cmds.push(Cmd::LearnMore(Focus::MapOut));
                self.expanded.insert(id);
            }
            Pick::Delete => cmds.push(Cmd::Delete),
            Pick::Copy => {}
        }
    }

    /// Delete and More ideas wait for a yes in a modal. More ideas asks only when it would replace suggestions
    /// on screen, and Delete only when it applies, so a retry or a no-op never asks.
    pub(super) fn confirm(&mut self, ctx: &egui::Context, layout: &Layout, cmds: &mut Vec<Cmd>) {
        if self.confirm.is_none() {
            // The node menu and a triple-click select their node first.
            let mut target = layout.current;
            let at = cmds.iter().position(|cmd| match cmd {
                Cmd::Select(id) => {
                    target = *id;
                    false
                }
                // A cut is on the clipboard, so it needs no yes.
                Cmd::Delete => layout.node(target).is_some_and(|n| n.parent.is_some()) && self.copy != Some(target),
                Cmd::Reroll => target != layout.current || layout.ghosts.iter().any(|s| matches!(s, Slot::Ghost(..))),
                _ => false,
            });
            if let Some(at) = at {
                let name = layout.node(target).map_or("", |n| n.text.as_str());
                let question = match cmds[at] {
                    Cmd::Delete => match layout.nodes.iter().filter(|n| layout.ancestors(n.id).any(|a| a == target)).count() {
                        0 => format!("Delete \"{name}\"?"),
                        1 => format!("Delete \"{name}\" and its 1 branch?"),
                        n => format!("Delete \"{name}\" and its {n} branches?"),
                    },
                    _ => format!("Replace the suggestions for \"{name}\" with new ones?"),
                };
                self.confirm = Some((cmds.remove(at), question));
            }
        }
        let Some((cmd, question)) = &self.confirm else { return };
        let yes = if matches!(cmd, Cmd::Delete) { "Delete" } else { "More ideas" };
        let modal = egui::Modal::new(egui::Id::unique("confirm")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.label(question.as_str());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let yes = ui.button(yes).on_hover_text("Enter").clicked() || ui.input(|i| i.key_pressed(Key::Enter));
                (yes, ui.button("Cancel").on_hover_text("Esc").clicked())
            })
            .inner
        });
        let (yes, no) = modal.inner;
        if yes || no || modal.should_close() {
            let (cmd, _) = self.confirm.take().unwrap();
            if yes {
                cmds.push(cmd);
            }
        }
    }
}
