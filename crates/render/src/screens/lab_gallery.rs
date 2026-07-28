//! `LabScreen` wgpu golden (AC6) + `egui_kittest` interaction test (AC4) —
//! mirrors `setup_gallery.rs`'s frame-1-install / frame-2-draw dance and
//! Rc<Cell> click-rect-capture idiom (design § *Test Design*).

use super::lab::{LabInput, LabScreen, PhaseStatus};
use crate::track::test_support::{scene_metrics, scene_track};
use gp_core::track::TrackArtifact;

/// The golden's fixed canvas: wide enough to fit the two-column layout
/// (left column + `COL_GAP` + 320px right column + outer padding) with
/// headroom for the header/canvas/action bands and the two right-column
/// `Card`s.
const CANVAS_SIZE: egui::Vec2 = egui::Vec2::new(900.0, 620.0);

/// The fixed phase-status array the golden/interaction tests render — a mix
/// of `Ok`/`Repair` so both badge tones are exercised in the same frame
/// (AC3).
const FIXED_PHASES: [PhaseStatus; 7] = [
    PhaseStatus::Ok,
    PhaseStatus::Ok,
    PhaseStatus::Ok,
    PhaseStatus::Repair,
    PhaseStatus::Ok,
    PhaseStatus::Repair,
    PhaseStatus::Ok,
];

/// The fixed header seed the golden/interaction tests render.
const FIXED_SEED: u64 = 42;

/// A `TrackArtifact` fixture: the shared chunky rounded-rect ring
/// ([`scene_track`]) with [`scene_metrics`]-populated `speed_heatmap` +
/// `fastest_lap`, plus the `vmax_attain`/`tempo` this screen's oracle report
/// renders (spec § Technical constraints; AC6). The S/F chord is 4 points
/// long, so `sf.width() == 4`.
fn fixture_track() -> TrackArtifact {
    let mut track = scene_track();
    let mut metrics = scene_metrics(&track.corridor);
    metrics.vmax_attain = Some(7);
    metrics.tempo = Some(0.87);
    track.metrics = metrics;
    track
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_SIZE, FIXED_PHASES, FIXED_SEED, LabInput, LabScreen, fixture_track};
    use crate::BakedTrackGeometry;
    use crate::gallery_support::click;
    use std::cell::Cell;
    use std::rc::Rc;

    /// AC1/AC2/AC3/AC6 — one wgpu frame renders the whole `LabScreen`
    /// (header + canvas + action row + oracle/phases `Card`s) and matches
    /// the minted `lab_screen.png` exactly (flat regions; AA edges exempt
    /// via `threshold(1.0)` + `failed_pixel_count_threshold(0)`, the
    /// text-bearing-screen setting, `setup_gallery.rs`'s precedent).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn lab_screen_matches_golden() {
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
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);

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
                let _ = LabScreen::new(LabInput {
                    track: &track,
                    geometry: &geometry,
                    phases: FIXED_PHASES,
                    valid: true,
                    seed: FIXED_SEED,
                })
                .show(ui);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        let options = egui_kittest::SnapshotOptions::new()
            .threshold(1.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) = egui_kittest::try_image_snapshot_options(&image, "lab_screen", &options) {
            panic!("{err}");
        }
    }

    /// AC4 — Regenerate/Test lap/Menu are click **signals**: rest frame →
    /// all three flags `false`; a click on each button's captured rect
    /// flips exactly that flag `true`. No `render()` here (default
    /// harness), so — like `setup_gallery.rs`'s interaction test —
    /// `Harness::builder()` itself is the Miri-abort cause (`getcwd` via
    /// `kittest.toml` lookup), not wgpu.
    #[cfg_attr(
        miri,
        ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml \
                  lookup, unsupported under Miri isolation (no render() here, so \
                  not the golden's Vulkan-dlopen cause)"
    )]
    #[test]
    fn lab_screen_click_signals_flip_flags() {
        let saw_regenerate = Rc::new(Cell::new(false));
        let saw_test_lap = Rc::new(Cell::new(false));
        let saw_menu = Rc::new(Cell::new(false));
        let regenerate_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let test_lap_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let menu_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));

        let saw_regenerate_c = Rc::clone(&saw_regenerate);
        let saw_test_lap_c = Rc::clone(&saw_test_lap);
        let saw_menu_c = Rc::clone(&saw_menu);
        let regenerate_rect_c = Rc::clone(&regenerate_rect);
        let test_lap_rect_c = Rc::clone(&test_lap_rect);
        let menu_rect_c = Rc::clone(&menu_rect);

        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_SIZE)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                let resp = LabScreen::new(LabInput {
                    track: &track,
                    geometry: &geometry,
                    phases: FIXED_PHASES,
                    valid: true,
                    seed: FIXED_SEED,
                })
                .show(ui);
                if resp.regenerate {
                    saw_regenerate_c.set(true);
                }
                if resp.test_lap {
                    saw_test_lap_c.set(true);
                }
                if resp.menu {
                    saw_menu_c.set(true);
                }
                regenerate_rect_c.set(Some(resp.regenerate_response.rect));
                test_lap_rect_c.set(Some(resp.test_lap_response.rect));
                menu_rect_c.set(Some(resp.menu_response.rect));
            });

        harness.run_steps(1);

        assert!(!saw_regenerate.get(), "AC4: rest frame — no click yet");
        assert!(!saw_test_lap.get(), "AC4: rest frame — no click yet");
        assert!(!saw_menu.get(), "AC4: rest frame — no click yet");

        click(&mut harness, &regenerate_rect);
        assert!(
            saw_regenerate.get(),
            "AC4: Regenerate click sets regenerate == true"
        );
        assert!(!saw_test_lap.get());
        assert!(!saw_menu.get());

        click(&mut harness, &test_lap_rect);
        assert!(
            saw_test_lap.get(),
            "AC4: Test lap click sets test_lap == true"
        );

        click(&mut harness, &menu_rect);
        assert!(saw_menu.get(), "AC4: Menu click sets menu == true");
    }
}
