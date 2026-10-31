//! Templates: maps with fixed branches that start a new map, with the typed topic as their root.

use crate::map::{Map, Text};
use std::{fs, path::Path};

const BUILT_IN: [&str; 5] = [
    include_str!("../../assets/templates/pros-and-cons.md"),
    include_str!("../../assets/templates/five-whys.md"),
    include_str!("../../assets/templates/first-principles.md"),
    include_str!("../../assets/templates/swot.md"),
    include_str!("../../assets/templates/decision.md"),
];

/// A template's title is its name, and the first line of its root note says what it is for.
pub(crate) struct Template {
    pub(crate) name: String,
    pub(crate) about: String,
    md: String,
}

impl Template {
    fn parse(md: String) -> Option<Template> {
        let mut lines = md.lines();
        let name = lines.next()?.strip_prefix("# ")?.trim().to_owned();
        let about = lines.next().and_then(|l| l.strip_prefix("> ")).unwrap_or_default().trim().to_owned();
        Some(Template { name, about, md })
    }

    /// The template's map with `topic` as its root and without the root note, which only described the template.
    pub(crate) fn map(&self, topic: &Text) -> Option<Map> {
        let branches: Vec<&str> = self.md.lines().skip(1).skip_while(|l| l.starts_with('>')).collect();
        Map::parse(&format!("# {}\n{}\n", topic.as_str(), branches.join("\n")))
    }
}

/// The built-in templates, then the user's own maps in `<config>/templates/`, by file name.
pub(crate) fn templates(config: Option<&Path>) -> Vec<Template> {
    let mut found: Vec<String> = BUILT_IN.iter().map(|t| t.to_string()).collect();
    if let Some(Ok(entries)) = config.map(|c| fs::read_dir(c.join("templates"))) {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "md")).collect();
        paths.sort();
        found.extend(paths.iter().filter_map(|p| fs::read_to_string(p).ok()));
    }
    found.into_iter().filter_map(Template::parse).collect()
}
