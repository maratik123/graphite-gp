//! AC8 — the widget gallery golden: a single wgpu snapshot of the full
//! variant/size/state matrix for all five core widgets, driven directly
//! through each widget's private `paint` layer with FORCED states (no
//! pointer-input simulation — mirrors `placeholder.rs`'s `golden_guard`).
//!
//! In-crate `#[cfg(test)]` (not `tests/`) so it can reach every widget's
//! crate-visible `paint` fn. `#[cfg_attr(miri, ignore)]` — drives wgpu,
//! which `dlopen`s the Vulkan ICD (no FFI under Miri).

use super::Size;
use super::badge::{Badge, Tone};
use super::button::{Button, Variant as ButtonVariant};
use super::card::{Card, Elevation};
use super::icon_button::{IconButton, Variant as IconButtonVariant};
use super::tag::Tag;
use crate::icons::{Icon, IconSet};
use egui::{Pos2, Rect};

/// The gallery's fixed canvas: 1040×900 logical points at
/// `pixels_per_point = 1.0`.
const CANVAS_RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(1040.0, 900.0),
};

/// A grid cell rect at `(col, row)` within a group starting at
/// `(x0, y0)`, `cell_w`/`cell_h` apart (with a small gap baked into the
/// cell size, matching `placeholder.rs`'s field-wise `Pos2` construction —
/// never `Pos2 + Vec2` operators, `clippy::arithmetic_side_effects`).
#[allow(
    clippy::too_many_arguments,
    reason = "a purely mechanical grid-placement helper for this test-only gallery layout; splitting it into a struct would not aid clarity here"
)]
fn cell(x0: f32, y0: f32, col: u32, row: u32, cell_w: f32, cell_h: f32, w: f32, h: f32) -> Rect {
    let col_f = f32::from(u16::try_from(col).unwrap_or(u16::MAX));
    let row_f = f32::from(u16::try_from(row).unwrap_or(u16::MAX));
    let min = Pos2::new(col_f.mul_add(cell_w, x0), row_f.mul_add(cell_h, y0));
    Rect {
        min,
        max: Pos2::new(min.x + w, min.y + h),
    }
}

/// Draws the full Button matrix: 4 variants x {rest, hover, press, disabled}
/// at `Md`, plus a size row (sm/md/lg, `Primary`, rest).
fn draw_buttons(painter: &egui::Painter, x0: f32, y0: f32) -> f32 {
    let variants = [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ];
    let cell_w = 130.0;
    let cell_h = 44.0;
    for (row, variant) in variants.iter().copied().enumerate() {
        let states: [(&str, bool, bool, bool); 4] = [
            ("Rest", false, false, true),
            ("Hover", true, false, true),
            ("Press", false, true, true),
            ("Disabled", false, false, false),
        ];
        for (col, (label, hovered, pressed, enabled)) in states.into_iter().enumerate() {
            let rect = cell(
                x0,
                y0,
                u32::try_from(col).unwrap_or(u32::MAX),
                u32::try_from(row).unwrap_or(u32::MAX),
                cell_w,
                cell_h,
                120.0,
                36.0,
            );
            let style = Button::resolve(variant, Size::Md, hovered, pressed);
            Button::paint(painter, rect, &style, label, None, None, enabled);
        }
    }
    let size_row_y = y0 + cell_h * 4.0 + 10.0;
    for (col, (label, size)) in [("Sm", Size::Sm), ("Md", Size::Md), ("Lg", Size::Lg)]
        .into_iter()
        .enumerate()
    {
        let rect = cell(
            x0,
            size_row_y,
            u32::try_from(col).unwrap_or(u32::MAX),
            0,
            cell_w,
            50.0,
            120.0,
            50.0,
        );
        let style = Button::resolve(ButtonVariant::Primary, size, false, false);
        Button::paint(painter, rect, &style, label, None, None, true);
    }
    size_row_y + 60.0
}

