//! Regions layer (design doc §4, layer 1): outfield, asphalt, infield.
//!
//! Asphalt is *derived* from the corridor `D` (never authored by hand — design
//! doc §0/§1 duality); the infield is the corridor's complement's one bounded
//! hole. The production fill (Amendment — Rounded track, PR #100) draws each
//! Chaikin-smoothed wall loop directly — [`classify_loops`] splits the loops
//! into the outer asphalt boundary vs infield holes, and [`fill`] triangulates
//! each loop into a solid-color [`Mesh`], so the fill shares the exact
//! boundary the wall stroke traces (no more per-cell square fill disagreeing
//! with the smoothed stroke at corners). [`classify`]/[`RegionCells`] (the
//! cell-based flood-fill classifier this fill used to draw from) are retained
//! as the test-only AC1/AC2 oracle — see their doc comments.
//!
//! **Miri:** `tests::fill_emits_asphalt_mesh_then_infield_mesh` stands up an
//! `egui::Context` and runs `fill` through a `run_ui` pass, so it carries
//! `#[cfg_attr(miri, ignore = "…")]` (design
//! `2026-07-21-miri-gate-render-tests`) — wall-clock cost, not an abort. The
//! remaining `classify_loops_*`/`triangulate_*`/pure-set-theory tests build
//! no `Context` and stay un-gated.

use super::TrackTransform;
use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape};
use gp_core::geom::{Corridor, Point};
use std::collections::HashSet;

/// Every drivable/non-drivable cell in `d`'s bounding box, classified into
/// the three regions (design doc §4, layer 1). Order within each `Vec` is an
/// implementation detail — callers that need a set compare should collect
/// into a `HashSet`.
#[derive(Clone, Debug, Default)]
pub(crate) struct RegionCells {
    /// Drivable cells (`== D`, AC1).
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "AC1 test-only field — classify's only production caller \
                      was removed by the Rounded-track amendment, see classify's doc"
        )
    )]
    pub asphalt: Vec<Point>,
    /// `¬D` cells reachable from the bounding-box border (AC2). `paint`
    /// fills the whole target rect as its outfield background instead of
    /// iterating this list cell-by-cell — kept for the AC2 set-membership
    /// tests (asphalt/infield/outfield mutual disjointness).
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "AC2 test-only field — see doc above")
    )]
    pub outfield: Vec<Point>,
    /// `¬D` cells **not** reachable from the border — the bounded hole(s)
    /// (AC2).
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "AC2 test-only field — classify's only production caller \
                      was removed by the Rounded-track amendment, see classify's doc"
        )
    )]
    pub infield: Vec<Point>,
}

/// Every cell point in the box `[origin, x1) × [origin, y1)`, in row-major
/// (`y`-outer, `x`-inner) order. Mirrors `gp_core::geom::Rect::points`, which
/// is not itself public.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "classify's only production caller was removed by the \
                  Rounded-track amendment — test-only helper now, see classify's doc"
    )
)]
fn bbox_points(origin: Point, x1: i32, y1: i32) -> impl Iterator<Item = Point> + Clone {
    (origin.y..y1).flat_map(move |y| (origin.x..x1).map(move |x| Point::new(x, y)))
}

/// The corridor bounding box's half-open exclusive corner `(x1, y1)`, i.e.
/// `origin + (width, height)`, saturating rather than overflowing — mirrors
/// `gp_core::geom::Rect::points`' own precondition treatment.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "classify's only production caller was removed by the \
                  Rounded-track amendment — test-only helper now, see classify's doc"
    )
)]
fn bbox_exclusive_max(d: &Corridor) -> (i32, i32) {
    let origin = d.origin();
    let x1 = i32::try_from(d.width()).map_or(i32::MAX, |w| origin.x.saturating_add(w));
    let y1 = i32::try_from(d.height()).map_or(i32::MAX, |h| origin.y.saturating_add(h));
    (x1, y1)
}

/// Whether `p` (already known to lie in the bbox) sits on its border.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "classify's only production caller was removed by the \
                  Rounded-track amendment — test-only helper now, see classify's doc"
    )
)]
const fn is_border(p: Point, origin: Point, x1: i32, y1: i32) -> bool {
    p.x == origin.x || p.y == origin.y || p.x == x1.saturating_sub(1) || p.y == y1.saturating_sub(1)
}

