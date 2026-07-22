//! # gp-game — Block 3b: game loop & orchestration (design doc §3b, §6)
//!
//! The runnable binary. Wires generation (block 1) → the physics core (block 3a)
//! → rendering (block 2), with player and AI controllers driving the same engine.
//! Kept separate from block 3a so training (thousands of headless envs) and live
//! play share one, non-diverging physics implementation.
//!
//! Owns the window + event loop (a deliberate override of issue #11's text in
//! favor of design doc §6 — see `ai-docs/key-decisions.md`); `gp-render`
//! stays draw-only.
//!
//! Wires `gp_render::AppShell` (issue #23) as the app's `eframe::App`, driven
//! by a **hand-built fixture** `TrackArtifact` + fixture cars/standings/
//! summary/phases — `gp_gen::generate` is an unimplemented `todo!()` stub
//! (`crates/gen/src/lib.rs`) that would panic at startup, so this binary
//! constructs the fixture directly from `gp_core::geom::{Corridor,
//! walls_from_boundary}` instead (design `2026-07-22-render-app-shell` §
//! *The binary hand-builds the fixture track*), mirroring the render-safe
//! pattern `gp-render`'s own `track/mod.rs::fixture_track_with_metrics` test
//! fixture already exercises. Real generation → sim → AI orchestration is
//! block 3b's own future work (spec § Deferred).

use eframe::egui;
use gp_core::geom::{Corridor, Orient, Point, Side, walls_from_boundary};
use gp_core::sim::CarState;
use gp_core::track::{
    Centerline, RaceDir, SField, StartFinish, StartGrid, TimingGate, TrackArtifact, TrackMetrics,
};
use gp_render::screens::{Difficulty, PhaseStatus, RaceConfig, RaceSummary, StandingEntry};
use gp_render::widgets::CarKind;
use gp_render::{AppShell, CarRender, ShellSession};

/// The mock's startup config default (`App.jsx`'s `useState` seed —
/// `docs/design-system/ui_kits/game/App.jsx`).
const STARTUP_CONFIG: RaceConfig = RaceConfig {
    cars: 4,
    laps: 5,
    v_target: 7,
    difficulty: Difficulty::Pro,
};

/// The fixture header `seed <N>` `LabScreen` displays.
const FIXTURE_SEED: i32 = 7;

/// The fixture cars' names/colors follow `gp_render::screens::race::CAR_NAMES`
/// order; only the first 4 are used (`STARTUP_CONFIG.cars`).
const FIXTURE_CAR_COUNT: usize = 4;

/// The app shell, driven by a hand-built fixture session (spec § Key
/// decisions — `gp-game` wiring).
struct GraphiteGpApp {
    /// The router owning `Screen`/`RaceConfig`/`Overlays`/`has_generated`.
    shell: AppShell,
    /// The hand-built fixture track (a wide chunky rounded-rect loop),
    /// shared by the Lab canvas and the Race canvas.
    track: TrackArtifact,
    /// Each fixture car's current state, in `CAR_NAMES` order.
    car_states: Vec<CarState>,
    /// Each fixture car's trail (prior cells), parallel to `car_states`.
    trails: Vec<Vec<Point>>,
    /// The fixture Ф1–Ф7 generation-phase statuses `LabScreen` displays.
    phases: [PhaseStatus; 7],
    /// The fixture rank-ordered standings `ResultsScreen` displays.
    standings: Vec<StandingEntry>,
    /// The fixture race summary `ResultsScreen` displays.
    summary: RaceSummary,
}

impl GraphiteGpApp {
    /// Builds the app shell over the hand-built fixture session data.
    fn new() -> Self {
        let track = fixture_track();
        let (car_states, trails) = fixture_cars();
        let standings = fixture_standings();

        Self {
            shell: AppShell::new(STARTUP_CONFIG),
            track,
            car_states,
            trails,
            phases: [PhaseStatus::Ok; 7],
            standings,
            summary: RaceSummary {
                fastest_lap: 38.4,
                tempo: 0.87,
                crashes: 0,
            },
        }
    }
}

impl eframe::App for GraphiteGpApp {
    // Not `update` — `eframe` 0.35's `App` trait has no such method; `ui` is
    // the required call, `logic` is the optional pre-paint hook (unused here).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let cars: Vec<CarRender<'_>> = self
            .car_states
            .iter()
            .zip(&self.trails)
            .enumerate()
            .map(|(index, (state, trail))| CarRender::new(*state, index, trail, index == 0, 0.0))
            .collect();

        let session = ShellSession {
            track: &self.track,
            cars: &cars,
            reduced_motion: false,
            active: 0,
            laps_done: 0,
            total_laps: i32::try_from(STARTUP_CONFIG.laps)
                .expect("STARTUP_CONFIG.laps is a small const literal — always fits i32"),
            phases: self.phases,
            valid: true,
            seed: FIXTURE_SEED,
            standings: &self.standings,
            summary: self.summary,
        };
        let _ = self.shell.show(ui, session);
    }
}

