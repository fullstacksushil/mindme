use super::theme::{self, Palette};
use super::footer::Footer;
use super::widgets::{heading, hint, palette, section};
use super::App;
use crate::ai;
use crate::graph::Forces;
use eframe::egui::{self, Key, Rect, Sense, Vec2};
use std::{fs, io, path::Path};

/// The app-wide settings: the graph panel's Display and Forces, like Obsidian's, and the look of the app.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct GraphSettings {
    /// Scales the node text, and with it every box.
    pub(super) text: f32,
    /// Scales the link lines.
    pub(super) links: f32,
    /// Rest length of a link between kept nodes.
    pub(super) distance: f32,
    pub(super) forces: Forces,
    /// The note in the code font.
    pub(super) mono: bool,
    /// The inspector shows the node's Wikipedia card, which sends the node's name to Wikipedia.
    pub(super) wiki: bool,
    /// The light palette of the theme, when it has one.
    pub(super) light: bool,
    /// A theme's family name or `theme::OMARCHY`. Empty is the default: Omarchy's theme when there is one.
    pub(super) theme: String,
    /// The whole UI's scale, egui's zoom factor.
    pub(super) zoom: f32,
    /// Font family names. Empty is the system's default.
    pub(super) text_font: String,
    pub(super) code_font: String,
}

impl Default for GraphSettings {
    fn default() -> GraphSettings {
        GraphSettings {
            text: 1.0,
            links: 1.0,
            distance: 150.0,
            forces: Forces::default(),
            mono: false,
            // The browser playground has no threads to fetch with.
            wiki: !cfg!(target_arch = "wasm32"),
            light: false,
            theme: String::new(),
            zoom: 1.0,
            text_font: String::new(),
            code_font: String::new(),
        }
    }
}

impl GraphSettings {
    /// A missing file or field keeps its default. An old file's `light` keeps its meaning.
    pub(super) fn load(file: &Path) -> GraphSettings {
        let json: serde_json::Value = fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let d = GraphSettings::default();
        let get = |key: &str, default: f32| json[key].as_f64().map_or(default, |v| v as f32);
        let flag = |key: &str, default: bool| json[key].as_bool().unwrap_or(default);
        let name = |key: &str| json[key].as_str().unwrap_or_default().to_owned();
        let forces = Forces { center: get("center", d.forces.center), repel: get("repel", d.forces.repel), link: get("link", d.forces.link) };
        GraphSettings {
            text: get("text", d.text),
            links: get("links", d.links),
            distance: get("distance", d.distance),
            forces,
            mono: flag("mono", d.mono),
            wiki: flag("wiki", d.wiki),
            light: flag("light", d.light),
            theme: name("theme"),
            zoom: get("zoom", d.zoom).clamp(0.5, 3.0),
            text_font: name("text_font"),
            code_font: name("code_font"),
        }
    }

    pub(super) fn save(&self, file: &Path) -> io::Result<()> {
        let f = &self.forces;
        let json = serde_json::json!({ "text": self.text, "links": self.links, "distance": self.distance,
            "center": f.center, "repel": f.repel, "link": f.link, "mono": self.mono, "wiki": self.wiki, "light": self.light,
            "theme": self.theme, "zoom": self.zoom, "text_font": self.text_font, "code_font": self.code_font });
        fs::write(file, serde_json::to_string_pretty(&json)?)
    }
}

impl GraphSettings {
    /// Display and Forces back to their defaults, leaving the theme, fonts and notes alone.
    pub(super) fn reset_graph(&mut self) {
        let d = GraphSettings::default();
        (self.text, self.links, self.distance, self.forces) = (d.text, d.links, d.distance, d.forces);
    }