/// Whether `p` lies in the half-open box `[origin, (x1, y1))`.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "classify's only production caller was removed by the \
                  Rounded-track amendment — test-only helper now, see classify's doc"
    )
)]
const fn in_bbox(p: Point, origin: Point, x1: i32, y1: i32) -> bool {
    p.x >= origin.x && p.x < x1 && p.y >= origin.y && p.y < y1
}

/// Classifies every bbox cell into asphalt / outfield / infield (AC1/AC2).
///
/// The infield is found by flooding the complement `¬D` from every
/// border cell that is itself `¬D`, confined to the bbox (4-connected, like
/// `gp_core::geom::flood_fill`, but over the complement predicate, which
/// gp-core does not expose). Any `¬D` cell the flood never reaches is a
/// bounded hole — the infield.
///
/// Test-only since the Rounded-track amendment (PR #100): `draw_frame` now
/// fills from the Chaikin-smoothed wall loops directly ([`fill`]), not from
/// this cell classification, but the cell-set result stays the AC1/AC2 oracle
/// (`asphalt == {p : d.contains(p)}`) this module's tests assert against.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "AC1/AC2 test-only oracle — see doc above (Rounded-track amendment, PR #100)"
    )
)]
pub(crate) fn classify(d: &Corridor) -> RegionCells {
    let origin = d.origin();
    let (x1, y1) = bbox_exclusive_max(d);

    let asphalt: Vec<Point> = bbox_points(origin, x1, y1)
        .filter(|&p| d.contains(p))
        .collect();

    let mut visited: HashSet<Point> = HashSet::new();
    let mut outfield: Vec<Point> = Vec::new();
    let mut stack: Vec<Point> = bbox_points(origin, x1, y1)
        .filter(|&p| !d.contains(p) && is_border(p, origin, x1, y1))
        .collect();
    for &p in &stack {
        visited.insert(p);
    }
    while let Some(p) = stack.pop() {
        outfield.push(p);
        for n in p.neighbors4() {
            if in_bbox(n, origin, x1, y1) && !d.contains(n) && visited.insert(n) {
                stack.push(n);
            }
        }
    }

    let infield: Vec<Point> = bbox_points(origin, x1, y1)
        .filter(|&p| !d.contains(p) && !visited.contains(&p))
        .collect();

    RegionCells {
        asphalt,
        outfield,
        infield,
    }
}

/// The role a chained, Chaikin-smoothed wall loop plays in the region fill
/// (Amendment — Rounded track, PR #100): [`classify_loops`]'s output.
///
/// Each field holds indices into the `loops` slice `classify_loops` was
/// called with, not copies of the loops themselves.
#[derive(Clone, Debug, Default)]
pub(crate) struct LoopRoles {
    /// Index/indices of the outer asphalt-boundary loop(s) — the loop(s) of
    /// largest absolute signed area.
    pub outer: Vec<usize>,
    /// Index/indices of the infield-hole loop(s) — every other loop.
    pub holes: Vec<usize>,
}

/// The shoelace signed area of a closed lattice-space loop (no duplicated
/// closing point, matching `walls::paint`'s own loop convention). Positive or
/// negative depending on winding; callers that only need magnitude take
/// `.abs()`.
fn signed_area(points: &[(f32, f32)]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    let next = points.iter().copied().cycle().skip(1);
    points
        .iter()
        .copied()
        .zip(next)
        .map(|((x0, y0), (x1, y1))| x1.mul_add(-y0, x0 * y1))
        .sum::<f32>()
        / 2.0
}

