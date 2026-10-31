//! AI backends. Each request runs on its own thread and returns over a channel with a repaint.
//! Nothing here knows the map's rules: it runs a prompt and parses the raw reply as ghosts or a whole map.

use eframe::egui;
use serde_json::Value;
use std::{
    io::{Read, Write},
    ops::AddAssign,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

/// For one node's ghosts and note.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(60);
/// For mapping out a topic or node, or an in-depth note, which can include reading documentation online.
pub const LONG_TIMEOUT: Duration = Duration::from_secs(600);

mod config;
mod parse;

pub use config::{config, save_models, Config, Skill, OLLAMA};
#[cfg(target_arch = "wasm32")]
pub use config::entries;
pub use parse::{parse_markdown, parse_outline, parse_question, parse_reply, Next};


#[derive(Clone)]
pub struct Model {
    pub name: String,
    backend: Backend,
    price: Price,
    /// Its entry in `config.json`, written back when models are edited. `None` for one from the environment.
    pub entry: Option<serde_json::Value>,
}

/// Dollars per million tokens. Cached input costs `input` unless `cached` says otherwise.
#[derive(Clone, Copy, Default)]
struct Price {
    input: f64,
    output: f64,
    cached: Option<f64>,
}

/// `input` includes `cached`.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
    pub cost: f64,
    /// As the reply names it, which a CLI command alone does not tell.
    pub model: Option<String>,
}

impl AddAssign for Usage {
    /// The latest reply's model wins.
    fn add_assign(&mut self, o: Usage) {
        (self.input, self.output, self.cached, self.cost) = (self.input + o.input, self.output + o.output, self.cached + o.cached, self.cost + o.cost);
        if o.model.is_some() {
            self.model = o.model;
        }
    }
}

impl Price {
    /// A reported `cost` wins over the price list.
    fn usage(&self, input: u64, output: u64, cached: u64, cost: Option<f64>) -> Usage {
        let cached_price = self.cached.unwrap_or(self.input);
        let priced = ((input - cached.min(input)) as f64 * self.input + cached as f64 * cached_price + output as f64 * self.output) / 1e6;
        Usage { input, output, cached, cost: cost.unwrap_or(priced), model: None }
    }
}

impl Model {
    /// No price: the cost shows only when the backend reports it.
    pub fn new(backend: Backend) -> Model {
        Model { name: backend.name(), backend, price: Price::default(), entry: None }
    }


    /// The whole command or endpoint behind the short name.
    pub fn command(&self) -> String {
        self.backend.command()
    }

    /// shortcut: a CLI counts when its arguments name WebSearch or WebFetch, even in a deny list. Read the CLI's own config if that misleads.
    pub fn browses(&self) -> bool {
        match &self.backend {
            Backend::Http { .. } => false,
            Backend::Cli { args, .. } => args.iter().any(|a| a.contains("WebSearch") || a.contains("WebFetch")),
        }
    }

