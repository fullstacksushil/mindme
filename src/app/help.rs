//! The help map: a map of the app's own features that ships inside it and starts fresh on every open.

use super::view::KEYS;
use super::{App, Doc};
use std::fs;

/// The built-in help map, with its Keyboard shortcuts branch made from the Keys list, so it never goes stale.
pub(super) fn text() -> String {
    let mut keys = String::from("- Keyboard shortcuts\n  > Every key, by task. Keys in the footer shows the same list.\n");
    for (group, list) in KEYS {
        keys += &format!("  - {group}\n");
        for (key, does) in *list {
            keys += &format!("    - {key}: {does}\n");
        }
    }
    include_str!("../../assets/help.md").replace("- Try it\n", &format!("{keys}- Try it\n"))
}

impl App {
    /// Writes the help map over `help.md` in the config folder and opens it, replacing its tab if one is open,
    /// so edits made while trying things are gone next time. It never joins the recent maps.
    pub(super) fn open_help(&mut self) {
        // Tests have no config folder, and run in parallel.
        let dir = self.dir.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("mindme-help-{}", std::process::id())));
        let path = dir.join("help.md");
        if let Err(e) = fs::create_dir_all(&dir).and_then(|()| fs::write(&path, text())) {
            self.open_error = Some(format!("could not write the help map: {e}"));
            return;
        }
        let doc = match Doc::open(path) {
            Ok(doc) => doc,
            Err(e) => return self.open_error = Some(e),
        };
        (self.article, self.on_settings) = (None, false);
        match self.docs.iter().position(|d| d.path == doc.path) {
            Some(i) => (self.active, self.docs[i]) = (i, doc),
            None if self.docs[self.active].is_blank() => self.docs[self.active] = doc,
            None => {
                self.docs.push(doc);
                self.active = self.docs.len() - 1;
            }
        }
    }
}
