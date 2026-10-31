//! Themes: palettes in Omarchy's `colors.toml` format, mapped to the app's roles. A theme names no roles of its own,
//! so any Omarchy theme works unchanged.

use eframe::egui::{self, Color32};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// What every surface is drawn with. Every text color is at least 4.5:1 against the fill it sits on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Palette {
    pub(super) dark: bool,
    /// The canvas behind the graph.
    pub(super) fog: Color32,
    pub(super) panel: Color32,
    pub(super) text_box: Color32,
    /// Buttons and hovered rows.
    pub(super) widget: Color32,
    pub(super) trail: Color32,
    /// A node on the path from the root to the current node, and its text.
    pub(super) node: Color32,
    pub(super) node_text: Color32,
    /// A node off that path, its text and the link to it: dimmer than the lit trail.
    pub(super) branch: Color32,
    pub(super) branch_text: Color32,
    pub(super) branch_link: Color32,
    /// Opaque, so the link behind a ghost never shows through its text.
    pub(super) ghost: Color32,
    pub(super) ghost_selected: Color32,
    pub(super) ghost_text: Color32,
    pub(super) body: Color32,
    pub(super) note_text: Color32,
    pub(super) strong: Color32,
    pub(super) hint: Color32,
    pub(super) link: Color32,
    pub(super) selection: Color32,
    pub(super) fail: Color32,
}

/// The themes that ship with the app. Their file names follow the family rule in `family`.
const BUNDLED: [(&str, &str); 15] = [
    ("mindme", include_str!("../../assets/themes/mindme.toml")),
    ("mindme-light", include_str!("../../assets/themes/mindme-light.toml")),
    ("tokyo-night", include_str!("../../assets/themes/tokyo-night.toml")),
    ("tokyo-night-day", include_str!("../../assets/themes/tokyo-night-day.toml")),
    ("catppuccin", include_str!("../../assets/themes/catppuccin.toml")),
    ("catppuccin-latte", include_str!("../../assets/themes/catppuccin-latte.toml")),
    ("gruvbox", include_str!("../../assets/themes/gruvbox.toml")),
    ("gruvbox-light", include_str!("../../assets/themes/gruvbox-light.toml")),
    ("nord", include_str!("../../assets/themes/nord.toml")),
    ("everforest", include_str!("../../assets/themes/everforest.toml")),
    ("everforest-light", include_str!("../../assets/themes/everforest-light.toml")),
    ("rose-pine", include_str!("../../assets/themes/rose-pine.toml")),
    ("rose-pine-dawn", include_str!("../../assets/themes/rose-pine-dawn.toml")),
    ("flexoki", include_str!("../../assets/themes/flexoki.toml")),
    ("flexoki-light", include_str!("../../assets/themes/flexoki-light.toml")),
];

pub(super) const DEFAULT: &str = "mindme";
/// The saved theme name that means "use Omarchy's current theme".
pub(super) const OMARCHY: &str = "omarchy";

/// One theme in the picker: a family of up to two palettes, the dark one and the light one.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Theme {
    /// The family name, also what `graph.json` saves.
    pub(super) name: String,
    pub(super) dark: Option<Palette>,
    pub(super) light: Option<Palette>,
}

impl Theme {
    /// The palette in the asked mode, or the one it has.
    pub(super) fn palette(&self, light: bool) -> Palette {
        let (want, other) = if light { (self.light, self.dark) } else { (self.dark, self.light) };
        want.or(other).unwrap_or_else(default_palette)
    }

    /// "tokyo-night" reads "Tokyo Night".
    pub(super) fn title(&self) -> String {
        if self.name == DEFAULT {
            return "Mind-Me".into();
        }
        self.name.split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect::<Vec<_>>().join(" ")
    }
}

/// `catppuccin-latte` and `catppuccin` are one family, as are `X` and `X-light`, `X-day` or `X-dawn`.
/// The mode comes from the file, not the name.
fn family(stem: &str) -> &str {
    ["-latte", "-light", "-day", "-dawn", "-dark"].iter().find_map(|s| stem.strip_suffix(s)).filter(|f| !f.is_empty()).unwrap_or(stem)
}