/// Draws the `IconButton` matrix: secondary/ghost x {rest, hover, press,
/// active, disabled}, with a baked glyph from `icon_set`.
fn draw_icon_buttons(painter: &egui::Painter, x0: f32, y0: f32, icon_set: &IconSet) -> f32 {
    let variants = [IconButtonVariant::Secondary, IconButtonVariant::Ghost];
    let cell_w = 56.0;
    let cell_h = 56.0;
    let glyph = icon_set.get(Icon::Settings);
    for (row, variant) in variants.iter().copied().enumerate() {
        let states: [(bool, bool, bool, bool); 5] = [
            (false, false, false, true),  // rest
            (false, true, false, true),   // hover
            (false, false, true, true),   // press
            (true, false, false, true),   // active
            (false, false, false, false), // disabled
        ];
        for (col, (active, hovered, pressed, enabled)) in states.into_iter().enumerate() {
            let rect = cell(
                x0,
                y0,
                u32::try_from(col).unwrap_or(u32::MAX),
                u32::try_from(row).unwrap_or(u32::MAX),
                cell_w,
                cell_h,
                46.0,
                46.0,
            );
            let style = IconButton::resolve(variant, Size::Md, active, hovered, pressed);
            IconButton::paint(painter, rect, &style, glyph, enabled);
        }
    }
    y0 + cell_h * 2.0 + 10.0
}

/// Draws the Badge matrix: 5 tones x {tinted, solid}.
fn draw_badges(painter: &egui::Painter, x0: f32, y0: f32) -> f32 {
    let tones = [
        (Tone::Neutral, "NEUTRAL"),
        (Tone::Accent, "ACCENT"),
        (Tone::Ok, "OK"),
        (Tone::Warn, "WARN"),
        (Tone::Danger, "DANGER"),
    ];
    let cell_w = 130.0;
    let cell_h = 32.0;
    for (row, (tone, label)) in tones.into_iter().enumerate() {
        for (col, solid) in [false, true].into_iter().enumerate() {
            let rect = cell(
                x0,
                y0,
                u32::try_from(col).unwrap_or(u32::MAX),
                u32::try_from(row).unwrap_or(u32::MAX),
                cell_w,
                cell_h,
                100.0,
                20.0,
            );
            let style = Badge::resolve(tone, solid);
            Badge::paint(painter, rect, &style, label);
        }
    }
    y0 + cell_h * 5.0 + 10.0
}

/// Draws the Tag row: rest, selected, with-dot, with-remove.
fn draw_tags(painter: &egui::Painter, x0: f32, y0: f32) -> f32 {
    let cell_w = 140.0;
    let cell_h = 40.0;

    let rest = cell(x0, y0, 0, 0, cell_w, cell_h, 110.0, 26.0);
    Tag::paint(
        painter,
        rest,
        &Tag::resolve(false),
        "L3",
        None,
        false,
        false,
    );

    let selected = cell(x0, y0, 1, 0, cell_w, cell_h, 110.0, 26.0);
    Tag::paint(
        painter,
        selected,
        &Tag::resolve(true),
        "SELECTED",
        None,
        false,
        false,
    );

    let with_dot = cell(x0, y0, 2, 0, cell_w, cell_h, 110.0, 26.0);
    Tag::paint(
        painter,
        with_dot,
        &Tag::resolve(false),
        "CAR-2",
        Some(crate::tokens::color::CAR_2),
        false,
        false,
    );

    let with_remove = cell(x0, y0, 3, 0, cell_w, cell_h, 110.0, 26.0);
    Tag::paint(
        painter,
        with_remove,
        &Tag::resolve(false),
        "REMOVE",
        None,
        true,
        false,
    );

    y0 + cell_h + 10.0
}