    /// Returns at once. The request runs on a new thread, and `reply` converts the raw answer or error to a
    /// message, sent with the tokens it spent. Setting `stop` kills a CLI request.
    pub fn spawn<M: Send + 'static>(
        &self,
        prompt: String,
        timeout: Duration,
        ctx: &egui::Context,
        tx: &Sender<(Usage, M)>,
        stop: Arc<AtomicBool>,
        reply: impl FnOnce(Result<String, String>) -> M + Send + 'static,
    ) {
        // The browser has no threads or processes: the page fetches from an API, and the playground's own model
        // answers at once from its written ideas.
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (timeout, stop);
            let (ctx, tx, price) = (ctx.clone(), tx.clone(), self.price);
            let done = move |answer: Result<(String, Usage), String>| {
                let (result, usage) = match answer {
                    Ok((text, usage)) => (Ok(text), usage),
                    Err(e) => (Err(e), Usage::default()),
                };
                let _ = tx.send((usage, reply(result)));
                ctx.request_repaint();
            };
            match &self.backend {
                Backend::Http { url, model, key } => crate::web::post(url, key.as_deref(), body(model, &prompt).to_string(), move |raw| {
                    done(raw.and_then(|text| serde_json::from_str(&text).map_err(|e| e.to_string())).and_then(|reply| chat_reply(&reply, &price)))
                }),
                Backend::Cli { .. } => done(crate::web::reply(&prompt).map(|text| (text, Usage::default()))),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let (model, ctx, tx) = (self.clone(), ctx.clone(), tx.clone());
        #[cfg(not(target_arch = "wasm32"))]
        thread::spawn(move || {
            let (result, usage) = match model.backend.run(&prompt, timeout, &stop, &model.price) {
                Ok((text, usage)) => (Ok(text), usage),
                Err(e) => (Err(e), Usage::default()),
            };
            // Fails only after the window closes.
            let _ = tx.send((usage, reply(result)));
            // egui draws only on input or a repaint request, so without it the reply sits unseen until the mouse moves.
            ctx.request_repaint();
        });
    }
}

#[derive(Clone)]
pub enum Backend {
    /// OpenAI-style chat completions: Ollama, LM Studio, llama.cpp's server, most cloud APIs.
    Http { url: String, model: String, key: Option<String> },
    /// `claude -p`, `ollama run <model>`, `llm`.
    Cli { command: String, args: Vec<String> },
}

impl Backend {

    /// A short label for the footer and pickers, such as "Claude Pro/Max" or "OpenAI gpt-5". `command` has the rest.
    /// shortcut: two unnamed models of one provider share a label, give them a "name" in config.json to tell them apart
    fn name(&self) -> String {
        match self {
            Backend::Http { url, model, .. } => {
                let providers = [("openai.com", "OpenAI"), ("anthropic.com", "Anthropic"), ("openrouter.ai", "OpenRouter"), ("googleapis.com", "Gemini"), ("groq.com", "Groq"), (":11434", "Ollama"), (":1234", "LM Studio")];
                match providers.iter().find(|(host, _)| url.contains(host)) {
                    Some((_, provider)) => format!("{provider} {model}"),
                    None => model.clone(),
                }
            }
            Backend::Cli { command, args } => {
                let program = std::path::Path::new(command).file_name().map_or(command.as_str(), |f| f.to_str().unwrap_or_default());
                // `claude -p` bills an API key when one is set, else the Pro or Max subscription it is logged in to.
                match program {
                    "claude" if std::env::var_os("ANTHROPIC_API_KEY").is_some() => "Claude API".into(),
                    "claude" => "Claude Pro/Max".into(),
                    "codex" => "OpenAI Codex".into(),
                    "gemini" => "Gemini CLI".into(),
                    "ollama" => args.iter().find(|a| !a.starts_with('-') && *a != "run").map_or("Ollama".into(), |m| format!("Ollama {m}")),
                    other => other.to_owned(),
                }
            }
        }
    }

    /// The whole command or endpoint, for hover text.
    fn command(&self) -> String {
        match self {
            Backend::Http { url, model, .. } => format!("{model} at {url}"),
            Backend::Cli { command, args } => format!("{command} {}", args.join(" ")),
        }
    }

    /// shortcut: an HTTP request runs to the end after a stop and its reply is dropped, use an agent with a cancel hook if that costs too much.
    fn run(&self, prompt: &str, timeout: Duration, stop: &AtomicBool, price: &Price) -> Result<(String, Usage), String> {
        match self {
            Backend::Http { url, model, key } => http(url, model, key.as_deref(), prompt, timeout).map(|reply| chat_reply(&reply, price))?,
            Backend::Cli { command, args } => cli(command, args, prompt, timeout, stop).map(|out| cli_reply(out, price))?,
        }
    }
}

