use crate::map::{Cmd, Focus, Layout, NodeId};
use eframe::egui::{self, Key, Modifiers};
use std::collections::HashSet;
use super::View;

/// Every key, grouped by task, for the footer's Keys menu and the help map.
pub(crate) const KEYS: &[(&str, &[(&str, &str)])] = &[
    ("Moving around", &[
        ("Up / Down", "Walk the visible tree and the suggestions"),
        ("Right or Enter", "Open the node one level"),
        ("Left or Enter", "Fold it again"),
        ("Esc", "Go to the parent"),
        ("F", "Fit the map"),
        ("Ctrl+scroll, drag", "Zoom, pan"),
        ("\\", "Graph only: hide both panels, again to restore"),
        ("R", "Reading mode, R or Esc leaves"),
    ]),
    ("Nodes", &[
        ("Enter on a suggestion", "Keep it"),
        ("Tab", "Add a branch by hand"),
        ("Double-click a node", "Rename it"),
        ("Right-click a node", "Add, copy, paste, rename, map out or delete"),
        ("Delete", "Remove the node and its branches, after a yes"),
        ("Ctrl+C / Ctrl+V", "Copy the node and its branches / paste under it"),
        ("Ctrl+X, then Ctrl+V", "Cut a node and its branches, then paste them elsewhere: a move"),
        ("Drag onto a node", "Move under it; in the outline, drag between two to reorder"),
        ("Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y", "Undo / redo a change to the tree"),
    ]),
    ("Asking the AI", &[
        ("M or triple-click", "More ideas, after a yes"),
        ("L", "Learn more"),
        ("E", "Examples"),
        ("G", "Map out the node: grow its whole subtree"),
        ("Enter / Shift+Enter", "In Discuss: send / new line"),
        ("Enter / Ctrl+Enter", "On the start screen: brainstorm / map out"),
    ]),
    ("Dialogs and pictures", &[
        ("Enter / Esc", "Confirm / cancel a question"),
        ("Left / Right", "Previous / next picture in the viewer"),
    ]),
    ("Files and the app", &[
        ("Ctrl+N / Ctrl+O", "New map / open a map"),
        ("Ctrl+T", "Type a path to open"),
        ("Ctrl+Tab", "Next tab"),
        ("Ctrl+,", "Settings"),
        ("F1", "The help map, fresh each time"),
        ("Ctrl+= / Ctrl+- / Ctrl+0", "Bigger, smaller or normal size"),
    ]),
];


/// Expands or collapses `id`. Collapsing a branch that holds the current node makes
/// `id` current, so the current node is never hidden.
pub(super) fn toggle(layout: &Layout, id: NodeId, expanded: &mut HashSet<NodeId>, cmds: &mut Vec<Cmd>) {
    if expanded.insert(id) {
        return;
    }
    expanded.remove(&id);
    if layout.ancestors(layout.current).any(|a| a == id) {
        cmds.push(Cmd::Select(id));
    }
}

impl View {
    /// Hotkeys outside a text box. Returns the ghost slot Enter picked.
    pub(super) fn keys(&mut self, ui: &mut egui::Ui, layout: &Layout, visible: &[NodeId], cmds: &mut Vec<Cmd>) -> Option<usize> {
        let mut picked = None;
        let pressed = |key| ui.input(|i| i.key_pressed(key));
        let (up, down, left, right, enter) =
            (pressed(Key::ArrowUp), pressed(Key::ArrowDown), pressed(Key::ArrowLeft), pressed(Key::ArrowRight), pressed(Key::Enter));
        let current = layout.current;
        let has_children = layout.children(current).next().is_some();
        if up || down {
                        let mut order: Vec<(NodeId, Option<usize>)> = Vec::new();
            for &id in visible {
                order.push((id, None));
                if id == current {
                    order.extend((0..layout.ghosts.len()).map(|i| (id, Some(i))));
                }
            }
            let at = order.iter().position(|&o| o == (current, self.ghost)).unwrap_or(0);
            let to = if down { (at + 1).min(order.len() - 1) } else { at.saturating_sub(1) };
            match order[to] {
                (_, Some(i)) => self.ghost = Some(i),
                (id, None) if id != current => cmds.push(Cmd::Select(id)),
                _ => self.ghost = None,
            }
        }
        if right && self.ghost.is_none() {
            if has_children && self.expanded.insert(current) {
            } else if let Some(child) = visible.iter().find(|&&id| layout.children(current).any(|c| c.id == id)) {
                cmds.push(Cmd::Select(*child));
            }
        }
        if left {
            if self.ghost.is_some() {
                self.ghost = None;
            } else if has_children && self.expanded.remove(&current) {
            } else {
                cmds.push(Cmd::Back);
            }
        }
        if enter {
            match self.ghost {
                Some(i) => picked = Some(i),
                None if has_children => toggle(layout, current, &mut self.expanded, cmds),
                None => {}
            }
        }
        ui.input_mut(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Copy => self.copy = Some(current),
                    // Cut copies, then deletes without asking: paste or Ctrl+Z brings it back.
                    egui::Event::Cut if layout.node(current).is_some_and(|n| n.parent.is_some()) => {
                        self.copy = Some(current);
                        cmds.push(Cmd::Delete);
                    }
                    egui::Event::Paste(text) => {
                        cmds.push(Cmd::Paste(text.clone()));
                        self.expanded.insert(current);
                    }
                    _ => {}
                }
            }
            // Ctrl+Z also matches Ctrl+Shift+Z, so this checks redo first.
            if input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Z) || input.consume_key(Modifiers::CTRL, Key::Y) {
                cmds.push(Cmd::Redo);
            }
            if input.consume_key(Modifiers::CTRL, Key::Z) {
                cmds.push(Cmd::Undo);
            }
            if input.consume_key(Modifiers::NONE, Key::Tab) {
                self.adding = Some(String::new());
                self.focus_add = true;
            }
            if input.key_pressed(Key::M) {
                cmds.push(Cmd::Reroll);
            }
            if input.key_pressed(Key::Backslash) {
                let hidden = (self.hide_outline, self.hide_inspector);
                if hidden == (true, true) {
                    (self.hide_outline, self.hide_inspector) = self.panels;
                } else {
                    self.panels = hidden;
                    (self.hide_outline, self.hide_inspector) = (true, true);
                }
            }
            if input.key_pressed(Key::R) {
                self.reading = true;
            }
            if input.key_pressed(Key::Escape) {
                cmds.push(Cmd::Back);
            }
            if input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace) {
                cmds.push(Cmd::Delete);
            }
            if input.key_pressed(Key::L) {
                cmds.push(Cmd::LearnMore(Focus::Depth));
            }
            if input.key_pressed(Key::E) {
                cmds.push(Cmd::LearnMore(Focus::Examples));
            }
            if input.key_pressed(Key::G) {
                cmds.push(Cmd::LearnMore(Focus::MapOut));
                self.expanded.insert(layout.current);
            }
            if input.key_pressed(Key::F) {
                self.camera = self.fit(layout);
                self.following = false;
            }
        });
        picked
    }

}
