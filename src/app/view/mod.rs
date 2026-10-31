use crate::app::settings::GraphSettings;
use crate::app::widgets::{label, label_font, FAN, FLASH, GHOST_LINK};
use crate::graph::{Graph, Spec};
use crate::map::{Cmd, Layout, NodeId, Slot, Text};
use crate::{ai, wiki};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Vec2};
use std::collections::HashSet;

mod canvas;
mod inspector;
mod keys;
mod menu;
mod panels;
mod start;

pub(super) use keys::KEYS;
pub(super) use start::Start;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Body {
    Node(NodeId),
    Ghost(NodeId, usize),
}

/// The closest the canvas zooms with + and Ctrl+scroll, and the closest Fit goes on its own.
const MAX_ZOOM: f32 = 4.0;
const FIT_ZOOM: f32 = 1.5;

/// UI-only state. Nothing here is saved or affects the map except through the `Cmd`s `show` returns.
#[derive(Default)]
pub(super) struct View {
    /// The name box's text. With no map yet, it is the central idea or topic.
    pub(super) draft: String,
    /// The node and text that `draft` came from. Another node, or its text changed by Undo, reloads it.
    pub(super) named: Option<(NodeId, Text)>,
    /// The message the user types in the inspector's Discuss box. Cleared when the current node changes.
    pub(super) chat_draft: String,
    /// Set by a double-click on a node, so the name box takes focus once.
    pub(super) focus_name: bool,
    /// The note the user edits, and its node. Saved when the box loses focus or the current node changes.
    pub(super) note_edit: Option<(NodeId, String)>,
    pub(super) focus_note: bool,
    pub(super) markdown: egui_commonmark::CommonMarkCache,
    pub(super) hide_outline: bool,
    pub(super) hide_inspector: bool,
    /// The panels hidden before `\` hid both, restored by the next `\`.
    pub(super) panels: (bool, bool),
    /// The inspector fills the window in a centered column, with no canvas behind it.
    pub(super) reading: bool,
    /// The new branch box's text while it is open.
    pub(super) adding: Option<String>,
    pub(super) focus_add: bool,
    /// A text box had focus at the end of the last frame.
    pub(super) typed: bool,
    /// Unchecked on the start screen: a brainstorm that never asks the AI on its own.
    pub(super) no_auto: bool,
    /// Checked on the start screen: interview first, then make the map.
    pub(super) grill: bool,
    /// The start screen's template card clicked with no topic typed yet. Enter makes the map from it.
    pub(super) template: Option<usize>,
    pub(super) focus_topic: bool,
    pub(super) show_settings: bool,
    /// A Delete or More ideas waiting for a yes, and the question the modal asks.
    pub(super) confirm: Option<(Cmd, String)>,
    /// The topic panel's pictures fill a modal.
    pub(super) viewer: bool,
    /// The node being dragged on the canvas, the node it would move under if released now, and whether Esc
    /// cancelled the move.
    pub(super) moving: Option<NodeId>,
    pub(super) drop_on: Option<NodeId>,
    pub(super) drop_cancelled: bool,
    /// The ghost slot the arrow keys point at. `None` is the current node itself.
    pub(super) ghost: Option<usize>,
    /// Nodes whose children show, on the canvas and in the outline. Everything else is folded.
    pub(super) expanded: HashSet<NodeId>,
    pub(super) flash: Option<Flash>,
    pub(super) graph: Graph<Body>,
    /// The visible part of the canvas. `None` until the first frame, which egui then fits to the map.
    pub(super) camera: Option<Rect>,
    /// Where the canvas sits in the window, so a canvas point converts to a screen point.
    pub(super) canvas: Option<Rect>,
    /// The current node the camera last started gliding to.
    pub(super) followed: Option<NodeId>,
    pub(super) following: bool,
    pub(super) lookup: wiki::Lookup,
    /// The picture showing in the topic panel's slider. Back to the first on another node.
    pub(super) slide: usize,
    /// The user clicked the Wikipedia link. The app opens it as a tab.
    pub(super) open_article: Option<wiki::Card>,
    /// The tab's picked model can search the web. Set by the tab every frame.
    pub(super) browses: bool,
    /// The node whose subtree goes on the clipboard. The tab copies it, since only the map has the notes.
    pub(super) copy: Option<NodeId>,
}

