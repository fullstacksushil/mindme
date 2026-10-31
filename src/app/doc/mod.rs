use crate::app::view::{Start, View};
use crate::app::{OpenFile, Shared};
use crate::map::{Cmd, Map, Slot, Text};
use crate::ai;
use eframe::egui;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

mod interview;
mod request;

pub(super) use interview::{Interview, Step};
use request::{Msg, Reply, Request};

/// `Ok(None)` starts an empty map: the file is missing or blank.
/// A non-blank file with no `# title` is an error, because autosave would overwrite it.
fn load(path: &Path) -> Result<Option<Map>, String> {
    let md = match fs::read_to_string(path) {
        Ok(md) => md,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if md.trim().is_empty() {
        return Ok(None);
    }
    Map::parse(&md).map(Some).ok_or(format!("{} has no '# title' line; not touching it", path.display()))
}

/// Writes `<path>.tmp`, then renames it over `path`. `fs::rename` replaces the target on Windows too.
fn save(path: &Path, md: &str) -> io::Result<()> {
    #[cfg(target_arch = "wasm32")]
    return crate::web::write(path, md);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, md)?;
    fs::rename(&tmp, path)
}

pub(super) enum Stage {
    /// The start screen. `error` is why the last map out failed.
    Start { error: Option<String> },
    Interview(Interview),
    MappingOut(Text),
    Map(Map),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Goal {
    Brainstorm { auto: bool },
    MapOut,
}

/// One open file: its map, its view and its own reply channel, so every reply lands in the tab that asked.
pub(crate) struct Doc {
    pub(super) path: PathBuf,
    pub(super) saved: String,
    stage: Stage,
    pub(super) view: View,
    /// The last save error, shown in the status bar until a save succeeds.
    pub(super) save_error: Option<String>,
    pub(super) in_flight: usize,
    /// Index into the app's models. Picked on the start screen.
    pub(super) model: usize,
    pub(super) usage: ai::Usage,
    tx: mpsc::Sender<(ai::Usage, Reply)>,
    rx: mpsc::Receiver<(ai::Usage, Reply)>,
    stops: Vec<request::Running>,
}

impl Doc {
    /// Makes the path absolute, so the same file opened two ways is one tab and one recent entry.
    pub(crate) fn open(path: PathBuf) -> Result<Doc, String> {
        let path = std::path::absolute(&path).unwrap_or(path);
        let map = load(&path)?;
        Ok(Doc::new(path, map))
    }

    /// A tab for `map`, saved to `path`. `None` starts on the start screen.
    pub(crate) fn new(path: PathBuf, map: Option<Map>) -> Doc {
        let (tx, rx) = mpsc::channel();
        // Seeded with the loaded map so opening a hand-edited file does not rewrite it.
        let saved = map.as_ref().map(Map::to_markdown).unwrap_or_default();
        let stage = Stage::Start { error: None };
        let mut doc = Doc { path, saved, stage, view: View::default(), save_error: None, in_flight: 0, model: 0, usage: ai::Usage::default(), tx, rx, stops: Vec::new() };
        if let Some(map) = map {
            // A bare title still brainstorms.
            let auto = map.layout().nodes.len() == 1;
            doc.adopt(map, auto);
        }
        doc
    }

    /// Unfolds the map's top `levels`, so a first look shows its shape. Opened files start folded.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn unfold(&mut self, levels: usize) {
        let Some(map) = self.map() else { return };
        let mut depth = std::collections::HashMap::new();
        let mut open = Vec::new();
        // Parents come before their children.
        for node in map.layout().nodes {
            let d = node.parent.map_or(0, |p| depth[&p] + 1);
            depth.insert(node.id, d);
            if d < levels {
                open.push(node.id);
            }
        }
        self.view.expanded.extend(open);
    }

    /// A fresh tab with nothing typed yet, which an opened file may replace.
    pub(super) fn is_blank(&self) -> bool {
        matches!(self.stage, Stage::Start { .. })
    }

    pub(super) fn map(&self) -> Option<&Map> {
        match &self.stage {
            Stage::Map(map) => Some(map),
            _ => None,
        }
    }

    /// Auto off means the user explores the map instead of growing it, so the AI waits to be asked. This applies
    /// to a mapped-out topic, an opened file with branches, or a brainstorm started without Auto-suggest.
    fn adopt(&mut self, mut map: Map, auto: bool) {
        if !auto {
            map.step(vec![Cmd::SetAuto(false)]);
        }
        self.stage = Stage::Map(map);
    }

    pub(super) fn name(&self) -> String {
        self.path.file_name().map_or(String::new(), |f| f.to_string_lossy().into_owned())
    }