/// The bundled themes, then Omarchy's installed ones, then the user's own in `<config>/themes/`.
/// A later file with the same name replaces an earlier one.
pub(super) fn themes(config: Option<&Path>) -> Vec<Theme> {
    let mut files: Vec<(String, String)> = BUNDLED.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<(PathBuf, bool)> = vec![(PathBuf::from("/usr/share/omarchy/themes"), true)];
    dirs.extend(home.map(|h| (h.join(".config/omarchy/themes"), true)));
    dirs.extend(config.map(|c| (c.join("themes"), false)));
    #[cfg(test)]
    dirs.clear();
    for (dir, omarchy) in dirs {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut found: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        found.sort();
        for path in found {
            // Omarchy keeps a theme as a folder, the user as one file.
            let (file, stem) = if omarchy { (path.join("colors.toml"), path.file_name()) } else { (path.clone(), path.file_stem()) };
            let (Some(stem), Ok(text)) = (stem.map(|s| s.to_string_lossy().into_owned()), fs::read_to_string(&file)) else { continue };
            files.retain(|(n, _)| *n != stem);
            files.push((stem, text));
        }
    }
    let mut out: Vec<Theme> = Vec::new();
    for (stem, text) in files {
        let palette = palette(&text);
        let name = family(&stem);
        let at = out.iter().position(|t| t.name == name).unwrap_or_else(|| {
            out.push(Theme { name: name.to_owned(), dark: None, light: None });
            out.len() - 1
        });
        *if palette.dark { &mut out[at].dark } else { &mut out[at].light } = Some(palette);
    }
    out
}

