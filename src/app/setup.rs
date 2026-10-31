//! Setting up a model: the first screen when none is configured, and Settings' "Add a model…".

use super::widgets::{hint, palette, waiting};
use super::App;
use crate::ai;
use eframe::egui::{self, Rect, TextEdit, Vec2};
use serde_json::{json, Value};
use std::{
    sync::{atomic::AtomicBool, mpsc, Arc},
    time::Duration,
};

/// The lean `claude -p` command from `README.md`: web tools on, Claude Code's other tools, servers and skills off.
const CLAUDE: &str = "claude -p --output-format json --tools WebSearch,WebFetch --allowedTools WebSearch,WebFetch --strict-mcp-config --setting-sources local --disable-slash-commands";

/// `codex exec` reading the prompt on stdin (`-`), as events Mind-Me reads the reply and tokens from. Read-only and
/// leaving no session file, since every suggestion is one call. It works outside a Git repository, where maps live.
const CODEX: &str = "codex exec --json --skip-git-repo-check --ephemeral --sandbox read-only --color never -";

/// OpenAI-style chat endpoints, and a model each one serves, as the model box's hint.
const PROVIDERS: [(&str, &str, &str); 4] = [
    ("Anthropic", "https://api.anthropic.com/v1/chat/completions", "claude-sonnet-5-5"),
    ("OpenAI", "https://api.openai.com/v1/chat/completions", "gpt-5"),
    ("OpenRouter", "https://openrouter.ai/api/v1/chat/completions", "anthropic/claude-sonnet-5-5"),
    ("Another URL", "", "model name"),
];

#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Claude,
    Codex,
    Api,
    Ollama,
    Command,
}

pub(super) struct Setup {
    /// No model exists yet, so there is nothing to go back to.
    first: bool,
    choice: Choice,
    provider: usize,
    url: String,
    model: String,
    key: String,
    command: String,
    claude_found: bool,
    codex_found: bool,
    /// Ollama's installed models, once its server answers.
    ollama: mpsc::Receiver<Vec<String>>,
    ollama_models: Option<Vec<String>>,
    /// The test call's reply, while one runs.
    test: Option<mpsc::Receiver<(ai::Usage, Result<String, String>)>>,
    tested: Option<Result<(), String>>,
    pub(super) error: Option<String>,
}

impl Setup {
    pub(super) fn new(first: bool) -> Setup {
        // Never in tests, so no test can start a real, billed claude call.
        let on_path = |name: &str| !cfg!(test) && std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file() || d.join(format!("{name}.exe")).is_file()));
        let (claude_found, codex_found) = (on_path("claude"), on_path("codex"));
        let (tx, ollama) = mpsc::channel();
        // The browser playground has no threads, and never shows setup since it has its model.
        if !cfg!(test) && !cfg!(target_arch = "wasm32") {
            std::thread::spawn(move || {
                let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(1))).build().new_agent();
                let tags = agent.get("http://localhost:11434/api/tags").call().and_then(|mut r| r.body_mut().read_json::<Value>());
                if let Ok(tags) = tags {
                    let _ = tx.send(tags["models"].as_array().into_iter().flatten().filter_map(|m| m["name"].as_str().map(String::from)).collect());
                }
            });
        }
        let choice = if claude_found { Choice::Claude } else if codex_found { Choice::Codex } else { Choice::Api };
        Setup { first, choice, provider: 0, url: String::new(), model: String::new(), key: String::new(), command: String::new(), claude_found, codex_found, ollama, ollama_models: None, test: None, tested: None, error: None }
    }

    /// The `config.json` entry for what is filled in, or none while something needed is empty.
    fn entry(&self) -> Option<Value> {
        let model = self.model.trim();
        Some(match self.choice {
            Choice::Claude => json!({ "name": "Claude Pro/Max", "cmd": CLAUDE }),
            Choice::Codex => json!({ "name": "OpenAI Codex", "cmd": CODEX }),
            Choice::Command => json!({ "cmd": Some(self.command.trim()).filter(|c| !c.is_empty())? }),
            Choice::Ollama => json!({ "name": format!("Ollama {model}"), "model": Some(model).filter(|m| !m.is_empty())?, "url": ai::OLLAMA }),
            Choice::Api => {
                let (provider, url, _) = PROVIDERS[self.provider];
                let url = if url.is_empty() { self.url.trim() } else { url };
                let mut entry = json!({ "name": format!("{provider} {model}"), "model": Some(model).filter(|m| !m.is_empty())?, "url": Some(url).filter(|u| !u.is_empty())? });
                // "$NAME" reads the key from that environment variable, so it never lands in the file.
                match self.key.trim() {
                    "" => {}
                    key if key.starts_with('$') => entry["key_env"] = key[1..].into(),
                    key => entry["key"] = key.into(),
                }
                entry
            }
        })
    }
}

