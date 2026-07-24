//! Ф7: render-only racing centerline producer (design doc §2 line 191:
//! `centerline = racing_line(medial_axis(D))`).
//!
//! [`medial_axis`] deliberately leaves a *thin but imperfect* ridge cell set:
//! even-width 2-cell bands unthinned, a diagonal gap at each rectilinear
//! corner, and spur branches on infield-finger / hairpin tracks (its own
//! rustdoc names these as `racing_line`'s job). [`racing_line`] turns that set
//! into one closed, arc-length-parameterised, `race_dir`-oriented loop:
//! bridge cross-component gaps (4-connectivity + [`supercover`]) → prune
//! degree-1 spurs → walk a straightest-continuation cycle anchored at
//! `gate`'s forward face → orient by integer shoelace winding vs `race_dir`
//! → resample by arc length → wraparound unit tangents. Every failure path
//! (empty medial axis, an unbridgeable gap, an empty post-prune core, or a
//! walk that cannot close) returns [`Centerline::default()`] — render-only,
//! `Centerline::at` already degrades gracefully on an empty centerline; this
//! producer never panics.

use std::collections::BTreeSet;

use gp_core::geom::{Corridor, DistanceTransform, Point, medial_axis, supercover};
use gp_core::track::{Centerline, RaceDir, TimingGate};

/// Maximum Manhattan gap (in cells) [`bridge_gaps`] will bridge between two
/// cross-component medial-axis cells; a wider minimal gap abandons bridging
/// (fallback to [`Centerline::default()`]). The annulus fixture's nearest
/// cross-strip corner pair (e.g. `(3, 1)` to `(1, 3)`) is Manhattan `4`; this
/// is that value plus a small margin.
const MAX_BRIDGE_GAP: i32 = 6;

/// The Manhattan distance between `a` and `b`, widened to avoid any overflow
/// concern regardless of the (bounded, grid-realistic) input magnitudes.
fn manhattan(a: Point, b: Point) -> i64 {
    i64::from(a.x.abs_diff(b.x)).saturating_add(i64::from(a.y.abs_diff(b.y)))
}

/// The 4-connected components of `cells`, as a `Vec` of disjoint `BTreeSet`s.
///
/// Deterministic: `cells` is walked in `BTreeSet` (`x`-then-`y`) order, so
/// repeated calls on the same input yield components in the same order with
/// the same membership.
fn components(cells: &BTreeSet<Point>) -> Vec<BTreeSet<Point>> {
    let mut remaining = cells.clone();
    let mut comps = Vec::new();
    while let Some(&start) = remaining.iter().next() {
        let mut comp = BTreeSet::new();
        let mut stack = vec![start];
        remaining.remove(&start);
        while let Some(p) = stack.pop() {
            comp.insert(p);
            for n in p.neighbors4() {
                if remaining.remove(&n) {
                    stack.push(n);
                }
            }
        }
        comps.push(comp);
    }
    comps
}

/// Bridges cross-component gaps in the medial cell set `medial` (design doc
/// § "The loop-trim / resample algorithm", step 2).
///
/// While more than one 4-connected component remains, finds the
/// cross-component cell pair `(a, b)` of minimal Manhattan distance
/// (deterministic tie-break: minimal `(a, b)` by `Point`'s derived `Ord`,
/// `a <= b`) and inserts every `supercover(a, b)` cell that lies in `d` into
/// the set. Returns `None` (fallback) if `medial` is empty, or if the
/// smallest remaining cross-component gap ever exceeds [`MAX_BRIDGE_GAP`].
fn bridge_gaps(d: &Corridor, medial: BTreeSet<Point>) -> Option<BTreeSet<Point>> {
    if medial.is_empty() {
        return None;
    }
    let mut cells = medial;
    loop {
        let comps = components(&cells);
        if comps.len() <= 1 {
            return Some(cells);
        }
        let mut best: Option<(i64, Point, Point)> = None;
        for i in 0..comps.len() {
            for j in (i.saturating_add(1))..comps.len() {
                for &a in &comps[i] {
                    for &b in &comps[j] {
                        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                        let cand = (manhattan(a, b), lo, hi);
                        if best.is_none_or(|cur| cand < cur) {
                            best = Some(cand);
                        }
                    }
                }
            }
        }
        // comps.len() > 1 guarantees at least one cross-component pair exists.
        let (dist, a, b) = best.expect("multiple components imply a cross-component pair");
        if dist > i64::from(MAX_BRIDGE_GAP) {
            return None;
        }
        for p in supercover(a, b) {
            if d.contains(p) {
                cells.insert(p);
            }
        }
    }
}

