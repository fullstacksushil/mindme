//! The browser playground at site/try/, built by `scripts/web.sh`. It opens an example map about Atomic Habits,
//! suggests from written ideas, and uses the visitor's own AI once they add one in Settings: Anthropic, OpenAI,
//! OpenRouter or Ollama, called straight from the page. Maps and models live in the browser's storage. The browser
//! has no files, threads, processes or system fonts, so the rest of the app marks what it skips there with
//! `target_arch = "wasm32"`.

mod sample;

#[cfg(target_arch = "wasm32")]
pub(crate) use browser::{post, save_models, write};
#[cfg(target_arch = "wasm32")]
pub(crate) use sample::reply;
#[cfg(target_arch = "wasm32")]
pub use browser::start;

/// The map the playground opens until the visitor changes it. site/try/ resets it by removing `map:/Atomic Habits.md`.
const EXAMPLE: &str = include_str!("atomic-habits.md");

#[cfg(target_arch = "wasm32")]
mod browser {
    use crate::{ai, app::{App, Doc}, map::Map};
    use eframe::{wasm_bindgen::JsCast, web_sys};
    use std::{io, path::{Path, PathBuf}};

    /// One tab with the example, or the visitor's edits to it, on the page's `mindme` canvas.
    pub fn start() {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("mindme"))
            .and_then(|c| c.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("the page has a <canvas id=\"mindme\">");
        let path = PathBuf::from("/Atomic Habits.md");
        let md = storage().and_then(|s| s.get_item(&key(&path)).ok()?).unwrap_or_else(|| super::EXAMPLE.into());
        let mut doc = Doc::new(path, Map::parse(&md).or_else(|| Map::parse(super::EXAMPLE)));
        doc.unfold(2);
        let mut written = ai::Model::new(ai::Backend::Cli { command: "playground".into(), args: Vec::new() });
        written.name = "Written ideas".into();
        // The visitor's own models come first, so the tab uses theirs.
        let models = models().into_iter().chain([written]).collect();
        // Only the model picker: the playground's written ideas cost nothing.
        let config = ai::Config { models, footer: Some(vec!["model".into()]), skills: Vec::new(), ghosts: None };
        wasm_bindgen_futures::spawn_local(async move {
            let started = eframe::WebRunner::new()
                .start(canvas, eframe::WebOptions::default(), Box::new(|cc| {
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    Ok(Box::new(App::new(config, vec![doc], None)))
                }))
                .await;
            started.expect("the playground starts");
        });
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    fn key(path: &Path) -> String {
        format!("map:{}", path.display())
    }

    /// Saves a map in the browser's storage, where `start` finds it on the next visit.
    pub(crate) fn write(path: &Path, md: &str) -> io::Result<()> {
        let storage = storage().ok_or_else(|| io::Error::other("this browser keeps no storage for the page"))?;
        storage.set_item(&key(path), md).map_err(|_| io::Error::other("the browser's storage for this page is full"))
    }

    /// The visitor's models, kept in the browser's storage as `config.json` keeps them on the desktop.
    fn models() -> Vec<ai::Model> {
        let config = storage().and_then(|s| s.get_item("config.json").ok()?).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        config.iter().flat_map(|c| c["models"].as_array().into_iter().flatten()).filter_map(|m| ai::Model::parse(m).ok()).collect()
    }

    pub(crate) fn save_models(models: &[ai::Model]) -> io::Result<()> {
        let storage = storage().ok_or_else(|| io::Error::other("this browser keeps no storage for the page"))?;
        storage.set_item("config.json", &serde_json::json!({ "models": ai::entries(models) }).to_string()).map_err(|_| io::Error::other("the browser's storage for this page is full"))
    }

    /// POSTs `body` to an OpenAI-style endpoint from the page, and hands the reply's text to `done`.
    pub(crate) fn post(url: &str, key: Option<&str>, body: String, done: impl FnOnce(Result<String, String>) + Send + 'static) {
        let mut request = ehttp::Request::post(url, body.into_bytes());
        request.headers.insert("Content-Type", "application/json");
        if let Some(key) = key {
            request.headers.insert("Authorization", format!("Bearer {key}"));
        }
        // Anthropic answers a web page only when the page says it knows the key is exposed to it.
        if url.contains("anthropic.com") {
            request.headers.insert("anthropic-dangerous-direct-browser-access", "true");
        }
        ehttp::fetch(request, move |response| {
            done(match response {
                Ok(r) if r.ok => r.text().map(String::from).ok_or_else(|| "the reply was not text".into()),
                Ok(r) => Err(format!("{} {}: {}", r.status, r.status_text, r.text().unwrap_or_default())),
                // The browser hides why a request failed. Usually the provider does not accept calls from a page.
                Err(e) => Err(format!("{e}. The provider may not accept calls from a web page. Ollama needs OLLAMA_ORIGINS set to this site.")),
            })
        });
    }
}
