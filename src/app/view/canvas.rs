use crate::app::settings::GraphSettings;
use crate::app::widgets::{self, label, label_font, palette, FLASH};
use crate::map::{Cmd, Layout, NodeId, Slot};
use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};
use super::menu::node_menu;
use super::{Body, View};

impl View {
    pub(super) fn canvas_tools(&mut self, ui: &egui::Ui, layout: &Layout, settings: &mut GraphSettings) {
        let Some(canvas) = self.canvas else { return };
        let corner = |name: &str, pos: Pos2, pivot| {
            egui::Area::new(ui.scope_id().with(name)).order(egui::Order::Foreground).fixed_pos(pos).pivot(pivot)
        };
        let margin = Vec2::splat(8.0);
        corner("zoom", canvas.right_bottom() - margin, egui::Align2::RIGHT_BOTTOM).show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                let zoom = |camera: &mut Option<Rect>, by: f32| *camera = camera.map(|c| Rect::from_center_size(c.center(), c.size() * by));
                if ui.button("+").on_hover_text("Zoom in (Ctrl+scroll)").clicked() {
                    zoom(&mut self.camera, 0.8);
                }
                if ui.button("-").on_hover_text("Zoom out (Ctrl+scroll)").clicked() {
                    zoom(&mut self.camera, 1.25);
                }
                if ui.button("Fit").on_hover_text("F").clicked() {
                    self.camera = self.fit(layout);
                    self.following = false;
                }
                if ui.button("Center").on_hover_text("Glide to the current node. Drag the background to pan.").clicked() {
                    self.following = true;
                }
                ui.toggle_value(&mut self.show_settings, "Graph").on_hover_text("Display and forces");
            });
        });
        if self.show_settings {
            let above = canvas.right_bottom() - Vec2::new(8.0, 44.0);
            corner("graph-settings", above, egui::Align2::RIGHT_BOTTOM).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(260.0);
                    ui.horizontal(|ui| {
                        ui.strong("Graph");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").on_hover_text("Close").clicked() {
                                self.show_settings = false;
                            }
                            if ui.small_button("Reset").on_hover_text("Back to the defaults").clicked() {
                                settings.reset_graph();
                            }
                        });
                    });
                    ui.separator();
                    settings.graph_controls(ui);
                    ui.add_space(6.0);
                    if ui.button("Rearrange").on_hover_text("Lay the graph out again from scratch").clicked() {
                        self.graph.rearrange();
                    }
                });
            });
        }
        // A fresh map shows how to grow it, until its first branch.
        if layout.nodes.len() == 1 {
            let hint = "Click a suggestion to keep it · Tab adds your own · M for more";
            corner("empty-hint", canvas.center_top() + Vec2::new(0.0, 12.0), egui::Align2::CENTER_TOP).show(ui.ctx(), |ui| ui.label(widgets::hint(ui, hint)));
        }
        if self.hide_outline {
            corner("show-outline", canvas.left_top() + margin, egui::Align2::LEFT_TOP).show(ui.ctx(), |ui| {
                self.hide_outline = !ui.button("» Outline").clicked();
            });
        }
        if self.hide_inspector {
            corner("show-inspector", canvas.right_top() + Vec2::new(-8.0, 8.0), egui::Align2::RIGHT_TOP).show(ui.ctx(), |ui| {
                self.hide_inspector = !ui.button("Topic «").clicked();
            });
        }
    }

    /// Draws nodes, trails and ghosts in canvas coordinates. Returns the ghost slot clicked this frame.
    pub(super) fn draw(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        visible: &[NodeId],
        settings: &GraphSettings,
        now: f64,
        cmds: &mut Vec<Cmd>,
    ) -> Option<usize> {
        let flash_t = self.flash.as_ref().map(|f| ((now - f.start) / FLASH) as f32);
        let painter = ui.painter().clone();
        let p = palette(ui);
        let ghost_font = label_font(layout.ancestors(layout.current).count() + 1, settings.text);
        let at = |body: Body| self.graph.pos(&body).unwrap_or_default();
        let focus = at(Body::Node(layout.current));
        let shown: Vec<_> = layout.nodes.iter().filter(|n| visible.contains(&n.id)).collect();
        let lit: Vec<NodeId> = layout.ancestors(layout.current).chain([layout.current]).collect();
        for node in &shown {
            if let Some(parent) = node.parent {
                let color = if lit.contains(&node.id) { p.trail } else { p.branch_link };
                painter.line_segment([at(Body::Node(parent)), at(Body::Node(node.id))], Stroke::new(2.0 * settings.links, color));
            }
        }
        for i in 0..layout.ghosts.len() {
            painter.line_segment([focus, at(Body::Ghost(layout.current, i))], Stroke::new(settings.links, p.ghost));
        }
        let (mut dragged, mut released) = (None, false);
        let (mut opened, mut menu) = (None, None);
        let mut boxes = Vec::new();
        for node in &shown {
            let body = Body::Node(node.id);
            let is_current = node.id == layout.current;
            let (fill, text) = if lit.contains(&node.id) { (p.node, p.node_text) } else { (p.branch, p.branch_text) };
            let galley = label(&painter, node.text.as_str(), label_font(layout.ancestors(node.id).count(), settings.text), false, text);
            let scale = match flash_t {
                Some(t) if is_current => 1.0 + 0.3 * (1.0 - t),
                _ => 1.0,
            };
            let rect = Rect::from_center_size(at(body), (galley.size() + Vec2::new(20.0, 10.0)) * scale);
            boxes.push((node.id, rect));
            painter.rect_filled(rect, 6.0, fill);
            if is_current {
                painter.rect_stroke(rect, 6.0, Stroke::new(2.0, p.trail), egui::StrokeKind::Outside);
            }
            painter.galley(rect.center() - galley.size() / 2.0, galley, text);
            // Without FOCUSABLE, so Tab never moves focus onto a node and blocks the hotkeys.
            let response = ui.interact(rect, ui.scope_id().with(("node", node.id)), Sense::CLICK | Sense::DRAG);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("node: {}", node.text.as_str())));
            if let Some(pick) = node_menu(&response, layout, node.id) {
                menu = Some((node.id, pick));
            }
            if response.clicked() {
                // The node is already under the pointer. Gliding to it would move it away before a second click.
                self.followed = Some(node.id);
            }
            if response.triple_clicked() {
                // The third press already took focus from the name box the double-click opened.
                cmds.extend([Cmd::Select(node.id), Cmd::Reroll]);
            } else if response.double_clicked() {
                cmds.push(Cmd::Select(node.id));
                self.focus_name = true;
                self.hide_inspector = false;
            } else if response.clicked() {
                cmds.push(Cmd::Select(node.id));
                opened = Some(node.id);
            } else if response.dragged() {
                dragged = Some((node.id, at(body) + response.drag_delta()));
            } else if response.drag_stopped() {
                released = true;
            }
        }

        self.expanded.extend(opened);
        let mut picked = None;
        for (i, slot) in layout.ghosts.iter().enumerate() {
            let pos = at(Body::Ghost(layout.current, i));
            let (name, color) = match slot {
                Slot::Ghost(_, text) => (text.as_str().to_owned(), p.ghost_text),
                Slot::More => ("more…".to_owned(), p.ghost_text),
                Slot::Retry(e) => (format!("retry: {e}"), p.fail),
                Slot::Shimmer => (String::new(), p.ghost_text),
            };
            let galley = label(&painter, &name, ghost_font.clone(), true, color);
            let size = if name.is_empty() { Vec2::new(120.0, 26.0) } else { galley.size() + Vec2::new(20.0, 10.0) };
            let rect = Rect::from_center_size(pos, size);
            let selected = self.ghost == Some(i);
            let fill = match slot {
                Slot::Shimmer => p.ghost.gamma_multiply(0.6 + 0.4 * (4.0 * now as f32 + i as f32).sin()),
                _ if selected => p.ghost_selected,
                _ => p.ghost,
            };
            painter.rect_filled(rect, 13.0, fill);
            if selected && !matches!(slot, Slot::Shimmer) {
                painter.rect_stroke(rect, 13.0, Stroke::new(1.5, p.ghost_text), egui::StrokeKind::Outside);
            }
            painter.galley(rect.center() - galley.size() / 2.0, galley, color);
            let response = ui.interact(rect, ui.scope_id().with(("ghost", i)), Sense::CLICK);
            let spoken = if name.is_empty() { "loading suggestion".to_owned() } else { format!("suggestion: {name}") };
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &spoken));
            if response.clicked() {
                picked = Some(i);
            }
        }

        if let (Some(flash), Some(t)) = (&self.flash, flash_t) {
            for (pos, label) in &flash.faded {
                let color = p.ghost_text.gamma_multiply(1.0 - t.min(1.0));
                painter.text(*pos, egui::Align2::CENTER_CENTER, label, ghost_font.clone(), color);
            }
        }
        self.drop_target(ui, layout, &boxes, dragged, released, cmds);
        if let Some((id, pos)) = dragged {
            self.graph.drag(Body::Node(id), pos);
        }
        if released {
            self.graph.release();
        }
        if let Some((id, pick)) = menu {
            self.pick(ui.ctx(), layout, id, pick, cmds);
        }
        picked
    }


    /// While a node is dragged over another it may move under, that node is outlined and named. Releasing there
    /// moves it. Esc during the drag cancels the move, and the drag only repositions.
    fn drop_target(&mut self, ui: &mut egui::Ui, layout: &Layout, boxes: &[(NodeId, Rect)], dragged: Option<(NodeId, Pos2)>, released: bool, cmds: &mut Vec<Cmd>) {
        if let Some((node, pos)) = dragged {
            if self.moving != Some(node) {
                (self.moving, self.drop_cancelled) = (Some(node), false);
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.drop_cancelled = true;
            }
            let parent = layout.node(node).and_then(|n| n.parent);
            let fits = |id: NodeId| parent.is_some() && id != node && Some(id) != parent && !layout.ancestors(id).any(|a| a == node);
            self.drop_on = boxes.iter().find(|(id, rect)| rect.contains(pos) && fits(*id)).map(|(id, _)| *id).filter(|_| !self.drop_cancelled);
            let target = self.drop_on.and_then(|t| Some((boxes.iter().find(|(id, _)| *id == t)?.1, layout.node(t)?)));
            if let Some((rect, target)) = target {
                let p = palette(ui);
                ui.painter().rect_stroke(rect, 6.0, Stroke::new(3.0, p.trail), egui::StrokeKind::Outside);
                let font = egui::TextStyle::Small.resolve(ui.style());
                ui.painter().text(rect.center_top() - Vec2::new(0.0, 6.0), egui::Align2::CENTER_BOTTOM, format!("Move under {}", target.text.as_str()), font, p.trail);
            }
        }
        if released {
            if let (Some(node), Some(parent)) = (self.moving, self.drop_on) {
                cmds.push(Cmd::Move { node, parent, before: None });
                self.expanded.insert(parent);
            }
            (self.moving, self.drop_on) = (None, None);
        }
    }
}