    /// Applies every reply that arrived since the last frame, whether or not this tab is showing.
    pub(super) fn receive(&mut self) {
        while let Ok((usage, (stopped, msg))) = self.rx.try_recv() {
            self.in_flight -= 1;
            self.usage += usage;
            // Every Stop already updated the screen, and the late reply would land on whatever was asked next.
            if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }
            match (msg, &mut self.stage) {
                (Msg::Map(ask, result), Stage::Map(map)) => map.receive(ask, result),
                // Only while mapping out, so a reply after a stop never replaces what the user did next.
                (Msg::Outline(Ok(map)), Stage::MappingOut(_)) => self.adopt(map, false),
                (Msg::Outline(Err(e)), Stage::MappingOut(_)) => self.stage = Stage::Start { error: Some(e) },
                (Msg::Question(result), Stage::Interview(interview)) => interview.receive(result),
                _ => {}
            }
        }
    }

    /// Returns a file the start screen asked to open, or the Wikipedia article the topic panel asked for.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, shared: &mut Shared) -> Option<OpenFile> {
        let ctx = ui.ctx().clone();
        self.view.browses = shared.models[self.model].browses();
        let map = match &mut self.stage {
            Stage::Map(map) => map,
            Stage::Interview(interview) => {
                let step = self.view.interview(ui, interview);
                if step == Some(Step::Stop) {
                    request::stop(&self.stops, None);
                    interview.receive(Err("stopped".into()));
                } else if interview.finished(step) {
                    let Stage::Interview(interview) = std::mem::replace(&mut self.stage, Stage::Start { error: None }) else { unreachable!() };
                    let (topic, goal, brief) = interview.into_brief();
                    self.begin(topic, goal, Some(brief), shared, &ctx);
                } else if matches!(step, Some(Step::Answer | Step::Retry)) {
                    let request = interview.request();
                    self.send(request, shared, &ctx);
                }
                return None;
            }
            Stage::Start { .. } | Stage::MappingOut(_) => {
                let names: Vec<&str> = shared.models.iter().map(|m| m.name.as_str()).collect();
                let others: Vec<PathBuf> = shared.recent.iter().filter(|p| **p != self.path).cloned().collect();
                match self.view.start(ui, &names, &mut self.model, &others, shared.templates, &self.stage) {
                    Some(Start::Template { topic, index }) => {
                        let auto = !self.view.no_auto;
                        if let Some(map) = shared.templates.get(index).and_then(|t| t.map(&topic)) {
                            self.adopt(map, auto);
                            ctx.request_repaint();
                        }
                    }
                    Some(Start::Open(file)) => return Some(file),
                    Some(Start::Begin { topic, goal, grill: true }) => {
                        let mut interview = Interview::new(topic, goal);
                        self.send(interview.request(), shared, &ctx);
                        self.stage = Stage::Interview(interview);
                    }
                    Some(Start::Begin { topic, goal, grill: false }) => self.begin(topic, goal, None, shared, &ctx),
                    Some(Start::Stop) => {
                        request::stop(&self.stops, None);
                        self.stage = Stage::Start { error: Some("stopped".into()) };
                    }
                    None => {}
                }
                return None;
            }
        };
        let layout = map.layout();
        let loading = layout.ghosts.iter().any(|slot| matches!(slot, Slot::Shimmer));
        let cmds = self.view.show(ui, &layout, shared.settings, shared.skills);
        for cmd in &cmds {
            if let Cmd::Stop(wait) = *cmd {
                request::stop(&self.stops, Some((layout.current, wait)));
            }
        }
        // Before the step, so a cut copies its node before deleting it.
        if let Some(id) = self.view.copy.take() {
            ctx.copy_text(map.subtree(id));
        }
        let asks = map.step(cmds);
        // egui draws only on input or a repaint request, so keep frames coming while ghosts shimmer or anything moves.
        if loading || !asks.is_empty() || self.view.animating() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        for ask in asks {
            self.send(Request::Map(ask), shared, &ctx);
        }
        self.view.open_article.take().map(OpenFile::Article)
    }

    fn begin(&mut self, topic: Text, goal: Goal, brief: Option<String>, shared: &Shared, ctx: &egui::Context) {
        match goal {
            Goal::Brainstorm { auto } => {
                let mut map = Map::new(topic);
                map.set_brief(brief);
                self.adopt(map, auto);
                // The app sends the root's first request next frame, and egui would not draw one unprompted.
                ctx.request_repaint();
            }
            Goal::MapOut => {
                self.send(Request::Outline { topic: topic.clone(), brief }, shared, ctx);
                self.stage = Stage::MappingOut(topic);
            }
        }
    }

    /// Saves the note being typed, which otherwise reaches the map only when its box loses focus. For a closing tab or window.
    pub(super) fn flush(&mut self) {
        if let (Stage::Map(map), Some((id, note))) = (&mut self.stage, self.view.note_edit.take()) {
            map.step(vec![Cmd::SetNote(id, note)]);
        }
        self.autosave();
    }

    /// shortcut: renders the Markdown every frame, gate it on applied commands if 500+ node maps stutter.
    pub(super) fn autosave(&mut self) {
        let Stage::Map(map) = &self.stage else { return };
        let md = map.to_markdown();
        if md == self.saved {
            return;
        }
        match save(&self.path, &md) {
            Ok(()) => {
                self.saved = md;
                self.save_error = None;
            }
            // `saved` stays stale, so the next frame retries.
            Err(e) => self.save_error = Some(format!("save failed: {e}")),
        }
    }
}
