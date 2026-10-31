//! The text and code fonts, picked from the fonts installed on the system. egui's built-in fonts stay behind
//! them as a fallback for any glyph they lack.

use super::widgets::BOLD;
use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use fontique::{Collection, CollectionOptions, FontStyle, FontWeight, FontWidth, GenericFamily, SourceCache};

pub(super) struct Fonts {
    /// `None` in tests, so screenshots never depend on the machine's fonts.
    collection: Option<Collection>,
    /// Every family the pickers offer, sorted.
    pub(super) names: Vec<String>,
    /// The system's default text and code fonts, used while the setting is empty.
    pub(super) text: String,
    pub(super) code: String,
    installed: Option<(String, String)>,
}

impl Fonts {
    /// shortcut: scans every installed font (about 0.1 s) on the UI thread at start-up, move it to a thread if start-up feels slow
    pub(super) fn system() -> Fonts {
        let mut fonts = Fonts { collection: None, names: Vec::new(), text: String::new(), code: String::new(), installed: None };
        // A browser does not hand its fonts to the page, so the playground stays on egui's built-in ones.
        if cfg!(test) || cfg!(target_arch = "wasm32") {
            return fonts;
        }
        let mut c = Collection::new(CollectionOptions { shared: false, system_fonts: true });
        // skrifa cannot read WOFF2, and an emoji or symbol family has no letters to show.
        let skip = |name: &str| ["Emoji", "Symbols"].iter().any(|s| name.contains(s));
        fonts.names = c.family_names().filter(|n| !skip(n)).map(str::to_owned).collect();
        fonts.names.sort();
        fonts.names.dedup();
        let mut first = |generics: &[GenericFamily]| {
            generics.iter().find_map(|g| {
                let id = c.generic_families(*g).next()?;
                c.family_name(id).map(str::to_owned)
            })
        };
        // macOS's Monospace generic is Courier; Menlo is its terminal font.
        let menlo = cfg!(target_os = "macos").then(|| "Menlo".to_owned());
        fonts.text = first(&[GenericFamily::SystemUi, GenericFamily::SansSerif]).unwrap_or_default();
        fonts.code = menlo.or_else(|| first(&[GenericFamily::Monospace])).unwrap_or_default();
        fonts.collection = Some(c);
        fonts
    }

    /// Puts the picked fonts, or the system's when a name is empty, ahead of egui's. Does nothing when unchanged.
    pub(super) fn install(&mut self, ctx: &egui::Context, text: &str, code: &str) {
        let want = (text.to_owned(), code.to_owned());
        if self.installed.as_ref() == Some(&want) {
            return;
        }
        self.installed = Some(want);
        let mut defs = FontDefinitions::default();
        let mut bold = Vec::new();
        if let Some(c) = &mut self.collection {
            let mut cache = SourceCache::default();
            let text = if text.is_empty() { self.text.as_str() } else { text };
            let code = if code.is_empty() { self.code.as_str() } else { code };
            let faces = [(text, FontWeight::NORMAL, FontFamily::Proportional), (code, FontWeight::NORMAL, FontFamily::Monospace), (text, FontWeight::BOLD, FontFamily::Name(BOLD.into()))];
            for (name, weight, family) in faces {
                // A font that is gone, say a map opened on another machine, leaves egui's in place.
                let Some(info) = c.family_by_name(name) else { continue };
                let Some(font) = info.match_font(FontWidth::NORMAL, FontStyle::Normal, weight, false) else { continue };
                let Some(blob) = font.load(Some(&mut cache)) else { continue };
                let key = format!("{name} {}", weight.value());
                let (bytes, _) = blob.into_raw_parts();
                defs.font_data.insert(key.clone(), FontData::from_blob(bytes, font.index()).into());
                match family {
                    FontFamily::Name(_) => bold.push(key),
                    family => defs.families.entry(family).or_default().insert(0, key),
                }
            }
        }
        // Without a bold face, Title and Section fall back to the regular text fonts.
        bold.extend(defs.families[&FontFamily::Proportional].clone());
        defs.families.insert(FontFamily::Name(BOLD.into()), bold);
        ctx.set_fonts(defs);
    }
}
