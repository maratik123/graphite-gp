//! AC8 — the HUD specimen golden: a single wgpu snapshot of the Telemetry
//! HUD strip + `LapMeter` + `CarChip` standings, driven directly through
//! each widget's private `paint` layer with FORCED values (no pointer-input
//! simulation — mirrors `gallery.rs`'s `widget_gallery`).
//!
//! Laid out to match `docs/design-system/components/game/game.card.html`
//! (`MovePad` region omitted, out of scope per the design/spec).
//!
//! In-crate `#[cfg(test)]` (not `tests/`) so it can reach every widget's
//! crate-visible `paint` fn. `#[cfg_attr(miri, ignore)]` — drives wgpu,
//! which `dlopen`s the Vulkan ICD (no FFI under Miri).

use super::car_chip::{CarChip, CarKind};
use super::lap_meter::LapMeter;
use super::telemetry::{Align, Telemetry, Tone, telemetry_galleys};
use egui::{Painter, Pos2, Rect};

/// The gallery's fixed canvas: 640×420 logical points at
/// `pixels_per_point = 1.0`.
const CANVAS_RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(640.0, 420.0),
};

/// The `GRAPHITE_900` HUD panel's radius (`game.card.html`'s `.hud`:
/// `border-radius: var(--radius-2)`).
const HUD_PANEL_RADIUS: f32 = crate::tokens::spacing::RADIUS_2;
/// The HUD panel's horizontal padding (`game.card.html`'s `.hud`:
/// `padding: 16px 20px`).
const HUD_PAD_X: f32 = 20.0;
/// The HUD panel's vertical padding (`game.card.html`'s `.hud`:
/// `padding: 16px 20px`).
const HUD_PAD_Y: f32 = 16.0;
/// The HUD panel's fixed width — generously sized to hold the 4-cell strip
/// without overlap (this is a specimen layout, not a measured auto-layout).
const HUD_PANEL_WIDTH: f32 = 580.0;
/// The HUD panel's fixed height.
const HUD_PANEL_HEIGHT: f32 = 100.0;
/// `CarChip` row height (`CarChip.jsx`: `height: 34`), mirrored here since
/// `car_chip::HEIGHT` is a private module const.
const CAR_CHIP_HEIGHT: f32 = 34.0;
/// `CarChip` row bounding-box width — generously sized to hold rank + dot +
/// name + kind pill without overlap.
const CAR_CHIP_WIDTH: f32 = 260.0;
/// Gap between stacked `CarChip` rows.
const CAR_CHIP_ROW_GAP: f32 = 10.0;

/// Draws the `GRAPHITE_900` HUD panel containing the on-ink Telemetry strip
/// (SPEED / v / POS / TEMPO), per `game.card.html`'s `.hud` div. Each
/// `Telemetry::paint` call is `Align::Left`, which only reads `rect.min` —
/// the passed rects carry a nominal size only, not used by `paint`.
fn draw_hud_strip(painter: &Painter, x0: f32, y0: f32) -> f32 {
    let panel_rect = Rect::from_min_size(
        Pos2::new(x0, y0),
        egui::vec2(HUD_PANEL_WIDTH, HUD_PANEL_HEIGHT),
    );
    painter.rect_filled(
        panel_rect,
        HUD_PANEL_RADIUS,
        crate::tokens::color::GRAPHITE_900,
    );

    let cell_y = panel_rect.min.y + HUD_PAD_Y;
    let cells_x0 = panel_rect.min.x + HUD_PAD_X;
    let cell_offsets = [0.0, 150.0, 280.0, 420.0];

    let speed_rect = Rect::from_min_size(
        Pos2::new(cells_x0 + cell_offsets[0], cell_y),
        egui::vec2(1.0, 1.0),
    );
    let speed_style = Telemetry::resolve(Tone::Accent, super::Size::Lg, true);
    let speed_galleys = telemetry_galleys(painter, "SPEED", "3.61", None, &speed_style);
    Telemetry::paint(
        painter,
        speed_rect,
        &speed_style,
        &speed_galleys,
        Align::Left,
    );

    let v_rect = Rect::from_min_size(
        Pos2::new(cells_x0 + cell_offsets[1], cell_y),
        egui::vec2(1.0, 1.0),
    );
    let v_style = Telemetry::resolve(Tone::Default, super::Size::Md, true);
    let v_galleys = telemetry_galleys(painter, "v", "(2, 3)", None, &v_style);
    Telemetry::paint(painter, v_rect, &v_style, &v_galleys, Align::Left);

    let pos_rect = Rect::from_min_size(
        Pos2::new(cells_x0 + cell_offsets[2], cell_y),
        egui::vec2(1.0, 1.0),
    );
    let pos_style = Telemetry::resolve(Tone::Default, super::Size::Md, true);
    let pos_galleys = telemetry_galleys(painter, "POS", "(41, 17)", None, &pos_style);
    Telemetry::paint(painter, pos_rect, &pos_style, &pos_galleys, Align::Left);

    let tempo_rect = Rect::from_min_size(
        Pos2::new(cells_x0 + cell_offsets[3], cell_y),
        egui::vec2(1.0, 1.0),
    );
    let tempo_style = Telemetry::resolve(Tone::Muted, super::Size::Md, true);
    let tempo_galleys = telemetry_galleys(painter, "TEMPO", "0.87", Some("c/t"), &tempo_style);
    Telemetry::paint(
        painter,
        tempo_rect,
        &tempo_style,
        &tempo_galleys,
        Align::Left,
    );

    y0 + HUD_PANEL_HEIGHT + 22.0
}