/// The 4-connected degree of `p` within `cells` — the number of `p`'s 4
/// neighbors that are themselves members of `cells`.
fn degree(cells: &BTreeSet<Point>, p: Point) -> usize {
    p.neighbors4()
        .into_iter()
        .filter(|n| cells.contains(n))
        .count()
}

/// Prunes spur (degree-1) branches from the bridged medial set `cells`
/// (design doc § "The loop-trim / resample algorithm", step 3).
///
/// Iterative degree-1 removal (synchronous rounds: each round's degrees are
/// computed against the round's starting set, then every degree-`< 2` cell is
/// removed together) until every surviving cell has 4-connected degree `>= 2`
/// — the graph's 2-core. A clean, already-thinned loop is all-degree-2 (the
/// pass is then a no-op); infield-finger / hairpin spurs are trees hanging
/// off the ring and peel away round by round. Returns `None` (fallback) if
/// the 2-core is empty (nothing survives, e.g. `cells` was itself a tree with
/// no cycle).
fn prune_spurs(cells: &BTreeSet<Point>) -> Option<BTreeSet<Point>> {
    let mut cur = cells.clone();
    loop {
        let leaves: Vec<Point> = cur
            .iter()
            .copied()
            .filter(|&p| degree(&cur, p) < 2)
            .collect();
        if leaves.is_empty() {
            break;
        }
        for p in leaves {
            cur.remove(&p);
        }
    }
    if cur.is_empty() { None } else { Some(cur) }
}

/// The `core` cell nearest (Manhattan) to the centroid of `gate`'s forward
/// face, tie-broken by minimal `Point` (design doc § "The loop-trim /
/// resample algorithm", step 4). `None` if either `core` or the forward face
/// is empty.
#[allow(
    clippy::cast_precision_loss,
    reason = "face.len() is a small, grid-realistic cell count, exactly \
              representable in f64"
)]
fn anchor(core: &BTreeSet<Point>, gate: &TimingGate) -> Option<Point> {
    let face: Vec<Point> = gate.forward_face().collect();
    if face.is_empty() || core.is_empty() {
        return None;
    }
    let n = face.len() as f64;
    let (sx, sy) = face.iter().fold((0.0f64, 0.0f64), |(sx, sy), p| {
        (sx + f64::from(p.x), sy + f64::from(p.y))
    });
    let (cx, cy) = (sx / n, sy / n);
    let dist_to_centroid = |p: Point| (f64::from(p.x) - cx).abs() + (f64::from(p.y) - cy).abs();

    let mut ranked: Vec<(f64, Point)> = core.iter().map(|&p| (dist_to_centroid(p), p)).collect();
    ranked.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    ranked.into_iter().next().map(|(_, p)| p)
}

/// The `(dx, dy)` step from `from` to a 4-connected neighbor `to` — bounded
/// to `{-1, 0, 1}` per axis in every caller here, since `to` is always drawn
/// from `from.neighbors4()`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "to is always one of from.neighbors4() in every call site below, \
              so each axis delta is bounded to {-1, 0, 1} and cannot overflow"
)]
const fn step_delta(from: Point, to: Point) -> (i32, i32) {
    (to.x - from.x, to.y - from.y)
}

