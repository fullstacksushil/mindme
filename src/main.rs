//! mindme: type a central idea, the AI suggests ghost branches, click the ones you keep.
// The browser playground skips files, threads and the network, so their code is unused there.
#![cfg_attr(target_arch = "wasm32", allow(unused))]

mod ai;
mod graph;
mod map;
mod recent;
mod wiki;
mod app;
#[cfg(any(test, target_arch = "wasm32"))]
mod web;

use app::{App, Doc};
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let mut paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        paths.push("map.md".into());
    }
    let config = ai::config().unwrap_or_else(|e| exit(&e));
    let docs = paths.into_iter().map(Doc::open).collect::<Result<_, _>>().unwrap_or_else(|e| exit(&e));
    eframe::run_native(
        "mindme",
        // The app id stays "mindme", so window rules written for it keep matching.
        eframe::NativeOptions { viewport: eframe::egui::ViewportBuilder::default()
            .with_title(app::title())
            .with_app_id("mindme")
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).expect("the icon is a PNG")), ..Default::default() },
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.add_bytes_loader(std::sync::Arc::new(wiki::WebImages::default()));
            Ok(Box::new(App::new(config, docs, config_dir())))
        }),
    )
}

/// The browser playground. See `web`.
#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

/// `mindme` in the platform's config folder: `$XDG_CONFIG_HOME`, `~/.config` or `%APPDATA%`.
fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join("mindme"))
}

#[cfg(not(target_arch = "wasm32"))]
fn exit(msg: &str) -> ! {
    eprintln!("mindme: {msg}");
    std::process::exit(2)
}
