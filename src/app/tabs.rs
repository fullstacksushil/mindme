use super::widgets::{markdown, note_style, palette};
use super::{App, OpenFile};
use crate::recent;
use eframe::egui::{self, Key, Rect, TextEdit, Vec2};
use std::path::PathBuf;

impl App {
    pub(super) fn tabs(&mut self, ui: &mut egui::Ui) {
        let mut close = None;
        let mut close_article = None;
        let mut file = None;
        let mut export = None;
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, doc) in self.docs.iter().enumerate() {
                    let busy = if doc.in_flight > 0 { " …" } else { "" };
                    if ui.selectable_label(i == self.active && self.article.is_none() && !self.on_settings, format!("{}{busy}", doc.name())).clicked() {
                        (self.active, self.article, self.on_settings) = (i, None, false);
                    }
                    if self.docs.len() > 1 && ui.small_button("×").on_hover_text(format!("Close {}", doc.name())).clicked() {
                        close = Some(i);
                    }
                }
                for (i, card) in self.articles.iter().enumerate() {
                    if ui.selectable_label(self.article == Some(i) && !self.on_settings, format!("W {}", card.title)).on_hover_text("Wikipedia article").clicked() {
                        (self.article, self.on_settings) = (Some(i), false);
                    }
                    if ui.small_button("×").on_hover_text(format!("Close {}", card.title)).clicked() {
                        close_article = Some(i);
                    }
                }
                if self.settings_open {
                    if ui.selectable_label(self.on_settings, "Settings").clicked() {
                        self.on_settings = true;
                    }
                    if ui.small_button("×").on_hover_text("Close Settings").clicked() {
                        (self.settings_open, self.on_settings) = (false, false);
                    }
                }
                match &mut self.opening {
                    None => {
                        ui.menu_button("+", |ui| {
                            if ui.button("New map…  Ctrl+N").clicked() {
                                file = Some(OpenFile::New);
                            }
                            if ui.button("Open…  Ctrl+O").clicked() {
                                file = Some(OpenFile::Pick);
                            }
                            if ui.button("Type a path…  Ctrl+T").clicked() {
                                self.opening = Some(String::new());
                            }
                            if ui.button("Settings…  Ctrl+,").clicked() {
                                (self.settings_open, self.on_settings) = (true, true);
                            }
                            if let Some(map) = self.docs[self.active].map().filter(|_| self.article.is_none() && !self.on_settings) {
                                ui.separator();
                                if ui.button("Copy map as Markdown").clicked() {
                                    ui.ctx().copy_text(map.to_markdown());
                                }
                                if ui.button("Copy map as JSON").clicked() {
                                    ui.ctx().copy_text(map.to_json());
                                }
                                let how = if cfg!(target_arch = "wasm32") { "Download it as Markdown. Maps here live only in this browser" } else { "Save as .md for Markdown or .json for JSON" };
                                if ui.button("Export…").on_hover_text(how).clicked() {
                                    export = Some((self.docs[self.active].name(), map.to_markdown(), map.to_json()));
                                }
                            }
                            if !self.recent.is_empty() {
                                ui.separator();
                                ui.label(egui::RichText::new("Recent").color(palette(ui).hint));
                                if let Some(f) = recent_list(ui, &self.recent) {
                                    file = Some(f);
                                }
                                if ui.button("Clear recent").clicked() {
                                    self.recent.clear();
                                    if let Some(dir) = &self.dir {
                                        let _ = recent::save(&dir.join("recent"), &[]);
                                    }
                                }
                            }
                        })
                        .response
                        .on_hover_text("New, open or recent map");
                    }
                    Some(path) => {
                        let response = ui.add(TextEdit::singleline(path).hint_text("path/to/map.md").desired_width(220.0));
                        if !response.has_focus() && !response.lost_focus() {
                            response.request_focus();
                        }
                        if response.lost_focus() {
                            let path = self.opening.take().unwrap_or_default();
                            if ui.input(|i| i.key_pressed(Key::Enter)) && !path.trim().is_empty() {
                                file = Some(OpenFile::Path(path.trim().into()));
                            }
                        }
                    }
                }
                if let Some(e) = &self.open_error {
                    ui.label(egui::RichText::new(e).color(palette(ui).fail));
                }
            });
        });
        if let Some(file) = file {
            self.open_file(file, ui.ctx());
        }
        // The browser downloads the Markdown, the format the playground reads back with Open.
        #[cfg(target_arch = "wasm32")]
        if let Some((name, md, _)) = export {
            crate::web::download(&name, &md);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some((name, md, json)) = export {
            let dialog = rfd::AsyncFileDialog::new().add_filter("Markdown", &["md"]).add_filter("JSON", &["json"]).set_file_name(name);
            std::thread::spawn(move || {
                let Some(file) = pollster::block_on(dialog.save_file()) else { return };
                let text = if file.path().extension().is_some_and(|e| e == "json") { json } else { md };
                // shortcut: a failed export is only printed, show it in the tab strip if anyone hits it
                if let Err(e) = std::fs::write(file.path(), text) {
                    eprintln!("export to {} failed: {e}", file.path().display());
                }
            });
        }
        if let Some(i) = close {
            // Replies that landed this frame, and the note being typed, are not on disk yet.
            self.docs.remove(i).flush();
            if self.active > i || self.active == self.docs.len() {
                self.active -= 1;
            }
        }
        if let Some(i) = close_article {
            self.articles.remove(i);
            self.article = self.article.filter(|a| *a != i).map(|a| if a > i { a - 1 } else { a });
        }
    }

    pub(super) fn article(&mut self, ui: &mut egui::Ui, i: usize) {
        let card = &self.articles[i];
        let settings = self.settings.clone();
        let text = self.article_text.text(&card.title, ui.ctx());
        let full = ui.available_rect_before_wrap();
        ui.painter().rect_filled(full, 0.0, ui.visuals().panel_fill);
        let inner = full.shrink(16.0);
        let column = Rect::from_center_size(inner.center(), Vec2::new(inner.width().min(680.0), inner.height()));
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(column));
        let back = format!("‹ Back to {}", self.docs[self.active].name());
        if ui.button(back).on_hover_text("Esc").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.article = None;
        }
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt(&card.url).show(ui, |ui| {
            if let Some(thumb) = &card.thumb {
                ui.add(egui::Image::from_uri(thumb).max_width(ui.available_width()).max_height(260.0));
            }
            ui.heading(&card.title);
            ui.hyperlink_to("Open on Wikipedia", &card.url);
            ui.add_space(8.0);
            ui.scope(|ui| {
                note_style(ui, &settings);
                match text {
                    Ok(md) => markdown(ui, &mut self.markdown, md),
                    Err(line) => _ = ui.label(egui::RichText::new(*line).color(palette(ui).hint)),
                }
            });
            ui.label(egui::RichText::new("From Wikipedia, CC BY-SA 4.0").color(palette(ui).hint).small());
        });
    }
}

pub(super) fn recent_list(ui: &mut egui::Ui, recent: &[PathBuf]) -> Option<OpenFile> {
    let mut picked = None;
    for path in recent {
        let name = path.file_name().map_or(String::new(), |f| f.to_string_lossy().into_owned());
        let folder = path.parent().and_then(|p| p.file_name()).map_or(String::new(), |f| f.to_string_lossy().into_owned());
        if ui.button(format!("{name}  ·  {folder}")).on_hover_text(path.display().to_string()).clicked() {
            picked = Some(OpenFile::Path(path.clone()));
        }
    }
    picked
}
