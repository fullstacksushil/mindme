use super::{encode, Card, Link};
use eframe::egui;
use serde_json::Value;
use std::time::Duration;

/// Wikipedia asks every client to name itself.
pub(super) const AGENT: &str = concat!("mindme/", env!("CARGO_PKG_VERSION"), " (desktop mind map)");

/// The pages of a `generator=search` query with `formatversion=2`, in search order. Empty when the search found nothing.
pub fn parse(json: &Value) -> Vec<Card> {
    let mut pages: Vec<&Value> = json["query"]["pages"].as_array().into_iter().flatten().collect();
    pages.sort_by_key(|page| page["index"].as_u64());
    let text = |v: &Value| v.as_str().map(String::from);
    pages
        .into_iter()
        .filter_map(|page| Some(Card { title: text(&page["title"])?, extract: text(&page["extract"]).unwrap_or_default(), url: text(&page["fullurl"])?, thumb: text(&page["thumbnail"]["source"]) }))
        .collect()
}

/// Of the hits that name every word of `own` (the node's text), the one that shares the most words with `context`
/// (its path and note). So a note about the language picks "Rust (programming language)" over the top hit, rust the oxide.
/// Ties keep search order. Search ranks made-up names like "Landing page" against unrelated pages, so those fit nothing.
pub fn best<'a>(cards: &'a [Card], own: &str, context: &str) -> Option<&'a Card> {
    let words = |s: &str| s.split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 3).map(str::to_lowercase).collect::<std::collections::HashSet<_>>();
    let (own, context) = (words(own), words(context));
    let fits = cards.iter().map(|card| (card, words(&format!("{} {}", card.title, card.extract)))).filter(|(_, hit)| own.is_subset(hit));
    fits.rev().max_by_key(|(_, hit)| hit.intersection(&context).count()).map(|(card, _)| card)
}

/// Most pictures kept for one article.
const PICTURES: usize = 8;

/// The JPEG and PNG thumbnails in an `imageinfo` reply. SVGs are mostly icons and logos, so this skips them.
pub fn bitmaps(json: &Value) -> Vec<String> {
    let pages = json["query"]["pages"].as_array().into_iter().flatten();
    pages
        .map(|page| &page["imageinfo"][0])
        .filter(|info| matches!(info["mime"].as_str(), Some("image/jpeg" | "image/png")))
        .filter_map(|info| info["thumburl"].as_str().map(String::from))
        .collect()
}

/// The article's own pictures first, then a Wikimedia Commons search for its title, up to `PICTURES`.
/// shortcut: keeps the API's alphabetical order, not the article's, and downloads them serially before any shows, rank and stream them if that feels slow.
pub(super) fn fetch_images(title: &str, ctx: &egui::Context) -> Vec<String> {
    if cfg!(test) || cfg!(target_arch = "wasm32") {
        return Vec::new();
    }
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).user_agent(AGENT).build().new_agent();
    let info = "prop=imageinfo&iiprop=url|mime&iiurlwidth=320&format=json&formatversion=2";
    let urls = [
        format!("https://en.wikipedia.org/w/api.php?action=query&generator=images&gimlimit=50&titles={}&{info}", encode(title)),
        format!("https://commons.wikimedia.org/w/api.php?action=query&generator=search&gsrnamespace=6&gsrlimit=8&gsrsearch={}&{info}", encode(&format!("{title} filetype:bitmap"))),
    ];
    let mut found = Vec::new();
    for url in &urls {
        let Ok(json) = agent.get(url).call().and_then(|mut r| r.body_mut().read_json::<Value>()) else { continue };
        for thumb in bitmaps(&json) {
            if found.len() == PICTURES {
                return found;
            }
            if let Ok(bytes) = agent.get(&thumb).call().and_then(|mut r| r.body_mut().read_to_vec()) {
                ctx.include_bytes(thumb.clone(), bytes);
                found.push(thumb);
            }
        }
    }
    found
}