/// Splits `loops` (chained, Chaikin-smoothed wall loops, from
/// [`super::walls::chain_walls`] + [`super::walls::chaikin_smooth`]) into the
/// outer asphalt boundary vs the infield hole(s), by signed area. The loop of
/// largest absolute area is the outer boundary; every other loop is a hole
/// (design § Decision — valid because the corridor `D` is an annulus with
/// exactly one bounded hole, design doc §1). Sign-agnostic: reversing a
/// loop's winding does not change its role, since only `.abs()` is compared.
///
/// An empty `loops` slice classifies to no outer and no holes.
pub(crate) fn classify_loops(loops: &[Vec<(f32, f32)>]) -> LoopRoles {
    let Some((outer_idx, _)) = loops
        .iter()
        .map(|loop_points| signed_area(loop_points).abs())
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
    else {
        return LoopRoles::default();
    };
    let holes = (0..loops.len()).filter(|&i| i != outer_idx).collect();
    LoopRoles {
        outer: vec![outer_idx],
        holes,
    }
}

/// The 2D cross product (z-component) of `(b - a)` and `(c - a)` — positive
/// when `a → b → c` turns left (matches the same orientation convention as
/// [`signed_area`]'s shoelace sum, so a loop's own `signed_area` sign is the
/// reference "left turn" polarity for that loop).
fn cross_z(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    (b.y - a.y).mul_add(-(c.x - a.x), (b.x - a.x) * (c.y - a.y))
}

/// The shoelace signed area of a closed screen-space loop (same convention as
/// [`signed_area`], over `Pos2` instead of lattice tuples).
fn signed_area_pos2(points: &[Pos2]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    let next = points.iter().copied().cycle().skip(1);
    points
        .iter()
        .copied()
        .zip(next)
        .map(|(p0, p1)| p1.x.mul_add(-p0.y, p0.x * p1.y))
        .sum::<f32>()
        / 2.0
}

/// Whether `p` lies in or on the closed triangle `(a, b, c)` — same-sign (or
/// zero) test against all three edge cross products; boundary-touching counts
/// as inside (conservative: a candidate ear that merely grazes another vertex
/// is rejected rather than risking an overlapping triangle).
fn point_in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let d1 = cross_z(a, b, p);
    let d2 = cross_z(b, c, p);
    let d3 = cross_z(c, a, p);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// One clippable ear found by [`find_ear`]: `cur` is the working-list vertex
/// value (an index into the original `points` triangulate was called with)
/// to remove, and `triangle` is the corresponding `[prev, cur, next]` index
/// triple to emit.
struct Ear {
    cur: usize,
    triangle: [u32; 3],
}

/// Scans `working` (a CCW-oriented permutation of original-point indices, per
/// [`triangulate`]'s reordering) for a convex, empty ear: a vertex whose
/// `(prev, cur, next)` turn is a left turn (matches CCW polarity) and whose
/// ear triangle contains no other working-list vertex. Returns the first one
/// found, or `None` if every vertex is reflex or blocked (degenerate/
/// self-intersecting input — [`triangulate`] stops rather than looping).
///
/// Builds the cyclic `(prev, cur, next)` triples via `chain`+`windows` rather
/// than modular index arithmetic (`clippy::arithmetic_side_effects`,
/// house style — mirrors `walls::chaikin_smooth`'s `cycle().skip(1)` idiom).
fn find_ear(working: &[usize], points: &[Pos2]) -> Option<Ear> {
    let extended: Vec<usize> = working
        .iter()
        .rev()
        .take(1)
        .copied()
        .chain(working.iter().copied())
        .chain(working.iter().take(1).copied())
        .collect();
    for triple in extended.windows(3) {
        let [prev, cur, next] = *triple else {
            continue;
        };
        let (pa, pb, pc) = (points[prev], points[cur], points[next]);
        if cross_z(pa, pb, pc) <= 0.0 {
            continue; // reflex or collinear — not a convex ear.
        }
        let blocked = working
            .iter()
            .copied()
            .filter(|&idx| idx != prev && idx != cur && idx != next)
            .any(|idx| point_in_triangle(points[idx], pa, pb, pc));
        if !blocked {
            return Some(Ear {
                cur,
                triangle: [
                    u32::try_from(prev).unwrap_or(u32::MAX),
                    u32::try_from(cur).unwrap_or(u32::MAX),
                    u32::try_from(next).unwrap_or(u32::MAX),
                ],
            });
        }
    }
    None
}