/// Draws the `LapMeter(2, 5)` block, per `game.card.html`'s `.stack`
/// (`LapMeter lap={2} total={5}`).
fn draw_lap_meter(painter: &Painter, x0: f32, y0: f32) -> f32 {
    let width = 220.0;
    let height = 40.0;
    let rect = Rect::from_min_size(Pos2::new(x0, y0), egui::vec2(width, height));
    let style = LapMeter::resolve(2, 5);
    let colors = LapMeter::ink_colors(false);
    LapMeter::paint(painter, rect, style, "LAP", colors);
    y0 + height + 22.0
}

/// Draws the 3-row `CarChip` standings column, per `game.card.html`'s
/// `.stack` (`You`/rank 1/active, `Rival Blue`/rank 2, `Rival Green`/rank 3).
fn draw_car_chips(painter: &Painter, x0: f32, y0: f32) -> f32 {
    let rows: [(egui::Color32, &str, u32, bool, CarKind); 3] = [
        (crate::tokens::color::CAR_1, "You", 1, true, CarKind::You),
        (
            crate::tokens::color::CAR_2,
            "Rival Blue",
            2,
            false,
            CarKind::Ai,
        ),
        (
            crate::tokens::color::CAR_3,
            "Rival Green",
            3,
            false,
            CarKind::Ai,
        ),
    ];
    let mut y = y0;
    for (color, name, rank, active, kind) in rows {
        let rect = Rect::from_min_size(
            Pos2::new(x0, y),
            egui::vec2(CAR_CHIP_WIDTH, CAR_CHIP_HEIGHT),
        );
        let style = CarChip::resolve(active, Some(kind));
        CarChip::paint(painter, rect, style, color, name, Some(rank), Some(kind));
        y += CAR_CHIP_HEIGHT + CAR_CHIP_ROW_GAP;
    }
    y
}

/// Draws the full HUD specimen into `rect` (currently only ever called with
/// [`CANVAS_RECT`]).
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first (every widget's text draw resolves a
/// `FontFamily::Name(..)`).
fn draw_game_gallery(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0, crate::tokens::color::SURFACE_PAGE);

    let x0 = rect.min.x + 24.0;
    let mut y = rect.min.y + 24.0;
    y = draw_hud_strip(painter, x0, y);
    y = draw_lap_meter(painter, x0, y);
    let _ = draw_car_chips(painter, x0, y);
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_RECT, draw_game_gallery};

    /// AC8 — one wgpu frame renders the HUD specimen (Telemetry strip +
    /// `LapMeter` + `CarChip` standings) and matches the minted golden
    /// exactly (flat regions; AA-exempt edges per `gallery.rs`'s
    /// `widget_gallery` precedent).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn game_gallery_matches_golden() {
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

        // Frame-1-install/frame-2-draw (gallery.rs's widget_gallery
        // precedent): fonts can only be installed from inside the harness
        // closure, and `set_fonts` only takes effect at the *next* pass.
        let mut fonts_installed = false;
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
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                draw_game_gallery(&painter, CANVAS_RECT);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        // threshold(1.0): matches gallery.rs's measured cross-renderer noise
        // ceiling (1-level channel rounding on AA text pixels).
        // failed_pixel_count_threshold stays exact (0).
        let options = egui_kittest::SnapshotOptions::new()
            .threshold(1.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) = egui_kittest::try_image_snapshot_options(&image, "game_gallery", &options)
        {
            panic!("{err}");
        }
    }
}