/// The straightest-continuation priority rank of step direction `dir`
/// relative to `heading` (design doc step 4): `0` = straight ahead, `1` = a
/// turn, `2` = a U-turn, and `3` when there is no established `heading` yet
/// (every direction ties, so only the `Point`-order tie-break in
/// [`walk_cycle`] discriminates).
#[allow(
    clippy::arithmetic_side_effects,
    reason = "heading and dir are always {-1, 0, 1}-per-axis unit step deltas \
              (see step_delta), so the dot product is bounded to {-1, 0, 1} \
              and cannot overflow"
)]
const fn turn_rank(heading: Option<(i32, i32)>, dir: (i32, i32)) -> u8 {
    let Some(h) = heading else { return 3 };
    match h.0 * dir.0 + h.1 * dir.1 {
        1 => 0,
        0 => 1,
        _ => 2,
    }
}

/// Walks `core` into an ordered, closed cycle starting at [`anchor`] (design
/// doc § "The loop-trim / resample algorithm", step 4).
///
/// At each step, picks the best straightest-continuation candidate among
/// `core` neighbors that are either unvisited or (once at least 4 cells have
/// been walked, the minimum possible 4-connected grid cycle length) the
/// `start` cell itself — closing the loop. Ties are broken by minimal
/// `Point`. Returns `None` (fallback) if `core`/the forward face is empty, if
/// a step dead-ends (no viable candidate), or if the walk exhausts `core`
/// without ever closing.
fn walk_cycle(core: &BTreeSet<Point>, gate: &TimingGate) -> Option<Vec<Point>> {
    let start = anchor(core, gate)?;
    let mut order = vec![start];
    let mut visited = BTreeSet::from([start]);
    let mut heading: Option<(i32, i32)> = None;

    loop {
        let current = *order.last().expect("order is never empty");
        let mut candidates: Vec<Point> = current
            .neighbors4()
            .into_iter()
            .filter(|n| core.contains(n))
            .filter(|&n| !visited.contains(&n) || (n == start && order.len() >= 4))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by_key(|&cand| {
            let dir = step_delta(current, cand);
            (turn_rank(heading, dir), cand)
        });
        let next = candidates[0];
        if next == start {
            break;
        }
        heading = Some(step_delta(current, next));
        visited.insert(next);
        order.push(next);
        if order.len() > core.len() {
            return None; // safety: cannot exceed core's own size without closing
        }
    }

    if order.len() < 4 { None } else { Some(order) }
}

