//! The track canvas (design doc §4): regions, walls, S/F, cars.
//!
//! Each layer is a submodule split into a pure lattice-space geometry fn
//! (Miri-clean, no `egui::Ui`, no allocation beyond a returned `Vec`) and a
//! thin `pub(crate) paint` fn that maps that geometry to screen space via
//! [`TrackTransform`] and strokes/fills it — the house pattern this crate's
//! sibling widgets already follow (design § *House pattern*).
//!
//! **Miri:** every `tests::render_shapes`-driven test below stands up an
//! `egui::Context` and runs a full-frame `run_ui` pass, so it carries
//! `#[cfg_attr(miri, ignore = "…")]` (design
//! `2026-07-21-miri-gate-render-tests`) — wall-clock cost under the
//! interpreter, not an abort (the helper only captures `output.shapes`,
//! never calls `tessellate`/`set_fonts`). `layer_order_is_documented` builds
//! no `Context` and stays un-gated.

mod car;
mod fastest_lap;
#[cfg(test)]
mod golden;
mod grid;
mod heatmap;
mod regions;
mod sf;
#[cfg(test)]
mod test_support;
mod transform;
mod walls;

pub use car::CarRender;
pub use transform::TrackTransform;

use crate::Overlays;
use egui::{Painter, Rect};
use gp_core::track::TrackArtifact;

/// The documented back-to-front draw order (AC5/AC9, final): `outfield →
/// asphalt → infield → heatmap → grid → walls → fastest_lap → S/F → cars`.
/// [`draw_frame`] follows this order exactly; `layer_order_is_documented`
/// pins the list itself as a tested contract.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "documents draw_frame's own order as a standalone, tested \
                  contract (AC9) rather than a value draw_frame reads at runtime"
    )
)]
pub(crate) const LAYER_ORDER: [&str; 9] = [
    "outfield",
    "asphalt",
    "infield",
    "heatmap",
    "grid",
    "walls",
    "fastest_lap",
    "sf",
    "cars",
];