/// Draws the Card row: elevations 0-3, selected, and a grid-watermark card.
fn draw_cards(painter: &egui::Painter, x0: f32, y0: f32) -> f32 {
    let cell_w = 170.0;
    let cell_h = 130.0;
    let card_w = 150.0;
    let card_h = 110.0;

    for (col, elevation) in [
        Elevation::Level0,
        Elevation::Level1,
        Elevation::Level2,
        Elevation::Level3,
    ]
    .into_iter()
    .enumerate()
    {
        let rect = cell(
            x0,
            y0,
            u32::try_from(col).unwrap_or(u32::MAX),
            0,
            cell_w,
            cell_h,
            card_w,
            card_h,
        );
        let style = Card::resolve(false, elevation);
        Card::paint(
            painter,
            rect,
            &style,
            false,
            Some("EYEBROW"),
            Some("Title"),
            12.0,
        );
    }

    let selected_rect = cell(x0, y0, 4, 0, cell_w, cell_h, card_w, card_h);
    Card::paint(
        painter,
        selected_rect,
        &Card::resolve(true, Elevation::Level1),
        false,
        None,
        Some("Selected"),
        12.0,
    );

    let grid_rect = cell(x0, y0, 5, 0, cell_w, cell_h, card_w, card_h);
    Card::paint(
        painter,
        grid_rect,
        &Card::resolve(false, Elevation::Level1),
        true,
        None,
        Some("Grid"),
        12.0,
    );

    y0 + cell_h + 10.0
}

/// Draws the full gallery matrix into `rect` (currently only ever called
/// with [`CANVAS_RECT`]).
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first (every widget's text draw resolves a
/// `FontFamily::Name(..)`).
fn draw_gallery(painter: &egui::Painter, rect: Rect, icon_set: &IconSet) {
    painter.rect_filled(rect, 0, crate::tokens::color::SURFACE_PAGE);

    let x0 = rect.min.x + 20.0;
    let mut y = rect.min.y + 20.0;
    y = draw_buttons(painter, x0, y);
    y = draw_icon_buttons(painter, x0, y, icon_set);
    y = draw_badges(painter, x0, y);
    y = draw_tags(painter, x0, y);
    let _ = draw_cards(painter, x0, y);
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_RECT, draw_gallery};

    /// AC8 — one wgpu frame renders the full widget gallery matrix and
    /// matches the minted golden exactly (flat regions; AA-exempt edges per
    /// `placeholder.rs`'s `golden_guard` precedent).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn widget_gallery_matches_golden() {
        let render_state = egui_kittest::wgpu::create_render_state(
            egui_kittest::wgpu::default_wgpu_setup(),
            egui_wgpu::RendererOptions::PREDICTABLE,
        );
        assert_eq!(
            render_state.adapter.get_info().device_type,
            egui_wgpu::wgpu::DeviceType::Cpu,
            "resolved wgpu adapter is not a CPU/software device — install a \
             Vulkan software ICD (mesa-vulkan-drivers / lavapipe) to match CI"
        );

        let renderer = egui_kittest::wgpu::WgpuTestRenderer::from_render_state(render_state);

        // Frame-1-install/frame-2-draw (placeholder.rs's golden_guard
        // precedent): fonts can only be installed from inside the harness
        // closure, and `set_fonts` only takes effect at the *next* pass.
        let mut fonts_installed = false;
        let mut icon_set: Option<crate::icons::IconSet> = None;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_RECT.size())
            .with_pixels_per_point(1.0)
            .with_theme(egui::Theme::Light)
            .renderer(renderer)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                let set = icon_set.get_or_insert_with(|| {
                    crate::icons::IconSet::new(ui.ctx()).expect("vendored icons bake")
                });
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                draw_gallery(&painter, CANVAS_RECT, set);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        let options = egui_kittest::SnapshotOptions::new()
            .threshold(0.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) =
            egui_kittest::try_image_snapshot_options(&image, "widget_gallery", &options)
        {
            panic!("{err}");
        }
    }
}