/// Ear-clipping triangulation of a simple polygon (design § Decision —
/// epaint 0.35 has no concave/even-odd fill, so gp-render supplies one).
///
/// `points` are screen-space vertices of one simple, closed loop (no
/// duplicated closing point). Winding is normalized to CCW first (by
/// [`signed_area_pos2`]'s sign) so [`find_ear`]'s convexity test has a
/// consistent polarity to compare against; returned triangles index into the
/// **original** `points` order regardless.
///
/// Returns `points.len() - 2` triangles for a well-formed simple polygon (the
/// standard ear-clipping count — the Two Ears Theorem guarantees at least one
/// clippable ear at every step down to a triangle). A collinear/degenerate
/// input that leaves no clippable ear stops early rather than looping or
/// indexing out of bounds — `triangulate` never panics.
pub(crate) fn triangulate(points: &[Pos2]) -> Vec<[u32; 3]> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    let mut working: Vec<usize> = if signed_area_pos2(points) < 0.0 {
        (0..n).rev().collect()
    } else {
        (0..n).collect()
    };

    let mut triangles = Vec::with_capacity(n.saturating_sub(2));
    while working.len() > 3 {
        let Some(ear) = find_ear(&working, points) else {
            return triangles;
        };
        triangles.push(ear.triangle);
        working.retain(|&idx| idx != ear.cur);
    }
    if let [a, b, c] = working[..] {
        triangles.push([
            u32::try_from(a).unwrap_or(u32::MAX),
            u32::try_from(b).unwrap_or(u32::MAX),
            u32::try_from(c).unwrap_or(u32::MAX),
        ]);
    }
    triangles
}

/// Maps `loop_points` (lattice-space) to screen space via `transform` and
/// ear-clips them **once**, returning the shared `(verts, indices)` pair
/// (design § Key decisions 1 — "triangulate once, reuse the shared index
/// buffer per cell"): [`paint_mesh`] then colors/clips that same pair as many
/// times as a caller needs (e.g. `heatmap::paint`'s per-cell recolor) without
/// re-triangulating. Loops shorter than a triangle (`< 3` points) return no
/// vertices and no triangles.
pub(crate) fn triangulated_loop(
    transform: &TrackTransform,
    loop_points: &[(f32, f32)],
) -> (Vec<Pos2>, Vec<[u32; 3]>) {
    if loop_points.len() < 3 {
        return (Vec::new(), Vec::new());
    }
    let screen_points: Vec<Pos2> = loop_points
        .iter()
        .copied()
        .map(|p| transform.map(p))
        .collect();
    let triangles = triangulate(&screen_points);
    (screen_points, triangles)
}

/// Builds one solid-`color` [`Mesh`] from *shared* `verts`/`indices` (as
/// produced by [`triangulated_loop`]) and adds it to `painter` — the
/// concave-capable replacement for `Shape::convex_polygon` (design §
/// Decision). Callers that need the same silhouette in several colors, or
/// clipped to several sub-rects (`Painter::with_clip_rect`), call this once
/// per coloring/clip without re-triangulating. Empty `verts`/`indices` (a
/// sub-triangle loop) draws nothing.
pub(crate) fn paint_mesh(painter: &Painter, verts: &[Pos2], indices: &[[u32; 3]], color: Color32) {
    if verts.is_empty() || indices.is_empty() {
        return;
    }
    let mut mesh = Mesh::default();
    for &p in verts {
        mesh.colored_vertex(p, color);
    }
    for triangle in indices {
        mesh.add_triangle(triangle[0], triangle[1], triangle[2]);
    }
    painter.add(Shape::mesh(mesh));
}

/// Triangulates `loop_points` and draws it as one solid-`color` [`Mesh`] —
/// composes [`triangulated_loop`] + [`paint_mesh`] for the common one-shot
/// case (a loop drawn in exactly one color, never reused).
fn paint_loop_mesh(
    painter: &Painter,
    transform: &TrackTransform,
    loop_points: &[(f32, f32)],
    color: Color32,
) {
    let (verts, indices) = triangulated_loop(transform, loop_points);
    paint_mesh(painter, &verts, &indices, color);
}

