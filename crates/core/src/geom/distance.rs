//! Distance transform + medial axis over the corridor `D` (design doc §2, Ф4).
//!
//! Two pure, integer-only, deterministic primitives that Ф4's width check reads
//! directly and the Ф7 centerline (`racing_line`, consuming [`medial_axis`],
//! `docs/design.md` §2 line 191) also consumes: a multi-source 4-connected BFS
//! wall-distance field ([`DistanceTransform`]) and its DT-ordered,
//! connectivity-preserving thinning skeleton ([`medial_axis`]). Both route
//! through [`Corridor`]'s private index/box helpers, exactly like
//! [`super::component_count`] and [`super::bounded_complement_components`].

use std::collections::{BTreeSet, VecDeque};

use super::{Corridor, Point, Rect};

/// The 4-connected wall-distance field of a corridor `D`.
///
/// `at(p)` is the Manhattan (4-conn step-count) distance from `p` to the nearest
/// `¬D` cell — `0` for any `p ∉ D` (including out-of-box points), `≥ 1` for every
/// drivable cell. A `D` cell on the box border is `¬D`-adjacent by construction
/// (out-of-box is `¬D`), so it always has `at == 1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistanceTransform {
    rect: Rect,
    dist: Vec<u32>,
}

impl DistanceTransform {
    /// Computes the wall-distance field of `d`.
    ///
    /// Multi-source 4-connected BFS: every `D` cell with a `¬D` 4-neighbor (a
    /// wall-adjacent cell, including the box border) seeds at distance `1`; BFS
    /// then relaxes outward through `D`, one layer at a time. `¬D` cells (and
    /// every out-of-box point) stay at the sentinel `0`, which never collides
    /// with a real distance since every reached `D` cell's distance is `≥ 1`.
    /// Deterministic (design doc §3a — integer-only, no floats).
    pub fn compute(d: &Corridor) -> Self {
        let rect = d.rect;
        let mut dist = vec![0u32; d.area()];
        let mut queue = VecDeque::new();

        // Seed pass: every D cell with a ¬D 4-neighbor starts at distance 1.
        for p in d.box_points() {
            let Some(idx) = d.index(p) else {
                continue;
            };
            if !d.contains(p) {
                continue;
            }
            if p.neighbors4().into_iter().any(|n| !d.contains(n)) {
                dist[idx] = 1;
                queue.push_back(p);
            }
        }

        // BFS relaxation: each popped cell's unvisited D neighbors get dist+1.
        while let Some(p) = queue.pop_front() {
            let Some(idx) = d.index(p) else {
                continue;
            };
            let next = dist[idx].saturating_add(1);
            for n in p.neighbors4() {
                let Some(ni) = d.index(n) else {
                    continue;
                };
                if d.contains(n) && dist[ni] == 0 {
                    dist[ni] = next;
                    queue.push_back(n);
                }
            }
        }

        Self { rect, dist }
    }

    /// The wall distance at `p` — `0` for any `p ∉ D` (out-of-box included).
    #[inline]
    pub fn at(&self, p: Point) -> u32 {
        self.rect.index(p).map_or(0, |i| self.dist[i])
    }

    /// The bounding box this field was computed over.
    #[inline]
    pub const fn rect(&self) -> Rect {
        self.rect
    }
}

/// The 8 Moore-neighborhood cells around `p`, in `E, NE, N, NW, W, SW, S, SE`
/// order — the axis (4-connected) neighbors sit at the even indices
/// `0`/`2`/`4`/`6`. Saturating offsets keep this total for any `Point`,
/// including at `i32::MAX`/`i32::MIN`.
const fn neighbors8(p: Point) -> [Point; 8] {
    [
        Point::new(p.x.saturating_add(1), p.y),
        Point::new(p.x.saturating_add(1), p.y.saturating_add(1)),
        Point::new(p.x, p.y.saturating_add(1)),
        Point::new(p.x.saturating_sub(1), p.y.saturating_add(1)),
        Point::new(p.x.saturating_sub(1), p.y),
        Point::new(p.x.saturating_sub(1), p.y.saturating_sub(1)),
        Point::new(p.x, p.y.saturating_sub(1)),
        Point::new(p.x.saturating_add(1), p.y.saturating_sub(1)),
    ]
}

