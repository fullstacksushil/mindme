use crate::app::templates::Template;
use crate::app::widgets::{enter_sends, heading, hint, palette, section, waiting, NO_WEB};
use crate::app::doc::{Goal, Interview, Stage, Step};
use crate::app::OpenFile;
use crate::map::{self, Text};
use eframe::egui::{self, Key, Rect, TextEdit, Vec2};
use std::path::{Path, PathBuf};
use super::View;

pub(crate) enum Start {
    /// Make a map of this idea or topic, after an interview when `grill` is on.
    Begin { topic: Text, goal: Goal, grill: bool },
    /// Make a map from the template at this index, with the topic as its root.
    Template { topic: Text, index: usize },
    Open(OpenFile),
    /// Give up the map out on its way.
    Stop,
}

/// "edited 2 days ago", from a file's modified time.
fn edited(path: &Path) -> String {
    let Ok(age) = std::fs::metadata(path).and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default()) else { return String::new() };
    let (minutes, hours, days) = (age.as_secs() / 60, age.as_secs() / 3600, age.as_secs() / 86400);
    let (n, unit) = match () {
        _ if minutes < 1 => return "edited just now".into(),
        _ if hours < 1 => (minutes, "minute"),
        _ if days < 1 => (hours, "hour"),
        _ if days < 14 => (days, "day"),
        _ => (days / 7, "week"),
    };
    format!("edited {n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// A card: a framed button whose name and line underneath are one click target. True when clicked.
fn card(ui: &mut egui::Ui, width: f32, name: &str, line: &str, hover: &str) -> bool {
    let frame = egui::Frame::group(ui.style()).fill(palette(ui).widget).corner_radius(6.0);
    let inner = frame.show(ui, |ui| {
        ui.set_width(width);
        // Stacked, even inside a row of cards.
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(name).text_style(section()).color(palette(ui).strong));
            ui.label(hint(ui, line));
        });
    });
    let response = ui.interact(inner.response.rect, ui.make_persistent_id(("card", name)), egui::Sense::click()).on_hover_text(hover);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.clicked()
}