    /// Display and Forces, in the canvas's Graph panel and the Settings tab.
    pub(super) fn graph_controls(&mut self, ui: &mut egui::Ui) {
        ui.label(hint(ui, "Display"));
        ui.add(egui::Slider::new(&mut self.text, 0.5..=2.0).text("Text size"));
        ui.add(egui::Slider::new(&mut self.links, 0.2..=4.0).text("Link thickness"));
        ui.add_space(6.0);
        ui.label(hint(ui, "Forces"));
        let f = &mut self.forces;
        ui.add(egui::Slider::new(&mut f.center, 0.0..=1.0).text("Center force"));
        ui.add(egui::Slider::new(&mut f.repel, 0.0..=20.0).text("Repel force"));
        ui.add(egui::Slider::new(&mut f.link, 0.0..=1.0).text("Link force"));
        ui.add(egui::Slider::new(&mut self.distance, 30.0..=500.0).text("Link distance"));
    }

    /// The note's font, the Wikipedia card and the writing guides, in the Aa menu and the Settings tab.
    pub(super) fn note_controls(&mut self, ui: &mut egui::Ui, skills: &mut [ai::Skill]) {
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.mono, false, "Sans");
            ui.radio_value(&mut self.mono, true, "Mono");
        });
        ui.checkbox(&mut self.wiki, "Wikipedia card").on_hover_text("Sends the node's name to Wikipedia");
        if !skills.is_empty() {
            ui.separator();
            heading(ui, "Writing guides");
            for skill in skills.iter_mut() {
                ui.checkbox(&mut skill.on, skill.name.as_str()).on_hover_text("Followed by Learn more, Discuss and Map out");
            }
        }
    }
}

impl App {
    /// The theme in use: the saved one, or by default Omarchy's when there is one.
    pub(super) fn theme_name(&self) -> &str {
        match self.settings.theme.as_str() {
            "" if self.omarchy.is_some() => theme::OMARCHY,
            "" => theme::DEFAULT,
            name => name,
        }
    }

    pub(super) fn palette(&self) -> Palette {
        let name = self.theme_name();
        if let (theme::OMARCHY, Some((_, p, _))) = (name, &self.omarchy) {
            return *p;
        }
        let theme = self.themes.iter().find(|t| t.name == name).or(self.themes.first());
        theme.map_or_else(theme::default_palette, |t| t.palette(self.settings.light))
    }

    /// The theme has both a dark and a light palette to switch between. Following Omarchy, Omarchy picks.
    pub(super) fn can_toggle(&self) -> bool {
        self.themes.iter().any(|t| t.name == self.theme_name() && t.dark.is_some() && t.light.is_some())
    }

