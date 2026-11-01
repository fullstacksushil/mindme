use super::settings::GraphSettings;
use eframe::egui::{self, Color32, Key};

/// Rest length of a link between the current node and a ghost. Links between kept nodes use the graph settings.
pub(super) const GHOST_LINK: f32 = 120.0;
/// The arc, in radians, that ghosts spread over, facing away from the current node's parent.
pub(super) const FAN: f32 = 2.4;
/// Seconds the keep animation runs.
pub(super) const FLASH: f64 = 0.3;

pub(super) use super::theme::Palette;

/// Windows narrower than this, in points, are phones held upright: the side panels start hidden and open one at a
/// time over the whole window, buttons grow to fit a finger, and hints talk about taps instead of keys.
pub(super) const COMPACT: f32 = 700.0;

pub(super) fn compact(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < COMPACT
}

/// Bigger buttons and gaps in a compact window, so a finger hits what it aims at. egui's own spacing otherwise.
pub(super) fn touch_spacing(style: &mut egui::Style, touch: bool) {
    let normal = egui::style::Spacing::default();
    let s = &mut style.spacing;
    (s.interact_size, s.button_padding, s.item_spacing) = if touch {
        (egui::vec2(40.0, 32.0), egui::vec2(10.0, 6.0), egui::vec2(10.0, 8.0))
    } else {
        (normal.interact_size, normal.button_padding, normal.item_spacing)
    };
}

/// The palette `theme::apply` set. A bare `Ui`, as in a widget test, gets the default theme.
pub(super) fn palette(ui: &egui::Ui) -> Palette {
    ui.data(|d| d.get_temp(egui::Id::NULL)).unwrap_or_else(super::theme::default_palette)
}

/// The type scale's Section level: panel and screen section names.
pub(super) fn section() -> egui::TextStyle {
    egui::TextStyle::Name("Section".into())
}

/// The one type scale, in points before the global zoom: Title (egui's Heading), Section, Body, Control (egui's
/// Button) and Hint (egui's Small). Title and Section use the bold face when the text font has one.
pub(super) fn type_scale(style: &mut egui::Style) {
    use egui::{FontFamily, FontId, TextStyle};
    let bold = FontFamily::Name(BOLD.into());
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (section(), FontId::new(15.0, bold)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
    ]
    .into();
}

/// The font family holding the text font's bold face, then the regular text fonts as fallback.
pub(super) const BOLD: &str = "bold";

/// A section name: Topic, Note, Branches, Discuss.
pub(super) fn heading(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    ui.label(egui::RichText::new(text).text_style(section()).color(palette(ui).strong))
}

/// A hint, a count or a warning: the smallest text, in the muted color.
pub(super) fn hint(ui: &egui::Ui, text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).small().color(palette(ui).hint)
}

/// A node label's size by its depth: the root 28 and bold, then 22, 18, 14, and 13 from depth 4 down.
/// The graph settings' Text size multiplies it.
pub(super) fn label_font(depth: usize, scale: f32) -> egui::FontId {
    let size = [28.0, 22.0, 18.0, 14.0, 13.0][depth.min(4)] * scale;
    let family = if depth == 0 { egui::FontFamily::Name(BOLD.into()) } else { egui::FontFamily::Proportional };
    egui::FontId::new(size, family)
}

/// A node or ghost label laid out on one line. Ghosts are italic, so a suggestion never reads as a kept node.
pub(super) fn label(painter: &egui::Painter, text: &str, font: egui::FontId, ghost: bool, color: Color32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), egui::TextFormat { font_id: font, color, italics: ghost, ..Default::default() });
    job.wrap.max_width = f32::INFINITY;
    painter.layout_job(job)
}
pub(super) const NO_WEB: &str = "This model can't search the web, so its sources may be made up.";

/// Enter, without Shift, in the multi-line box `id` while it has focus. This takes the key before the box sees it,
/// so it sends instead of starting a line. `consume_key` would also take Shift+Enter, as it ignores Shift.
pub(super) fn enter_sends(ui: &mut egui::Ui, id: egui::Id) -> bool {
    ui.memory(|m| m.has_focus(id))
        && ui.input_mut(|i| {
            let at = i.events.iter().position(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift));
            at.map(|at| i.events.remove(at)).is_some()
        })
}

/// A spinner, the thing the app waits for, and the elapsed time and a Stop button on the right. True when Stop
/// was clicked. The clock starts on the first frame the spinner shows for `key`. It restarts after the spinner is gone for a second.
pub(super) fn waiting(ui: &mut egui::Ui, key: impl std::hash::Hash + std::fmt::Debug, what: &str) -> bool {
    let now = ui.input(|i| i.time);
    let id = ui.make_persistent_id(("waiting", key));
    let start = ui.data_mut(|d| {
        let (start, seen) = d.get_temp_mut_or_insert_with(id, || (now, now));
        if now - *seen > 1.0 {
            *start = now;
        }
        *seen = now;
        *start
    });
    let secs = (now - start) as u64;
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(egui::RichText::new(what).color(palette(ui).hint));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let stop = ui.small_button("Stop").on_hover_text("Stop this AI call").clicked();
            let elapsed = if secs < 60 { format!("{secs}s") } else { format!("{}m {:02}s", secs / 60, secs % 60) };
            ui.label(egui::RichText::new(elapsed).color(palette(ui).hint).monospace());
            stop
        })
        .inner
    })
    .inner
}

/// Notes, chat replies and articles as Markdown, with LaTeX math. An HTML block shows its text without the tags.
/// shortcut: inline HTML such as `<b>` still shows its tags, egui_commonmark has no hook for it; strip it before rendering if models send it
pub(super) fn markdown(ui: &mut egui::Ui, cache: &mut egui_commonmark::CommonMarkCache, text: &str) {
    let viewer = egui_commonmark::CommonMarkViewer::new().render_html_fn(Some(&|ui, html| _ = ui.label(without_tags(html))));
    #[cfg(not(target_arch = "wasm32"))]
    let viewer = viewer.render_math_fn(Some(&super::math::math));
    viewer.show(ui, cache, text);
}

/// The text of some HTML: everything between tags, its whitespace collapsed.
pub(super) fn without_tags(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                text.push(' ');
            }
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The note's text: brighter than egui's dim label grey, in the code font when Mono is picked. A Markdown heading
/// inside a note stays below the panel's own Title.
pub(super) fn note_style(ui: &mut egui::Ui, settings: &GraphSettings) {
    ui.visuals_mut().widgets.noninteractive.fg_stroke.color = palette(ui).note_text;
    // Always-visible bars, so a table wider than the panel shows that it scrolls sideways.
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
    for (style, font) in ui.style_mut().text_styles.iter_mut() {
        // egui_commonmark draws `#` at this size and steps the deeper levels down toward Body.
        if *style == egui::TextStyle::Heading {
            font.size = 17.0;
        }
        if settings.mono {
            font.family = egui::FontFamily::Monospace;
        }
    }
}
