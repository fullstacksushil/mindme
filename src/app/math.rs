//! LaTeX math in notes, `$…$` inline and `$$…$$` as a block, drawn by RaTeX.

use eframe::egui;
use std::sync::Arc;

/// Draws one formula at the note's text size. A formula RaTeX cannot read shows its source in the error color.
/// shortcut: rendered formulas stay in memory until the app closes, evict them if notes with many formulas use too much
pub(super) fn math(ui: &mut egui::Ui, tex: &str, inline: bool) {
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    let ppp = ui.ctx().pixels_per_point();
    let id = egui::Id::unique((tex, inline, size.to_bits(), ppp.to_bits()));
    let png: Option<Arc<[u8]>> = ui.data_mut(|d| d.get_temp_mut_or_insert_with(id, || render(tex, inline, size, ppp)).clone());
    let Some(png) = png else {
        ui.colored_label(ui.visuals().error_fg_color, tex);
        return;
    };
    // Drawn white, so the tint gives it the theme's text color without drawing it again.
    let image = egui::Image::from_bytes(format!("bytes://math/{}.png", id.value()), png).fit_to_original_size(1.0 / ppp).tint(ui.visuals().text_color());
    if inline {
        ui.add(image);
    } else {
        ui.end_row();
        ui.add(image);
        ui.end_row();
    }
}

fn render(tex: &str, inline: bool, size: f32, ppp: f32) -> Option<Arc<[u8]>> {
    use ratex_types::{color::Color, math_style::MathStyle};
    let nodes = ratex_parser::parse(tex).ok()?;
    let style = if inline { MathStyle::Text } else { MathStyle::Display };
    let layout = ratex_layout::layout(&nodes, &ratex_layout::LayoutOptions { style, color: Color::WHITE, ..Default::default() });
    let options = ratex_render::RenderOptions {
        font_size: size,
        padding: 1.0,
        background_color: Color::new(0.0, 0.0, 0.0, 0.0),
        device_pixel_ratio: ppp,
        ..Default::default()
    };
    ratex_render::render_to_png(&ratex_layout::to_display_list(&layout), &options).ok().map(Into::into)
}
