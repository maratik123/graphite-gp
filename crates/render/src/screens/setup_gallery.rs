//! `SetupScreen` wgpu golden (design `2026-07-20-render-setup-screen` §
//! *Golden test*) + the `egui_kittest` interaction test (§ *Interaction
//! test*, subtask 4).
//!
//! Unlike the widget galleries (`widgets/game_gallery.rs`), this drives the
//! **real** [`super::setup::SetupScreen::show`] inside an
//! `egui_kittest::Harness` — no separate manual-layout `paint` path to keep
//! in sync. `#[cfg_attr(miri, ignore)]` on the golden: it drives wgpu, which
//! `dlopen`s the Vulkan ICD (no FFI under Miri).

use super::setup::SetupScreen;
use super::{Difficulty, RaceConfig};

/// The golden's fixed canvas: 640×760 logical points at
/// `pixels_per_point = 1.0` — taller than `widgets/game_gallery.rs`'s
/// 640×420 to fit the stacked `SetupScreen` (wordmark + card + button +
/// footer, design § *Golden test*). Confirmed at mint: the full stack fits
/// with a small margin below the footer — a fresh 760px start clipped
/// nothing but left ~170px of dead space, so this is trimmed down from that
/// starting point rather than left oversized.
const CANVAS_SIZE: egui::Vec2 = egui::Vec2::new(640.0, 620.0);

/// The fixed config the golden renders — mid-range values so every widget
/// shows a non-default-looking state.
const FIXED_CONFIG: RaceConfig = RaceConfig {
    cars: 4,
    laps: 3,
    v_target: 5,
    difficulty: Difficulty::Pro,
};

#[cfg(test)]
mod tests {
    use super::{CANVAS_SIZE, FIXED_CONFIG, SetupScreen};

    /// `AC8b` / AC5 / AC7 — one wgpu frame renders the whole `SetupScreen`
    /// (wordmark + card with the four inputs + primary button + footer) at
    /// rest and matches the minted `setup_screen.png` exactly (flat
    /// regions; AA edges exempt via `threshold(1.0)` +
    /// `failed_pixel_count_threshold(0)`, the `game_gallery` precedent).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn setup_screen_matches_golden() {
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
        // `set_fonts` only takes effect at the *next* pass. No pointer
        // input is ever injected, so every widget renders at rest,
        // deterministically.
        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_SIZE)
            .with_pixels_per_point(1.0)
            .with_theme(egui::Theme::Light)
            .renderer(renderer)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                let _ = SetupScreen::new(FIXED_CONFIG).show(ui);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        // threshold(1.0): matches game_gallery.rs's measured cross-renderer
        // noise ceiling (1-level channel rounding on AA text pixels).
        // failed_pixel_count_threshold stays exact (0).
        let options = egui_kittest::SnapshotOptions::new()
            .threshold(1.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) = egui_kittest::try_image_snapshot_options(&image, "setup_screen", &options)
        {
            panic!("{err}");
        }
    }
}