impl App {
    /// The setup page. Saving adds the model, writes `config.json`, and closes the page.
    pub(super) fn setup_page(&mut self, ui: &mut egui::Ui) {
        let Some(setup) = &mut self.setup else { return };
        let full = ui.available_rect_before_wrap();
        ui.painter().rect_filled(full, 0.0, ui.visuals().panel_fill);
        let column = Rect::from_center_size(full.center(), Vec2::new(full.width().min(560.0), full.height() - 32.0));
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(column));
        if let Ok(models) = setup.ollama.try_recv() {
            setup.ollama_models = Some(models);
        }
        ui.heading(if setup.first { "Welcome to Mind-Me" } else { "Add a model" });
        ui.label(hint(ui, "Mind-Me asks an AI for its suggestions and notes. Pick the one it should talk to. You can add more later in Settings, Models."));
        ui.add_space(12.0);
        let found = |yes: bool| if yes { "  · found" } else { "" };
        let ollama = setup.ollama_models.is_some();
        for (choice, label, about) in [
            (Choice::Claude, format!("Claude Pro/Max{}", found(setup.claude_found)), "Uses the claude command you are logged in to. Web search is on."),
            (Choice::Codex, format!("OpenAI Codex{}", found(setup.codex_found)), "Uses the codex command you are logged in to, with ChatGPT or an API key. Codex can read files on this computer."),
            (Choice::Api, "An API key".into(), "Anthropic, OpenAI, OpenRouter, or any OpenAI-style URL."),
            (Choice::Ollama, format!("Ollama{}", found(ollama)), "A model running on this computer. Free, no key."),
            (Choice::Command, "Another command".into(), "Any command that reads a prompt and prints a reply, such as llm."),
        ]
        .into_iter()
        // A web page can call an API, but cannot run a command.
        .filter(|(choice, ..)| !cfg!(target_arch = "wasm32") || matches!(choice, Choice::Api | Choice::Ollama))
        {
            if ui.radio_value(&mut setup.choice, choice, label).changed() {
                setup.tested = None;
            }
            ui.label(hint(ui, about));
        }
        ui.add_space(12.0);
        match setup.choice {
            Choice::Claude if !setup.claude_found => _ = ui.label(egui::RichText::new("The claude command is not on your PATH. Install Claude Code and log in first.").color(palette(ui).fail)),
            Choice::Claude => _ = ui.label(hint(ui, CLAUDE)),
            Choice::Codex if !setup.codex_found => _ = ui.label(egui::RichText::new("The codex command is not on your PATH. Install Codex and run codex login first.").color(palette(ui).fail)),
            Choice::Codex => _ = ui.label(hint(ui, CODEX)),
            Choice::Api => {
                egui::ComboBox::from_label("Provider").selected_text(PROVIDERS[setup.provider].0).show_index(ui, &mut setup.provider, PROVIDERS.len(), |i| PROVIDERS[i].0);
                if PROVIDERS[setup.provider].1.is_empty() {
                    ui.add(TextEdit::singleline(&mut setup.url).hint_text("https://…/v1/chat/completions").desired_width(f32::INFINITY));
                }
                ui.add(TextEdit::singleline(&mut setup.model).hint_text(PROVIDERS[setup.provider].2).desired_width(f32::INFINITY));
                let hidden = !setup.key.starts_with('$');
                ui.add(TextEdit::singleline(&mut setup.key).hint_text("API key, or $NAME of an environment variable that holds it").password(hidden).desired_width(f32::INFINITY));
                ui.label(hint(ui, if cfg!(target_arch = "wasm32") {
                    "A pasted key stays in this browser's storage and goes only to the provider."
                } else {
                    "A pasted key is saved in config.json, readable by you only."
                }));
            }
            Choice::Ollama => match &setup.ollama_models {
                Some(models) if !models.is_empty() => {
                    let shown = if setup.model.is_empty() { "Pick a model".to_owned() } else { setup.model.clone() };
                    egui::ComboBox::from_label("Model").selected_text(shown).show_ui(ui, |ui| {
                        for m in models {
                            ui.selectable_value(&mut setup.model, m.clone(), m.as_str());
                        }
                    });
                }
                _ => {
                    ui.add(TextEdit::singleline(&mut setup.model).hint_text("Model name, such as llama3.2").desired_width(f32::INFINITY));
                    ui.label(hint(ui, if cfg!(target_arch = "wasm32") {
                        "A web page can reach Ollama only when it is started with OLLAMA_ORIGINS set to this site's address."
                    } else {
                        "Ollama is not answering on localhost:11434. Start it, or type a model name anyway."
                    }));
                }
            },
            Choice::Command => _ = ui.add(TextEdit::singleline(&mut setup.command).hint_text("llm -m gpt-5").desired_width(f32::INFINITY)),
        }
        ui.add_space(12.0);
        let entry = setup.entry();
        let model = entry.as_ref().map(ai::Model::parse);
        if let Some(rx) = &setup.test {
            if let Ok((_, reply)) = rx.try_recv() {
                setup.tested = Some(reply.map(|_| ()));
                setup.test = None;
            } else {
                waiting(ui, "model test", "Testing…");
            }
        }
        match &setup.tested {
            Some(Ok(())) => _ = ui.label(egui::RichText::new("It answered.").color(palette(ui).trail)),
            Some(Err(e)) => _ = ui.label(egui::RichText::new(format!("The test failed: {e}")).color(palette(ui).fail)),
            None => {}
        }
        let (mut saved, mut cancel) = (None, false);
        ui.horizontal(|ui| {
            let ready = matches!(model, Some(Ok(_)));
            if ui.add_enabled(ready && setup.test.is_none(), egui::Button::new("Test")).on_hover_text("Sends \"Reply with OK\"").clicked()
                && let Some(Ok(model)) = &model
            {
                let (tx, rx) = mpsc::channel();
                model.spawn("Reply with OK".into(), Duration::from_secs(30), ui.ctx(), &tx, Arc::new(AtomicBool::new(false)), |reply| reply);
                (setup.test, setup.tested) = (Some(rx), None);
            }
            let untested = setup.tested.as_ref().is_none_or(|t| t.is_err());
            let save = ui.add_enabled(ready, egui::Button::new("Save"));
            if save.on_hover_text(if untested { "Saves it even though no test passed" } else { "Saves it to config.json" }).clicked() {
                saved = model.clone().and_then(Result::ok);
            }
            cancel = !setup.first && ui.button("Cancel").clicked();
        });
        if cancel {
            self.setup = None;
            return;
        }
        if let Some(error) = self.setup.as_ref().and_then(|s| s.error.clone()) {
            ui.label(egui::RichText::new(error).color(palette(ui).fail));
        }
        if let Some(model) = saved {
            self.models.push(model);
            // The playground's tab moves off its written ideas as soon as the visitor's own AI is added.
            if cfg!(target_arch = "wasm32") {
                self.docs.iter_mut().for_each(|d| d.model = self.models.len() - 1);
            }
            match self.save_models() {
                Err(e) => self.setup.as_mut().unwrap().error = Some(format!("Added for this session, but config.json could not be written: {e}")),
                Ok(()) => self.setup = None,
            }
        }
    }

    /// Where `config.json` is: `MINDME_CONFIG`, else the config folder. Tests pass a temporary folder and never
    /// read `MINDME_CONFIG`, so they cannot touch the user's file.
    fn config_json(&self) -> Option<std::path::PathBuf> {
        let env = std::env::var_os("MINDME_CONFIG").filter(|_| !cfg!(test)).map(Into::into);
        env.or_else(|| self.dir.as_ref().map(|d| d.join("config.json")))
    }

    /// Writes the models and the suggestions model to `config.json`. Without a config folder there is nothing to write.
    pub(super) fn save_models(&self) -> std::io::Result<()> {
        #[cfg(target_arch = "wasm32")]
        return crate::web::save_models(&self.models);
        self.config_json().map_or(Ok(()), |path| ai::save_models(&path, &self.models, self.ghosts))
    }

    /// Settings' Models tab: rename, pick the suggestions model, remove after a yes, or add one.
    pub(super) fn models_tab(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        egui::Grid::new("models").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
            for (i, m) in self.models.iter_mut().enumerate() {
                if m.entry.is_some() {
                    // A fixed size: inside a grid, a text box would only take the column's width from the last frame.
                    changed |= ui.add_sized([240.0, 24.0], TextEdit::singleline(&mut m.name).hint_text("Name")).on_hover_text(m.command()).lost_focus();
                } else {
                    ui.label(&m.name).on_hover_text(format!("{}\nFrom MINDME_CMD or MINDME_MODEL, so it is changed there, not here.", m.command()));
                }
                let suggests = self.ghosts == Some(i);
                if ui.radio(suggests, "Suggestions").on_hover_text("This model answers the suggestions in every tab").clicked() {
                    self.ghosts = if suggests { None } else { Some(i) };
                    changed = true;
                }
                if m.entry.is_some() && ui.small_button("Remove").clicked() {
                    self.removing = Some(i);
                }
                ui.end_row();
            }
        });
        ui.label(hint(ui, "New tabs use the first model. With no Suggestions mark, each tab's own model answers its suggestions."));
        if let Some(i) = self.removing {
            ui.horizontal(|ui| {
                ui.label(format!("Remove \"{}\" from config.json?", self.models[i].name));
                if ui.button("Remove").clicked() {
                    self.models.remove(i);
                    self.ghosts = self.ghosts.filter(|g| *g != i).map(|g| if g > i { g - 1 } else { g });
                    for doc in &mut self.docs {
                        doc.model = if doc.model == i { 0 } else if doc.model > i { doc.model - 1 } else { doc.model };
                    }
                    (self.removing, changed) = (None, true);
                }
                if ui.button("Cancel").clicked() {
                    self.removing = None;
                }
            });
        }
        ui.add_space(8.0);
        if ui.button("Add a model…").clicked() {
            self.setup = Some(Setup::new(false));
        }
        if changed && let Err(e) = self.save_models() {
            self.open_error = Some(format!("config.json could not be written: {e}"));
        }
    }
}
