//! Test-only fixtures shared across `track`'s submodule test suites — hoisted
//! out of `car.rs`/`fastest_lap.rs`/`grid.rs`/`regions.rs`/`walls.rs`/
//! `heatmap.rs`/`mod.rs` (each of which duplicated one or more of these
//! verbatim) per the `JetBrains` duplicate-code inspection.

use super::TrackTransform;
use egui::epaint::ClippedShape;
use egui::{Mesh, Pos2, Rect, Shape, pos2};
use gp_core::geom::{Corridor, Point};
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
