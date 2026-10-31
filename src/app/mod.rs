//! The shell. It owns the window, the open files and their worker channels. Every map rule lives in `map`,
//! every byte to or from a model lives in `ai`.

use crate::{ai, recent, wiki};
use eframe::egui::{self, Key, Modifiers};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
};

mod doc;
mod fonts;
mod help;
mod footer;
#[cfg(not(target_arch = "wasm32"))]
mod math;
mod settings;
mod setup;
mod tabs;
mod templates;
mod theme;
mod view;
mod widgets;

pub(crate) use doc::Doc;
use footer::Footer;
use settings::GraphSettings;
use widgets::palette;

/// "Mind-Me v0.1", from the version in `Cargo.toml`. Shown in the app's own title bar, since tiling window
/// managers such as Hyprland draw none, and as the window title.
pub(crate) fn title() -> String {
    let version: Vec<&str> = env!("CARGO_PKG_VERSION").split('.').take(2).collect();
    format!("Mind-Me v{}", version.join("."))
}

pub(crate) struct App {
    /// Empty only on first run, while the setup page shows.
    models: Vec<ai::Model>,
    /// The model every tab's ghost requests use, when the config names one.
    ghosts: Option<usize>,
    /// The note's Aa menu toggles them for the session. The config sets where they start.
    skills: Vec<ai::Skill>,
    footer: Footer,
    /// Never empty.
    docs: Vec<Doc>,
    active: usize,
    /// Wikipedia article tabs, after the map tabs. Session only.
    articles: Vec<wiki::Card>,
    /// The article tab showing, over the active map tab.
    article: Option<usize>,
    article_text: wiki::Articles,
    /// The Settings tab is open, and showing over the map and article tabs.
    settings_open: bool,
    on_settings: bool,
    /// The model setup page, over everything else while it shows. Always shown while there is no model.
    setup: Option<setup::Setup>,
    /// The model Settings asks about removing.
    removing: Option<usize>,
    /// The Settings tab showing, an index into its sections.
    section: usize,
    markdown: egui_commonmark::CommonMarkCache,
    /// The path typed into the new tab box, while it is open.
    opening: Option<String>,
    open_error: Option<String>,
    /// The Feedback modal's text, while it is open.
    feedback: Option<String>,
    /// Newest first, saved in the config folder on every open.
    recent: Vec<PathBuf>,
    /// Shared by every tab, and saved in the config folder once the user releases a change.
    settings: GraphSettings,
    saved_settings: GraphSettings,
    /// The start screen's templates, read once at start-up.
    templates: Vec<templates::Template>,
    /// Every theme in the picker, read once at start-up.
    themes: Vec<theme::Theme>,
    /// Omarchy's current theme, when Omarchy is installed. Re-read when it changes.
    omarchy: Option<(String, theme::Palette, std::time::SystemTime)>,
    omarchy_checked: f64,
    /// The palette and zoom last handed to egui, so a change is applied once.
    applied: Option<theme::Palette>,
    zoomed: f32,
    fonts: fonts::Fonts,
    /// Paths the file dialog thread picked, opened on the next frame.
    picked_tx: mpsc::Sender<Vec<PathBuf>>,
    picked: mpsc::Receiver<Vec<PathBuf>>,
    /// The config folder. Tests pass none, so they never touch the real recent files or settings.
    dir: Option<PathBuf>,
}

/// What every tab draws with and asks through, borrowed from the app each frame.
struct Shared<'a> {
    models: &'a [ai::Model],
    ghosts: Option<usize>,
    skills: &'a mut [ai::Skill],
    settings: &'a mut GraphSettings,
    recent: &'a [PathBuf],
    templates: &'a [templates::Template],
}

enum OpenFile {
    Path(PathBuf),
    /// The dialog to pick existing maps.
    Pick,
    /// The dialog to name a new map.
    New,
    Article(wiki::Card),
    Help,
}