/// Whether `a`/`b` are one 4-connected (Manhattan) step apart.
const fn is_4adjacent(a: Point, b: Point) -> bool {
    a.x.abs_diff(b.x).saturating_add(a.y.abs_diff(b.y)) == 1
}

/// Whether `a`/`b` are one 8-connected (Chebyshev) step apart.
const fn is_8adjacent(a: Point, b: Point) -> bool {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    dx <= 1 && dy <= 1 && (dx == 1 || dy == 1)
}

/// The 4-connected degree of `p` within `s` — the number of `p`'s 4-neighbors
/// that are themselves members of `s`.
fn degree4(s: &BTreeSet<Point>, p: Point) -> usize {
    p.neighbors4().into_iter().filter(|q| s.contains(q)).count()
}

/// Whether `p` is a *simple* point of `s` — deletable without changing `s`'s
/// local (4,8)-connectivity topology (§ module doc's thinning algorithm).
///
/// Computed entirely within the 3×3 window around `p` ([`neighbors8`]), via an
/// explicit, allocation-free flood fill over the fixed 8-slot window — not a
/// remembered crossing-number formula. Split `A` = window cells in `s`, `B` =
/// window cells not in `s` (out-of-box cells count as `B`). `p` is simple iff:
///
/// 1. `A`'s 4-connected components (adjacency computed only within the
///    window) that contain at least one of `p`'s axis neighbors number
///    exactly one, **and**
/// 2. `B` is non-empty and forms exactly one 8-connected component (also
///    computed only within the window).
fn is_simple(s: &BTreeSet<Point>, p: Point) -> bool {
    let window = neighbors8(p);
    let member = window.map(|q| s.contains(&q));

    let mut visited = [false; 8];
    let mut axis_touching_components: u32 = 0;
    for start in 0..8 {
        if !member[start] || visited[start] {
            continue;
        }
        let mut touches_axis = false;
        let mut stack = [0usize; 8];
        let mut top = 0usize;
        stack[top] = start;
        top = top.saturating_add(1);
        visited[start] = true;
        while top > 0 {
            top = top.saturating_sub(1);
            let cur = stack[top];
            if cur % 2 == 0 {
                touches_axis = true;
            }
            for next in 0..8 {
                if member[next] && !visited[next] && is_4adjacent(window[cur], window[next]) {
                    visited[next] = true;
                    stack[top] = next;
                    top = top.saturating_add(1);
                }
            }
        }
        if touches_axis {
            axis_touching_components = axis_touching_components.saturating_add(1);
        }
    }
    if axis_touching_components != 1 {
        return false;
    }

    let background = member.map(|m| !m);
    if !background.into_iter().any(|b| b) {
        return false;
    }
    let mut visited_bg = [false; 8];
    let mut background_components: u32 = 0;
    for start in 0..8 {
        if !background[start] || visited_bg[start] {
            continue;
        }
        let mut stack = [0usize; 8];
        let mut top = 0usize;
        stack[top] = start;
        top = top.saturating_add(1);
        visited_bg[start] = true;
        while top > 0 {
            top = top.saturating_sub(1);
            let cur = stack[top];
            for next in 0..8 {
                if background[next] && !visited_bg[next] && is_8adjacent(window[cur], window[next])
                {
                    visited_bg[next] = true;
                    stack[top] = next;
                    top = top.saturating_add(1);
                }
            }
        }
        background_components = background_components.saturating_add(1);
    }
    background_components == 1
}

/// Whether `p` is an *anchored end point* of `s` under `dt` — a genuine
/// medial branch tip that thinning must never delete.
///
/// `p` has exactly one 4-neighbor in `s` **and** `dt(p) >= dt(q)` for every
/// `q` in `p`'s 8-neighborhood. An unanchored degree-1 cell (a boundary
/// artefact whose `dt` is dominated by a neighbor) is **not** anchored, so
/// thinning still peels it back to the ridge.
fn is_anchored_endpoint(s: &BTreeSet<Point>, dt: &DistanceTransform, p: Point) -> bool {
    if degree4(s, p) != 1 {
        return false;
    }
    let dp = dt.at(p);
    neighbors8(p).into_iter().all(|q| dp >= dt.at(q))
}