/// The keep animation: the new branch pops, the ghosts it beat fade out.
/// The popping node is the current one, since keeping a ghost makes it current.
pub(super) struct Flash {
    pub(super) start: f64,
    pub(super) faded: Vec<(Pos2, String)>,
}

impl View {
    pub(super) fn show(&mut self, ui: &mut egui::Ui, layout: &Layout, settings: &mut GraphSettings, skills: &mut [ai::Skill]) -> Vec<Cmd> {
        // egui drops focus on Esc before any widget runs, so a box focused last frame still counts.
        // The Esc that closes it must not also jump to the parent.
        // An open modal keeps the arrows, Enter and Esc for itself.
        let typing = ui.ctx().egui_wants_keyboard_input() || self.typed || self.confirm.is_some() || self.viewer;
        let now = ui.input(|i| i.time);
        if self.flash.as_ref().is_some_and(|f| now - f.start >= FLASH) {
            self.flash = None;
        }
        if self.reading {
            if !typing && ui.input(|i| i.key_pressed(Key::R) || i.key_pressed(Key::Escape)) {
                self.reading = false;
            }
            let full = ui.available_rect_before_wrap();
            ui.painter().rect_filled(full, 0.0, ui.visuals().panel_fill);
            let inner = full.shrink(16.0);
            let column = Rect::from_center_size(inner.center(), Vec2::new(inner.width().min(680.0), inner.height()));
            let mut cmds = Vec::new();
            self.inspector(&mut ui.new_child(egui::UiBuilder::new().max_rect(column)), layout, settings, skills, &mut cmds);
            self.confirm(ui.ctx(), layout, &mut cmds);
            self.typed = ui.ctx().egui_wants_keyboard_input();
            return cmds;
        }
        // The current node is always in view: every node above it stays expanded.
        self.expanded.extend(layout.ancestors(layout.current));
        let visible = self.visible(layout);
        self.graph.set_forces(settings.forces);
        self.sync(ui, layout, &visible, settings);
        self.graph.step();
        if self.followed != Some(layout.current) {
            self.followed = Some(layout.current);
            self.following = true;
        }
        // Glide: ease the camera a fifth of the way to the current node each frame.
        if let (Some(camera), Some(focus)) = (&mut self.camera, self.graph.pos(&Body::Node(layout.current)))
            && self.following
        {
            let gap = focus - camera.center();
            *camera = camera.translate(if gap.length() < 1.0 { gap } else { gap * 0.2 });
            self.following = gap.length() >= 1.0 || !self.graph.settled();
        }

        let mut cmds = Vec::new();
        self.shell(ui, layout, settings, skills, &mut cmds);
        self.canvas = Some(ui.available_rect_before_wrap());
        if self.camera.is_none() {
            self.camera = self.fit(layout);
        }
        let mut camera = self.camera.unwrap_or(Rect::ZERO);
        let scene =
            egui::Scene::new().zoom_range(0.1..=MAX_ZOOM).show(ui, &mut camera, |ui| self.draw(ui, layout, &visible, settings, now, &mut cmds));
        // Dragging or zooming hands the camera to the user until the current node changes.
        if scene.response.changed() {
            self.following = false;
        }
        self.camera = Some(camera);
        self.canvas_tools(ui, layout, settings);
        if self.hide_inspector && self.adding.is_some()
            && let Some(canvas) = self.canvas
        {
            let area = egui::Area::new(ui.scope_id().with("add")).order(egui::Order::Foreground).pivot(egui::Align2::CENTER_TOP);
            area.fixed_pos(canvas.center_top() + Vec2::new(0.0, 8.0)).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(260.0);
                    self.add_box(ui, layout.current, &mut cmds);
                })
            });
        }
        let mut picked = scene.inner;
        let ghosts = &layout.ghosts;
        if !typing && let Some(i) = self.keys(ui, layout, &visible, &mut cmds) {
            picked = Some(i);
        }

        if let Some(i) = picked
            && let Some(cmd) = ghosts.get(i).and_then(Slot::pick)
        {
            if matches!(cmd, Cmd::Keep(_)) {
                let faded = ghosts
                    .iter()
                    .enumerate()
                    .filter(|&(j, _)| j != i)
                    .filter_map(|(j, slot)| match slot {
                        Slot::Ghost(_, text) => Some((self.graph.pos(&Body::Ghost(layout.current, j))?, text.as_str().to_owned())),
                        _ => None,
                    })
                    .collect();
                self.flash = Some(Flash { start: now, faded });
            }
            cmds.push(cmd);
        }

        if !cmds.is_empty() || self.ghost.is_some_and(|i| i >= ghosts.len()) {
            self.ghost = None;
        }
        self.confirm(ui.ctx(), layout, &mut cmds);
        self.typed = ui.ctx().egui_wants_keyboard_input();
        cmds
    }

    /// The nodes on the canvas, in outline order: the root, and the children of every expanded visible node.
    pub(super) fn visible(&self, layout: &Layout) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![layout.nodes[0].id];
        while let Some(id) = stack.pop() {
            out.push(id);
            if self.expanded.contains(&id) {
                let kids: Vec<_> = layout.children(id).map(|n| n.id).collect();
                stack.extend(kids.into_iter().rev());
            }
        }
        out
    }

    /// Tells the force layout what exists this frame. Ghosts spawn on an arc facing away from the
    /// current node's parent, so new suggestions spread into empty space instead of crossing the trail.
    pub(super) fn sync(&mut self, ui: &egui::Ui, layout: &Layout, visible: &[NodeId], settings: &GraphSettings) {
        let current = Body::Node(layout.current);
        let size = |text: &str, depth: usize, ghost: bool| label(ui.painter(), text, label_font(depth, settings.text), ghost, Color32::WHITE).size() + Vec2::new(20.0, 10.0);
        let below = layout.ancestors(layout.current).count() + 1;
        let mut specs: Vec<Spec<Body>> = layout
            .nodes
            .iter()
            .filter(|n| visible.contains(&n.id))
            .map(|n| Spec {
                key: Body::Node(n.id),
                link: n.parent.map(|p| (Body::Node(p), settings.distance)),
                size: size(n.text.as_str(), layout.ancestors(n.id).count(), false),
            })
            .collect();
        specs.extend(layout.ghosts.iter().enumerate().map(|(i, slot)| {
            let text = match slot {
                Slot::Ghost(_, text) => text.as_str(),
                _ => "more…",
            };
            Spec { key: Body::Ghost(layout.current, i), link: Some((current, GHOST_LINK)), size: size(text, below, true) }
        }));
        let parent = layout.node(layout.current).and_then(|n| n.parent);
        let away = match (parent.and_then(|p| self.graph.pos(&Body::Node(p))), self.graph.pos(&current)) {
            (Some(from), Some(to)) if from != to => Some((to - from).angle()),
            _ => None,
        };
        let count = layout.ghosts.len() as f32;
        self.graph.sync(&specs, |key, anchor| {
            let Body::Ghost(_, i) = key else { return None };
            let i = *i as f32;
            let angle = match away {
                Some(away) => away + (i - (count - 1.0) / 2.0) * FAN / (count - 1.0).max(1.0),
                None => i * std::f32::consts::TAU / count,
            };
            Some(anchor + Vec2::angled(angle) * GHOST_LINK)
        });
    }

    /// The whole map in view, never closer than `FIT_ZOOM`, so a small map does not open huge and + can still zoom in.
    pub(super) fn fit(&self, layout: &Layout) -> Option<Rect> {
        let nodes = layout.nodes.iter().map(|n| Body::Node(n.id));
        let ghosts = (0..layout.ghosts.len()).map(|i| Body::Ghost(layout.current, i));
        let points: Vec<Pos2> = nodes.chain(ghosts).filter_map(|b| self.graph.pos(&b)).collect();
        let map = Rect::from_points(&points).expand(GHOST_LINK);
        let least = self.canvas.map_or(Vec2::ZERO, |c| c.size() / FIT_ZOOM);
        Some(Rect::from_center_size(map.center(), map.size().max(least)))
    }

    /// The screen point showing canvas point `pos`, the way `egui::Scene` fits the camera into the canvas.
    #[cfg(test)]
    pub(super) fn to_screen(&self, pos: Pos2) -> Pos2 {
        let (camera, canvas) = (self.camera.unwrap(), self.canvas.unwrap());
        let scale = (canvas.size() / camera.size()).min_elem().clamp(0.1, MAX_ZOOM);
        canvas.center() + (pos - camera.center()) * scale
    }

    /// True while the view needs frames without input: the keep flash, the camera glide or the layout moving.
    pub(super) fn animating(&self) -> bool {
        self.flash.is_some() || self.following || !self.graph.settled()
    }
}