    fn appearance(&mut self, ui: &mut egui::Ui) {
        let current = self.theme_name().to_owned();
        let title = match self.themes.iter().find(|t| t.name == current) {
            Some(t) => t.title(),
            None => "Follow Omarchy".into(),
        };
        let mut picked = None;
        ui.horizontal(|ui| {
            strip(ui, &self.palette());
            egui::ComboBox::from_label("Theme").selected_text(title).height(400.0).show_ui(ui, |ui| {
                if let Some((name, p, _)) = &self.omarchy
                    && swatch_row(ui, p, &format!("Follow Omarchy ({name})"), current == theme::OMARCHY)
                {
                    picked = Some(theme::OMARCHY.to_owned());
                }
                for t in &self.themes {
                    let modes = match (t.dark, t.light) {
                        (Some(_), Some(_)) => "",
                        (Some(_), None) => "  · dark only",
                        _ => "  · light only",
                    };
                    if swatch_row(ui, &t.palette(self.settings.light), &format!("{}{modes}", t.title()), current == t.name) {
                        picked = Some(t.name.clone());
                    }
                }
            });
        });
        if let Some(name) = picked {
            self.settings.theme = name;
        }
        ui.add_enabled_ui(self.can_toggle(), |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.settings.light, false, "Dark");
                ui.radio_value(&mut self.settings.light, true, "Light");
            });
        });
        ui.add_space(6.0);
        ui.add(egui::Slider::new(&mut self.settings.zoom, 0.75..=2.0).text("Size"));
        ui.label(hint(ui, "Ctrl+= and Ctrl+- also change it, Ctrl+0 resets it."));
        ui.add_space(6.0);
        let fonts = &self.fonts;
        for (label, picked, default) in [("Text font", &mut self.settings.text_font, &fonts.text), ("Code font", &mut self.settings.code_font, &fonts.code)] {
            let system = if default.is_empty() { "System".to_owned() } else { format!("System ({default})") };
            let shown = if picked.is_empty() { system.clone() } else { picked.clone() };
            egui::ComboBox::from_label(label).selected_text(shown).height(320.0).show_ui(ui, |ui| {
                ui.selectable_value(picked, String::new(), system);
                for name in &fonts.names {
                    ui.selectable_value(picked, name.clone(), name.as_str());
                }
            });
        }
    }

    /// Every setting, in vertical tabs, opened from the + menu, the footer or Ctrl+,.
    pub(super) fn settings_page(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        ui.painter().rect_filled(full, 0.0, ui.visuals().panel_fill);
        let inner = full.shrink(16.0);
        let column = Rect::from_center_size(inner.center(), Vec2::new(inner.width().min(760.0), inner.height()));
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(column));
        let back = format!("‹ Back to {}", self.docs[self.active].name());
        if ui.button(back).on_hover_text("Esc").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.on_settings = false;
        }
        ui.add_space(8.0);
        ui.heading("Settings");
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(140.0);
                for (i, name) in SECTIONS.iter().enumerate() {
                    if ui.selectable_label(self.section == i, egui::RichText::new(*name).text_style(section())).clicked() {
                        self.section = i;
                    }
                }
            });
            ui.separator();
            // Its own top-down layout: the row beside the tab list would put every control side by side.
            egui::ScrollArea::vertical().show(ui, |ui| ui.vertical(|ui| {
                heading(ui, SECTIONS[self.section]);
                ui.separator();
                let s = &mut self.settings;
                // What "Reset this tab" puts back. Models has none: they come from config.json.
                let reset: Option<fn(&mut App)> = match self.section {
                    0 => {
                        self.appearance(ui);
                        Some(|a| {
                            let d = GraphSettings::default();
                            (a.settings.theme, a.settings.light, a.settings.zoom, a.settings.text_font, a.settings.code_font) = (d.theme, d.light, d.zoom, d.text_font, d.code_font);
                        })
                    }
                    1 => {
                        s.note_controls(ui, &mut self.skills);
                        Some(|a| {
                            let d = GraphSettings::default();
                            (a.settings.mono, a.settings.wiki) = (d.mono, d.wiki);
                        })
                    }
                    2 => {
                        s.graph_controls(ui);
                        if ui.button("Rearrange").on_hover_text("Lay the current map's graph out again from scratch").clicked() {
                            self.docs[self.active].view.graph.rearrange();
                        }
                        Some(|a| a.settings.reset_graph())
                    }
                    3 => {
                        self.models_tab(ui);
                        None
                    }
                    _ => {
                        self.footer.controls(ui);
                        ui.label(hint(ui, "The footer's figures last until the app closes. config.json's \"footer\" sets where they start."));
                        Some(|a| a.footer = Footer::ALL)
                    }
                };
                if let Some(reset) = reset {
                    ui.add_space(16.0);
                    if ui.button("Reset this tab").clicked() {
                        reset(self);
                    }
                }
            }));
        });
    }
}

/// The Settings tabs, in order.
const SECTIONS: [&str; 5] = ["Appearance", "Notes", "Graph", "Models", "Footer"];

/// A palette's canvas, panel, trail, node and ghost colors side by side.
fn strip(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 16.0), Sense::hover());
    for (i, color) in [p.fog, p.panel, p.trail, p.node, p.ghost].into_iter().enumerate() {
        let x = rect.left() + i as f32 * 14.0;
        ui.painter().rect_filled(Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(14.0, 16.0)), 0.0, color);
    }
    ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.0, palette(ui).branch_link), egui::StrokeKind::Outside);
}

/// One theme in the picker: its color strip, then its name. True when clicked.
fn swatch_row(ui: &mut egui::Ui, p: &Palette, title: &str, selected: bool) -> bool {
    ui.horizontal(|ui| {
        strip(ui, p);
        ui.selectable_label(selected, title).clicked()
    })
    .inner
}
