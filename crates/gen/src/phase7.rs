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
use gp_core::track::{Centerline, CenterlineSample, RaceDir, TimingGate};

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

/// Bridges gaps in the medial cell set `medial` (design doc § "The loop-trim
/// / resample algorithm", step 2).
///
/// Repeatedly finds the minimal-Manhattan-distance pair of degree-`< 2`
/// ("leaf") cells `(a, b)` that are not already 4-adjacent, preferring a
/// cross-component pair over a same-component one whenever both exist, and
/// inserts every `supercover(a, b)` cell that lies in `d` into the set
/// (deterministic tie-break: minimal `(a, b)` by `Point`'s derived `Ord`,
/// `a <= b`). Stops when fewer than 2 leaves remain.
///
/// **Why same-component pairs too, not only cross-component:** merely
/// reaching one 4-connected component does not imply a *closed* loop — a
/// ring's last corner gap can bridge its two flanking strips into the same
/// component (via the other 3 corners) while leaving that last gap's own two
/// leaf cells unconnected, cross-component-blind bridging would then declare
/// victory one gap early. Preferring cross- over same-component candidates
/// each round still closes the "easy" gaps between genuinely separate
/// pieces first, before ever touching a same-component pair.
///
/// Returns `None` (fallback) if `medial` is empty, or if the smallest
/// candidate gap ever exceeds [`MAX_BRIDGE_GAP`]. Also stops (without
/// panicking or looping) if a chosen candidate's bridge inserts no new cell —
/// a leftover open path (not a ring) has no more available progress.
fn bridge_gaps(d: &Corridor, medial: BTreeSet<Point>) -> Option<BTreeSet<Point>> {
    if medial.is_empty() {
        return None;
    }
    let mut cells = medial;
    loop {
        let leaves: Vec<Point> = cells
            .iter()
            .copied()
            .filter(|&p| degree(&cells, p) < 2)
            .collect();
        if leaves.len() < 2 {
            return Some(cells);
        }
        let comps = components(&cells);
        let comp_of = |p: Point| comps.iter().position(|c| c.contains(&p));

        let mut cross: Option<(i64, Point, Point)> = None;
        let mut same: Option<(i64, Point, Point)> = None;
        for i in 0..leaves.len() {
            for j in (i.saturating_add(1))..leaves.len() {
                let (a, b) = (leaves[i], leaves[j]);
                if a.neighbors4().into_iter().any(|n| n == b) {
                    continue; // already directly connected; nothing to bridge
                }
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                let cand = (manhattan(a, b), lo, hi);
                let slot = if comp_of(a) == comp_of(b) {
                    &mut same
                } else {
                    &mut cross
                };
                if slot.is_none_or(|cur| cand < cur) {
                    *slot = Some(cand);
                }
            }
        }
        let Some((dist, a, b)) = cross.or(same) else {
            return Some(cells); // no viable leaf pair left to bridge
        };
        if dist > i64::from(MAX_BRIDGE_GAP) {
            return None;
        }
        let before = cells.len();
        for p in supercover(a, b) {
            if d.contains(p) {
                cells.insert(p);
            }
        }
        if cells.len() == before {
            return Some(cells); // this pair's bridge added nothing new; stop
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

/// Arc-length spacing (in cells) between resampled `CenterlineSample`s —
/// approximately one cell, per the design's "even spacing" requirement (AC2).
const RESAMPLE_STEP: f32 = 1.0;

/// The integer shoelace signed area `Σ(xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ)` of the closed
/// polygon `order` (design doc step 5). This grid is x-east/y-north
/// right-handed, so the standard convention holds: `> 0` for a
/// counter-clockwise ring, `< 0` for clockwise (pinned by
/// `shoelace_sign_matches_ccw_convention` below). All-`saturating` integer
/// arithmetic — no raw `+`/`-`/`*` — so no `arithmetic_side_effects` allow is
/// needed.
fn shoelace(order: &[Point]) -> i64 {
    order
        .iter()
        .zip(order.iter().skip(1).chain(order.iter().take(1)))
        .fold(0i64, |sum, (&a, &b)| {
            let term = i64::from(a.x)
                .saturating_mul(i64::from(b.y))
                .saturating_sub(i64::from(b.x).saturating_mul(i64::from(a.y)));
            sum.saturating_add(term)
        })
}

/// Orients the closed cycle `order` to match `race_dir`'s winding sense
/// (design doc step 5): reverses `order` iff its shoelace sign disagrees with
/// `race_dir` (`Ccw` ⇔ `> 0`, `Cw` ⇔ `< 0`).
fn orient(mut order: Vec<Point>, race_dir: RaceDir) -> Vec<Point> {
    let is_ccw = shoelace(&order) > 0;
    let wants_ccw = matches!(race_dir, RaceDir::Ccw);
    if is_ccw != wants_ccw {
        order.reverse();
    }
    order
}

/// The sub-cell `(f32, f32)` position of cell center `p`.
#[allow(
    clippy::cast_precision_loss,
    reason = "grid coordinates are bounded by corridor dimensions, far below \
              f32's 24-bit exact-integer range"
)]
const fn point_pos(p: Point) -> (f32, f32) {
    (p.x as f32, p.y as f32)
}

/// The next index after `i` in a `n`-length wraparound cycle.
const fn next_index(i: usize, n: usize) -> usize {
    let j = i.saturating_add(1);
    if j < n { j } else { 0 }
}

/// The index before `i` in a `n`-length wraparound cycle.
const fn prev_index(i: usize, n: usize) -> usize {
    if i == 0 {
        n.saturating_sub(1)
    } else {
        i.saturating_sub(1)
    }
}

/// A unit `(f32, f32)` vector along `v`, or the flat fallback `(1.0, 0.0)`
/// when `v` is degenerate (zero length).
fn normalize_vec(v: (f32, f32)) -> (f32, f32) {
    let len = v.0.hypot(v.1);
    if len > 0.0 {
        (v.0 / len, v.1 / len)
    } else {
        (1.0, 0.0)
    }
}

/// Resamples the ordered, closed cycle `order` by arc length (design doc
/// steps 6-7): emits a `CenterlineSample` every [`RESAMPLE_STEP`] of
/// accumulated perimeter, seeding `samples[0].s == 0`, then fills each
/// sample's unit tangent from its wraparound neighbors' positions. `None`
/// (fallback) if `order` is empty or its perimeter is non-positive/non-finite
/// (a degenerate, zero-area cycle).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the sample count is length/RESAMPLE_STEP floored and clamped to \
              >= 1: a small, grid-realistic, non-negative cell count, exactly \
              representable in f32 and well within usize range"
)]
fn resample(order: &[Point]) -> Option<Centerline> {
    let positions: Vec<(f32, f32)> = order.iter().map(|&p| point_pos(p)).collect();
    let n = positions.len();
    if n == 0 {
        return None;
    }
    let seg_lengths: Vec<f32> = positions
        .iter()
        .zip(positions.iter().skip(1).chain(positions.iter().take(1)))
        .map(|(&(ax, ay), &(bx, by))| (bx - ax).hypot(by - ay))
        .collect();
    let length: f32 = seg_lengths.iter().copied().sum();
    if length.is_sign_negative() || length == 0.0 || !length.is_finite() {
        return None;
    }
    let mut cum = vec![0.0f32; n];
    for i in 1..n {
        cum[i] = cum[prev_index(i, n)] + seg_lengths[prev_index(i, n)];
    }

    let step_count = (length / RESAMPLE_STEP).floor().max(1.0) as usize;
    let mut samples = Vec::with_capacity(step_count);
    for i in 0..step_count {
        let s = i as f32 * RESAMPLE_STEP;
        let idx = cum
            .iter()
            .rposition(|&c| c <= s)
            .unwrap_or(0)
            .min(n.saturating_sub(1));
        let (ax, ay) = positions[idx];
        let (bx, by) = positions[next_index(idx, n)];
        let seg = seg_lengths[idx];
        let t = if seg > 0.0 { (s - cum[idx]) / seg } else { 0.0 };
        let pos = ((bx - ax).mul_add(t, ax), (by - ay).mul_add(t, ay));
        samples.push(CenterlineSample {
            s,
            pos,
            tangent: (0.0, 0.0), // filled below, once every sample position exists
        });
    }

    let m = samples.len();
    let sample_positions: Vec<(f32, f32)> = samples.iter().map(|sm| sm.pos).collect();
    for (i, sample) in samples.iter_mut().enumerate() {
        let (px, py) = sample_positions[prev_index(i, m)];
        let (nx, ny) = sample_positions[next_index(i, m)];
        sample.tangent = normalize_vec((nx - px, ny - py));
    }

    Some(Centerline { samples, length })
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
pub fn racing_line(d: &Corridor, gate: &TimingGate, race_dir: RaceDir) -> Centerline {
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
    let Some(cycle) = walk_cycle(&core, gate) else {
        return Centerline::default();
    };
    let oriented = orient(cycle, race_dir);
    resample(&oriented).unwrap_or_default()
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

    /// Subtask 5 (GO-note 2): the CCW unit square's integer shoelace sums to
    /// `+2` — pinning the sign convention this grid's x-east/y-north
    /// handedness implies.
    #[test]
    fn shoelace_ccw_unit_square_is_positive_two() {
        let square = vec![
            Point::new(0, 0),
            Point::new(1, 0),
            Point::new(1, 1),
            Point::new(0, 1),
        ];
        assert_eq!(shoelace(&square), 2);
    }

    /// Subtask 5 (GO-note 2): the reversed (CW) square sums to `-2` — pinning
    /// the mapping in both directions, not merely "the two are reversed".
    #[test]
    fn shoelace_cw_unit_square_is_negative_two() {
        let mut square = vec![
            Point::new(0, 0),
            Point::new(1, 0),
            Point::new(1, 1),
            Point::new(0, 1),
        ];
        square.reverse();
        assert_eq!(shoelace(&square), -2);
    }

    /// Subtask 5: `orient` reverses (or not) to make the shoelace sign match
    /// `race_dir` (`Ccw` ⇔ `> 0`, `Cw` ⇔ `< 0`).
    #[test]
    fn orient_matches_race_dir_sign() {
        let ccw_square = vec![
            Point::new(0, 0),
            Point::new(1, 0),
            Point::new(1, 1),
            Point::new(0, 1),
        ];
        assert!(shoelace(&orient(ccw_square.clone(), RaceDir::Ccw)) > 0);
        assert!(shoelace(&orient(ccw_square, RaceDir::Cw)) < 0);
    }

    /// The `small_ring` fixture (subtask 3) as a real `Corridor` — a clean,
    /// already-width-1 4×4 border loop, so its true `medial_axis` is exactly
    /// `small_ring` itself (no bridging/pruning needed).
    fn small_ring_corridor() -> Corridor {
        let cells: Vec<(i32, i32)> = small_ring().into_iter().map(|p| (p.x, p.y)).collect();
        crate::testfix::corridor((0, 0), 4, 4, &cells)
    }

    /// The float shoelace signed area of `cl`'s resampled sample polygon —
    /// mirrors [`shoelace`] but over the `f32` sample positions, to check
    /// that orientation survives resampling.
    fn sample_signed_area(cl: &Centerline) -> f64 {
        cl.samples
            .iter()
            .zip(cl.samples.iter().skip(1).chain(cl.samples.iter().take(1)))
            .map(|(a, b)| {
                f64::from(b.pos.0)
                    .mul_add(-f64::from(a.pos.1), f64::from(a.pos.0) * f64::from(b.pos.1))
            })
            .sum()
    }

    /// Subtask 5 (end-to-end): `racing_line` on a clean ring produces
    /// `samples[0].s == 0`, strictly increasing `s` at ~`RESAMPLE_STEP`
    /// spacing, unit tangents, and an overall sample-polygon orientation
    /// matching `race_dir`.
    #[test]
    fn racing_line_orients_resamples_and_tangents_a_clean_ring() {
        let d = small_ring_corridor();
        let gate = small_ring_gate();

        for race_dir in [RaceDir::Ccw, RaceDir::Cw] {
            let cl = racing_line(&d, &gate, race_dir);
            assert!(!cl.samples.is_empty(), "{race_dir:?} must produce samples");
            assert!(cl.samples[0].s.abs() < f32::EPSILON);
            for w in cl.samples.windows(2) {
                assert!(w[1].s > w[0].s, "s must be strictly increasing");
                assert!(
                    (w[1].s - w[0].s - RESAMPLE_STEP).abs() < 0.5,
                    "spacing must be close to RESAMPLE_STEP"
                );
            }
            for sample in &cl.samples {
                let mag = sample.tangent.0.hypot(sample.tangent.1);
                assert!((mag - 1.0).abs() < 1e-4, "tangent must be unit-length");
            }

            let area = sample_signed_area(&cl);
            match race_dir {
                RaceDir::Ccw => assert!(area > 0.0, "Ccw sample polygon must wind positive"),
                RaceDir::Cw => assert!(area < 0.0, "Cw sample polygon must wind negative"),
            }
        }
    }
}
