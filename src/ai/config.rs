use super::{Backend, Model, Price};
use serde_json::Value;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

pub const OLLAMA: &str = "http://localhost:11434/v1/chat/completions";

pub struct Config {
    pub models: Vec<Model>,
    pub footer: Option<Vec<String>>,
    pub skills: Vec<Skill>,
    /// Index into `models` for ghost requests. Prose requests use the tab's picked model.
    pub ghosts: Option<usize>,
}

/// A writing guide put ahead of every request whose reply is prose, while `on`.
#[derive(Clone)]
pub struct Skill {
    pub name: String,
    pub text: String,
    pub on: bool,
}

impl Skill {
    /// `{"name", "file", "on"?}`. Reads the file once, here, and `~/` starts at the home folder.
    pub(super) fn parse(entry: &Value) -> Result<Skill, String> {
        let file = entry["file"].as_str().ok_or("a skill needs \"file\"")?;
        let path = match (file.strip_prefix("~/"), env::var_os("HOME")) {
            (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
            _ => PathBuf::from(file),
        };
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Skill { name: entry["name"].as_str().unwrap_or(file).into(), text, on: entry["on"].as_bool().unwrap_or(true) })
    }
}

pub fn config() -> Result<Config, String> {
    let mut models: Vec<Model> = Backend::from_env().map(Model::new).into_iter().collect();
    let (mut footer, mut skills, mut ghosts) = (None, Vec::new(), None);
    if let Some(path) = config_path() {
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let config: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        for entry in config["models"].as_array().into_iter().flatten() {
            models.push(Model::parse(entry).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        for entry in config["skills"].as_array().into_iter().flatten() {
            skills.push(Skill::parse(entry).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        ghosts = ghost_model(&models, &config).map_err(|e| format!("{}: {e}", path.display()))?;
        footer = config["footer"].as_array().map(|f| f.iter().filter_map(|v| v.as_str().map(String::from)).collect());
    }
    // No model is not an error: the app opens on its setup screen.
    Ok(Config { models, footer, skills, ghosts })
}

/// `"ghosts": "<model name>"`, resolved to its index once, so a typo stops the app instead of failing later.
pub(super) fn ghost_model(models: &[Model], config: &Value) -> Result<Option<usize>, String> {
    let Some(name) = config["ghosts"].as_str() else { return Ok(None) };
    models.iter().position(|m| m.name == name).map(Some).ok_or_else(|| format!("\"ghosts\" names {name:?}, which is not a listed model"))
}

/// Writes the models that came from `config.json` back to it, each under its current name, and `"ghosts"`, keeping every
/// other key. Through a `.tmp` file and a rename, as maps are saved. Readable by the user only, since it can hold API keys.
pub fn save_models(path: &Path, models: &[Model], ghosts: Option<usize>) -> io::Result<()> {
    let mut config: Value = fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
    config["models"] = entries(models).into();
    match ghosts.and_then(|i| models.get(i)) {
        Some(m) => config["ghosts"] = m.name.clone().into(),
        None => _ = config.as_object_mut().map(|c| c.remove("ghosts")),
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&config)?)?;
    #[cfg(unix)]
    fs::set_permissions(&tmp, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    fs::rename(&tmp, path)
}

/// The models that came from a config, each under its current name.
pub fn entries(models: &[Model]) -> Vec<Value> {
    models
        .iter()
        .filter_map(|m| {
            let mut entry = m.entry.clone()?;
            entry["name"] = m.name.clone().into();
            Some(entry)
        })
        .collect()
}

fn config_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("MINDME_CONFIG") {
        return Some(path.into());
    }
    crate::config_dir().map(|d| d.join("config.json")).filter(|p| p.exists())
}

impl Model {
    /// `{"name", "cmd"}` or `{"name", "model", "url"?, "key"?, "key_env"?}`, with an optional `"price": {"in", "out", "cached"}`.
    pub fn parse(entry: &Value) -> Result<Model, String> {
        let text = |key: &str| entry[key].as_str().map(String::from);
        let backend = if let Some(cmd) = text("cmd") {
            Backend::cli(&cmd)?
        } else {
            let model = text("model").ok_or("a model needs \"cmd\" or \"model\"")?;
            let url = text("url").unwrap_or_else(|| OLLAMA.into());
            let key = text("key").or_else(|| text("key_env").and_then(|k| env::var(k).ok()));
            Backend::Http { url, model, key }
        };
        let p = &entry["price"];
        let price = Price { input: p["in"].as_f64().unwrap_or(0.0), output: p["out"].as_f64().unwrap_or(0.0), cached: p["cached"].as_f64() };
        Ok(Model { name: text("name").unwrap_or_else(|| backend.name()), backend, price, entry: Some(entry.clone()) })
    }
}

impl Backend {
    pub fn from_env() -> Result<Backend, String> {
        if let Ok(cmd) = env::var("MINDME_CMD") {
            return Backend::cli(&cmd);
        }
        let model = env::var("MINDME_MODEL")
            .map_err(|_| "set MINDME_MODEL (MINDME_URL defaults to local Ollama), or MINDME_CMD")?;
        let url = env::var("MINDME_URL").unwrap_or_else(|_| OLLAMA.into());
        Ok(Backend::Http { url, model, key: env::var("MINDME_KEY").ok() })
    }

    /// Split on whitespace, never passed to a shell.
    pub(super) fn cli(cmd: &str) -> Result<Backend, String> {
        let mut words = cmd.split_whitespace().map(String::from);
        let command = words.next().ok_or("the command is empty")?;
        Ok(Backend::Cli { command, args: words.collect() })
    }
}