impl View {
    /// The topic box and its buttons, the template cards, then recent maps and Help side by side.
    pub(crate) fn start(&mut self, ui: &mut egui::Ui, models: &[&str], model: &mut usize, recent: &[PathBuf], templates: &[Template], stage: &Stage) -> Option<Start> {
        let full = ui.available_rect_before_wrap().shrink(16.0);
        let column = Rect::from_center_size(full.center(), Vec2::new(full.width().min(700.0), full.height()));
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(column));
        if let Stage::MappingOut(topic) = stage {
            ui.add_space(full.height() / 3.0);
            let stop = waiting(ui, ("map out", topic.as_str()), &format!("Mapping out \"{}\"…", topic.as_str()));
            ui.label(hint(ui, "Reading the docs can take a few minutes."));
            return stop.then_some(Start::Stop);
        }
        let (mut file, mut picked_template) = (None, None);
        let (mut brainstorm, mut map_out) = (false, false);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(24.0);
            let hint_text = match self.template.and_then(|i| templates.get(i)) {
                Some(t) => format!("What is the {} about?", t.name),
                None => "Central idea or topic".into(),
            };
            let response = ui.add(TextEdit::singleline(&mut self.draft).hint_text(hint_text).font(egui::TextStyle::Heading).desired_width(f32::INFINITY));
            // Only when nothing else has focus, so the new tab box can keep it.
            if ui.memory(|m| m.focused().is_none()) || std::mem::take(&mut self.focus_topic) {
                response.request_focus();
            }
            let (enter, ctrl) = ui.input(|i| (i.key_pressed(Key::Enter), i.modifiers.command));
            let entered = enter && (response.lost_focus() || response.has_focus());
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                self.template = None;
            }
            ui.horizontal(|ui| {
                brainstorm |= ui.button("Brainstorm").on_hover_text("Enter").clicked() || (entered && !ctrl);
                map_out |= ui.button("Map out").on_hover_text("Ctrl+Enter").clicked() || (entered && ctrl);
                ui.checkbox(&mut self.grill, "Grill me").on_hover_text("Before the map starts, the AI interviews you one question at a time, so the map fits your goal and level.");
                let mut auto = !self.no_auto;
                ui.checkbox(&mut auto, "Auto-suggest").on_hover_text("Brainstorm with AI suggestions. Off, you add branches yourself with Tab.");
                self.no_auto = !auto;
                if models.len() > 1 {
                    egui::ComboBox::from_id_salt("model").selected_text(models[*model]).show_index(ui, model, models.len(), |i| models[i]).on_hover_text("The model this tab asks");
                }
            });
            if !self.browses {
                ui.label(hint(ui, NO_WEB));
            }
            if let Stage::Start { error: Some(e) } = stage {
                ui.label(egui::RichText::new(format!("Map out failed: {e}")).color(palette(ui).fail));
            }

            ui.add_space(20.0);
            heading(ui, "Templates");
            // Rows of as many cards as fit, so they wrap on a narrow window.
            let per_row = ((ui.available_width() / 220.0) as usize).max(1);
            for (row, chunk) in templates.chunks(per_row).enumerate() {
                ui.horizontal_top(|ui| {
                    for (j, t) in chunk.iter().enumerate() {
                        if card(ui, 190.0, &t.name, &t.about, "Start a map with these branches") {
                            picked_template = Some(row * per_row + j);
                        }
                    }
                });
            }

            ui.add_space(20.0);
            let wide = ui.available_width() > 560.0;
            let recent_cards = |ui: &mut egui::Ui, file: &mut Option<OpenFile>| {
                heading(ui, "Recent");
                ui.horizontal(|ui| {
                    if ui.button("Open…").on_hover_text("Ctrl+O").clicked() {
                        *file = Some(OpenFile::Pick);
                    }
                    if ui.button("New map…").on_hover_text("Ctrl+N").clicked() {
                        *file = Some(OpenFile::New);
                    }
                });
                for path in recent.iter().take(6) {
                    let name = path.file_name().map_or(String::new(), |f| f.to_string_lossy().into_owned());
                    let folder = path.parent().and_then(|p| p.file_name()).map_or(String::new(), |f| f.to_string_lossy().into_owned());
                    if card(ui, 300.0, &name, &format!("{folder} · {}", edited(path)), &path.display().to_string()) {
                        *file = Some(OpenFile::Path(path.clone()));
                    }
                }
            };
            let help_card = |ui: &mut egui::Ui, file: &mut Option<OpenFile>| {
                heading(ui, "Help");
                if card(ui, 260.0, "The help map", "Learn Mind-Me by trying it · F1", "Opens fresh each time") {
                    *file = Some(OpenFile::Help);
                }
            };
            if wide {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| recent_cards(ui, &mut file));
                    ui.add_space(24.0);
                    ui.vertical(|ui| help_card(ui, &mut file));
                });
            } else {
                recent_cards(ui, &mut file);
                ui.add_space(12.0);
                help_card(ui, &mut file);
            }
        });
        if let Some(file) = file {
            return Some(Start::Open(file));
        }
        if let Some(i) = picked_template {
            self.template = Some(i);
            self.focus_topic = true;
        }
        let topic = Text::new(&self.draft).filter(|_| brainstorm || map_out || picked_template.is_some())?;
        self.draft.clear();
        if let Some(index) = self.template.take() {
            return Some(Start::Template { topic, index });
        }
        let goal = if map_out { Goal::MapOut } else { Goal::Brainstorm { auto: !self.no_auto } };
        Some(Start::Begin { topic, goal, grill: self.grill })
    }

    pub(crate) fn interview(&mut self, ui: &mut egui::Ui, interview: &mut Interview) -> Option<Step> {
        let full = ui.available_rect_before_wrap().shrink(16.0);
        let column = Rect::from_center_size(full.center(), Vec2::new(full.width().min(560.0), full.height()));
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(column));
        let mut step = None;
        let (before, start) = match interview.goal() {
            Goal::Brainstorm { .. } => ("the brainstorm", "Start the brainstorm now"),
            Goal::MapOut => ("mapping it out", "Map it out now"),
        };
        ui.heading(interview.topic().as_str());
        ui.label(egui::RichText::new(format!("A few questions before {before}, so it fits you.")).color(palette(ui).hint));
        ui.add_space(8.0);
        egui::ScrollArea::vertical().max_height(full.height() * 0.5).stick_to_bottom(true).show(ui, |ui| {
            for (question, answer) in interview.turns() {
                ui.label(egui::RichText::new(question).strong());
                ui.label(answer);
                ui.add_space(6.0);
            }
        });
        if let Some(e) = interview.error() {
            ui.label(egui::RichText::new(format!("Interview failed: {e}")).color(palette(ui).fail));
            if ui.button("Try again").clicked() {
                step = Some(Step::Retry);
            }
        }
        match interview.asking().cloned() {
            None if interview.error().is_none() => {
                if waiting(ui, ("interview", interview.turns().len()), "thinking…") {
                    step = Some(Step::Stop);
                }
            }
            None => {}
            Some((question, recommended)) => {
                heading(ui, question.as_str());
                let id = ui.make_persistent_id("interview answer");
                let enter = enter_sends(ui, id);
                let hint = if recommended.is_empty() { "Your answer".to_owned() } else { format!("Recommended: {recommended}") };
                let response = ui.add(TextEdit::multiline(&mut interview.answer).id(id).hint_text(hint).desired_rows(3).desired_width(f32::INFINITY));
                if ui.memory(|m| m.focused().is_none()) {
                    response.request_focus();
                }
                ui.horizontal(|ui| {
                    let answer = ui.button("Answer").on_hover_text("Enter. Shift+Enter starts a new line").clicked() || enter;
                    let take = !recommended.is_empty() && ui.button("Use recommended").clicked();
                    if take {
                        interview.take_recommended();
                    }
                    if (answer || take) && interview.answer() {
                        step = Some(Step::Answer);
                    }
                    ui.label(egui::RichText::new(format!("{} of up to {}", interview.turns().len() + 1, map::QUESTIONS)).color(palette(ui).hint).small());
                });
            }
        }
        ui.add_space(12.0);
        if ui.button(start).on_hover_text("Skip the rest of the questions").clicked() {
            step = Some(Step::Start);
        }
        step
    }

}
