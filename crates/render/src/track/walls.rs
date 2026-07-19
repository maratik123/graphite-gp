//! Wall geometry layer (design doc §4, layer 2): `Wall` → half-grid edge
//! segment, chained into ordered boundary polylines.
//!
//! A wall never passes through an integer lattice point by construction
//! (design doc §1 duality) — every segment endpoint sits at a half-grid
//! corner `(±0.5, ±0.5)` relative to some cell. Corners are represented as
//! **doubled integers** (`DualCorner = (2x, 2y)`), not `f32`: every corner
//! coordinate is an exact half-integer derived from an `i32` cell coordinate,
//! so doubling keeps the whole chaining algorithm integer-exact — no float
//! equality/hashing pitfalls, and "never an integer lattice point" becomes
//! the simple, exactly-testable "doubled coordinate is odd" (AC3).

use super::TrackTransform;
use egui::{Painter, Pos2, Shape, Stroke};
use gp_core::geom::{Side, Wall};
use std::collections::HashMap;

/// A half-grid dual-vertex corner, as doubled integer lattice coordinates
/// (`(2x, 2y)`) — exact and hashable, unlike the `f32` corner it represents.
pub(crate) type DualCorner = (i32, i32);

/// The two `DualCorner` endpoints of `wall`'s edge segment, on the side of
/// its cell the wall faces.
///
/// Bounded, allocatable-grid domain (mirrors `gp_core::geom::Size::area`'s
/// own treatment): a cell coordinate doubling/±1 that overflows `i32` would
/// require a corridor wider than `i32::MAX / 2` cells, which could not
/// itself allocate a backing `Vec<bool>` first.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "cell coordinates come from a real, allocatable Corridor (bounded by \
              Size::area's own allocatability precondition); doubling + ±1 cannot \
              overflow i32 in that domain — see gp-core geom/mod.rs Size::area"
)]
pub(crate) const fn wall_corners(wall: Wall) -> (DualCorner, DualCorner) {
    let (cx, cy) = (wall.cell.x * 2, wall.cell.y * 2);
    match wall.side {
        Side::East => ((cx + 1, cy - 1), (cx + 1, cy + 1)),
        Side::West => ((cx - 1, cy - 1), (cx - 1, cy + 1)),
        Side::North => ((cx - 1, cy + 1), (cx + 1, cy + 1)),
        Side::South => ((cx - 1, cy - 1), (cx + 1, cy - 1)),
    }
}

/// Converts a `DualCorner` back to a lattice-space `(f32, f32)` point, for
/// [`TrackTransform::map`].
#[allow(
    clippy::cast_precision_loss,
    reason = "doubled cell coordinates are grid-realistic i32s, far below f32's \
              exact-integer range; precedent: gp-core track.rs::normalize"
)]
fn to_lattice((dx, dy): DualCorner) -> (f32, f32) {
    (dx as f32 / 2.0, dy as f32 / 2.0)
}

/// Chains `walls`' dual edges into ordered, closed boundary polylines.
///
/// Each returned loop's points are consecutive around the loop and the loop
/// implicitly closes back to its first point (no duplicated closing point —
/// pair with [`Shape::closed_line`]). Walks by shared `DualCorner`, so it
/// requires no ordering from `walls` itself (`walls_from_boundary` returns an
/// unordered `Vec<Wall>` — design § Risks). Every edge is consumed exactly
/// once. Assumes a simple boundary (every corner has degree ≤ 2, true for
/// the closed rings this crate draws); a higher-degree pinch vertex would
/// still terminate the walk (never loop forever — a corner runs out of
/// unused incident edges eventually) but is not asserted to produce a single
/// simple loop.
pub(crate) fn chain_walls(walls: &[Wall]) -> Vec<Vec<DualCorner>> {
    let edges: Vec<(DualCorner, DualCorner)> = walls.iter().copied().map(wall_corners).collect();

    let mut adjacency: HashMap<DualCorner, Vec<usize>> = HashMap::new();
    for (i, &(a, b)) in edges.iter().enumerate() {
        adjacency.entry(a).or_default().push(i);
        adjacency.entry(b).or_default().push(i);
    }

    let mut used = vec![false; edges.len()];
    let mut loops = Vec::new();
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let (a0, b0) = edges[start];
        used[start] = true;
        let mut points = vec![a0, b0];
        let mut current = b0;
        while current != a0 {
            let Some(&next_idx) = adjacency
                .get(&current)
                .and_then(|ids| ids.iter().find(|&&i| !used[i]))
            else {
                break;
            };
            used[next_idx] = true;
            let (na, nb) = edges[next_idx];
            let next_point = if na == current { nb } else { na };
            if next_point == a0 {
                break;
            }
            points.push(next_point);
            current = next_point;
        }
        loops.push(points);
    }
    loops
}

