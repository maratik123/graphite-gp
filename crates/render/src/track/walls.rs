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
use gp_core::geom::{Corridor, Point, Side, Wall};
use std::collections::HashMap;

/// The M6 guard's clamp radius: a generated Chaikin vertex may move at most
/// half a cell from the original boundary point it replaces (design doc §4
/// M6). Every raw wall edge has lattice length exactly `1.0` (a single
/// `Wall`'s own segment), so a Chaikin quarter-point is always `0.25` from
/// its nearer endpoint — comfortably inside this bound; the check still runs
/// (not just asserted structurally) so a future edge-length change cannot
/// silently break M6.
const HALF_CELL_GAP: f32 = 0.5;

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

/// Whether lattice point `(x, y)` lies strictly inside some drivable cell's
/// closed-boundary-excluded unit square — i.e. is a grazeable/drivable
/// point, not merely on a cell's edge.
///
/// A point can lie strictly inside at most one cell's square (squares tile
/// the plane edge-to-edge), so rounding to the nearest cell center and
/// re-checking the half-open bound is exact: a point exactly on a boundary
/// (distance `0.5` from the nearest center either way) fails the strict `<`
/// test regardless of which way `round()` breaks the tie.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "lattice coordinates here always originate from a real Corridor's \
              grid-realistic i32 cell range; round() lands within i32 range for \
              any on-canvas point — precedent: placeholder.rs::pixel_at"
)]
fn point_in_drivable(d: &Corridor, (x, y): (f32, f32)) -> bool {
    let cx = x.round() as i32;
    let cy = y.round() as i32;
    #[allow(
        clippy::cast_precision_loss,
        reason = "cx/cy are grid-realistic i32 cell coordinates; precedent: \
                  gp-core track.rs::normalize"
    )]
    let inside = (x - cx as f32).abs() < HALF_CELL_GAP && (y - cy as f32).abs() < HALF_CELL_GAP;
    inside && d.contains(Point::new(cx, cy))
}

/// Linear interpolation between two lattice points at parameter `t`.
fn lerp_point(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (t.mul_add(b.0 - a.0, a.0), t.mul_add(b.1 - a.1, a.1))
}

/// The M6 guard: accepts `candidate` (a generated Chaikin vertex) only when
/// it stays within [`HALF_CELL_GAP`] of `original` (the raw boundary point
/// it replaces) **and** does not land inside any drivable cell's square;
/// otherwise falls back to `original` — the raw, always-correct stroke
/// (design § *Raw-stroke fallback invariant*).
fn guarded(d: &Corridor, candidate: (f32, f32), original: (f32, f32)) -> (f32, f32) {
    let dist = (candidate.0 - original.0).hypot(candidate.1 - original.1);
    if dist <= HALF_CELL_GAP && !point_in_drivable(d, candidate) {
        candidate
    } else {
        original
    }
}

