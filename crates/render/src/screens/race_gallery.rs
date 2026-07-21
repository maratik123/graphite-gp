//! `RaceScreen` wgpu golden (AC7) + `egui_kittest` interaction test (AC8) —
//! mirrors `lab_gallery.rs`'s frame-1-install / frame-2-draw dance and
//! Rc<Cell> click-rect-capture idiom (design § *Test Design*).

use super::race::{CAR_NAMES, RaceInput, RaceScreen};
use crate::{CarRender, Overlays, Scene};
use gp_core::geom::{Corridor, Orient, Point, Side, walls_from_boundary};
use gp_core::sim::CarState;
use gp_core::track::{
    Centerline, RaceDir, SField, StartFinish, StartGrid, TimingGate, TrackArtifact, TrackMetrics,
};

/// The golden's fixed canvas: wide enough to fit the two-column layout
/// (a comfortable left column + `COL_GAP` + `COL_RIGHT_W` right column) with
/// headroom for the HUD/toolbar bands and both right-column `Card`s —
/// mirrors `lab_gallery.rs::CANVAS_SIZE`'s sizing rationale.
const CANVAS_SIZE: egui::Vec2 = egui::Vec2::new(900.0, 620.0);

/// The fixed overlays state the golden/interaction tests render: the JSX
/// initial default (`Screens.jsx:78` `{ grid: true, heatmap: false, fastest:
/// false }`).
const FIXED_OVERLAYS: Overlays = Overlays {
    speed_heatmap: false,
    fastest_lap: false,
    grid: true,
};

/// The fixed active-car index (the player, `CAR_NAMES[0]`).
const FIXED_ACTIVE: usize = 0;
/// The fixed laps-done/total the golden/interaction tests render.
const FIXED_LAPS_DONE: i32 = 2;
/// The fixed total-laps the golden/interaction tests render.
const FIXED_TOTAL_LAPS: i32 = 5;

/// A hand-built `TrackArtifact` fixture: a chunky rounded-rect ring (mirrors
/// `lab_gallery.rs::fixture_track`'s pattern).
fn fixture_track() -> TrackArtifact {
    let mut corridor = Corridor::new(Point::new(0, 0), 16, 16);
    for x in 2..=13 {
        for y in 2..=13 {
            let in_hole = (6..=9).contains(&x) && (6..=9).contains(&y);
            if !in_hole {
                corridor.set(Point::new(x, y), true);
            }
        }
    }
    let walls = walls_from_boundary(&corridor);

    TrackArtifact {
        walls,
        sf: StartFinish {
            chord: vec![
                Point::new(7, 2),
                Point::new(7, 3),
                Point::new(7, 4),
                Point::new(7, 5),
            ],
            orient: Orient::Vertical,
            gate: TimingGate {
                behind: vec![],
                forward: Side::East,
            },
        },
        corridor,
        race_dir: RaceDir::Cw,
        s_field: SField::default(),
        start_grid: StartGrid::default(),
        centerline: Centerline::default(),
        metrics: TrackMetrics::default(),
        width_min: 3,
    }
}