/// The distance-ordered, connectivity-preserving thinning skeleton of `dt`
/// (design doc §D2 — "the distance-transform ridge … a branching geometric
/// object").
///
/// `D` is recovered from `dt` alone (`p ∈ D ⟺ dt.at(p) > 0`). Foreground is
/// **4-connected**, background **8-connected** — the complementary `(4, 8)`
/// digital-topology pair every consumer of this skeleton already assumes
/// (`racing_line`'s `components`/`degree`/`walk_cycle`, all 4-conn).
///
/// Algorithm: repeatedly delete the lowest-`dt` *simple* cell (`is_simple`)
/// that is not an *anchored end point* (`is_anchored_endpoint`), until none
/// remains deletable. The deletion order is a `BTreeSet<(u32, Point)>`
/// min-first queue — a total integer order, no hashing, no float, no
/// address- or iteration-order dependence — so the result is a pure,
/// deterministic function of `dt`, identical on every run and platform
/// (`compute_and_medial_axis_are_deterministic`).
///
/// Deleting a simple point preserves the (4,8)-homotopy type (component
/// counts of both foreground and background), and anchoring only *forbids*
/// deletions — so for a corridor that is 4-connected with exactly one
/// bounded hole (Ф4's `Disconnected`/`BadTopology` gate), the resulting
/// skeleton is connected and carries exactly one cycle. A constriction's
/// cross-section is crossed by construction (connectivity is preserved), so a
/// 1-cell neck's center cell — the neck's own cross-section — is always on
/// the skeleton.
///
/// "No 2×2 block of skeleton cells" (thinness) is an *empirical* property of
/// this thinning, not a proven guarantee: a pinwheel-attached 2×2 (four arms
/// leaving diagonally opposite corners) makes all four cells non-simple and
/// would survive. It has not been observed on any in-tree or generated
/// fixture; a caller that hits it degrades gracefully (`racing_line`'s
/// `walk_cycle` fallback), not a panic.
///
/// Returns a [`BTreeSet`] for deterministic, cross-platform iteration order
/// ([`Point`]'s derived `Ord`, `x`-then-`y`).
pub fn medial_axis(dt: &DistanceTransform) -> BTreeSet<Point> {
    let mut s: BTreeSet<Point> = dt.rect().points().filter(|&p| dt.at(p) > 0).collect();
    let mut queue: BTreeSet<(u32, Point)> = s.iter().map(|&p| (dt.at(p), p)).collect();

    while let Some((_, p)) = queue.pop_first() {
        if !s.contains(&p) {
            continue; // stale entry — p was already removed
        }
        if is_anchored_endpoint(&s, dt, p) {
            continue;
        }
        if !is_simple(&s, p) {
            continue;
        }
        s.remove(&p);
        for q in neighbors8(p) {
            if s.contains(&q) {
                queue.insert((dt.at(q), q)); // re-examine; BTreeSet dedups
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::common::cells;
    use crate::geom::{Coord, component_count};

    /// Build a corridor over `[origin, origin + (w, h))` with the given `(x, y)`
    /// cells marked drivable.
    fn corridor(
        origin: (Coord, Coord),
        w: usize,
        h: usize,
        drivable: &[(Coord, Coord)],
    ) -> Corridor {
        let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
        for &(x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    #[test]
    fn dt_straight_band_is_distance_to_nearest_wall() {
        // A fully-filled 5x3 box: DT along the 3-row height is 1,2,1 pattern, and
        // the middle row peaks at 2 for interior columns, dropping to 1 at the
        // left/right box edges (design doc's "1,2,...,2,1" fixture).
        let d = Corridor::filled(Point::new(0, 0), 5, 3);
        let dt = DistanceTransform::compute(&d);

        for x in 0..5 {
            assert_eq!(dt.at(Point::new(x, 0)), 1, "bottom row at x={x}");
            assert_eq!(dt.at(Point::new(x, 2)), 1, "top row at x={x}");
        }
        assert_eq!(dt.at(Point::new(0, 1)), 1);
        assert_eq!(dt.at(Point::new(1, 1)), 2);
        assert_eq!(dt.at(Point::new(2, 1)), 2);
        assert_eq!(dt.at(Point::new(3, 1)), 2);
        assert_eq!(dt.at(Point::new(4, 1)), 1);
    }

    #[test]
    fn at_is_zero_outside_d_and_outside_box() {
        let d = corridor((0, 0), 3, 3, &[(1, 1)]);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(dt.at(Point::new(0, 0)), 0, "in-box, not in D");
        assert_eq!(dt.at(Point::new(-1, -1)), 0, "out of box");
        assert_eq!(dt.at(Point::new(1, 1)), 1, "sole D cell, wall-adjacent");
    }

    #[test]
    fn medial_axis_is_thin_centerline_on_straight_band() {
        // Same 5x3 filled band: DT-ordered thinning peels every cell down to
        // the interior of the middle row (the cross-flow centerline). The two
        // end columns are not anchored end points (each is dominated by its
        // higher-dt neighbor toward the middle) so they thin away too,
        // leaving exactly the 3-cell interior strand — unchanged from the
        // old strict-local-max definition on this fixture (§ Test Design).
        let d = Corridor::filled(Point::new(0, 0), 5, 3);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(
            medial_axis(&dt),
            cells(&[(1, 1), (2, 1), (3, 1)])
                .into_iter()
                .collect::<BTreeSet<_>>(),
        );
    }

    #[test]
    fn medial_axis_includes_neck_and_is_connected_across_it() {
        // A wide 3-row-tall corridor pinched to a single-row neck at x=3: the
        // medial axis must include the neck cell and stay 4-connected through
        // it (the topology-preservation guarantee — a constriction's
        // cross-section is always crossed). Strengthened to the exact set:
        // unchanged from the old strict-local-max definition on this fixture.
        let mut drivable = Vec::new();
        for x in 0..7 {
            if x == 3 {
                drivable.push((x, 2));
            } else {
                for y in 1..4 {
                    drivable.push((x, y));
                }
            }
        }
        let d = corridor((0, 0), 7, 5, &drivable);
        let dt = DistanceTransform::compute(&d);
        let medial = medial_axis(&dt);

        assert_eq!(
            medial,
            cells(&[(1, 2), (2, 2), (3, 2), (4, 2), (5, 2)])
                .into_iter()
                .collect::<BTreeSet<_>>(),
        );
        for p in [Point::new(2, 2), Point::new(3, 2), Point::new(4, 2)] {
            assert!(medial.contains(&p), "{p:?} should be on the ridge");
        }
    }

    #[test]
    fn medial_axis_on_annulus_is_one_closed_thin_loop() {
        // An odd-thickness-3 square frame (11x11 outer minus a 5x5 centered
        // hole). Under the old strict-local-max definition this fixture's
        // ridge was 4 disjoint corner-gapped strips (20 cells); DT-ordered
        // anchored thinning instead produces one connected, thin, 32-cell
        // loop — the west strip thins back one cell short at each end while
        // the west corner survives as a square notch, and the other three
        // corners stay full-square, a deterministic consequence of the
        // pinned (dt, Point) deletion order (design doc § A1 subtask 5/6
        // table), not a bug.
        let mut d = Corridor::filled(Point::new(0, 0), 11, 11);
        for y in 3..8 {
            for x in 3..8 {
                d.set(Point::new(x, y), false);
            }
        }
        let dt = DistanceTransform::compute(&d);
        let medial = medial_axis(&dt);

        // Verbatim 32-cell predicted set (design doc § A1 subtask 5/6): the
        // west strip (x=1) is thinned back to y in 2..=8, x=2 keeps two
        // separate 2-cell corner notches (y in {1,2} and y in {8,9}) — not one
        // contiguous block — the north/south strips at x in 3..=8 stay 1-cell
        // each (y in {1,9}), and the east strip (x=9) keeps its full square
        // corners.
        let mut expected = BTreeSet::new();
        for y in 2..=8 {
            expected.insert(Point::new(1, y));
        }
        for &y in &[1, 2, 8, 9] {
            expected.insert(Point::new(2, y));
        }
        for x in 3..=8 {
            expected.insert(Point::new(x, 1));
            expected.insert(Point::new(x, 9));
        }
        for y in 1..=9 {
            expected.insert(Point::new(9, y));
        }
        assert_eq!(medial, expected);

        // The loop is one connected component (a genuine centerline, not
        // scattered strips).
        let mut ring = Corridor::new(Point::new(0, 0), 11, 11);
        for &p in &medial {
            ring.set(p, true);
        }
        assert_eq!(
            component_count(&ring),
            1,
            "the thinned annulus ridge must be one connected loop"
        );
    }

    #[test]
    fn medial_axis_even_width_band_is_a_two_cell_thin_skeleton() {
        // A 4x3 filled box: the cross-width (x, 4 cells) is even, so the two
        // middle columns tie on DT; both are anchored end points of the
        // final skeleton (this fixture is already 1-cell thin along its
        // short axis) — unchanged from the old strict-local-max definition.
        let d = Corridor::filled(Point::new(0, 0), 4, 3);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(
            medial_axis(&dt),
            cells(&[(1, 1), (2, 1)])
                .into_iter()
                .collect::<BTreeSet<_>>(),
        );
    }

    /// AC9 fixture provenance: under the old strict-local-max `medial_axis`
    /// this 61x61-minus-13x13-hole ring shatters into 32 cells / 32 singleton
    /// components; DT-ordered anchored thinning instead produces 146 cells in
    /// 1 component with no 2x2 block and 0 leaves (design doc § A1 subtask
    /// 5/6 table) — the same class of wide-corridor fragmentation the
    /// orchestrator's public-API probe measured on real generated corridors
    /// (40-84 components, `dt_peak` 14-21). Regression-guards the fix this AC9
    /// fixture exists for.
    #[cfg_attr(
        miri,
        ignore = "cost: 3642 thinning pops over 3552 cells ≈ 8.5 min under Tree Borrows; \
                  pure-integer, no UB signal the small distance.rs fixtures don't already cover"
    )]
    #[test]
    fn medial_axis_thins_a_wide_ring_to_one_connected_loop() {
        let mut d = Corridor::filled(Point::new(0, 0), 61, 61);
        for y in 24..37 {
            for x in 24..37 {
                d.set(Point::new(x, y), false);
            }
        }
        let dt = DistanceTransform::compute(&d);
        let medial = medial_axis(&dt);

        assert!(!medial.is_empty(), "the wide ring must not thin to nothing");
        for &p in &medial {
            assert!(dt.at(p) > 0, "{p:?} must be a drivable cell");
            assert!(degree4(&medial, p) >= 1, "{p:?} must have 4-degree >= 1");
        }

        let mut ring = Corridor::new(Point::new(0, 0), 61, 61);
        for &p in &medial {
            ring.set(p, true);
        }
        assert_eq!(
            component_count(&ring),
            1,
            "the wide ring's skeleton must be one connected loop"
        );

        // No 2x2 block of skeleton cells.
        for &p in &medial {
            let east = Point::new(p.x.saturating_add(1), p.y);
            let north = Point::new(p.x, p.y.saturating_add(1));
            let north_east = Point::new(p.x.saturating_add(1), p.y.saturating_add(1));
            assert!(
                !(medial.contains(&east)
                    && medial.contains(&north)
                    && medial.contains(&north_east)),
                "no 2x2 block: {p:?}/{east:?}/{north:?}/{north_east:?}"
            );
        }
    }

    // ---- Predicate unit tests (is_simple / is_anchored_endpoint) --------

    #[test]
    fn is_simple_rejects_an_isolated_cell() {
        let p = Point::new(5, 5);
        let s = BTreeSet::from([p]);
        assert!(!is_simple(&s, p), "an isolated cell has no A component");
    }

    #[test]
    fn is_simple_rejects_an_interior_cell_of_a_filled_block() {
        // p's entire 3x3 window is in S, so B (background) is empty.
        let p = Point::new(1, 1);
        let s: BTreeSet<Point> = (0..3)
            .flat_map(|x| (0..3).map(move |y| Point::new(x, y)))
            .collect();
        assert!(!is_simple(&s, p), "an interior cell's B is empty");
    }

    #[test]
    fn is_simple_rejects_a_straight_line_interior_cell() {
        // p's east/west neighbors are both in S but not adjacent to each
        // other within the window — two separate A components.
        let p = Point::new(1, 0);
        let s = BTreeSet::from([Point::new(0, 0), p, Point::new(2, 0)]);
        assert!(!is_simple(&s, p), "east/west form two A components");
    }

    #[test]
    fn is_simple_accepts_an_l_corner_of_a_two_by_two_block() {
        // p is one corner of a 2x2 block: its E/NE/N window neighbors form
        // one A component, and the remaining 5 window cells form one B
        // component.
        let p = Point::new(0, 0);
        let s = BTreeSet::from([p, Point::new(1, 0), Point::new(0, 1), Point::new(1, 1)]);
        assert!(is_simple(&s, p));
    }

    #[test]
    fn is_anchored_endpoint_accepts_a_one_cell_wide_finger_tip() {
        // A straight 1-wide 5-cell corridor: uniform dt == 1 everywhere, so
        // the tip (degree4 == 1 in s) is anchored (dp >= every neighbor,
        // out-of-box neighbors read as 0).
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        let dt = DistanceTransform::compute(&d);
        let s: BTreeSet<Point> = (0..5).map(|x| Point::new(x, 0)).collect();
        let tip = Point::new(0, 0);
        assert!(is_anchored_endpoint(&s, &dt, tip));
    }

    #[test]
    fn is_anchored_endpoint_rejects_a_low_dt_degree_one_corner_artefact() {
        // A 5x5 filled block with a single-cell east tail attached at its
        // middle row: the tail tip's own dt (1, wall-adjacent on 3 sides) is
        // dominated by its block-side neighbor's real dt (2, one cell more
        // interior) — a degree-1 cell that is NOT a genuine medial tip.
        let mut d = Corridor::new(Point::new(0, 0), 6, 5);
        for x in 0..5 {
            for y in 0..5 {
                d.set(Point::new(x, y), true);
            }
        }
        d.set(Point::new(5, 2), true);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(dt.at(Point::new(5, 2)), 1, "tail tip's own dt");
        assert_eq!(dt.at(Point::new(4, 2)), 2, "tail's block-side neighbor dt");

        let tip = Point::new(5, 2);
        let s = BTreeSet::from([tip, Point::new(4, 2)]);
        assert_eq!(degree4(&s, tip), 1, "tip has exactly one 4-neighbor in s");
        assert!(!is_anchored_endpoint(&s, &dt, tip));
    }

    #[test]
    fn empty_corridor_has_zero_dt_and_empty_medial_axis() {
        let d = Corridor::new(Point::new(0, 0), 4, 4);
        let dt = DistanceTransform::compute(&d);
        for p in d.box_points() {
            assert_eq!(dt.at(p), 0);
        }
        assert!(medial_axis(&dt).is_empty());
    }

    #[test]
    fn rect_round_trips_the_box() {
        let d = Corridor::new(Point::new(2, 3), 6, 4);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(
            dt.rect(),
            Rect {
                origin: Point::new(2, 3),
                size: crate::geom::Size::new(6, 4),
            }
        );
    }

    #[test]
    fn compute_and_medial_axis_are_deterministic() {
        let mut drivable = Vec::new();
        for x in 0..7 {
            if x == 3 {
                drivable.push((x, 2));
            } else {
                for y in 1..4 {
                    drivable.push((x, y));
                }
            }
        }
        let d = corridor((0, 0), 7, 5, &drivable);

        let dt1 = DistanceTransform::compute(&d);
        let dt2 = DistanceTransform::compute(&d);
        assert_eq!(dt1, dt2);

        let m1 = medial_axis(&dt1);
        let m2 = medial_axis(&dt2);
        assert_eq!(m1, m2);
    }
}