/// Applies one Chaikin corner-cutting pass to a closed loop of `DualCorner`s
/// (design doc §4, layer 2/subtask 4), M6-guarded: each generated vertex
/// individually falls back to its nearer raw endpoint when the guard would
/// otherwise be violated, so the returned polyline is always safe to draw
/// even when the cosmetic smoothing is locally rejected (design §
/// *Raw-stroke fallback invariant* — AC3 on the underlying raw stroke never
/// depends on this pass).
///
/// `d` is read-only (`&Corridor`) — this pass cannot mutate `D` (AC7
/// invariance holds by construction, not by a runtime check).
pub(crate) fn chaikin_smooth(d: &Corridor, loop_corners: &[DualCorner]) -> Vec<(f32, f32)> {
    let lattice: Vec<(f32, f32)> = loop_corners.iter().copied().map(to_lattice).collect();
    if lattice.len() < 3 {
        return lattice;
    }
    let mut out = Vec::with_capacity(lattice.len().saturating_mul(2));
    // Zipping against a `cycle().skip(1)` view (rather than `(i + 1) % n`
    // index arithmetic) gives each point's successor, wrapping the last back
    // to the first — `clippy::arithmetic_side_effects` (deny) flags a raw
    // `% n` even though it cannot actually overflow/panic here.
    let next = lattice.iter().copied().cycle().skip(1);
    for (p0, p1) in lattice.iter().copied().zip(next) {
        let q = lerp_point(p0, p1, 0.25);
        let r = lerp_point(p0, p1, 0.75);
        out.push(guarded(d, q, p0));
        out.push(guarded(d, r, p1));
    }
    out
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

/// Converts a raw (unsmoothed) `DualCorner` loop to lattice-space `(f32,
/// f32)` points — the same output shape [`chaikin_smooth`] produces, so
/// [`paint`] draws either uniformly (design § *Raw-stroke fallback
/// invariant*). `draw_frame` always draws the Chaikin-smoothed stroke
/// (`chaikin_smooth` itself guards every vertex back to raw when needed), so
/// this converter's only production-code caller was that raw-vs-smoothed
/// choice; it stays as the tests' shared conversion.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "test-only: mirrors chaikin_smooth's output shape for direct \
                  raw-vs-smoothed comparison in this file's own tests"
    )
)]
pub(crate) fn dual_loop_to_lattice(loop_corners: &[DualCorner]) -> Vec<(f32, f32)> {
    loop_corners.iter().copied().map(to_lattice).collect()
}