/// The fixed 3-car render slice (player index 0 + 2 rivals), each with a
/// non-zero velocity so the HUD/velocity-arrow layers are non-degenerate.
///
/// Player (index 0) sits at `(5, 7)` with `v = (-1, 0)`: the Coast-cell
/// destination `(4, 7)` stays in the outer ring (outside the `[6, 9]²`
/// hole), so `Action::Coast` is legal for the active car — AC3/AC8's
/// `MovePad` interaction test needs a real, clickable Coast cell.
fn fixture_cars(trails: &[[Point; 2]; 3]) -> [CarRender<'_>; 3] {
    [
        CarRender::new(
            CarState {
                x: 5,
                y: 7,
                vx: -1,
                vy: 0,
            },
            0,
            &trails[0],
            true,
            0.0,
        ),
        CarRender::new(
            CarState {
                x: 8,
                y: 4,
                vx: 0,
                vy: 1,
            },
            1,
            &trails[1],
            false,
            0.0,
        ),
        CarRender::new(
            CarState {
                x: 4,
                y: 10,
                vx: -1,
                vy: 1,
            },
            2,
            &trails[2],
            false,
            0.0,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        CANVAS_SIZE, CAR_NAMES, FIXED_ACTIVE, FIXED_LAPS_DONE, FIXED_OVERLAYS, FIXED_TOTAL_LAPS,
        RaceInput, RaceScreen, Scene, fixture_cars, fixture_track,
    };
    use gp_core::geom::Point;
    use std::cell::Cell;
    use std::rc::Rc;

    /// AC1/AC2/AC3/AC5/AC7 — one wgpu frame renders the whole `RaceScreen`
    /// and matches the minted `race_screen.png` exactly (flat regions; AA
    /// edges exempt via `threshold(1.0)` + `failed_pixel_count_threshold(0)`,
    /// the text-bearing-screen setting, `lab_gallery.rs`'s precedent).
    ///
    /// The on-ink `LapMeter`'s `/total` readout renders `TEXT_FAINT`
    /// (deliberately de-emphasized next to the bright `PAPER_0` laps-done
    /// number) — not a defect, see design § Test Design subtask 8.
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn race_screen_matches_golden() {
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
        let trails: [[Point; 2]; 3] = [
            [Point::new(3, 7), Point::new(4, 7)],
            [Point::new(8, 2), Point::new(8, 3)],
            [Point::new(6, 10), Point::new(5, 10)],
        ];
        let cars = fixture_cars(&trails);

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
                let _ = RaceScreen::new(RaceInput {
                    scene: Scene {
                        track: &track,
                        cars: &cars,
                        reduced_motion: false,
                        overlays: FIXED_OVERLAYS,
                    },
                    active: FIXED_ACTIVE,
                    laps_done: FIXED_LAPS_DONE,
                    total_laps: FIXED_TOTAL_LAPS,
                })
                .show(ui);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        let options = egui_kittest::SnapshotOptions::new()
            .threshold(1.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) = egui_kittest::try_image_snapshot_options(&image, "race_screen", &options)
        {
            panic!("{err}");
        }
    }

    /// AC3/AC4/AC6/AC8 — a rest frame emits `action == None`, `finish ==
    /// false`; clicking the Coast button rect emits `Action::Coast`; clicking
    /// the `MovePad` rect center (the Coast cell of the plus) also emits
    /// `Action::Coast`. No `render()` here (default harness), so —
    /// like `lab_gallery.rs`'s interaction test — `Harness::builder()` itself
    /// is the Miri-abort cause (`getcwd`), not wgpu.
    #[cfg_attr(
        miri,
        ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml \
                  lookup, unsupported under Miri isolation (no render() here, so \
                  not the golden's Vulkan-dlopen cause)"
    )]
    #[test]
    fn race_screen_coast_and_movepad_emit_action() {
        let saw_action: Rc<Cell<Option<gp_core::sim::Action>>> = Rc::new(Cell::new(None));
        let saw_finish = Rc::new(Cell::new(false));
        let coast_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let finish_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let movepad_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));

        let saw_action_c = Rc::clone(&saw_action);
        let saw_finish_c = Rc::clone(&saw_finish);
        let coast_rect_c = Rc::clone(&coast_rect);
        let finish_rect_c = Rc::clone(&finish_rect);
        let movepad_rect_c = Rc::clone(&movepad_rect);

        let track = fixture_track();
        let trails: [[Point; 2]; 3] = [
            [Point::new(3, 7), Point::new(4, 7)],
            [Point::new(8, 2), Point::new(8, 3)],
            [Point::new(6, 10), Point::new(5, 10)],
        ];
        let cars = fixture_cars(&trails);

        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_SIZE)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                let resp = RaceScreen::new(RaceInput {
                    scene: Scene {
                        track: &track,
                        cars: &cars,
                        reduced_motion: false,
                        overlays: FIXED_OVERLAYS,
                    },
                    active: FIXED_ACTIVE,
                    laps_done: FIXED_LAPS_DONE,
                    total_laps: FIXED_TOTAL_LAPS,
                })
                .show(ui);
                if let Some(action) = resp.action {
                    saw_action_c.set(Some(action));
                }
                if resp.finish {
                    saw_finish_c.set(true);
                }
                coast_rect_c.set(Some(resp.coast_response.rect));
                finish_rect_c.set(Some(resp.finish_response.rect));
                movepad_rect_c.set(Some(resp.movepad_response.rect));
            });

        harness.run_steps(1);

        assert!(
            saw_action.get().is_none(),
            "AC8: rest frame — no action yet"
        );
        assert!(!saw_finish.get(), "AC8: rest frame — no finish yet");

        let click = |harness: &mut egui_kittest::Harness<'_, _>,
                     rect: &Cell<Option<egui::Rect>>| {
            let center = rect
                .get()
                .expect("rest frame captured the button/pad rect")
                .center();
            harness.hover_at(center);
            harness.step();
            harness.drag_at(center);
            harness.step();
            harness.drop_at(center);
            harness.step();
        };

        click(&mut harness, &coast_rect);
        assert_eq!(
            saw_action.get(),
            Some(gp_core::sim::Action::Coast),
            "AC4: Coast button click emits Action::Coast"
        );

        saw_action.set(None);
        click(&mut harness, &movepad_rect);
        assert_eq!(
            saw_action.get(),
            Some(gp_core::sim::Action::Coast),
            "AC3/AC8: MovePad center (Coast cell) click emits Action::Coast"
        );
    }

    /// AC5 — `CAR_NAMES` backs the fixture's Standings rows (drift guard: the
    /// gallery's 3-car fixture stays within the 6-name table).
    #[test]
    fn fixture_car_count_is_within_car_names_table() {
        let trails: [[Point; 2]; 3] = [
            [Point::new(0, 0), Point::new(0, 0)],
            [Point::new(0, 0), Point::new(0, 0)],
            [Point::new(0, 0), Point::new(0, 0)],
        ];
        let cars = fixture_cars(&trails);
        assert!(cars.len() <= CAR_NAMES.len());
    }
}