/// Strokes every chained loop as a closed polyline in `WALL`.
pub(crate) fn paint(painter: &Painter, transform: &TrackTransform, loops: &[Vec<DualCorner>]) {
    let stroke = Stroke::new(crate::tokens::spacing::BW_1, crate::tokens::color::WALL);
    for loop_corners in loops {
        if loop_corners.len() < 2 {
            continue;
        }
        let points: Vec<Pos2> = loop_corners
            .iter()
            .copied()
            .map(|c| transform.map(to_lattice(c)))
            .collect();
        painter.add(Shape::closed_line(points, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::{chain_walls, wall_corners};
    use gp_core::geom::{Corridor, Point, Side, Wall, walls_from_boundary};
    use std::collections::HashSet;

    fn corridor(origin: (i32, i32), w: usize, h: usize, drivable: &[(i32, i32)]) -> Corridor {
        let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
        for &(x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    fn solid_2x2() -> Corridor {
        corridor((0, 0), 4, 4, &[(1, 1), (2, 1), (1, 2), (2, 2)])
    }

    fn ring_3x3() -> Corridor {
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
        corridor((0, 0), 5, 5, &cells)
    }

    /// AC3 — every segment endpoint's doubled coordinate is odd (a
    /// half-integer, never an integer lattice point), over both fixtures.
    #[test]
    fn every_endpoint_is_a_half_integer() {
        for d in [solid_2x2(), ring_3x3()] {
            let walls = walls_from_boundary(&d);
            assert!(!walls.is_empty());
            for wall in walls {
                let corners: [(i32, i32); 2] = wall_corners(wall).into();
                for (x, y) in corners {
                    assert_eq!(x.rem_euclid(2), 1, "x={x} not odd (wall={wall:?})");
                    assert_eq!(y.rem_euclid(2), 1, "y={y} not odd (wall={wall:?})");
                }
            }
        }
    }

    /// Direct construction sanity check — `wall_corners` places both
    /// endpoints on the expected side of the cell, e.g. `East` at
    /// `x = 2*cell.x + 1`.
    #[test]
    fn wall_corners_matches_side() {
        let cell = Point::new(0, 0);
        assert_eq!(
            wall_corners(Wall {
                cell,
                side: Side::East
            }),
            ((1, -1), (1, 1))
        );
        assert_eq!(
            wall_corners(Wall {
                cell,
                side: Side::West
            }),
            ((-1, -1), (-1, 1))
        );
        assert_eq!(
            wall_corners(Wall {
                cell,
                side: Side::North
            }),
            ((-1, 1), (1, 1))
        );
        assert_eq!(
            wall_corners(Wall {
                cell,
                side: Side::South
            }),
            ((-1, -1), (1, -1))
        );
    }

    /// Chaining — the 3×3-ring's 16 boundary edges chain into exactly 2
    /// closed loops (outer + inner), using every edge exactly once.
    #[test]
    fn ring_chains_into_outer_and_inner_loops() {
        let d = ring_3x3();
        let walls = walls_from_boundary(&d);
        assert_eq!(walls.len(), 16);

        let loops = chain_walls(&walls);
        assert_eq!(loops.len(), 2, "expected outer + inner loop, got {loops:?}");

        let total_edges: usize = loops.iter().map(Vec::len).sum();
        assert_eq!(total_edges, 16, "every edge must be used exactly once");

        // Each loop is a simple closed polygon: no repeated corner within a
        // loop (a repeat would mean an edge was reused or mis-chained).
        for loop_corners in &loops {
            let unique: HashSet<_> = loop_corners.iter().copied().collect();
            assert_eq!(
                unique.len(),
                loop_corners.len(),
                "loop has a repeated corner"
            );
        }
    }
}