/// Produces the render-only racing centerline for corridor `d` (design doc §2
/// line 191).
///
/// Computes the distance transform + medial axis internally, trims and
/// orders the result into a single closed loop anchored at `gate`'s forward
/// face and oriented along `race_dir`, then resamples it by arc length.
/// Never panics: every failure path (empty medial axis and — once wired —
/// every later-stage fallback) returns [`Centerline::default()`], which
/// degrades gracefully under [`Centerline::at`].
pub fn racing_line(d: &Corridor, gate: &TimingGate, _race_dir: RaceDir) -> Centerline {
    let dt = DistanceTransform::compute(d);
    let medial = medial_axis(&dt);
    if medial.is_empty() {
        return Centerline::default();
    }
    let Some(bridged) = bridge_gaps(d, medial) else {
        return Centerline::default();
    };
    let Some(core) = prune_spurs(&bridged) else {
        return Centerline::default();
    };
    let Some(_cycle) = walk_cycle(&core, gate) else {
        return Centerline::default();
    };
    // Subtask 5 wires the rest of the pipeline; until then an ordered cycle
    // also falls back (no producer overclaims yet).
    Centerline::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::Side;
    use gp_core::track::TimingGate;

    /// Subtask 1: an empty corridor (no drivable cells) has an empty medial
    /// axis, so `racing_line` falls back to `Centerline::default()` — empty
    /// samples, zero length, and `at` returning `None`.
    #[test]
    fn empty_corridor_yields_default_centerline() {
        let d = Corridor::new(Point::new(0, 0), 4, 4);
        let gate = TimingGate {
            behind: vec![],
            forward: Side::East,
        };
        let cl = racing_line(&d, &gate, RaceDir::Ccw);
        assert!(cl.samples.is_empty());
        assert!(cl.length.abs() < f32::EPSILON);
        assert!(cl.at(0.0).is_none());
    }

    /// Subtask 2 (happy): the annulus fixture's 4 corner-gapped medial strips
    /// bridge into one 4-connected component.
    #[test]
    fn bridge_gaps_joins_annulus_corner_gaps_into_one_component() {
        let d = crate::testfix::annulus_corridor();
        let dt = DistanceTransform::compute(&d);
        let medial = medial_axis(&dt);
        assert!(
            components(&medial).len() > 1,
            "the annulus's medial axis starts as >1 disjoint strip"
        );

        let bridged = bridge_gaps(&d, medial).expect("annulus corner gaps are bridgeable");
        assert_eq!(
            components(&bridged).len(),
            1,
            "bridging must join all 4 strips into one component"
        );
    }

    /// Subtask 2 (edge): two components a `MAX_BRIDGE_GAP`-exceeding Manhattan
    /// distance apart abandon bridging (fallback signal).
    #[test]
    fn bridge_gaps_abandons_over_max_gap() {
        let d = crate::testfix::corridor((0, 0), 1, 40, &[(0, 0), (0, 39)]);
        let medial = BTreeSet::from([Point::new(0, 0), Point::new(0, 39)]);
        assert!(bridge_gaps(&d, medial).is_none());
    }

    /// Subtask 2 (edge): two components within `MAX_BRIDGE_GAP` bridge into
    /// one component, using only cells that lie in `d`.
    #[test]
    fn bridge_gaps_joins_a_close_gap() {
        let d = crate::testfix::corridor((0, 0), 1, 5, &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]);
        let medial = BTreeSet::from([
            Point::new(0, 0),
            Point::new(0, 1),
            Point::new(0, 3),
            Point::new(0, 4),
        ]);
        let bridged = bridge_gaps(&d, medial).expect("a 2-cell gap is within MAX_BRIDGE_GAP");
        assert_eq!(components(&bridged).len(), 1);
        assert!(bridged.contains(&Point::new(0, 2)));
    }

    /// Subtask 2: an empty medial set is not bridgeable (fallback signal).
    #[test]
    fn bridge_gaps_rejects_empty_medial() {
        let d = Corridor::new(Point::new(0, 0), 4, 4);
        assert!(bridge_gaps(&d, BTreeSet::new()).is_none());
    }

    /// A hand-built 4×4 square ring (border of a 4×4 box), 4-connected and
    /// already all-degree-2 — the "clean loop" prune fixture.
    fn small_ring() -> BTreeSet<Point> {
        BTreeSet::from([
            Point::new(0, 0),
            Point::new(1, 0),
            Point::new(2, 0),
            Point::new(3, 0),
            Point::new(3, 1),
            Point::new(3, 2),
            Point::new(3, 3),
            Point::new(2, 3),
            Point::new(1, 3),
            Point::new(0, 3),
            Point::new(0, 2),
            Point::new(0, 1),
        ])
    }

    /// Subtask 3 (happy): an all-degree-2 loop is unaffected by pruning.
    #[test]
    fn prune_spurs_is_a_no_op_on_a_clean_loop() {
        let ring = small_ring();
        assert_eq!(prune_spurs(&ring), Some(ring));
    }

    /// Subtask 3 (happy): a 2-cell dead-end finger hanging off ring cell
    /// `(1, 0)` (poking outward, below the ring's own box) is fully peeled
    /// away, leaving exactly the ring, all-degree-2.
    #[test]
    fn prune_spurs_removes_a_dangling_finger() {
        let mut with_finger = small_ring();
        with_finger.insert(Point::new(1, -1)); // attaches to ring's (1, 0)
        with_finger.insert(Point::new(1, -2)); // the dead-end tip

        let core = prune_spurs(&with_finger).expect("the ring survives pruning");
        assert_eq!(core, small_ring());
        for &p in &core {
            assert!(degree(&core, p) >= 2, "{p:?} must have degree >= 2");
        }
    }

    /// Subtask 3 (edge): a pure tree (no cycle) prunes to nothing (fallback
    /// signal).
    #[test]
    fn prune_spurs_rejects_a_pure_tree() {
        let tree = BTreeSet::from([Point::new(0, 0), Point::new(1, 0), Point::new(2, 0)]);
        assert!(prune_spurs(&tree).is_none());
    }

    /// A gate whose `behind`/`forward_face` anchor sits just outside
    /// `small_ring`'s bottom edge, near `(1, 0)`/`(2, 0)`.
    fn small_ring_gate() -> TimingGate {
        TimingGate {
            behind: vec![Point::new(1, -1), Point::new(2, -1)],
            forward: Side::North,
        }
    }

    /// Subtask 4 (happy): the ring core walks into an ordered cycle that
    /// covers every ring cell and closes back to its own start.
    #[test]
    fn walk_cycle_covers_and_closes_a_clean_ring() {
        let ring = small_ring();
        let order = walk_cycle(&ring, &small_ring_gate()).expect("a clean ring must close");
        assert_eq!(order.len(), ring.len());
        assert_eq!(order.iter().copied().collect::<BTreeSet<_>>(), ring);
        let start = order[0];
        assert!(start.neighbors4().contains(order.last().unwrap()));
    }

    /// Subtask 4 (thinning): a 2-cell-wide band traces a single strand — the
    /// walk never visits both rails of the band.
    #[test]
    fn walk_cycle_thins_a_two_cell_band_to_one_strand() {
        // A closed loop with an even-width (2-cell) top/bottom band: rows
        // y=0 and y=3 are single-wide; the "band" is the two parallel side
        // columns x=0..=1 (west) and x=4..=5 (east), each 2 cells wide across
        // rows y=1..=2 — mimicking medial_axis_even_width_band_is_two_cell's
        // documented 2-cell ridge.
        let mut band = BTreeSet::new();
        for x in 0..6 {
            band.insert(Point::new(x, 0));
            band.insert(Point::new(x, 3));
        }
        for y in 1..3 {
            for x in [0, 1, 4, 5] {
                band.insert(Point::new(x, y));
            }
        }
        let gate = TimingGate {
            behind: vec![Point::new(2, -1)],
            forward: Side::North,
        };
        let order = walk_cycle(&band, &gate).expect("a thinnable band must close");
        // Every visited cell distinct (a simple cycle, not a doubled-back walk).
        assert_eq!(
            order.len(),
            order.iter().collect::<BTreeSet<_>>().len(),
            "walk must not revisit a cell"
        );
        // The walk never uses both rails of a side band in the same row: for
        // each side, at most one of the two columns appears per row.
        for y in 1..3 {
            let west = [Point::new(0, y), Point::new(1, y)]
                .iter()
                .filter(|p| order.contains(p))
                .count();
            let east = [Point::new(4, y), Point::new(5, y)]
                .iter()
                .filter(|p| order.contains(p))
                .count();
            assert!(west <= 1, "row {y} west rail must be single-strand");
            assert!(east <= 1, "row {y} east rail must be single-strand");
        }
    }

    /// Subtask 4 (edge): a broken (open) core — a straight line, not a ring —
    /// dead-ends and never returns to `start` (fallback signal).
    #[test]
    fn walk_cycle_rejects_an_open_core() {
        let open: BTreeSet<Point> = (0..5).map(|x| Point::new(x, 0)).collect();
        let gate = TimingGate {
            behind: vec![Point::new(2, -1)],
            forward: Side::North,
        };
        assert!(walk_cycle(&open, &gate).is_none());
    }
}
