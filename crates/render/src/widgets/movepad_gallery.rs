//! AC7 — the `MovePad` state-matrix golden: a single wgpu snapshot of three
//! `MovePad`s (legal/selected, legal+illegal/pressed, all-illegal), driven
//! directly through [`MovePad::paint`] with FORCED values (no pointer-input
//! simulation — mirrors `game_gallery.rs`'s draw-through-`paint` pattern).
//!
//! A NEW module rather than an extension of `game_gallery` (design § Open
//! question 2 — RESOLVED): `game_gallery.rs`'s docstring explicitly scopes
//! `MovePad` out, and its specimen layout does not fit this state matrix.
//! Deliberately duplicates the wgpu golden harness verbatim (design §
//! Deferred — the shared in-crate helper is out of scope for this task).
//!
//! In-crate `#[cfg(test)]` (not `tests/`) so it can reach `MovePad::paint`
//! (`pub(crate)`). `#[cfg_attr(miri, ignore)]` — drives wgpu, which
//! `dlopen`s the Vulkan ICD (no FFI under Miri).

use super::movepad::MovePad;
use egui::{Pos2, Rect};
use gp_core::sim::{Action, BitFlags};

/// The gallery's fixed canvas: 640×260 logical points at
/// `pixels_per_point = 1.0` — wide enough for 3 pads side by side at
/// `size = 52.0` (`Screens.jsx:129` override).
const CANVAS_RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(640.0, 260.0),
};

/// Pad cell edge (`Screens.jsx:129` override of the `48.0` default).
const PAD_SIZE: f32 = 52.0;
/// Horizontal gap between the three pads.
const PAD_GAP: f32 = 60.0;

/// Draws Pad A (all-legal, `North` selected), Pad B (`Coast`/`East`/`West`
/// legal, `East` pressed — `North`/`South` illegal), and Pad C (all-illegal)
/// left to right, covering legal / illegal / selected / pressed in one
/// frame (AC7).
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first (every widget's text draw resolves a
/// `FontFamily::Name(..)`).
fn draw_movepad_gallery(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 0, crate::tokens::color::SURFACE_PAGE);

    let pad_extent = PAD_SIZE.mul_add(3.0, crate::tokens::spacing::SPACE_1 * 2.0);
    let y0 = rect.min.y + 24.0;
    let x0 = rect.min.x + 24.0;

    let rect_1 = Rect::from_min_size(Pos2::new(x0, y0), egui::vec2(pad_extent, pad_extent));
    MovePad::paint(
        painter,
        rect_1,
        BitFlags::all(),
        Some(Action::North),
        None,
        PAD_SIZE,
    );

    let origin_2 = x0 + pad_extent + PAD_GAP;
    let rect_2 = Rect::from_min_size(Pos2::new(origin_2, y0), egui::vec2(pad_extent, pad_extent));
    let legal_2 = Action::Coast | Action::East | Action::West;
    MovePad::paint(painter, rect_2, legal_2, None, Some(Action::East), PAD_SIZE);

    let origin_3 = origin_2 + pad_extent + PAD_GAP;
    let rect_3 = Rect::from_min_size(Pos2::new(origin_3, y0), egui::vec2(pad_extent, pad_extent));
    MovePad::paint(painter, rect_3, BitFlags::empty(), None, None, PAD_SIZE);
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_RECT, draw_movepad_gallery};

    /// AC7 — one wgpu frame renders the `MovePad` state matrix (legal /
    /// illegal / selected / pressed) and matches the minted golden exactly
    /// (flat regions; AA-exempt edges per `game_gallery.rs`'s precedent).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn movepad_gallery_matches_golden() {
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

        // Frame-1-install/frame-2-draw (game_gallery.rs's precedent): fonts
        // can only be installed from inside the harness closure, and
        // `set_fonts` only takes effect at the *next* pass.
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
                draw_movepad_gallery(&painter, CANVAS_RECT);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        // threshold(1.0): matches the three prior text-bearing goldens
        // (gallery/forms_gallery/game_gallery) — the measured cross-renderer
        // noise ceiling, 1-level channel rounding on AA glyph pixels. Applied
        // up-front (owner decision) because MovePad draws arrow + sublabel
        // glyphs; local mint passes at 0.0, but text goldens historically red
        // CI at 0.0 (the AA-rounding trap). failed_pixel_count_threshold stays
        // exact (0), so any real regression (many pixels, or >1-level) fails.
        let options = egui_kittest::SnapshotOptions::new()
            .threshold(1.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) =
            egui_kittest::try_image_snapshot_options(&image, "movepad_gallery", &options)
        {
            panic!("{err}");
        }
    }
}
