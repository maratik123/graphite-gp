//! `AppShell` wgpu golden (AC3) + `egui_kittest` click-through smoke (AC4)
//! (design § Test Design, subtask 4) — mirrors `screens::setup_gallery`'s
//! frame-1-install / frame-2-draw dance and `Rc<Cell<..>>` click-rect-capture
//! idiom. Drives the real [`AppShell::show`] inside a `Harness` — no
//! separate manual-layout `paint` path to keep in sync.

use crate::app::{AppShell, ShellSession};
use crate::gallery_support::{FIXED_SUMMARY, fixture_standings};
use crate::track::test_support::scene_track_with_metrics;
use crate::{BakedTrackGeometry, CarRender, Difficulty, PhaseStatus, RaceConfig, StandingEntry};
use gp_core::geom::Point;
use gp_core::sim::CarState;
use gp_core::track::TrackArtifact;

/// The golden/click-through's fixed canvas. Height fits `setup_gallery.rs`'s
/// `SetupScreen` body (640×620) plus [`crate::app::TOP_BAR_H`] of top-bar
/// band on top; confirmed at mint. Width is `race_gallery.rs::CANVAS_SIZE`'s
/// 900 (not `setup_gallery.rs`'s narrower 640) — measured requirement: at
/// 640, `RaceScreen`'s narrow toolbar column packs its 3 `Switch`es right up
/// against the right-aligned "Finish →" button, and the overlapping hit
/// regions swallow the button's hover/click in
/// [`click_through_setup_to_menu`] (`RaceScreen`'s own gallery avoids this by
/// always using the 900-wide canvas).
const CANVAS_SIZE: egui::Vec2 = egui::Vec2::new(960.0, 680.0);

/// The fixed config the golden/click-through render — the mock's startup
/// default (design § Key decisions).
const FIXED_CONFIG: RaceConfig = RaceConfig {
    cars: 4,
    laps: 5,
    v_target: 7,
    difficulty: Difficulty::Pro,
};

/// The app-shell golden/click-through's fixture track — reviewer follow-up
/// (PR #118 round 2): this fn previously hand-rolled a thin 3×3-ring
/// corridor whose 1-cell-wide arm made the S/F chord read as a stripe along
/// the straightaway rather than crossing it (the exact pre-#100 look PR #100
/// already fixed once, in `track::golden`'s fixture, for the track-canvas
/// goldens). Reuses `track::test_support::scene_track_with_metrics` — the
/// same wide "chunky rounded-rect" corridor `track::golden` draws — so the
/// Lab oracle report/heatmap render representatively and the S/F crosses the
/// top arm, instead of duplicating the geometry a second time. Keeps the
/// `fixture_track` name; only its body changed, so callers are unaffected.
fn fixture_track() -> TrackArtifact {
    scene_track_with_metrics()
}

/// A 4-car render fixture (player index 0 + 3 rivals, matching
/// [`fixture_standings`]'s 4 entries), each with a non-zero velocity so the
/// HUD/velocity-arrow layers are non-degenerate — reviewer follow-up (PR
/// #118 round 1): `app_shell_race_matches_golden` previously rendered with
/// an empty `cars` slice, an unrepresentative fixture that showed a
/// degenerate HUD (`0.00`/`(0,0)`) and clipped the Standings card's title.
/// Mirrors `screens::race_gallery::fixture_cars`'s shape.
///
/// Repositioned (PR #118 round 2, alongside [`fixture_track`]'s widening)
/// onto [`fixture_track`]'s top straightaway — drivable cells with `x` in
/// `2..=13` and `y` in `2..=5` are outside the corridor's centered hole
/// (`x∈[6,9] × y∈[6,9]`) regardless of `x`, so every car and trail point
/// below sits inside `2..=13 × 2..=5`, near the S/F chord at `x = 7`.
fn fixture_race_cars(trails: &[[Point; 2]; 4]) -> [CarRender<'_>; 4] {
    [
        CarRender::new(
            CarState {
                x: 10,
                y: 3,
                vx: 1,
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
                y: 2,
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
                x: 12,
                y: 4,
                vx: -1,
                vy: 0,
            },
            2,
            &trails[2],
            false,
            0.0,
        ),
        CarRender::new(
            CarState {
                x: 9,
                y: 5,
                vx: 0,
                vy: -1,
            },
            3,
            &trails[3],
            false,
            0.0,
        ),
    ]
}