/// Re-cuts each `roles.holes` loop as a solid-`color` mesh on top of whatever
/// was drawn before it (mirrors `fill`'s own asphalt-then-infield-on-top
/// structure) — shared by both [`fill`] and `heatmap::paint`'s post-per-cell
/// infield re-cut (design § Key decisions 1). `loops` and `roles` must come
/// from the same `classify_loops` call; an out-of-range role index is
/// skipped rather than panicking.
pub(crate) fn paint_infield_holes(
    painter: &Painter,
    transform: &TrackTransform,
    loops: &[Vec<(f32, f32)>],
    roles: &LoopRoles,
    color: Color32,
) {
    for &idx in &roles.holes {
        if let Some(loop_points) = loops.get(idx) {
            paint_loop_mesh(painter, transform, loop_points, color);
        }
    }
}

/// Fills the three regions back-to-front, `outfield → asphalt → infield`
/// (design doc §4, layer 1; AC9's documented layer order; Amendment —
/// Rounded track, PR #100): the whole `rect` filled `PAPER_1` (outfield
/// background), then the outer loop(s) in `roles` as `SURFACE_ASPHALT`
/// mesh(es), then the hole loop(s) as `SURFACE_INFIELD` mesh(es) drawn on top
/// — asphalt and infield never overlap (disjoint by the annulus invariant),
/// so their relative draw order is visually inert; this order is the one AC9
/// pins. `loops` and `roles` must come from the same `classify_loops` call —
/// an out-of-range role index is skipped rather than panicking.
pub(crate) fn fill(
    painter: &Painter,
    rect: Rect,
    transform: &TrackTransform,
    loops: &[Vec<(f32, f32)>],
    roles: &LoopRoles,
) {
    painter.rect_filled(rect, 0, crate::tokens::color::SURFACE_PAGE);
    for &idx in &roles.outer {
        if let Some(loop_points) = loops.get(idx) {
            paint_loop_mesh(
                painter,
                transform,
                loop_points,
                crate::tokens::color::SURFACE_ASPHALT,
            );
        }
    }
    paint_infield_holes(
        painter,
        transform,
        loops,
        roles,
        crate::tokens::color::SURFACE_INFIELD,
    );
}

#[cfg(test)]
mod tests {
    use super::{TrackTransform, classify, classify_loops, fill, triangulate};
    use crate::track::test_support::{corridor, ring_3x3};
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::{Point, walls_from_boundary};
    use std::collections::HashSet;

    fn set(points: &[Point]) -> HashSet<Point> {
        points.iter().copied().collect()
    }

