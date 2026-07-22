//! `ResultsScreen` wgpu golden (AC6) + `egui_kittest` interaction test (AC5) —
//! mirrors `race_gallery.rs`'s frame-1-install / frame-2-draw dance and
//! `Rc<Cell<..>>` click-rect-capture idiom (design § *Test Design*).

use super::results::{ResultsInput, ResultsScreen};

/// The golden's fixed canvas: wide enough for the 560 column + side margins
/// (`setup_gallery.rs::CANVAS_SIZE`'s 640 width), and taller than
/// `setup_gallery.rs`'s `640×620` — Results stacks more vertical content on
/// the same 560 column (header + a 4-row standings `Card` with a hairline
/// divider + telemetry row + the action row). Confirmed at mint: a fresh
/// 640×720 start clipped nothing but left ~120-130px of dead space below the
/// action row, so this is trimmed down to 610 from that starting point
/// (`setup_gallery.rs`'s trim-down precedent), still fully containing every
/// element down through the action row with a small margin.
const CANVAS_SIZE: egui::Vec2 = egui::Vec2::new(640.0, 610.0);

#[cfg(test)]
mod tests {
    use super::{CANVAS_SIZE, ResultsInput, ResultsScreen};
    use crate::gallery_support::{FIXED_SUMMARY, click, fixture_standings};
    use std::cell::Cell;
    use std::rc::Rc;

    /// AC1/AC2/AC3/AC4/AC6 — one wgpu frame renders the whole
    /// `ResultsScreen` and matches the minted `results_screen.png` exactly
    /// (flat regions; AA edges exempt via `threshold(1.0)` +
    /// `failed_pixel_count_threshold(0)`, the text-bearing-screen setting,
    /// `race_gallery.rs`'s precedent). `again_icon` is left unset
    /// (`rotate-ccw` is unvendored — text-only "Race again" button).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn results_screen_matches_golden() {
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
        let standings = fixture_standings();

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
                let _ = ResultsScreen::new(ResultsInput {
                    standings: &standings,
                    summary: FIXED_SUMMARY,
                })
                .show(ui);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        let options = egui_kittest::SnapshotOptions::new()
            .threshold(1.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) =
            egui_kittest::try_image_snapshot_options(&image, "results_screen", &options)
        {
            panic!("{err}");
        }
    }

    /// AC5 — a rest frame emits `again == false && menu == false`; clicking
    /// the "Race again" button rect emits `again == true` (menu unchanged);
    /// after resetting, clicking the "Menu" button rect emits `menu == true`.
    /// No `render()` here (default harness), so — like `race_gallery.rs`'s
    /// interaction test — `Harness::builder()` itself is the Miri-abort
    /// cause (`getcwd`), not wgpu.
    #[cfg_attr(
        miri,
        ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml \
                  lookup, unsupported under Miri isolation (no render() here, so \
                  not the golden's Vulkan-dlopen cause)"
    )]
    #[test]
    fn results_again_and_menu_emit_intents() {
        let saw_again: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let saw_menu: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let again_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let menu_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));

        let saw_again_c = Rc::clone(&saw_again);
        let saw_menu_c = Rc::clone(&saw_menu);
        let again_rect_c = Rc::clone(&again_rect);
        let menu_rect_c = Rc::clone(&menu_rect);

        let standings = fixture_standings();

        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_SIZE)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                let resp = ResultsScreen::new(ResultsInput {
                    standings: &standings,
                    summary: FIXED_SUMMARY,
                })
                .show(ui);
                if resp.again {
                    saw_again_c.set(true);
                }
                if resp.menu {
                    saw_menu_c.set(true);
                }
                again_rect_c.set(Some(resp.again_response.rect));
                menu_rect_c.set(Some(resp.menu_response.rect));
            });

        harness.run_steps(1);

        assert!(!saw_again.get(), "rest frame — no again click yet");
        assert!(!saw_menu.get(), "rest frame — no menu click yet");

        click(&mut harness, &again_rect);
        assert!(saw_again.get(), "AC5: Race again click emits again == true");
        assert!(!saw_menu.get(), "AC5: Race again click does not emit menu");

        saw_again.set(false);
        click(&mut harness, &menu_rect);
        assert!(saw_menu.get(), "AC5: Menu click emits menu == true");
    }
}
