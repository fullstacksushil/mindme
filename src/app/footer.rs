use super::widgets::{heading, hint, palette};
use crate::wiki::encode;
use super::view::KEYS;
use super::App;
use eframe::egui;

impl App {
    /// True when the Keys menu asked for the help map.
    pub(super) fn footer(&mut self, ui: &mut egui::Ui) -> bool {
        let mut help = false;
        let (toggle, dark, following) = (self.can_toggle(), self.palette().dark, self.theme_name() == super::theme::OMARCHY);
        let doc = &mut self.docs[self.active];
        let f = &mut self.footer;
        let light = &mut self.settings.light;
        let (settings_open, on_settings) = (&mut self.settings_open, &mut self.on_settings);
        let feedback = &mut self.feedback;
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                match &doc.save_error {
                    Some(e) => ui.label(egui::RichText::new(e).color(palette(ui).fail)),
                    None if doc.map().is_some() => ui.label(egui::RichText::new("saved").color(palette(ui).hint)),
                    None => ui.label(egui::RichText::new("not saved yet").color(palette(ui).hint)),
                };
                if doc.in_flight > 0 {
                    ui.label(format!("· asking the AI ({})", doc.in_flight));
                }
                ui.separator();
                let u = &doc.usage;
                if f.model {
                    let names: Vec<&str> = self.models.iter().map(|m| m.name.as_str()).collect();
                    let picked = names[doc.model];
                    let shown = u.model.as_deref().map_or(picked.to_owned(), |m| format!("{picked} · {m}"));
                    let hint = format!("{picked}: {}\nAfter a reply, the model it named.", self.models[doc.model].command());
                    if names.len() > 1 {
                        egui::ComboBox::from_id_salt("footer model").selected_text(&shown).show_index(ui, &mut doc.model, names.len(), |i| names[i]).on_hover_text(hint);
                    } else {
                        ui.add(egui::Label::new(egui::RichText::new(shown).color(palette(ui).hint)).truncate()).on_hover_text(hint);
                    }
                }
                let figures = [
                    (f.input, format!("in {}", tokens(u.input))),
                    (f.output, format!("out {}", tokens(u.output))),
                    (f.total, format!("total {}", tokens(u.input + u.output))),
                    (f.cached, format!("cached {}", tokens(u.cached))),
                    (f.cost, if u.cost < 1.0 { format!("~${:.4}", u.cost) } else { format!("~${:.2}", u.cost) }),
                ];
                for (_, text) in figures.into_iter().filter(|(show, _)| *show) {
                    ui.add(egui::Label::new(egui::RichText::new(text).color(palette(ui).hint)).truncate());
                }
                ui.menu_button("…", |ui| f.controls(ui))
                .response
                .on_hover_text("Choose what the footer shows");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (glyph, hint) = match (dark, toggle) {
                        _ if following => (if dark { "☀" } else { "🌙" }, "Omarchy picks the theme. Choose another in Settings to switch modes"),
                        (_, false) => (if dark { "☀" } else { "🌙" }, "This theme has only one mode"),
                        (true, true) => ("☀", "Light mode of this theme"),
                        (false, true) => ("🌙", "Dark mode of this theme"),
                    };
                    if ui.add_enabled(toggle, egui::Button::new(glyph)).on_hover_text(hint).on_disabled_hover_text(hint).clicked() {
                        *light = dark;
                    }
                    if ui.button("Settings").on_hover_text("Ctrl+,").clicked() {
                        (*settings_open, *on_settings) = (true, true);
                    }
                    if ui.button("Feedback").on_hover_text("Write to the maker by email").clicked() {
                        *feedback = Some(String::new());
                    }
                    ui.menu_button("Keys", |ui| {
                        help = ui.button("Open the help map  F1").clicked();
                        ui.separator();
                        // Two columns of groups, so the list fits a window's height.
                        ui.horizontal_top(|ui| {
                            for half in [&KEYS[..2], &KEYS[2..]] {
                                egui::Grid::new(half[0].0).num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                                    for (group, keys) in half {
                                        heading(ui, *group);
                                        ui.end_row();
                                        for (key, does) in *keys {
                                            ui.strong(*key);
                                            ui.label(*does);
                                            ui.end_row();
                                        }
                                    }
                                });
                                ui.add_space(24.0);
                            }
                        });
                    })
                    .response
                    .on_hover_text("Keyboard shortcuts and the help map");
                });
            });
        });
        help
    }

    /// The Feedback modal. It opens an email draft, since Mind-Me has no server; the user attaches pictures there.
    pub(super) fn feedback(&mut self, ctx: &egui::Context) {
        let Some(text) = &mut self.feedback else { return };
        let modal = egui::Modal::new(egui::Id::unique("feedback")).show(ctx, |ui| {
            ui.set_width(420.0);
            heading(ui, "Feedback");
            ui.add(egui::TextEdit::multiline(text).hint_text("What happened, or what would you like?").desired_rows(6).desired_width(f32::INFINITY));
            ui.label(hint(ui, "This opens an email to the maker. Attach a screenshot or a screen recording there, it helps a lot."));
            ui.add_space(8.0);
            ui.horizontal(|ui| (ui.add_enabled(!text.trim().is_empty(), egui::Button::new("Write email")).clicked(), ui.button("Cancel").on_hover_text("Esc").clicked()))
                .inner
        });
        let (send, cancel) = modal.inner;
        if send {
            let body = format!("{text}\n\n--\nMind-Me {} on {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS);
            ctx.open_url(egui::OpenUrl::new_tab(format!("mailto:{FEEDBACK}?subject={}&body={}", encode("Mind-Me feedback"), encode(&body))));
        }
        if send || cancel || modal.should_close() {
            self.feedback = None;
        }
    }
}

const FEEDBACK: &str = "feedback@mindme.cc";

/// Which figures the footer shows. The config's `"footer"` list sets it, and the footer's menu changes it.
pub(super) struct Footer {
    model: bool,
    input: bool,
    output: bool,
    total: bool,
    cached: bool,
    cost: bool,
}

impl Footer {
    pub(super) fn controls(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.model, "Model");
        ui.checkbox(&mut self.input, "Tokens in");
        ui.checkbox(&mut self.output, "Tokens out");
        ui.checkbox(&mut self.total, "Total tokens");
        ui.checkbox(&mut self.cached, "Cached tokens");
        ui.checkbox(&mut self.cost, "Cost (approx.)");
    }

    pub(super) const ALL: Footer = Footer { model: true, input: true, output: true, total: true, cached: true, cost: true };

    /// Names from `"model", "in", "out", "total", "cached", "cost"`.
    pub(super) fn from(names: &[String]) -> Footer {
        let has = |name: &str| names.iter().any(|n| n == name);
        Footer { model: has("model"), input: has("in"), output: has("out"), total: has("total"), cached: has("cached"), cost: has("cost") }
    }
}

/// 950, 12.3k, 1.2M.
fn tokens(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}