impl App {
    pub(crate) fn new(config: ai::Config, docs: Vec<Doc>, dir: Option<PathBuf>) -> App {
        let footer = config.footer.as_deref().map_or(Footer::ALL, Footer::from);
        let recent = dir.as_ref().map(|d| recent::load(&d.join("recent"))).unwrap_or_default();
        let settings = dir.as_ref().map(|d| GraphSettings::load(&d.join("graph.json"))).unwrap_or_default();
        let (picked_tx, picked) = mpsc::channel();
        let themes = theme::themes(dir.as_deref());
        let templates = templates::templates(dir.as_deref());
        let mut app = App {
            models: config.models,
            ghosts: config.ghosts,
            skills: config.skills,
            footer,
            docs,
            active: 0,
            articles: Vec::new(),
            article: None,
            article_text: wiki::Articles::default(),
            settings_open: false,
            on_settings: false,
            section: 0,
            setup: None,
            removing: None,
            markdown: egui_commonmark::CommonMarkCache::default(),
            opening: None,
            open_error: None,
            feedback: None,
            recent,
            saved_settings: settings.clone(),
            settings,
            themes,
            templates,
            omarchy: theme::omarchy(),
            omarchy_checked: 0.0,
            applied: None,
            zoomed: 1.0,
            fonts: fonts::Fonts::system(),
            picked_tx,
            picked,
            dir,
        };
        for path in app.docs.iter().rev().map(|d| d.path.clone()).collect::<Vec<_>>() {
            app.remember(&path);
        }
        app
    }

    fn remember(&mut self, path: &Path) {
        recent::remember(&mut self.recent, path);
        if let Some(dir) = &self.dir
            && let Err(e) = recent::save(&dir.join("recent"), &self.recent)
        {
            self.open_error = Some(format!("recent files: {e}"));
        }
    }