    /// AC1 — the classified asphalt cell set equals `{p : corridor.contains(p)}`
    /// exactly, on a hand-built ring.
    #[test]
    fn asphalt_equals_corridor_contains() {
        let d = ring_3x3();
        let regions = classify(&d);
        let want: HashSet<Point> = [
            (1, 1),
            (2, 1),
            (3, 1),
            (1, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
        ]
        .into_iter()
        .map(|(x, y)| Point::new(x, y))
        .collect();
        assert_eq!(set(&regions.asphalt), want);
    }

    /// AC2 — the ring's center cell is the bounded infield hole; every other
    /// `¬D` cell (the 1-cell outfield margin) is border-reachable outfield;
    /// the two sets are disjoint from each other and from asphalt.
    #[test]
    fn infield_hole_and_outfield_are_disjoint() {
        let d = ring_3x3();
        let regions = classify(&d);

        assert_eq!(set(&regions.infield), set(&[Point::new(2, 2)]));
        assert!(!regions.outfield.contains(&Point::new(2, 2)));
        assert_eq!(regions.outfield.len(), 25 - 8 - 1);

        let asphalt = set(&regions.asphalt);
        let infield = set(&regions.infield);
        let outfield = set(&regions.outfield);
        assert!(asphalt.is_disjoint(&infield));
        assert!(asphalt.is_disjoint(&outfield));
        assert!(infield.is_disjoint(&outfield));
    }

    /// Edge — a solid block (no hole) has an empty infield; every `¬D` cell
    /// is border-reachable outfield.
    #[test]
    fn solid_block_has_empty_infield() {
        let cells: Vec<(i32, i32)> = (0..3).flat_map(|y| (0..3).map(move |x| (x, y))).collect();
        let d = corridor((0, 0), 5, 5, &cells);
        let regions = classify(&d);
        assert!(regions.infield.is_empty());
        assert_eq!(regions.outfield.len(), 25 - 9);
    }

    /// Edge — a ring flush against the bbox edge (no outfield margin) still
    /// yields its bounded hole (mirrors `bounded_complement_components`'s
    /// flush-to-edge fixture).
    #[test]
    fn flush_to_edge_ring_still_has_bounded_hole() {
        let cells: Vec<(i32, i32)> = [
            (0, 0),
            (1, 0),
            (2, 0),
            (0, 1),
            (2, 1),
            (0, 2),
            (1, 2),
            (2, 2),
        ]
        .to_vec();
        let d = corridor((0, 0), 3, 3, &cells);
        let regions = classify(&d);
        assert_eq!(set(&regions.infield), set(&[Point::new(1, 1)]));
        assert!(regions.outfield.is_empty());
    }

    /// The 3×3-ring fixture's chained, Chaikin-smoothed wall loops — the
    /// same computation `draw_frame` runs (mod.rs) — used as the
    /// `classify_loops`/`triangulate`/`fill` test input.
    fn ring_3x3_smoothed_loops() -> Vec<Vec<(f32, f32)>> {
        let d = ring_3x3();
        let walls = walls_from_boundary(&d);
        super::super::walls::chain_walls(&walls)
            .iter()
            .map(|corners| super::super::walls::chaikin_smooth(&d, corners))
            .collect()
    }

    /// A2/triangulate test fixture — a hand-built concave polygon: the raw
    /// (unsmoothed) boundary loop of the L-tromino corridor `walls.rs`'s own
    /// `l_shape` fixture mirrors (cells `(1,1), (2,1), (2,2)`), which has a
    /// reflex corner around the missing `(1,2)` cell.
    fn l_shape_loop() -> Vec<Pos2> {
        let d = corridor((0, 0), 4, 4, &[(1, 1), (2, 1), (2, 2)]);
        let walls = walls_from_boundary(&d);
        let loops = super::super::walls::chain_walls(&walls);
        assert_eq!(loops.len(), 1, "l-tromino boundary must chain to one loop");
        super::super::walls::dual_loop_to_lattice(&loops[0])
            .into_iter()
            .map(|(x, y)| pos2(x, y))
            .collect()
    }

    /// A1 (happy) — the ring's 2 smoothed loops split into exactly one outer
    /// (larger `|area|`) and one hole.
    #[test]
    fn classify_loops_splits_ring_into_outer_and_hole() {
        let loops = ring_3x3_smoothed_loops();
        assert_eq!(loops.len(), 2);
        let roles = classify_loops(&loops);
        assert_eq!(roles.outer.len(), 1);
        assert_eq!(roles.holes.len(), 1);
        let outer_area = super::signed_area(&loops[roles.outer[0]]).abs();
        let hole_area = super::signed_area(&loops[roles.holes[0]]).abs();
        assert!(
            outer_area > hole_area,
            "outer={outer_area} hole={hole_area}"
        );
    }

    /// A1 (sign-agnostic) — reversing every loop's winding does not change
    /// which loop is the outer boundary vs. the hole.
    #[test]
    fn classify_loops_role_is_winding_agnostic() {
        let loops = ring_3x3_smoothed_loops();
        let roles = classify_loops(&loops);

        let mut reversed = loops;
        for loop_points in &mut reversed {
            loop_points.reverse();
        }
        let reversed_roles = classify_loops(&reversed);

        assert_eq!(roles.outer, reversed_roles.outer);
        assert_eq!(roles.holes, reversed_roles.holes);
    }

    /// A1 (edge) — a single solid-cell corridor (no bounded hole) yields one
    /// outer loop and zero holes.
    #[test]
    fn classify_loops_solid_cell_has_no_holes() {
        let d = corridor((0, 0), 3, 3, &[(1, 1)]);
        let walls = walls_from_boundary(&d);
        let loops: Vec<Vec<(f32, f32)>> = super::super::walls::chain_walls(&walls)
            .iter()
            .map(|corners| super::super::walls::chaikin_smooth(&d, corners))
            .collect();
        assert_eq!(loops.len(), 1);

        let roles = classify_loops(&loops);
        assert_eq!(roles.outer.len(), 1);
        assert!(roles.holes.is_empty());
    }

    /// The area of triangle `(a, b, c)` via the same cross product
    /// `triangulate`'s own convexity/containment tests use.
    fn triangle_area(a: Pos2, b: Pos2, c: Pos2) -> f32 {
        super::cross_z(a, b, c).abs() / 2.0
    }

    /// The summed area of every `[a, b, c]` vertex-index triangle, indexed
    /// into `points` — shared by the convex/concave `triangulate` area-
    /// coverage tests.
    fn triangle_area_sum(triangles: &[[u32; 3]], points: &[Pos2]) -> f32 {
        triangles
            .iter()
            .map(|&[a, b, c]| {
                triangle_area(points[a as usize], points[b as usize], points[c as usize])
            })
            .sum()
    }

    /// A2 (convex) — a square loop triangulates to `n - 2` triangles whose
    /// area sum equals the polygon's own `|area|`.
    #[test]
    fn triangulate_convex_square_covers_area() {
        let square = vec![
            pos2(0.0, 0.0),
            pos2(4.0, 0.0),
            pos2(4.0, 4.0),
            pos2(0.0, 4.0),
        ];
        let triangles = triangulate(&square);
        assert_eq!(triangles.len(), square.len() - 2);

        let area_sum = triangle_area_sum(&triangles, &square);
        let expected = super::signed_area_pos2(&square).abs();
        assert!(
            (area_sum - expected).abs() < 1e-4,
            "area_sum={area_sum} expected={expected}"
        );
    }

    /// A2 (concave) — the hand-built L-shaped loop triangulates to `n - 2`
    /// triangles whose area sum equals the polygon's own `|area|`, and no
    /// triangle covers the notch point (the center of the one non-drivable
    /// cell, `(1, 2)`, the L cuts out) — the case `Shape::convex_polygon`
    /// would fail (design § Decision).
    #[test]
    fn triangulate_concave_l_shape_covers_area_without_exiting() {
        let loop_points = l_shape_loop();
        let triangles = triangulate(&loop_points);
        assert_eq!(triangles.len(), loop_points.len() - 2);

        let area_sum = triangle_area_sum(&triangles, &loop_points);
        let expected = super::signed_area_pos2(&loop_points).abs();
        assert!(
            (area_sum - expected).abs() < 1e-4,
            "area_sum={area_sum} expected={expected}"
        );

        let notch = pos2(1.0, 2.0);
        for &[a, b, c] in &triangles {
            assert!(
                !super::point_in_triangle(
                    notch,
                    loop_points[a as usize],
                    loop_points[b as usize],
                    loop_points[c as usize],
                ),
                "triangle [{a},{b},{c}] covers the notch point, exiting the polygon"
            );
        }
    }

    /// A2 (fill order) — `fill` emits the outer loop as an `ASPHALT` mesh
    /// then the hole loop as a `SURFACE_INFIELD` mesh, in that order.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "this test drives a Context::run_ui + layer_painter pass \
                  through regions::fill, capturing the asphalt/infield \
                  meshes — interpreted-pass wall-clock cost, not an abort"
    )]
    fn fill_emits_asphalt_mesh_then_infield_mesh() {
        let loops = ring_3x3_smoothed_loops();
        let roles = classify_loops(&loops);
        let d = ring_3x3();
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let transform = TrackTransform::new(&d, rect);

        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            fill(&painter, rect, &transform, &loops, &roles);
        });

        let meshes = crate::track::test_support::captured_meshes(&output.shapes);
        assert_eq!(
            meshes.len(),
            2,
            "expected exactly one asphalt mesh + one infield mesh"
        );
        assert_eq!(
            meshes[0].vertices[0].color,
            crate::tokens::color::SURFACE_ASPHALT
        );
        assert_eq!(
            meshes[1].vertices[0].color,
            crate::tokens::color::SURFACE_INFIELD
        );
    }
}
