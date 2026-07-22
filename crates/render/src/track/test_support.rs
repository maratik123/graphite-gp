//! Test-only fixtures shared across `track`'s submodule test suites — hoisted
//! out of `car.rs`/`fastest_lap.rs`/`grid.rs`/`regions.rs`/`walls.rs`/
//! `heatmap.rs`/`mod.rs` (each of which duplicated one or more of these
//! verbatim) per the `JetBrains` duplicate-code inspection.
//!
//! [`scene_track`]/[`scene_metrics`]/[`scene_track_with_metrics`] are
//! `pub(crate)` (wider than this module's other `pub(super)` fixtures)
//! because `app_gallery.rs` — a crate-root sibling of `track`, not a
//! descendant — needs them too (PR #118 round 2 reviewer follow-up): the
//! S/F-crossing golden fixture must not be duplicated between
//! `track::golden` and `app_gallery`, so both call this single shared
//! definition. See `crates/render/src/track/golden.rs`'s "Amendment" doc
//! comment (PR #100) for why the corridor is wide rather than the thin
//! `ring_3x3`.

use super::TrackTransform;
use egui::epaint::ClippedShape;
use egui::{Mesh, Pos2, Rect, Shape, pos2};
use gp_core::geom::{Corridor, Orient, Point, Side, walls_from_boundary};
use gp_core::track::{RaceDir, StartFinish, TimingGate, TrackArtifact};
use std::sync::Arc;

/// A `TrackTransform` over a 10×10 corridor filling a 100×100 rect —
/// `cell_size == 10.0`, no centering offset, for readable expected values.
pub(super) fn transform_10x10() -> TrackTransform {
    let d = Corridor::new(Point::new(0, 0), 10, 10);
    TrackTransform::new(&d, Rect::from_min_max(Pos2::ZERO, pos2(100.0, 100.0)))
}

/// Builds a corridor over `[origin, origin + (w, h))` with the given
/// `(x, y)` cells drivable — mirrors `gp_core::geom::graph::tests::corridor`.
pub(super) fn corridor(
    origin: (i32, i32),
    w: usize,
    h: usize,
    drivable: &[(i32, i32)],
) -> Corridor {
    let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
    for &(x, y) in drivable {
        d.set(Point::new(x, y), true);
    }
    d
}

/// A 3×3 ring around one hole cell, over a 5×5 bbox with a 1-cell outfield
/// margin (mirrors `geom::graph::tests`' ring fixture).
pub(super) fn ring_3x3() -> Corridor {
    let cells: [(i32, i32); 8] = [
        (1, 1),
        (2, 1),
        (3, 1),
        (1, 2),
        (3, 2),
        (1, 3),
        (2, 3),
        (3, 3),
    ];
    corridor((0, 0), 5, 5, &cells)
}

/// `d`'s `contains` value at every cell of its `n × n` bounding box, in
/// row-major order — a before/after snapshot for "did this call mutate the
/// corridor" tests.
pub(super) fn corridor_cells(d: &Corridor, n: i32) -> Vec<bool> {
    (0..n)
        .flat_map(|y| (0..n).map(move |x| (x, y)))
        .map(|(x, y)| d.contains(Point::new(x, y)))
        .collect()
}

/// Every `Mesh` shape captured in a painter's output, in draw order —
/// filters out non-mesh shapes (strokes, text).
pub(super) fn captured_meshes(shapes: &[ClippedShape]) -> Vec<Arc<Mesh>> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            Shape::Mesh(mesh) => Some(mesh.clone()),
            _ => None,
        })
        .collect()
}

/// Amendment — widened golden fixture (PR #100), moved here from
/// `track::golden` (PR #118 round 2) so `app_gallery.rs` can share it: a
/// hand-built chunky rounded-rect corridor `TrackArtifact` over a 16×16
/// bbox — the outer block `x∈[2,13] × y∈[2,13]` minus a centered hole
/// `x∈[6,9] × y∈[6,9]`, a thick loop with 4-cell-wide arms. The S/F chord is
/// a `Vertical` column across the top straight (thin in x = racing
/// direction), matching `Track.jsx`'s cross-track checkered bar and
/// crossing the corridor rather than running parallel to it (the pre-#100
/// defect). Every field `draw_frame` does not read stays at its cheapest
/// valid default.
pub(crate) fn scene_track() -> TrackArtifact {
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
        s_field: gp_core::track::SField::default(),
        start_grid: gp_core::track::StartGrid::default(),
        centerline: gp_core::track::Centerline::default(),
        metrics: gp_core::track::TrackMetrics::default(),
        width_min: 3,
    }
}

/// A spatially-graded `speed_heatmap` over every drivable cell of
/// [`scene_track`]'s `corridor` (so the ramp spans `HEAT_0`→`HEAT_3` across
/// the fixture, and the rounded-rect's own convex corners are covered) plus
/// a `fastest_lap` loop of cell centers around one of the corridor's arms.
/// Moved here from `track::golden` alongside [`scene_track`] (PR #118
/// round 2).
pub(crate) fn scene_metrics(corridor: &Corridor) -> gp_core::track::TrackMetrics {
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
    gp_core::track::TrackMetrics {
        speed_heatmap,
        fastest_lap,
        ..gp_core::track::TrackMetrics::default()
    }
}

/// [`scene_track`], with [`scene_metrics`] populated over its own
/// corridor — the AC6 per-overlay golden fixture (block 1's metrics
/// generator is not yet built, so goldens hand-populate `TrackMetrics`, per
/// design § Technical constraints). Also the wide-corridor fixture
/// `app_gallery.rs`'s `fixture_track` reuses (PR #118 round 2) so the S/F
/// crosses the corridor in the Lab/Race app-shell goldens.
pub(crate) fn scene_track_with_metrics() -> TrackArtifact {
    let mut track = scene_track();
    track.metrics = scene_metrics(&track.corridor);
    track
}
