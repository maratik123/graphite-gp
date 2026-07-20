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
    use std::cell::Cell;
    use std::rc::Rc;

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

    /// AC6 — the gap the golden cannot reach: it renders no pointer input,
    /// so `generated`'s `.clicked()` wiring and the widget-value → `assemble`
    /// plumbing are never exercised there. Default (non-wgpu) harness, no
    /// `render()` — asserts on the returned `SetupResponse`, not pixels.
    ///
    /// Rest frame: `generated == false` ("emits nothing until pressed") and
    /// `resp.config == FIXED_CONFIG` (the value plumbing round-trips through
    /// the real `show`/`assemble`). Click frame: `hover_at` → `drag_at` →
    /// `drop_at` at the captured rest-frame `resp.response.rect.center()`
    /// (deterministic — no accessible label to query, § design *Interaction
    /// test*) → `generated == true` on some pass, config unchanged.
    ///
    /// Miri-ignored: NOT for the golden's reason (no `render()` here, so no
    /// Vulkan `dlopen`) — `Harness::builder()` itself aborts under Miri
    /// isolation, since its default `SnapshotOptions`/`Config::global()`
    /// calls `std::env::current_dir()` (`getcwd`) while searching for a
    /// `kittest.toml`, which Miri's isolation blocks. Measured via
    /// `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test -p gp-render
    /// setup_screen_generate_click`: the abort is `getcwd not available when
    /// isolation is enabled`, with a backtrace through
    /// `egui_kittest::config::find_kittest_toml`.
    #[cfg_attr(
        miri,
        ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml \
                  lookup, unsupported under Miri isolation (measured — not the \
                  golden's Vulkan-dlopen cause, since this test never calls render())"
    )]
    #[test]
    fn setup_screen_generate_click_plumbs_config_and_flag() {
        // `generated` is a one-shot, single-frame flag (true only on the
        // exact pass the click registers, false again on the very next
        // pass — egui's multi-pass layout can run several internal passes
        // per `step()`/`run()` call, so a plain "latest frame" `Cell` can
        // miss it entirely). `saw_generated` OR-accumulates across every
        // pass instead of overwriting, so it can't miss a one-frame pulse
        // regardless of how many internal passes occur. `latest_config`
        // still only needs the latest value: no widget other than the
        // button is ever interacted with, so it stays `FIXED_CONFIG` on
        // every pass.
        let saw_generated = Rc::new(Cell::new(false));
        let latest_config: Rc<Cell<Option<super::RaceConfig>>> = Rc::new(Cell::new(None));
        let latest_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let saw_generated_for_closure = Rc::clone(&saw_generated);
        let latest_config_for_closure = Rc::clone(&latest_config);
        let latest_rect_for_closure = Rc::clone(&latest_rect);

        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_SIZE)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                let resp = SetupScreen::new(FIXED_CONFIG).show(ui);
                if resp.generated {
                    saw_generated_for_closure.set(true);
                }
                latest_config_for_closure.set(Some(resp.config));
                latest_rect_for_closure.set(Some(resp.response.rect));
            });

        // Frame 2 (frame 1 installed fonts inside `build_ui` itself): draws
        // at rest, no pointer input queued yet.
        harness.run_steps(1);

        assert!(
            !saw_generated.get(),
            "AC6: nothing is emitted before the button is pressed"
        );
        assert_eq!(
            latest_config.get(),
            Some(FIXED_CONFIG),
            "the rest-frame config round-trips the fixed widget values"
        );

        let center = latest_rect
            .get()
            .expect("rest frame captured the button rect")
            .center();
        harness.hover_at(center);
        harness.step();
        harness.drag_at(center);
        harness.step();
        harness.drop_at(center);
        harness.step();

        assert!(
            saw_generated.get(),
            "AC6: clicking Generate track sets generated == true on some pass"
        );
        assert_eq!(
            latest_config.get(),
            Some(FIXED_CONFIG),
            "the config is unchanged by the click itself"
        );
    }
}