/// The fixed `ShellSession` the gallery renders, over `cars` (empty for the
/// non-Race screens) — the single source of the session literal every test
/// otherwise rebuilt verbatim.
fn shell_session<'a>(
    track: &'a TrackArtifact,
    geometry: &'a BakedTrackGeometry,
    cars: &'a [CarRender<'a>],
    standings: &'a [StandingEntry],
) -> ShellSession<'a> {
    ShellSession {
        track,
        geometry,
        cars,
        reduced_motion: false,
        active: 0,
        laps_done: 0,
        total_laps: 1,
        phases: [PhaseStatus::Ok; 7],
        valid: true,
        seed: 7,
        standings,
        summary: FIXED_SUMMARY,
    }
}

/// Renders `shell` (already driven to its target screen) for one wgpu frame
/// over the fixed [`shell_session`] and asserts it matches the `snapshot_name`
/// golden — the shared body of the `app_shell*` golden tests (flat regions;
/// AA edges exempt via `threshold(1.0)` + `failed_pixel_count_threshold(0)`).
fn render_shell_golden(
    mut shell: AppShell,
    track: &TrackArtifact,
    geometry: &BakedTrackGeometry,
    cars: &[CarRender<'_>],
    standings: &[StandingEntry],
    snapshot_name: &str,
) {
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
            let session = shell_session(track, geometry, cars, standings);
            let _ = shell.show(ui, session);
        });

    harness.run_steps(1);

    let image = harness.render().expect("offscreen wgpu render failed");

    let options = egui_kittest::SnapshotOptions::new()
        .threshold(1.0)
        .failed_pixel_count_threshold(0);
    if let Err(err) = egui_kittest::try_image_snapshot_options(&image, snapshot_name, &options) {
        panic!("{err}");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CANVAS_SIZE, FIXED_CONFIG, fixture_race_cars, fixture_standings, fixture_track,
        render_shell_golden, shell_session,
    };
    use crate::BakedTrackGeometry;
    use crate::app::{AppShell, Nav, Screen, TOP_BAR_H};
    use crate::screens::race::{RaceInput, RaceScreen, active_legal_mask};
    use crate::{Overlays, Scene};
    use egui::epaint::Primitive;
    use gp_core::geom::Point;
    use gp_core::sim::Action;
    use std::cell::Cell;
    use std::rc::Rc;

    /// AC9 — relocated from the deleted `placeholder.rs::tessellation_smoke`
    /// canary, repointed onto the production [`AppShell::show`] draw path
    /// (design § Test Design, subtask 6). A full pass over the shell yields
    /// non-empty tessellated output, with vertex/index counts strengthened
    /// past "non-empty": a non-empty `Vec<ClippedPrimitive>` can still carry
    /// zero-geometry meshes.
    ///
    /// This test owns its `egui::Context` and calls `set_fonts` **before**
    /// the first (and only) `run_ui` pass, so one pass suffices — `run_ui`'s
    /// internal `begin_pass` consumes the deferred `new_font_definitions`
    /// and the fonts are live for that same pass (design D11).
    #[test]
    // Drawing the sample rasterises real glyphs — `epaint`'s glyph cache
    // (`epaint::text::font::FontCell::allocate_glyph_uncached`,
    // `epaint-0.35.0/src/text/font.rs:280`) reaches `vello_cpu`'s
    // `U8Kernel::copy_solid`, whose body is a **checked**
    // `bytemuck::cast_slice_mut::<u8, u32>` over the pixmap buffer. Native
    // malloc over-aligns the buffer so the cast succeeds; Miri's allocator
    // grants only `u8`'s alignment of 1, so the checked cast correctly
    // refuses, panicking with `TargetAlignmentGreaterAndInputNotAligned`.
    // This is a distinct abort site from the golden's FFI/`dlopen` ignore
    // below — do not copy that reason here, it would be a false
    // justification for a different failure.
    #[cfg_attr(
        miri,
        ignore = "drawing text rasterises glyphs via vello_cpu, whose checked \
                  u8->u32 pixmap cast panics under Miri's 1-byte allocator \
                  alignment (TargetAlignmentGreaterAndInputNotAligned)"
    )]
    fn tessellation_smoke() {
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let standings = fixture_standings();
        let mut shell = AppShell::new(FIXED_CONFIG);

        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::definitions());
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, CANVAS_SIZE)),
            ..Default::default()
        };

        let output = ctx.run_ui(input, |ui| {
            let session = shell_session(&track, &geometry, &[], &standings);
            let _ = shell.show(ui, session);
        });
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);

        assert!(
            !primitives.is_empty(),
            "AppShell::show produced no tessellated primitives"
        );

        let (vertex_count, index_count) = primitives.iter().fold((0usize, 0usize), |acc, p| {
            let Primitive::Mesh(mesh) = &p.primitive else {
                return acc;
            };
            (acc.0 + mesh.vertices.len(), acc.1 + mesh.indices.len())
        });

        assert!(vertex_count > 0, "tessellated meshes carried zero vertices");
        assert!(index_count > 0, "tessellated meshes carried zero indices");
    }

    /// AC3 — one wgpu frame renders the composed shell (top bar + fresh
    /// `SetupScreen` body) and matches the minted `app_shell.png` exactly
    /// (flat regions; AA edges exempt via `threshold(1.0)` +
    /// `failed_pixel_count_threshold(0)`, the text-bearing-frame setting,
    /// `setup_gallery.rs`'s precedent). Fresh shell on `Setup`: `New race`
    /// active, `Race`/`Track lab` disabled.
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn app_shell_matches_golden() {
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let standings = fixture_standings();
        let shell = AppShell::new(FIXED_CONFIG);
        render_shell_golden(shell, &track, &geometry, &[], &standings, "app_shell");
    }

    /// Reviewer follow-up (PR #118 round 1) — same harness/fixtures/options
    /// as [`app_shell_matches_golden`], but the shell is driven to
    /// [`Screen::Lab`] (`Nav::Generate`) before the render so the top bar's
    /// active-pill treatment on the "Track lab" nav item is covered (the
    /// base golden only shows "New race" active on `Setup`).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn app_shell_lab_matches_golden() {
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let standings = fixture_standings();
        let mut shell = AppShell::new(FIXED_CONFIG);
        shell.apply(Nav::Generate);
        assert_eq!(shell.screen(), Screen::Lab, "Generate -> Lab");
        render_shell_golden(shell, &track, &geometry, &[], &standings, "app_shell_lab");
    }

    /// Reviewer follow-up (PR #118 round 1) — same harness/fixtures/options
    /// as [`app_shell_matches_golden`], but the shell is driven to
    /// [`Screen::Race`] (`Nav::Generate` then `Nav::TestLap`) before the
    /// render so the top bar's active-pill treatment on the "Race" nav item
    /// is covered.
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn app_shell_race_matches_golden() {
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let standings = fixture_standings();
        let trails: [[Point; 2]; 4] = [
            [Point::new(8, 3), Point::new(9, 3)],
            [Point::new(8, 3), Point::new(8, 4)],
            [Point::new(13, 4), Point::new(13, 3)],
            [Point::new(10, 5), Point::new(11, 5)],
        ];
        let cars = fixture_race_cars(&trails);
        let mut shell = AppShell::new(FIXED_CONFIG);
        shell.apply(Nav::Generate);
        shell.apply(Nav::TestLap);
        assert_eq!(
            shell.screen(),
            Screen::Race,
            "Generate -> Lab -> TestLap -> Race"
        );
        render_shell_golden(
            shell,
            &track,
            &geometry,
            &cars,
            &standings,
            "app_shell_race",
        );
    }

    /// AC4 — an `egui_kittest` click-through smoke test drives the full loop
    /// Setup→Lab→Race→Results→Menu(Setup) by clicking each screen's forward
    /// control (`AppShell::show`'s `advance_rect`) and asserts the shell
    /// lands on the expected screen at each step. Default (non-wgpu)
    /// harness, no `render()`.
    ///
    /// Miri-ignored: NOT for a golden's reason (no `render()` here) —
    /// `Harness::builder()` itself aborts under Miri isolation via
    /// `getcwd` (`egui_kittest`'s `kittest.toml` lookup), the same cause
    /// `setup_gallery.rs`'s interaction test documents.
    #[cfg_attr(
        miri,
        ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml \
                  lookup, unsupported under Miri isolation (not the golden's \
                  Vulkan-dlopen cause, since this test never calls render())"
    )]
    #[test]
    fn click_through_setup_to_menu() {
        let latest_screen: Rc<Cell<Screen>> = Rc::new(Cell::new(Screen::Setup));
        let latest_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let latest_screen_c = Rc::clone(&latest_screen);
        let latest_rect_c = Rc::clone(&latest_rect);

        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let standings = fixture_standings();
        let mut shell = AppShell::new(FIXED_CONFIG);

        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_SIZE)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                let session = shell_session(&track, &geometry, &[], &standings);
                let resp = shell.show(ui, session);
                latest_screen_c.set(resp.screen);
                latest_rect_c.set(Some(resp.advance_rect));
            });

        // One rest frame, then read the just-drawn screen's forward-control
        // rect, then click it (hover/drag/drop, 3 `step()`s — the
        // `race_gallery.rs` click idiom, no AccessKit label to query).
        let advance = |harness: &mut egui_kittest::Harness<'_, _>| -> egui::Rect {
            harness.run_steps(1);
            latest_rect
                .get()
                .expect("rest frame captured the advance rect")
        };
        let click = |harness: &mut egui_kittest::Harness<'_, _>, rect: egui::Rect| {
            let center = rect.center();
            harness.hover_at(center);
            harness.step();
            harness.drag_at(center);
            harness.step();
            harness.drop_at(center);
            harness.step();
        };

        let rect = advance(&mut harness);
        click(&mut harness, rect);
        assert_eq!(latest_screen.get(), Screen::Lab, "Generate -> Lab");

        let rect = advance(&mut harness);
        click(&mut harness, rect);
        assert_eq!(latest_screen.get(), Screen::Race, "Test lap -> Race");

        let rect = advance(&mut harness);
        click(&mut harness, rect);
        assert_eq!(latest_screen.get(), Screen::Results, "Finish -> Results");

        let rect = advance(&mut harness);
        click(&mut harness, rect);
        assert_eq!(latest_screen.get(), Screen::Setup, "Menu -> Setup");
    }

    /// AC10 — a `MovePad` centre click reaches `ShellResponse.action`
    /// (design § *Q3* — the two-`egui::Context` layout probe). Drives the
    /// real [`AppShell::show`] to [`Screen::Race`], captures the rest
    /// frame's `ui.max_rect()`, re-derives the body rect from
    /// [`TOP_BAR_H`] (the shell's own const — no layout
    /// constant duplicated), and draws the **real** [`RaceScreen`] under
    /// that rect in a fresh `egui::Context` to read the production
    /// `movepad_response.rect`. Clicking that rect's centre on the
    /// original harness selects the pad's `(1,1)` cell — `Action::Coast`
    /// (`movepad.rs::MOVES[0]`) — and the assertion is that
    /// `ShellResponse.action` carries it.
    ///
    /// Miri-ignored for the same *own cause* as
    /// [`click_through_setup_to_menu`]: `Harness::builder()`'s `getcwd`,
    /// not the golden's Vulkan-`dlopen` (no `render()` here).
    #[cfg_attr(
        miri,
        ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml \
                  lookup, unsupported under Miri isolation (no render() here, so \
                  not the golden's Vulkan-dlopen cause)"
    )]
    #[test]
    fn shell_race_arm_forwards_movepad_action() {
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let standings = fixture_standings();
        let trails: [[Point; 2]; 4] = [
            [Point::new(8, 3), Point::new(9, 3)],
            [Point::new(8, 3), Point::new(8, 4)],
            [Point::new(13, 4), Point::new(13, 3)],
            [Point::new(10, 5), Point::new(11, 5)],
        ];
        let cars = fixture_race_cars(&trails);

        // Precondition guard: a fixture drift that leaves Coast illegal for
        // car 0 must fail loudly here, not as a mysterious `None` below.
        assert!(
            active_legal_mask(&track, &cars, 0).contains(Action::Coast),
            "fixture car 0's legal mask must include Coast for the pad-centre \
             click to select it"
        );

        let mut shell = AppShell::new(FIXED_CONFIG);
        shell.apply(Nav::Generate);
        shell.apply(Nav::TestLap);
        assert_eq!(
            shell.screen(),
            Screen::Race,
            "Generate -> Lab -> TestLap -> Race"
        );

        let latest_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let latest_action: Rc<Cell<Option<Action>>> = Rc::new(Cell::new(None));
        let latest_rect_c = Rc::clone(&latest_rect);
        let latest_action_c = Rc::clone(&latest_action);

        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_SIZE)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                latest_rect_c.set(Some(ui.max_rect()));
                let session = shell_session(&track, &geometry, &cars, &standings);
                let resp = shell.show(ui, session);
                // Latched, not overwritten: `clicked()` is level-triggered
                // for exactly one frame, and `Harness::step()` draws further
                // settling frames afterward that would otherwise wipe the
                // captured `Some` back to `None` (mirrors
                // `race_gallery.rs::race_screen_coast_and_movepad_emit_action`'s
                // idiom).
                if let Some(action) = resp.action {
                    latest_action_c.set(Some(action));
                }
            });

        harness.run_steps(1); // fonts
        harness.run_steps(1); // first real draw (rest frame)
        assert_eq!(
            latest_action.get(),
            None,
            "the rest frame must not select an action"
        );

        let full = latest_rect
            .get()
            .expect("rest frame captured the shell's max_rect");
        let body_rect = egui::Rect::from_min_max(
            egui::Pos2::new(full.min.x, full.min.y + TOP_BAR_H),
            full.max,
        );

        // Re-fetch the fixtures — the harness closure above moved its own
        // copies.
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let cars = fixture_race_cars(&trails);

        let probe_ctx = egui::Context::default();
        probe_ctx.set_fonts(crate::fonts::definitions());
        let probe_input = egui::RawInput {
            screen_rect: Some(full),
            ..Default::default()
        };
        let movepad_rect: Rc<Cell<Option<egui::Rect>>> = Rc::new(Cell::new(None));
        let movepad_rect_c = Rc::clone(&movepad_rect);
        let _ = probe_ctx.run_ui(probe_input, |ui| {
            ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
                let input = RaceInput {
                    scene: Scene {
                        track: &track,
                        geometry: &geometry,
                        cars: &cars,
                        reduced_motion: false,
                        overlays: Overlays::default(),
                    },
                    active: 0,
                    laps_done: 0,
                    total_laps: 1,
                };
                let resp = RaceScreen::new(input).show(ui);
                movepad_rect_c.set(Some(resp.movepad_response.rect));
            });
        });
        let pad_rect = movepad_rect
            .get()
            .expect("probe pass captured the MovePad's rect");

        let center = pad_rect.center();
        harness.hover_at(center);
        harness.step();
        harness.drag_at(center);
        harness.step();
        harness.drop_at(center);
        harness.step();

        assert_eq!(latest_action.get(), Some(Action::Coast));
    }

    /// AC10's other half — a non-`Race` screen never carries an action.
    /// Checked on a fresh `Setup` shell and again after `Nav::Generate`
    /// (`Screen::Lab`). Same Miri cause as
    /// [`shell_race_arm_forwards_movepad_action`].
    #[cfg_attr(
        miri,
        ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml \
                  lookup, unsupported under Miri isolation (no render() here, so \
                  not the golden's Vulkan-dlopen cause)"
    )]
    #[test]
    fn shell_non_race_screen_yields_no_action() {
        fn assert_no_action(shell: AppShell, expected_screen: Screen) {
            let track = fixture_track();
            let geometry = BakedTrackGeometry::new(&track);
            let standings = fixture_standings();

            let latest_screen: Rc<Cell<Option<Screen>>> = Rc::new(Cell::new(None));
            let latest_action: Rc<Cell<Option<Action>>> = Rc::new(Cell::new(None));
            let latest_screen_c = Rc::clone(&latest_screen);
            let latest_action_c = Rc::clone(&latest_action);

            let mut shell = shell;
            let mut fonts_installed = false;
            let mut harness = egui_kittest::Harness::builder()
                .with_size(CANVAS_SIZE)
                .build_ui(move |ui| {
                    if !fonts_installed {
                        ui.ctx().set_fonts(crate::fonts::definitions());
                        fonts_installed = true;
                        return;
                    }
                    let session = shell_session(&track, &geometry, &[], &standings);
                    let resp = shell.show(ui, session);
                    latest_screen_c.set(Some(resp.screen));
                    // Latch, mirroring the positive test: a settling frame
                    // would otherwise wipe a transient `Some` back to `None`
                    // and let this assertion pass vacuously.
                    if let Some(action) = resp.action {
                        latest_action_c.set(Some(action));
                    }
                });

            harness.run_steps(2);

            assert_eq!(latest_screen.get(), Some(expected_screen));
            assert_eq!(latest_action.get(), None);
        }

        let mut shell = AppShell::new(FIXED_CONFIG);
        assert_no_action(shell, Screen::Setup);

        shell.apply(Nav::Generate);
        assert_eq!(shell.screen(), Screen::Lab, "Generate -> Lab");
        assert_no_action(shell, Screen::Lab);
    }
}