/// Draws one frame of the track canvas (design doc §4) into `rect`, back to
/// front per [`LAYER_ORDER`]: the three regions (`regions::fill` — outfield,
/// asphalt, infield in that order), the `speed_heatmap` analytics overlay
/// (layer 1b, over the asphalt, design § Key decisions 1), the notebook-
/// sheet `grid` overlay (layer 4, over the regions, design § Key decisions
/// 4), the Chaikin-smoothed, M6-guarded walls, the `fastest_lap` analytics
/// overlay (layer 5, over the walls, design § Key decisions 3), the
/// checkered S/F chord, then every car (trail, dot, velocity arrow,
/// optional "you" ring).
///
/// `overlays` drives which analytics/grid layers are drawn (design § Key
/// decisions) — each flag adds or removes exactly its own layer's shapes;
/// all-off reproduces the #17 baseline byte-for-byte (design § Draw order).
pub(crate) fn draw_frame(
    painter: &Painter,
    rect: Rect,
    track: &TrackArtifact,
    cars: &[CarRender<'_>],
    reduced_motion: bool,
    overlays: Overlays,
) {
    let transform = TrackTransform::new(&track.corridor, rect);

    let wall_loops = walls::chain_walls(&track.walls);
    let smoothed_loops: Vec<Vec<(f32, f32)>> = wall_loops
        .iter()
        .map(|loop_corners| walls::chaikin_smooth(&track.corridor, loop_corners))
        .collect();

    // Amendment — Rounded track (PR #100): fill and stroke share the exact
    // same smoothed loops, so they cannot disagree at a corner by
    // construction (design § Decision, "Boundary reuse").
    let loop_roles = regions::classify_loops(&smoothed_loops);
    regions::fill(painter, rect, &transform, &smoothed_loops, &loop_roles);

    if overlays.speed_heatmap {
        heatmap::paint(
            painter,
            &transform,
            &smoothed_loops,
            &loop_roles,
            &track.metrics.speed_heatmap,
        );
    }

    if overlays.grid {
        grid::paint(painter, rect, &transform);
    }

    walls::paint(painter, &transform, &smoothed_loops);

    if overlays.fastest_lap {
        fastest_lap::paint(painter, &transform, &track.metrics.fastest_lap);
    }

    let checker = sf::checker_cells(&track.sf.chord);
    sf::paint(painter, &transform, &checker, track.sf.orient);

    for render in cars {
        car::paint(painter, &transform, render, render.progress, reduced_motion);
    }
}

#[cfg(test)]
mod tests {
    use super::LAYER_ORDER;
    use crate::{CarRender, Overlays};
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::{Orient, Point, Side, walls_from_boundary};
    use gp_core::sim::CarState;
    use gp_core::track::{RaceDir, StartFinish, TimingGate, TrackArtifact};

    /// AC5/AC9 — the documented back-to-front layer order is exactly (final,
    /// 9-entry list) `outfield → asphalt → infield → heatmap → grid → walls
    /// → fastest_lap → sf → cars`.
    #[test]
    fn layer_order_is_documented() {
        assert_eq!(
            LAYER_ORDER,
            [
                "outfield",
                "asphalt",
                "infield",
                "heatmap",
                "grid",
                "walls",
                "fastest_lap",
                "sf",
                "cars"
            ]
        );
    }

    /// A minimal, hand-built `TrackArtifact` (a 3×3 ring) — every field
    /// `draw_frame` does not read stays at its cheapest valid default.
    fn fixture_track() -> TrackArtifact {
        let corridor = super::test_support::ring_3x3();
        let walls = walls_from_boundary(&corridor);
        TrackArtifact {
            walls,
            sf: StartFinish {
                chord: vec![Point::new(1, 1), Point::new(2, 1), Point::new(3, 1)],
                orient: Orient::Horizontal,
                gate: TimingGate {
                    behind: vec![],
                    forward: Side::East,
                },
            },
            corridor,
            race_dir: RaceDir::Cw,
            s_field: gp_core::track::SField::default(),
            start_grid: gp_core::track::StartGrid::default(),
            centerline: gp_core::track::Centerline::default(),
            metrics: gp_core::track::TrackMetrics::default(),
        }
    }

    /// Renders `track`/`cars` once with a bare (fontless) `egui::Context` —
    /// the track canvas draws no text, so no `set_fonts` install is needed —
    /// and returns the tessellation-independent `Shape` list for comparison.
    fn render_shapes(
        track: &TrackArtifact,
        cars: &[CarRender<'_>],
        reduced_motion: bool,
        overlays: Overlays,
    ) -> String {
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            crate::render_frame(&painter, rect, track, cars, reduced_motion, overlays);
        });
        format!("{:?}", output.shapes)
    }

    /// AC9 — `render_frame` no longer `todo!()`s: a full pass over a
    /// non-trivial track + a moving, trailed "you" car produces at least one
    /// shape and does not panic.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn render_frame_draws_without_panicking() {
        let track = fixture_track();
        let trail = [Point::new(1, 1), Point::new(2, 1)];
        let cars = [CarRender::new(
            CarState {
                x: 2,
                y: 1,
                vx: 1,
                vy: 0,
            },
            0,
            &trail,
            true,
            0.5,
        )];
        let shapes = render_shapes(&track, &cars, false, Overlays::default());
        assert_ne!(shapes, "[]", "render_frame produced no shapes");
    }

    // `overlays_are_inert` (AC9, #17) is retired here (AC5): the `grid`
    // overlay wired in subtask 3 draws unconditionally on metrics, so
    // turning every flag on is no longer byte-identical to all-off. The
    // suite below (subtask 4) replaces it with the full per-overlay
    // difference/no-op/pure-visual coverage (design § Test Design, subtask
    // 4).

    /// [`fixture_track`], with `speed_heatmap` and `fastest_lap` hand-
    /// populated over the ring's own drivable cells — the AC1/AC2/AC4
    /// metric-populated fixture (block 1's generator is not yet built, so
    /// tests hand-populate `TrackMetrics` per design § Technical
    /// constraints).
    fn fixture_track_with_metrics() -> TrackArtifact {
        let mut track = fixture_track();
        track.metrics = gp_core::track::TrackMetrics {
            speed_heatmap: vec![
                (Point::new(1, 1), 2),
                (Point::new(2, 1), 5),
                (Point::new(3, 1), 8),
                (Point::new(3, 2), 6),
            ],
            fastest_lap: vec![
                Point::new(1, 1),
                Point::new(2, 1),
                Point::new(3, 1),
                Point::new(3, 2),
                Point::new(3, 3),
            ],
            ..gp_core::track::TrackMetrics::default()
        };
        track
    }

    /// AC4/AC5 — on the metric-populated fixture, each of the 3 overlay
    /// flags, turned on alone, changes the drawn output relative to
    /// all-off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn each_overlay_changes_output_when_on() {
        let track = fixture_track_with_metrics();
        let cars: [CarRender<'_>; 0] = [];
        let baseline = render_shapes(&track, &cars, false, Overlays::default());

        let heatmap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                speed_heatmap: true,
                ..Overlays::default()
            },
        );
        assert_ne!(heatmap_on, baseline, "speed_heatmap did not change output");

        let fastest_lap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                fastest_lap: true,
                ..Overlays::default()
            },
        );
        assert_ne!(
            fastest_lap_on, baseline,
            "fastest_lap did not change output"
        );

        let grid_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                grid: true,
                ..Overlays::default()
            },
        );
        assert_ne!(grid_on, baseline, "grid did not change output");
    }

    /// AC4 — every one of the 8 `Overlays` flag combinations renders at
    /// least one shape without panicking, on the metric-populated fixture.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call); \
                  this test additionally loops all 8 overlay combinations, \
                  multiplying the per-pass cost"
    )]
    fn all_overlay_combinations_render_without_panic() {
        let track = fixture_track_with_metrics();
        let cars: [CarRender<'_>; 0] = [];
        for speed_heatmap in [false, true] {
            for fastest_lap in [false, true] {
                for grid in [false, true] {
                    let overlays = Overlays {
                        speed_heatmap,
                        fastest_lap,
                        grid,
                    };
                    let shapes = render_shapes(&track, &cars, false, overlays);
                    assert_ne!(shapes, "[]", "no shapes for overlays={overlays:?}");
                }
            }
        }
    }

    /// AC4 — the all-off frame is exactly the #17, metrics-independent
    /// baseline: rendering the populated fixture with every flag off equals
    /// rendering the empty-metrics fixture with every flag off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn all_off_equals_metrics_independent_baseline() {
        let cars: [CarRender<'_>; 0] = [];
        let populated = render_shapes(
            &fixture_track_with_metrics(),
            &cars,
            false,
            Overlays::default(),
        );
        let empty = render_shapes(&fixture_track(), &cars, false, Overlays::default());
        assert_eq!(populated, empty);
    }

    /// AC7 — on the empty-metrics fixture, turning `speed_heatmap` on alone
    /// is a no-op: identical output to all-off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn heatmap_is_noop_on_empty_metrics() {
        let track = fixture_track();
        let cars: [CarRender<'_>; 0] = [];
        let baseline = render_shapes(&track, &cars, false, Overlays::default());
        let heatmap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                speed_heatmap: true,
                ..Overlays::default()
            },
        );
        assert_eq!(heatmap_on, baseline);
    }

    /// AC7 — on the empty-metrics fixture, turning `fastest_lap` on alone
    /// is a no-op: identical output to all-off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn fastest_lap_is_noop_on_empty_metrics() {
        let track = fixture_track();
        let cars: [CarRender<'_>; 0] = [];
        let baseline = render_shapes(&track, &cars, false, Overlays::default());
        let fastest_lap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                fastest_lap: true,
                ..Overlays::default()
            },
        );
        assert_eq!(fastest_lap_on, baseline);
    }

    /// AC2 — `fastest_lap` is pure-visual: a full render with it on leaves
    /// the corridor `D` and every metric unchanged (mirrors
    /// `walls.rs::corridor_is_unchanged_by_smoothing`). `draw_frame` only
    /// ever borrows `&TrackArtifact`, so this is enforced by the type
    /// system too — the test pins it as a documented, re-checkable AC2
    /// contract rather than relying on that alone.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn fastest_lap_paint_does_not_mutate() {
        let track = fixture_track_with_metrics();
        let cells_before = super::test_support::corridor_cells(&track.corridor, 5);
        let fastest_lap_before = track.metrics.fastest_lap.clone();
        let speed_heatmap_before = track.metrics.speed_heatmap.clone();

        let cars: [CarRender<'_>; 0] = [];
        let _ = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                fastest_lap: true,
                ..Overlays::default()
            },
        );

        let cells_after = super::test_support::corridor_cells(&track.corridor, 5);
        assert_eq!(cells_before, cells_after);
        assert_eq!(fastest_lap_before, track.metrics.fastest_lap);
        assert_eq!(speed_heatmap_before, track.metrics.speed_heatmap);
    }
}
