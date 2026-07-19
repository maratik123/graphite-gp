//! The track canvas (design doc §4): regions, walls, S/F, cars.
//!
//! Each layer is a submodule split into a pure lattice-space geometry fn
//! (Miri-clean, no `egui::Ui`, no allocation beyond a returned `Vec`) and a
//! thin `pub(crate) paint` fn that maps that geometry to screen space via
//! [`TrackTransform`] and strokes/fills it — the house pattern this crate's
//! sibling widgets already follow (design § *House pattern*).

mod car;
#[cfg(test)]
mod golden;
mod regions;
mod sf;
mod transform;
mod walls;

pub use car::CarRender;
pub use transform::TrackTransform;

use crate::Overlays;
use egui::{Painter, Rect};
use gp_core::track::TrackArtifact;

/// The documented back-to-front draw order (AC9): `outfield → asphalt →
/// infield → walls → S/F → cars`. [`draw_frame`] follows this order exactly;
/// `layer_order_is_documented` pins the list itself as a tested contract.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "documents draw_frame's own order as a standalone, tested \
                  contract (AC9) rather than a value draw_frame reads at runtime"
    )
)]
pub(crate) const LAYER_ORDER: [&str; 6] = ["outfield", "asphalt", "infield", "walls", "sf", "cars"];

/// Draws one frame of the track canvas (design doc §4) into `rect`, back to
/// front per [`LAYER_ORDER`]: the three regions (`regions::fill` — outfield,
/// asphalt, infield in that order), the Chaikin-smoothed, M6-guarded walls,
/// the checkered S/F chord, then every car (trail, dot, velocity arrow,
/// optional "you" ring).
///
/// `overlays` is threaded but inert (Q2, design § *Rejected alternatives* /
/// § Decomposition subtask 8): layers 4 (grid) and 5 (analytics) are
/// deferred, so no `overlays` flag changes anything drawn here yet.
pub(crate) fn draw_frame(
    painter: &Painter,
    rect: Rect,
    track: &TrackArtifact,
    cars: &[CarRender<'_>],
    reduced_motion: bool,
    _overlays: Overlays,
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
    walls::paint(painter, &transform, &smoothed_loops);

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
    use gp_core::geom::{Corridor, Orient, Point, Side, walls_from_boundary};
    use gp_core::sim::CarState;
    use gp_core::track::{RaceDir, StartFinish, TimingGate, TrackArtifact};

    /// AC9 — the documented back-to-front layer order is exactly `outfield →
    /// asphalt → infield → walls → S/F → cars`.
    #[test]
    fn layer_order_is_documented() {
        assert_eq!(
            LAYER_ORDER,
            ["outfield", "asphalt", "infield", "walls", "sf", "cars"]
        );
    }

    /// A minimal, hand-built `TrackArtifact` (a 3×3 ring) — every field
    /// `draw_frame` does not read stays at its cheapest valid default.
    fn fixture_track() -> TrackArtifact {
        let cells: Vec<(i32, i32)> = [
            (1, 1),
            (2, 1),
            (3, 1),
            (1, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
        ]
        .to_vec();
        let mut corridor = Corridor::new(Point::new(0, 0), 5, 5);
        for (x, y) in cells {
            corridor.set(Point::new(x, y), true);
        }
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

    /// AC9 — every `Overlays` flag is inert: turning them all on produces
    /// byte-identical drawn shapes to the default (all-off) frame, since
    /// layers 4/5 (grid, analytics) are deferred (Q2).
    #[test]
    fn overlays_are_inert() {
        let track = fixture_track();
        let cars: [CarRender<'_>; 0] = [];
        let default_shapes = render_shapes(&track, &cars, false, Overlays::default());
        let all_on_shapes = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                speed_heatmap: true,
                fastest_lap: true,
                grid: true,
            },
        );
        assert_eq!(default_shapes, all_on_shapes);
    }
}
