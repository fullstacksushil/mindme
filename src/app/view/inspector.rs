use crate::app::settings::GraphSettings;
use crate::app::widgets::{heading, markdown, note_style, palette, waiting, NO_WEB};
use crate::map::{Cmd, Focus, Layout, Note, Text, Turn, Wait};
use crate::{ai, wiki};
use eframe::egui::{self, Key, TextEdit};
use super::View;

impl View {
    fn sources(&mut self, ui: &mut egui::Ui, note: &str) {
        let sources = self.lookup.sources(note, ui.ctx());
        if sources.total == 0 {
            let hint = "The note cites no link. Learn more and Examples ask the AI for sources.";
            ui.label(egui::RichText::new("From the model's own knowledge, no sources cited").color(palette(ui).hint).small()).on_hover_text(hint);
            return;
        }
        ui.label(egui::RichText::new(sources.line()).color(palette(ui).hint).small()).on_hover_text("A reachable link does not prove the page supports the note.");
        for (url, why) in sources.dead {
            ui.label(egui::RichText::new(format!("Dead: {url} ({why})")).color(palette(ui).fail).small());
        }
    }

    /// The pictures in a modal 80% of the window, with previous and next. Esc or a click outside closes it.
    fn viewer(&mut self, ctx: &egui::Context, pictures: &[String], card: &Result<wiki::Card, &str>) {
        let size = ctx.content_rect().size() * 0.8;
        let modal = egui::Modal::new(egui::Id::unique("pictures")).show(ctx, |ui| {
            let at = self.slide % pictures.len();
            let image = egui::Image::from_uri(&pictures[at]).max_size(size - egui::Vec2::new(0.0, 40.0));
            ui.add_sized(size - egui::Vec2::new(0.0, 40.0), image);
            ui.horizontal(|ui| {
                let (left, right) = ui.input(|i| (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight)));
                if ui.button("‹ Previous").on_hover_text("Left arrow").clicked() || left {
                    self.slide = (at + pictures.len() - 1) % pictures.len();
                }
                ui.label(egui::RichText::new(format!("{} / {}", at + 1, pictures.len())).color(palette(ui).hint));
                if ui.button("Next ›").on_hover_text("Right arrow").clicked() || right {
                    self.slide = (at + 1) % pictures.len();
                }
                if let Ok(card) = card
                    && ui.link(format!("Read \"{}\"", card.title)).on_hover_text("Opens the article in a new tab").clicked()
                {
                    (self.open_article, self.viewer) = (Some(card.clone()), false);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.button("Close").on_hover_text("Esc").clicked()).inner
            })
            .inner
        });
        if modal.inner || modal.should_close() {
            self.viewer = false;
        }
    }

    pub(super) fn inspector(&mut self, ui: &mut egui::Ui, layout: &Layout, settings: &mut GraphSettings, skills: &mut [ai::Skill], cmds: &mut Vec<Cmd>) {
        let Some(node) = layout.node(layout.current) else { return };
        if self.named.as_ref() != Some(&(node.id, node.text.clone())) {
            self.named = Some((node.id, node.text.clone()));
            self.draft = node.text.as_str().to_owned();
            self.chat_draft.clear();
            self.slide = 0;
        }
        if let Some((id, note)) = self.note_edit.take_if(|(id, _)| *id != node.id) {
            cmds.push(Cmd::SetNote(id, note));
        }
        let ancestors: Vec<_> = layout.ancestors(node.id).filter_map(|id| layout.node(id)).collect();
        let path: Vec<&str> = ancestors.iter().rev().map(|n| n.text.as_str()).chain([node.text.as_str()]).collect();
        let query = wiki::query(&path);
        let note = match layout.note {
            Some(Note::Text(text)) => Some(text),
            _ => None,
        };
        let wiki::Topic { card, pictures } =
            if settings.wiki { self.lookup.topic(&path, note, self.slide, ui.ctx()) } else { wiki::Topic { card: Err(""), pictures: Vec::new() } };
        ui.horizontal(|ui| {
            heading(ui, "Topic");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.reading && ui.small_button("»").on_hover_text("Hide the topic panel").clicked() {
                    self.hide_inspector = true;
                }
                ui.toggle_value(&mut self.reading, "Read").on_hover_text("Reading mode (R)");
                ui.hyperlink_to("YouTube", wiki::youtube_url(&query)).on_hover_text("Search YouTube in the browser");
                match &card {
                    Ok(card) => {
                        if ui.link("Wikipedia").on_hover_text(format!("Read \"{}\" in a new tab", card.title)).clicked() {
                            self.open_article = Some(card.clone());
                        }
                    }
                    Err(_) => _ = ui.hyperlink_to("Wikipedia", wiki::wikipedia_url(&query)).on_hover_text("Search Wikipedia in the browser"),
                }
            });
        });
        ui.separator();
        egui::Panel::bottom("discuss box").show(ui, |ui| self.chat_box(ui, layout, cmds));
        egui::ScrollArea::vertical().show(ui, |ui| {
            // A wrapped code block can round a fraction of a pixel past its width on a scaled screen.
            // That widened the panel a little every frame. The spare pixel takes the overflow.
            ui.set_max_width(ui.available_width() - 1.0);
            if !pictures.is_empty() {
                let at = self.slide % pictures.len();
                let width = ui.available_width();
                // A fixed frame, so pictures of other shapes do not move the controls under them.
                let image = egui::Image::from_uri(&pictures[at]).max_width(width).max_height(200.0).sense(egui::Sense::click());
                if ui.add_sized([width, 200.0], image).on_hover_text("View larger").clicked() {
                    self.viewer = true;
                }
                if self.viewer {
                    self.viewer(ui.ctx(), &pictures, &card);
                }
                if pictures.len() > 1 {
                    ui.horizontal(|ui| {
                        if ui.button("‹").on_hover_text("Previous picture").clicked() {
                            self.slide = (at + pictures.len() - 1) % pictures.len();
                        }
                        ui.label(egui::RichText::new(format!("{} / {}", at + 1, pictures.len())).color(palette(ui).hint).small());
                        if ui.button("›").on_hover_text("Next picture").clicked() {
                            self.slide = (at + 1) % pictures.len();
                        }
                    });
                }
            }
            if !ancestors.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for n in ancestors.iter().rev() {
                        if ui.link(egui::RichText::new(n.text.as_str()).text_style(egui::TextStyle::Button)).clicked() {
                            cmds.push(Cmd::Select(n.id));
                        }
                        ui.label(egui::RichText::new("›").color(palette(ui).hint));
                    }
                });
            }
            let name = egui::TextEdit::singleline(&mut self.draft).hint_text("Name").font(egui::TextStyle::Heading).desired_width(f32::INFINITY);
            let response = ui.add(name);
            if std::mem::take(&mut self.focus_name) {
                response.request_focus();
            }
            if response.lost_focus() {
                match Text::new(&self.draft) {
                    Some(text) if ui.input(|i| i.key_pressed(Key::Enter)) => cmds.push(Cmd::Rename(node.id, text)),
                    _ => self.draft = node.text.as_str().to_owned(),
                }
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                heading(ui, "Note");
                if self.note_edit.is_some() {
                    if ui.small_button("Done").clicked()
                        && let Some((id, note)) = self.note_edit.take()
                    {
                        cmds.push(Cmd::SetNote(id, note));
                    }
                } else if ui.small_button("Edit").on_hover_text("Markdown").clicked() {
                    let note = match layout.note {
                        Some(Note::Text(text)) => text.to_owned(),
                        _ => String::new(),
                    };
                    self.note_edit = Some((node.id, note));
                    self.focus_note = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut auto = layout.auto;
                    let hint = "Ask the AI whenever you open a node. Off, only More ideas asks.";
                    if ui.checkbox(&mut auto, "Auto-suggest").on_hover_text(hint).changed() {
                        cmds.push(Cmd::SetAuto(auto));
                    }
                    ui.menu_button("Aa", |ui| settings.note_controls(ui, skills))
                    .response
                    .on_hover_text("Note text");
                });
            });
            ui.scope(|ui| {
                note_style(ui, settings);
                if let Some((id, note)) = &mut self.note_edit {
                    let edit = TextEdit::multiline(note).desired_width(f32::INFINITY).desired_rows(10).hint_text("Markdown");
                    let response = ui.add(edit);
                    if std::mem::take(&mut self.focus_note) {
                        response.request_focus();
                    }
                    if response.lost_focus() {
                        let (id, note) = (*id, std::mem::take(note));
                        self.note_edit = None;
                        if !ui.input(|i| i.key_pressed(Key::Escape)) {
                            cmds.push(Cmd::SetNote(id, note));
                        }
                    }
                    ui.label(egui::RichText::new("Click away to save · Esc cancels").color(palette(ui).hint).small());
                } else {
                    match &layout.note {
                        Some(Note::Text(text)) => {
                            markdown(ui, &mut self.markdown, text);
                            self.sources(ui, text);
                        }
                        Some(Note::Loading) => {
                            if waiting(ui, (node.id, "note"), "thinking…") {
                                cmds.push(Cmd::Stop(Wait::Ghosts));
                            }
                        }
                        None => {
                            ui.label(egui::RichText::new("No note yet.").color(palette(ui).hint));
                        }
                    }
                    match layout.learning {
                        Some(None) => {
                            if waiting(ui, (node.id, "learn"), "learning more…") {
                                cmds.push(Cmd::Stop(Wait::Learn));
                            }
                        }
                        Some(Some(e)) => {
                            ui.label(egui::RichText::new(format!("Learn more failed: {e}")).color(palette(ui).fail));
                        }
                        None => {}
                    }
                }
            });
            if self.note_edit.is_none() {
                ui.horizontal(|ui| {
                    let idle = layout.learning != Some(None);
                    let hint = "Ask the AI to explain this in depth, with sources, added under the note (L)";
                    if ui.add_enabled(idle, egui::Button::new("Learn more").small()).on_hover_text(hint).clicked() {
                        cmds.push(Cmd::LearnMore(Focus::Depth));
                    }
                    let hint = "Ask the AI for 2 to 3 examples, with sources, added under the note (E)";
                    if ui.add_enabled(idle, egui::Button::new("Examples").small()).on_hover_text(hint).clicked() {
                        cmds.push(Cmd::LearnMore(Focus::Examples));
                    }
                    let hint = "Ask the AI to map out this node, 3 levels deep with sourced notes, merged into its branches (G)";
                    if ui.add_enabled(idle, egui::Button::new("Map out").small()).on_hover_text(hint).clicked() {
                        cmds.push(Cmd::LearnMore(Focus::MapOut));
                        self.expanded.insert(node.id);
                    }
                });
                if !self.browses {
                    ui.label(egui::RichText::new(NO_WEB).color(palette(ui).hint).small());
                }
            }

            if let Some(brief) = layout.brief.filter(|_| node.parent.is_none()) {
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Interview").show(ui, |ui| {
                    ui.label(egui::RichText::new(brief).color(palette(ui).hint));
                    ui.label(egui::RichText::new("Sent with every AI request for this map").color(palette(ui).hint).small());
                });
            }
            let children: Vec<_> = layout.children(node.id).collect();
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                heading(ui, format!("Branches ({})", children.len()));
                if ui.small_button("+ Add").on_hover_text("Add a branch by hand (Tab)").clicked() {
                    self.adding = Some(String::new());
                    self.focus_add = true;
                }
            });
            for child in &children {
                if ui.link(child.text.as_str()).clicked() {
                    cmds.push(Cmd::Select(child.id));
                    self.expanded.insert(child.id);
                }
            }
            self.add_box(ui, node.id, cmds);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("More ideas").on_hover_text("M").clicked() {
                    cmds.push(Cmd::Reroll);
                }
                let hint = "Delete this node and its branches, after a yes. Ctrl+Z brings them back";
                if ui.add_enabled(node.parent.is_some(), egui::Button::new("Delete")).on_hover_text(hint).clicked() {
                    cmds.push(Cmd::Delete);
                }
            });

            ui.add_space(12.0);
            heading(ui, "Discuss");
            ui.scope(|ui| {
                note_style(ui, settings);
                for (i, turn) in layout.chat.iter().enumerate() {
                    match turn {
                        Turn::You(text) => _ = ui.label(egui::RichText::new(text).strong()),
                        Turn::Ai(reply) => {
                            ui.push_id(i, |ui| markdown(ui, &mut self.markdown, reply));
                            if ui.small_button("Add to note").clicked() {
                                let note = match layout.note {
                                    Some(Note::Text(old)) => format!("{old}\n\n{reply}"),
                                    _ => reply.clone(),
                                };
                                cmds.push(Cmd::SetNote(node.id, note));
                            }
                        }
                        Turn::Failed(e) => _ = ui.label(egui::RichText::new(format!("Failed: {e}")).color(palette(ui).fail)),
                    }
                }
            });
            if matches!(layout.chat.last(), Some(Turn::You(_))) && waiting(ui, (node.id, "chat"), "thinking…") {
                cmds.push(Cmd::Stop(Wait::Chat));
            }
        });
    }

}