    /// The file dialog runs on its own thread so the window
    /// keeps drawing. It uses the async dialog because macOS only allows the sync one on the main thread.
    fn open_file(&mut self, file: OpenFile, ctx: &egui::Context) {
        let file = match file {
            OpenFile::Path(path) => return self.open(path),
            OpenFile::Help => return self.open_help(),
            OpenFile::Article(card) => {
                let i = self.articles.iter().position(|a| a.url == card.url).unwrap_or_else(|| {
                    self.articles.push(card);
                    self.articles.len() - 1
                });
                (self.article, self.on_settings) = (Some(i), false);
                return;
            }
            file => file,
        };
        // shortcut: the browser playground has no file dialogs, so Open and New do nothing there; add upload and download if visitors look for them
        #[cfg(target_arch = "wasm32")]
        let _ = (file, ctx);
        #[cfg(not(target_arch = "wasm32"))]
        self.pick(file, ctx);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick(&self, file: OpenFile, ctx: &egui::Context) {
        let (tx, ctx) = (self.picked_tx.clone(), ctx.clone());
        let dialog = rfd::AsyncFileDialog::new().add_filter("Markdown", &["md", "markdown"]);
        std::thread::spawn(move || {
            let files = match file {
                OpenFile::Path(_) | OpenFile::Article(_) | OpenFile::Help => unreachable!("opened above"),
                OpenFile::Pick => pollster::block_on(dialog.pick_files()).unwrap_or_default().iter().map(|f| f.path().into()).collect(),
                OpenFile::New => pollster::block_on(dialog.set_file_name("map.md").save_file()).iter().map(|f| f.path().into()).collect(),
            };
            let _ = tx.send(files);
            ctx.request_repaint();
        });
    }


    /// Switches to the tab for `path`, opening it first when it is not open yet. A blank active tab gives
    /// its place to the file instead of staying empty.
    fn open(&mut self, path: PathBuf) {
        let path = std::path::absolute(&path).unwrap_or(path);
        (self.article, self.on_settings) = (None, false);
        if let Some(i) = self.docs.iter().position(|d| d.path == path) {
            self.active = i;
            self.remember(&path);
            return;
        }
        match Doc::open(path) {
            Ok(doc) => {
                let path = doc.path.clone();
                if self.docs[self.active].is_blank() {
                    self.docs[self.active] = doc;
                } else {
                    self.docs.push(doc);
                    self.active = self.docs.len() - 1;
                }
                self.open_error = None;
                self.remember(&path);
            }
            Err(e) => self.open_error = Some(e),
        }
    }

}

impl App {
    /// Hands egui the theme, the fonts and the zoom whenever one changed. A new style draws from the next
    /// frame, so it asks for one.
    fn look(&mut self, ctx: &egui::Context) {
        // Omarchy cannot tell other apps it switched themes, so look when the window gets focus or input, never on a timer.
        let now = ctx.input(|i| i.time);
        if self.omarchy.is_some() && now - self.omarchy_checked > 1.0 && ctx.input(|i| !i.events.is_empty()) {
            self.omarchy_checked = now;
            if let Some(fresh) = theme::omarchy().filter(|f| Some(f.2) != self.omarchy.as_ref().map(|o| o.2)) {
                self.omarchy = Some(fresh);
            }
        }
        let p = self.palette();
        if self.applied != Some(p) {
            theme::apply(ctx, p);
            self.applied = Some(p);
            ctx.request_repaint();
        }
        self.fonts.install(ctx, &self.settings.text_font, &self.settings.code_font);
        // egui's own Ctrl+= and Ctrl+- change the zoom; the Settings slider changes the setting.
        let zoom = ctx.zoom_factor();
        if zoom != self.zoomed {
            self.settings.zoom = zoom;
        } else if self.settings.zoom != zoom {
            ctx.set_zoom_factor(self.settings.zoom);
        }
        self.zoomed = self.settings.zoom;
    }
}

impl eframe::App for App {
    fn on_exit(&mut self) {
        for doc in &mut self.docs {
            doc.flush();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.look(ui.ctx());
        // The context's new style reaches `Ui`s from the next frame; this one was copied before it.
        widgets::type_scale(ui.style_mut());
        ui.painter().rect_filled(ui.max_rect(), 0.0, palette(ui).fog);
        for doc in &mut self.docs {
            doc.receive();
        }
        // Tab, the arrows and Enter drive the map. So outside a text box, egui must not also move focus with them
        // or press the last clicked button again.
        if !ui.ctx().egui_wants_keyboard_input() {
            ui.memory_mut(|m| {
                m.move_focus(egui::FocusDirection::None);
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        let mut help = false;
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::CTRL, Key::Tab) {
                self.active = (self.active + 1) % self.docs.len();
                (self.article, self.on_settings) = (None, false);
            }
            if i.consume_key(Modifiers::NONE, Key::F1) {
                help = true;
            }
            if i.consume_key(Modifiers::CTRL, Key::Comma) {
                (self.settings_open, self.on_settings) = (true, true);
            }
            if i.consume_key(Modifiers::CTRL, Key::T) {
                self.opening = Some(String::new());
            }
        });
        let (open, new) = ui.input_mut(|i| (i.consume_key(Modifiers::CTRL, Key::O), i.consume_key(Modifiers::CTRL, Key::N)));
        egui::Panel::top("title").show(ui, |ui| {
            ui.vertical_centered(|ui| ui.label(widgets::hint(ui, title())));
        });
        // With no model nothing else can work, so setup is the only screen until one is saved.
        if self.models.is_empty() && self.setup.is_none() {
            self.setup = Some(setup::Setup::new(true));
        }
        if self.setup.is_some() {
            self.setup_page(ui);
            return;
        }
        self.tabs(ui);
        if self.footer(ui) || help {
            self.open_help();
        }
        self.feedback(ui.ctx());
        let file = match self.article {
            _ if self.on_settings => {
                self.settings_page(ui);
                None
            }
            Some(i) => {
                self.article(ui, i);
                None
            }
            None => {
                let mut shared = Shared { models: &self.models, ghosts: self.ghosts, skills: &mut self.skills, settings: &mut self.settings, recent: &self.recent, templates: &self.templates };
                self.docs[self.active].show(ui, &mut shared)
            }
        };
        if let Some(file) = file.or(open.then_some(OpenFile::Pick)).or(new.then_some(OpenFile::New)) {
            self.open_file(file, ui.ctx());
        }
        for path in self.picked.try_iter().flatten().collect::<Vec<_>>() {
            self.open(path);
        }
        for doc in &mut self.docs {
            doc.autosave();
        }
        // Saved when the user releases a slider, not on every frame of the drag.
        if self.settings != self.saved_settings && !ui.input(|i| i.pointer.any_down()) {
            match &self.dir {
                // Left unsaved, so the next frame retries.
                Some(dir) if fs::create_dir_all(dir).and_then(|()| self.settings.save(&dir.join("graph.json"))).is_err() => {
                    self.open_error = Some("could not save the graph settings".into());
                }
                _ => self.saved_settings = self.settings.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests;