fn http(url: &str, model: &str, key: Option<&str>, prompt: &str, timeout: Duration) -> Result<Value, String> {
    let agent = ureq::Agent::config_builder().timeout_global(Some(timeout)).build().new_agent();
    let mut request = agent.post(url);
    if let Some(key) = key {
        request = request.header("Authorization", &format!("Bearer {key}"));
    }
    request
        .send_json(body(model, prompt))
        .and_then(|mut response| response.body_mut().read_json())
        .map_err(|e| e.to_string())
}

fn body(model: &str, prompt: &str) -> Value {
    serde_json::json!({ "model": model, "messages": [{ "role": "user", "content": prompt }] })
}

fn chat_reply(reply: &Value, price: &Price) -> Result<(String, Usage), String> {
    let text = reply["choices"][0]["message"]["content"].as_str().ok_or("reply has no message content")?;
    let count = |v: &Value| v.as_u64().unwrap_or(0);
    let u = &reply["usage"];
    let usage = price.usage(count(&u["prompt_tokens"]), count(&u["completion_tokens"]), count(&u["prompt_tokens_details"]["cached_tokens"]), None);
    Ok((text.to_owned(), Usage { model: reply["model"].as_str().map(String::from), ..usage }))
}

fn cli_reply(out: String, price: &Price) -> Result<(String, Usage), String> {
    let Ok(json) = serde_json::from_str::<Value>(&out) else { return Ok((out, Usage::default())) };
    let Some(result) = json["result"].as_str() else { return Ok((out, Usage::default())) };
    if json["is_error"].as_bool() == Some(true) {
        return Err(result.to_owned());
    }
    let count = |key: &str| json["usage"][key].as_u64().unwrap_or(0);
    let cached = count("cache_read_input_tokens");
    let input = count("input_tokens") + count("cache_creation_input_tokens") + cached;
    let usage = price.usage(input, count("output_tokens"), cached, json["total_cost_usd"].as_f64());
    // `modelUsage` lists every model the CLI ran, small helpers included. The one that cost most answered.
    let cost = |v: &Value| v["costUSD"].as_f64().unwrap_or(0.0);
    let model = json["modelUsage"].as_object().and_then(|m| m.iter().max_by(|a, b| cost(a.1).total_cmp(&cost(b.1)))).map(|(name, _)| name.clone());
    Ok((result.to_owned(), Usage { model, ..usage }))
}

fn cli(command: &str, args: &[String], prompt: &str, timeout: Duration, stop: &AtomicBool) -> Result<String, String> {
    let mut child = Command::new(command);
    // A window app's child opens a console window of its own on Windows unless told not to.
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut child, 0x0800_0000); // CREATE_NO_WINDOW
    let mut child = child
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{command}: {e}"))?;
    // Started before writing, so a chatty child never blocks on a full pipe.
    let out = drain(child.stdout.take().unwrap());
    let err = drain(child.stderr.take().unwrap());
    // Dropping stdin closes it, which matters: `claude -p`, `ollama run` and `llm` read until end of input.
    // A command that never reads its input, such as `echo`, can exit before the write ends. Its output still counts.
    if let Err(e) = child.stdin.take().unwrap().write_all(prompt.as_bytes())
        && e.kind() != std::io::ErrorKind::BrokenPipe
    {
        return Err(format!("{command}: {e}"));
    }
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if Instant::now() > deadline || stop.load(Ordering::Relaxed) => {
                let _ = child.kill();
                let _ = child.wait();
                // The drains are not joined: a grandchild can hold the pipe open after the kill.
                return Err(if stop.load(Ordering::Relaxed) { "stopped" } else { "timed out" }.into());
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    };
    let out = out.join().unwrap_or_default();
    if !status.success() {
        let err = err.join().unwrap_or_default();
        return Err(if err.trim().is_empty() { status.to_string() } else { err.trim().to_owned() });
    }
    Ok(out)
}

fn drain(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut text = String::new();
        let _ = pipe.read_to_string(&mut text);
        text
    })
}

#[cfg(test)]
mod tests;