/// A hand-built `TrackArtifact` — a wide chunky rounded-rect loop (the
/// outer block `x∈[2,13] × y∈[2,13]` minus a centered `x∈[6,9] × y∈[6,9]`
/// hole, ~4-cell-wide arms) with hand-populated `speed_heatmap`/
/// `fastest_lap` metrics — mirrors `gp-render`'s own
/// `track::test_support::scene_track`/`scene_metrics` test fixtures (design
/// § *The binary hand-builds the fixture track*), proven to render under
/// all overlay combinations without panicking.
///
/// Replicated inline rather than shared: `main.rs` is production code and
/// cannot reference `gp-render`'s `#[cfg(test)]`-only `test_support`
/// fixtures. Reviewer follow-up (PR #118 round 2) — the previous 3×3-ring
/// corridor was too thin for the S/F chord to cross the straightaway (the
/// same pre-#100 defect the `gp-render` app-shell goldens had).
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

    let mut speed_heatmap = Vec::new();
    for x in 2..=13 {
        for y in 2..=13 {
            let in_hole = (6..=9).contains(&x) && (6..=9).contains(&y);
            if in_hole {
                continue;
            }
            let point = Point::new(x, y);
            if corridor.contains(point) {
                // A simple, deterministic per-cell gradient — no physical
                // meaning, only spatial spread across the ramp's full range.
                let speed = x.saturating_add(y);
                speed_heatmap.push((point, speed));
            }
        }
    }
    let fastest_lap = vec![
        Point::new(3, 3),
        Point::new(12, 3),
        Point::new(12, 12),
        Point::new(3, 12),
    ];

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
        metrics: TrackMetrics {
            vmax_attain: Some(6),
            tempo: Some(0.87),
            fastest_lap,
            speed_heatmap,
        },
        width_min: 3,
    }
}

/// [`FIXTURE_CAR_COUNT`] fixture cars, each with a distinct at-rest state
/// and a short trail (mirrors `gp-render`'s own gallery fixtures — the
/// physics/AI-faked posture the spec endorses for this binary's wiring).
///
/// Repositioned (PR #118 round 2, alongside [`fixture_track`]'s widening)
/// onto the new wide track's top straightaway — drivable cells with `x` in
/// `2..=13` and `y` in `2..=5` sit outside the corridor's centered hole
/// (`x∈[6,9] × y∈[6,9]`) regardless of `x`, so every cell below is inside
/// `8..=12 × 2..=5`, near the S/F chord at `x = 7`.
fn fixture_cars() -> (Vec<CarState>, Vec<Vec<Point>>) {
    // One resting cell per car, in `CAR_NAMES` order — literal, not
    // index-computed (mirrors `results_gallery.rs::fixture_standings`'s
    // literal-fixture idiom; avoids `clippy::arithmetic_side_effects` on
    // index math for a fixed, tiny fixture).
    const FIXTURE_CELLS: [(i32, i32); FIXTURE_CAR_COUNT] = [(10, 3), (8, 2), (12, 4), (9, 5)];

    let states: Vec<CarState> = FIXTURE_CELLS
        .iter()
        .map(|&(x, y)| CarState { x, y, vx: 0, vy: 0 })
        .collect();
    let trails: Vec<Vec<Point>> = states
        .iter()
        .map(|state| vec![Point::new(state.x, state.y)])
        .collect();
    (states, trails)
}

/// [`FIXTURE_CAR_COUNT`] fixture standings, ranked in car-index order (the
/// player's car, index 0, finishes first) — literal, mirrors
/// `results_gallery.rs::fixture_standings`'s idiom exactly.
fn fixture_standings() -> Vec<StandingEntry> {
    const FIXTURE_STANDINGS: [StandingEntry; FIXTURE_CAR_COUNT] = [
        StandingEntry {
            car_index: 0,
            kind: CarKind::You,
            rank: 1,
            finish_time: 38.0,
        },
        StandingEntry {
            car_index: 1,
            kind: CarKind::Ai,
            rank: 2,
            finish_time: 39.6,
        },
        StandingEntry {
            car_index: 2,
            kind: CarKind::Ai,
            rank: 3,
            finish_time: 41.2,
        },
        StandingEntry {
            car_index: 3,
            kind: CarKind::Ai,
            rank: 4,
            finish_time: 42.8,
        },
    ];
    FIXTURE_STANDINGS.to_vec()
}

/// Open the window and run the app shell.
///
/// # Errors
/// Returns [`eframe::Error`] if the native window/graphics context fails to
/// initialize (e.g. no compatible Vulkan/GL adapter).
fn main() -> eframe::Result {
    eframe::run_native(
        "graphite-gp",
        eframe::NativeOptions::default(),
        Box::new(|creation_context| {
            creation_context
                .egui_ctx
                .set_fonts(gp_render::fonts::definitions());
            // Pin one fixed palette (PR #118 round 3 — the app is
            // custom-drawn entirely from design tokens and has no
            // light/dark-switching feature; without this, egui defaults to
            // its dark `Visuals`, which is what made the body render black).
            creation_context
                .egui_ctx
                .set_visuals(egui::Visuals::light());
            Ok(Box::new(GraphiteGpApp::new()))
        }),
    )
}
