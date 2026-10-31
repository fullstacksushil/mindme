//! Real material for a node, at no token cost: search links, a Wikipedia card, and whether the links in its
//! note still answer. The card holds the article's first sentences and thumbnail.

use eframe::egui;
use std::{collections::HashMap, sync::mpsc};

mod fetch;
mod images;
mod topic;

use fetch::{best, check, fetch, fetch_article, fetch_images};
pub use images::WebImages;
pub use topic::{Lookup, Topic};


/// One Wikipedia search hit for a node.
#[derive(Clone, Debug)]
pub struct Card {
    pub title: String,
    pub extract: String,
    pub url: String,
    /// Kept only once its bytes are in egui, so the inspector never draws a broken image.
    pub thumb: Option<String>,
}

/// The path's last two names, so "Rust > Ownership" searches for Rust's ownership and not the word alone.
pub fn query(path: &[&str]) -> String {
    path[path.len().saturating_sub(2)..].join(" ")
}

pub fn wikipedia_url(text: &str) -> String {
    format!("https://en.wikipedia.org/wiki/Special:Search?search={}", encode(text))
}

pub fn youtube_url(query: &str) -> String {
    format!("https://www.youtube.com/results?search_query={}", encode(query))
}

/// Percent-encodes everything outside RFC 3986's unreserved characters, UTF-8 byte by byte.
pub fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}


/// Fetched values by key, each fetched once per session per tab on its own thread.
pub struct Cache<T> {
    pub got: HashMap<String, T>,
    tx: mpsc::Sender<(String, T)>,
    rx: mpsc::Receiver<(String, T)>,
}

impl<T> Default for Cache<T> {
    fn default() -> Cache<T> {
        let (tx, rx) = mpsc::channel();
        Cache { got: HashMap::new(), tx, rx }
    }
}

impl<T: Send + 'static> Cache<T> {
    /// The value for `key`, `pending` until `fetch` returns on its thread.
    fn get(&mut self, key: &str, ctx: &egui::Context, pending: T, fetch: fn(&str, &egui::Context) -> T) -> &T {
        self.got.extend(self.rx.try_iter());
        self.got.entry(key.to_owned()).or_insert_with(|| {
            let (key, ctx, tx) = (key.to_owned(), ctx.clone(), self.tx.clone());
            std::thread::spawn(move || {
                let value = fetch(&key, &ctx);
                let _ = tx.send((key, value));
                ctx.request_repaint();
            });
            pending
        })
    }
}

/// Search hits by query. An `Err` is the quiet line shown instead of a card: still looking, or nothing found.
pub type Wiki = Cache<Result<Vec<Card>, &'static str>>;

impl Wiki {
    /// The hit for `query` that best fits `context`. Kept per query, not per note, so editing a note never refetches.
    pub fn card(&mut self, path: &[&str], context: &str, ctx: &egui::Context) -> Result<&Card, &'static str> {
        let cards = self.get(&query(path), ctx, Err("Looking up Wikipedia…"), fetch).as_ref().map_err(|e| *e)?;
        best(cards, path.last().copied().unwrap_or_default(), context).ok_or("No Wikipedia article found.")
    }
}


/// Pictures for an article, by title, already loaded into egui. Empty while on the way and when the search found none.
pub type Images = Cache<Vec<String>>;

impl Images {
    pub fn images(&mut self, title: &str, ctx: &egui::Context) -> &[String] {
        self.get(title, ctx, Vec::new(), fetch_images)
    }
}


/// Whole articles as Markdown, by title.
pub type Articles = Cache<Result<String, &'static str>>;

impl Articles {
    pub fn text(&mut self, title: &str, ctx: &egui::Context) -> &Result<String, &'static str> {
        self.get(title, ctx, Err("Loading the article…"), fetch_article)
    }
}


/// Whether a source link answers. Reachable says nothing about whether the page backs the note.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Checking,
    Ok,
    Dead(String),
}

pub type Links = Cache<Link>;

impl Links {
    pub fn link(&mut self, url: &str, ctx: &egui::Context) -> &Link {
        self.get(url, ctx, Link::Checking, check)
    }
}

/// The http(s) links in a note, each once, in order: Markdown `[title](url)`, `<url>` and bare URLs.
pub fn links(note: &str) -> Vec<&str> {
    let mut found = Vec::new();
    for (i, _) in note.match_indices("http") {
        let tail = &note[i..];
        if !(tail.starts_with("https://") || tail.starts_with("http://")) {
            continue;
        }
        // Balanced parentheses stay in the URL, so `(.../Rust_(language))` keeps its last `)` only.
        let mut depth = 0;
        let end = tail
            .find(|c: char| match c {
                '(' => {
                    depth += 1;
                    false
                }
                ')' if depth == 0 => true,
                ')' => {
                    depth -= 1;
                    false
                }
                c => c.is_whitespace() || "<>[]\"'`".contains(c),
            })
            .unwrap_or(tail.len());
        let url = tail[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if !found.contains(&url) {
            found.push(url);
        }
    }
    found
}

#[cfg(test)]
mod tests;