/// Omarchy's current theme: its family name, its palette, and when it last changed.
/// Omarchy 4 keeps it in `~/.local/state`, Omarchy 3 in `~/.config`.
pub(super) fn omarchy() -> Option<(String, Palette, SystemTime)> {
    if cfg!(test) {
        return None;
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let current = [".local/state/omarchy/current", ".config/omarchy/current"].iter().map(|d| home.join(d)).find(|d| d.join("theme/colors.toml").exists())?;
    let changed = fs::metadata(current.join("theme.name")).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    let name = fs::read_to_string(current.join("theme.name")).unwrap_or_default();
    let text = fs::read_to_string(current.join("theme/colors.toml")).ok()?;
    Some((family(name.trim()).to_owned(), palette(&text), changed))
}

pub(super) fn default_palette() -> Palette {
    palette(BUNDLED[0].1)
}

/// Reads `key = "#rrggbb"` lines. Anything else is skipped, so a malformed file still gives a full palette.
/// shortcut: themes older than `colors.toml` that only have `color0`..`color15` get the defaults, map those keys if anyone has one
pub(super) fn palette(toml: &str) -> Palette {
    let get = |key: &str| {
        toml.lines().find_map(|line| {
            let (k, v) = line.split_once('=')?;
            let v = v.trim().trim_matches('"');
            (k.trim() == key).then(|| Color32::from_hex(v).ok()).flatten()
        })
    };
    let dark = !toml.lines().any(|l| l.split_once('=').is_some_and(|(k, v)| k.trim() == "mode" && v.contains("light")));
    let bg = get("background").unwrap_or(if dark { Color32::from_gray(27) } else { Color32::from_gray(248) });
    let fg = get("foreground").unwrap_or(if dark { Color32::from_gray(200) } else { Color32::from_gray(60) });
    let bright = get("bright_foreground").unwrap_or(fg);
    let accent = get("accent").or(get("blue")).unwrap_or(fg);
    let lighter = get("lighter_background").unwrap_or(mix(bg, fg, 0.1));
    let selection = get("selection").unwrap_or(lighter);
    let muted = get("muted").unwrap_or(mix(fg, bg, 0.5));
    // Omarchy's own fallbacks for the deeper surfaces.
    let fog = get("darker_background").unwrap_or(mix(bg, Color32::BLACK, 0.5));
    let text_box = get("dark_background").unwrap_or(mix(bg, Color32::BLACK, 0.25));
    let readable = |text: Color32, on: Color32| readable(text, on, [fg, bright]).unwrap_or(bright);
    let node = settle(mix(bg, accent, 0.3), bg, bright);
    let ghost_selected = settle(mix(selection, accent, 0.3), selection, bright);
    let ghost_text = get("light_foreground").unwrap_or(fg);
    Palette {
        dark,
        fog,
        panel: bg,
        text_box,
        widget: lighter,
        trail: accent,
        node,
        node_text: readable(bright, node),
        branch: lighter,
        branch_text: readable(fg, lighter),
        branch_link: muted,
        ghost: selection,
        ghost_selected,
        ghost_text: readable(readable(ghost_text, selection), ghost_selected),
        body: readable(fg, bg),
        note_text: readable(get("light_foreground").unwrap_or(fg), bg),
        strong: readable(bright, bg),
        hint: readable(get("dark_foreground").unwrap_or(muted), bg),
        link: readable(get("blue").unwrap_or(accent), bg),
        selection,
        fail: readable(get("red").unwrap_or(Color32::from_rgb(220, 90, 80)), bg),
    }
}

/// An accent-tinted `fill`, moved back toward `back` until `text` reads on it. Only a fill so bright that
/// no text color reads on it moves.
fn settle(mut fill: Color32, back: Color32, text: Color32) -> Color32 {
    for _ in 0..20 {
        if readable_ok(text, fill).is_some() {
            break;
        }
        fill = mix(fill, back, 0.2);
    }
    fill
}

fn readable_ok(text: Color32, on: Color32) -> Option<()> {
    (contrast(text, on) >= 4.5).then_some(())
}

/// `text` moved toward each of `toward` in turn, 5% at a time, until it reads at 4.5:1 on `on`.
fn readable(mut text: Color32, on: Color32, toward: [Color32; 2]) -> Option<Color32> {
    for target in toward {
        if let Some(c) = (0..=20).map(|i| mix(text, target, i as f32 / 20.0)).find(|c| readable_ok(*c, on).is_some()) {
            return Some(c);
        }
        text = target;
    }
    None
}

pub(super) fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The WCAG contrast ratio between two opaque colors.
pub(super) fn contrast(a: Color32, b: Color32) -> f32 {
    let channel = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let luminance = |c: Color32| 0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b());
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Styles every egui widget from the palette, and keeps it where `widgets::palette` finds it.
pub(super) fn apply(ctx: &egui::Context, p: Palette) {
    let theme = if p.dark { egui::Theme::Dark } else { egui::Theme::Light };
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    (v.panel_fill, v.window_fill, v.extreme_bg_color, v.code_bg_color, v.faint_bg_color) = (p.panel, p.panel, p.text_box, p.widget, p.widget);
    (v.hyperlink_color, v.error_fg_color, v.selection.bg_fill) = (p.link, p.fail, p.selection);
    v.selection.stroke.color = p.strong;
    v.widgets.noninteractive.fg_stroke.color = p.body;
    v.widgets.noninteractive.bg_stroke.color = p.branch_link;
    for (w, text) in [(&mut v.widgets.inactive, p.body), (&mut v.widgets.hovered, p.strong), (&mut v.widgets.active, p.strong), (&mut v.widgets.open, p.strong)] {
        w.fg_stroke.color = text;
    }
    for w in [&mut v.widgets.inactive, &mut v.widgets.open] {
        (w.bg_fill, w.weak_bg_fill) = (p.widget, p.widget);
    }
    for w in [&mut v.widgets.hovered, &mut v.widgets.active] {
        (w.bg_fill, w.weak_bg_fill) = (p.ghost_selected, p.ghost_selected);
    }
    ctx.set_visuals_of(theme, v);
    ctx.all_styles_mut(super::widgets::type_scale);
    ctx.set_theme(theme);
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, p));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_pair_into_one_theme_by_name() {
        assert_eq!([family("catppuccin-latte"), family("tokyo-night-day"), family("rose-pine-dawn"), family("mindme-light")], ["catppuccin", "tokyo-night", "rose-pine", "mindme"]);
        assert_eq!(family("light"), "light", "a bare suffix is a name");
        let themes = themes(None);
        let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["mindme", "tokyo-night", "catppuccin", "gruvbox", "nord", "everforest", "rose-pine", "flexoki"]);
        for t in &themes {
            assert_eq!((t.dark.is_some(), t.light.is_some()), (true, t.name != "nord"), "{} has both modes, Nord only dark", t.name);
            assert!(t.dark.is_none_or(|p| p.dark) && t.light.is_none_or(|p| !p.dark), "{}'s modes come from its files", t.name);
        }
    }

    #[test]
    fn every_text_color_reads_at_4_5_to_1_on_its_fill() {
        // The hint and ghost colors of these themes fail as written, so this also proves the fix.
        let raw = "mode = \"dark\"\nbackground = \"#2e3440\"\nforeground = \"#d8dee9\"\ndark_foreground = \"#4c566a\"\n";
        let mut palettes: Vec<Palette> = themes(None).iter().flat_map(|t| [t.dark, t.light]).flatten().collect();
        palettes.push(palette(raw));
        for p in palettes {
            for (text, on) in [(p.node_text, p.node), (p.branch_text, p.branch), (p.ghost_text, p.ghost), (p.ghost_text, p.ghost_selected), (p.body, p.panel), (p.note_text, p.panel), (p.hint, p.panel), (p.link, p.panel), (p.fail, p.panel)] {
                assert!(contrast(text, on) >= 4.5, "{text:?} on {on:?} is {:.2}:1 in {p:?}", contrast(text, on));
            }
        }
        assert_ne!(palette(raw).hint, Color32::from_hex("#4c566a").unwrap(), "the too-dim hint was moved");
    }

    #[test]
    fn a_file_with_only_a_mode_still_gives_a_full_palette() {
        let p = palette("mode = \"light\"\n# nothing else\nnot a line");
        assert!(!p.dark && p.panel == Color32::from_gray(248));
    }
}