/// Strokes every chained loop as a closed polyline in `WALL`. `loops` are
/// already lattice-space points — either the raw stroke
/// ([`dual_loop_to_lattice`]) or the M6-guarded Chaikin-smoothed stroke
/// ([`chaikin_smooth`]); `paint` itself is agnostic to which.
pub(crate) fn paint(painter: &Painter, transform: &TrackTransform, loops: &[Vec<(f32, f32)>]) {
    let stroke = Stroke::new(crate::tokens::spacing::BW_1, crate::tokens::color::WALL);
    for loop_points in loops {
        if loop_points.len() < 2 {
            continue;
        }
        let points: Vec<Pos2> = loop_points
            .iter()
            .copied()
            .map(|p| transform.map(p))
            .collect();
        painter.add(Shape::closed_line(points, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::{chain_walls, wall_corners};
    use crate::track::test_support::{corridor, ring_3x3};
    use gp_core::geom::{Corridor, Point, Side, Wall, walls_from_boundary};
    use std::collections::HashSet;

    fn solid_2x2() -> Corridor {
        corridor((0, 0), 4, 4, &[(1, 1), (2, 1), (1, 2), (2, 2)])
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

    /// Solid, isolated single cell — a fully convex 4-corner boundary loop
    /// (no reflex corners, so every guard passes and every generated vertex
    /// stays distinct from its raw parent).
    fn solid_single_cell() -> Corridor {
        corridor((0, 0), 3, 3, &[(1, 1)])
    }

    /// An L-shaped 3-cell corridor with one reflex (concave, from `D`'s
    /// side) interior corner — the fixture the M6 clamp exists for.
    fn l_shape() -> Corridor {
        corridor((0, 0), 4, 4, &[(1, 1), (2, 1), (2, 2)])
    }

    /// AC7 bound — every Chaikin-generated vertex stays within
    /// `HALF_CELL_GAP` of the raw boundary point it replaces, on both the
    /// convex (single-cell) and reflex (L-shape) fixtures.
    #[test]
    fn every_generated_vertex_stays_within_half_cell_gap() {
        for d in [solid_single_cell(), l_shape()] {
            let walls = walls_from_boundary(&d);
            for loop_corners in chain_walls(&walls) {
                let raw = super::dual_loop_to_lattice(&loop_corners);
                let smoothed = super::chaikin_smooth(&d, &loop_corners);
                // Each raw edge (p0, p1) contributes two smoothed vertices,
                // nearer to p0 then to p1 respectively — mirrors
                // `chaikin_smooth`'s own `zip(cycle().skip(1))` pairing, so
                // no index arithmetic is needed here either.
                let next = raw.iter().copied().cycle().skip(1);
                let expected_nearer: Vec<(f32, f32)> = raw
                    .iter()
                    .copied()
                    .zip(next)
                    .flat_map(|pair| <[(f32, f32); 2]>::from(pair).into_iter())
                    .collect();
                assert_eq!(smoothed.len(), expected_nearer.len());
                for (&(sx, sy), &nearer) in smoothed.iter().zip(&expected_nearer) {
                    let dist = (sx - nearer.0).hypot(sy - nearer.1);
                    assert!(
                        dist <= super::HALF_CELL_GAP + f32::EPSILON,
                        "vertex at ({sx},{sy}) is {dist} from its nearer raw point {nearer:?}"
                    );
                }
            }
        }
    }

    /// AC7 M6 — no Chaikin-generated vertex lies inside any drivable cell's
    /// square, on the reflex L-shape fixture.
    #[test]
    fn no_generated_vertex_enters_a_drivable_cell() {
        let d = l_shape();
        let walls = walls_from_boundary(&d);
        for loop_corners in chain_walls(&walls) {
            let smoothed = super::chaikin_smooth(&d, &loop_corners);
            for &p in &smoothed {
                assert!(
                    !super::point_in_drivable(&d, p),
                    "generated vertex {p:?} lies inside a drivable cell"
                );
            }
        }
    }

    /// AC7 invariance — `chaikin_smooth` takes `&Corridor`, so `D` cannot be
    /// mutated by construction; this test pins that by comparing `D`'s own
    /// cell contents before/after the call.
    #[test]
    fn corridor_is_unchanged_by_smoothing() {
        let d = l_shape();
        let before = crate::track::test_support::corridor_cells(&d, 4);
        let walls = walls_from_boundary(&d);
        for loop_corners in chain_walls(&walls) {
            let _ = super::chaikin_smooth(&d, &loop_corners);
        }
        let after = crate::track::test_support::corridor_cells(&d, 4);
        assert_eq!(before, after);
    }

    /// Edge — a straight run of collinear boundary edges stays collinear
    /// after smoothing: every generated vertex on the isolated single
    /// cell's east side (a straight, convex edge) keeps that side's `x`.
    #[test]
    fn straight_run_stays_collinear() {
        let d = solid_single_cell();
        let east_corners = [(3, 1), (3, -1)]; // East wall of cell (1,1): (2*1+1, 2*1±1)
        let smoothed = super::chaikin_smooth(&d, &east_corners);
        for &(x, _) in &smoothed {
            crate::test_util::assert_f32("straight run x", x, 1.5);
        }
    }

    /// Direct guard test — a candidate landing strictly inside a drivable
    /// cell's square is clamped back to its original point, even though it
    /// is well within the [`super::HALF_CELL_GAP`] distance bound (isolates
    /// the M6 cell-containment trigger from the distance trigger). Real
    /// wall geometry never manufactures this input by construction (every
    /// generated point sits exactly on an axis-aligned boundary line, so it
    /// can only ever *touch*, never enter, a cell's square — see
    /// `no_generated_vertex_enters_a_drivable_cell`); this test exercises
    /// the clamp mechanism itself with a hand-built adversarial input,
    /// standing in for "concave corner clamped, not bulged" at the unit
    /// level.
    #[test]
    fn guard_clamps_a_candidate_that_enters_a_drivable_cell() {
        let d = corridor((0, 0), 3, 3, &[(1, 1)]);
        let original = (0.5, 0.5); // on cell (1,1)'s own boundary — not inside it
        let candidate = (0.6, 0.6); // strictly inside cell (1,1)'s square
        assert!(super::point_in_drivable(&d, candidate));
        assert_eq!(super::guarded(&d, candidate, original), original);
    }

    /// Direct guard test — a safe candidate (within the distance bound, not
    /// inside any drivable cell) passes through unclamped.
    #[test]
    fn guard_accepts_a_safe_candidate() {
        let d = corridor((0, 0), 3, 3, &[(1, 1)]);
        let original = (0.5, 0.5);
        let candidate = (0.5, 0.6); // still on the boundary line, not inside
        assert!(!super::point_in_drivable(&d, candidate));
        assert_eq!(super::guarded(&d, candidate, original), candidate);
    }
}