/// Wikipedia's plain text marks headings as `== Name ==`, which become Markdown `## Name`. Each line is its own
/// paragraph, as Wikipedia's plain text puts one paragraph per line.
pub fn markdown(extract: &str) -> String {
    let mut out = String::new();
    for line in extract.lines() {
        let level = line.chars().take_while(|c| *c == '=').count();
        let name = line.trim_matches('=').trim();
        if level >= 2 && line.trim_end().ends_with('=') {
            out += &format!("\n{} {name}\n\n", "#".repeat(level.min(6)));
        } else if !line.trim().is_empty() {
            out += &format!("{line}\n\n");
        }
    }
    out
}

pub(super) fn fetch_article(title: &str, _: &egui::Context) -> Result<String, &'static str> {
    if cfg!(test) || cfg!(target_arch = "wasm32") {
        return Err("Offline in tests and in the playground.");
    }
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).user_agent(AGENT).build().new_agent();
    let url = format!("https://en.wikipedia.org/w/api.php?action=query&prop=extracts&explaintext=1&redirects=1&titles={}&format=json&formatversion=2", encode(title));
    let json: Value = agent.get(&url).call().and_then(|mut r| r.body_mut().read_json()).map_err(|_| "Wikipedia could not be reached.")?;
    json["query"]["pages"][0]["extract"].as_str().filter(|t| !t.is_empty()).map(markdown).ok_or("Wikipedia has no text for this article.")
}

/// Sends `HEAD` first. Some servers refuse `HEAD`, so those receive a `GET`.
pub(super) fn check(url: &str, _: &egui::Context) -> Link {
    if !checkable(url) || cfg!(target_arch = "wasm32") {
        return Link::Unchecked;
    }
    if cfg!(test) {
        return Link::Dead("offline in tests".into());
    }
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).user_agent(AGENT).http_status_as_error(false).build().new_agent();
    let mut status = agent.head(url).call().map(|r| r.status().as_u16());
    if let Ok(403 | 405 | 501) = status {
        status = agent.get(url).call().map(|r| r.status().as_u16());
    }
    match status {
        Ok(code) if code < 400 => Link::Ok,
        Ok(code) => Link::Dead(format!("HTTP {code}")),
        Err(e) => Link::Dead(e.to_string()),
    }
}

/// Only `https://` links to a named host. A note can come from a shared file, so the app must not request
/// addresses on the user's own machine or network, such as `http://192.168.1.1/...` or `https://localhost/...`.
/// shortcut: a public name that resolves to a private address still gets checked, resolve it first if that matters
pub(super) fn checkable(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.rsplit_once(':').map_or(host, |(h, _)| h).trim_end_matches('.').to_lowercase();
    // Digits and dots also catch the short forms resolvers accept, such as `127.1` and `2130706433`.
    let ip = host.starts_with('[') || host.chars().all(|c| c.is_ascii_digit() || c == '.');
    !(ip || host == "localhost" || host.ends_with(".localhost"))
}

pub(super) fn fetch(query: &str, ctx: &egui::Context) -> Result<Vec<Card>, &'static str> {
    if cfg!(test) || cfg!(target_arch = "wasm32") {
        return Err("Offline in tests and in the playground.");
    }
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).user_agent(AGENT).build().new_agent();
    let url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&generator=search&gsrsearch={}&gsrlimit=5&prop=pageimages|extracts|info&exintro=1&explaintext=1&exsentences=3&exlimit=max&pilimit=max&pithumbsize=320&inprop=url&format=json&formatversion=2",
        encode(query)
    );
    let json: Value = agent.get(&url).call().and_then(|mut r| r.body_mut().read_json()).map_err(|_| "Wikipedia could not be reached.")?;
    let mut cards = parse(&json);
    if cards.is_empty() {
        return Err("No Wikipedia article found.");
    }
    // shortcut: fetches every hit's thumbnail (up to 5 small images) since the pick happens later, fetch lazily if slow links make this noticeable.
    for card in &mut cards {
        card.thumb = card.thumb.take().filter(|thumb| {
            let Ok(bytes) = agent.get(thumb).call().and_then(|mut r| r.body_mut().read_to_vec()) else { return false };
            ctx.include_bytes(thumb.clone(), bytes);
            true
        });
    }
    Ok(cards)
}
