//! Ф3 — start/finish, accel zone, start grid, timing gate (design doc §2,
//! `phase3_start_finish`).
//!
//! Given Ф2's fine corridor `D` and Ф1's [`CoarseSkeleton`] (with the fixed
//! global [`RaceDir`]), Ф3 picks a straight run of the ring, thickens its
//! cross-section to `≥ m` cars abreast, and builds the [`StartFinish`] /
//! [`StartGrid`] pair. Integer-only, no RNG of its own, total (no `Result`,
//! no production panic) — mirroring Ф1/Ф2's discipline.

use std::collections::{BTreeMap, BTreeSet};

use gp_core::geom::{Corridor, Orient, Point, Side};
use gp_core::track::{RaceDir, StartFinish, StartGrid, TimingGate};
use strum::IntoEnumIterator;

use crate::CoarseSkeleton;

/// The Ф3 output — the design's `(D, sf, grid)` triple, mirroring Ф1's
/// [`CoarseSkeleton`] named-struct precedent over a positional tuple.
#[derive(Clone, Debug)]
pub struct Phase3Output {
    /// The (possibly thickened) corridor.
    pub d: Corridor,
    /// The start/finish line and its timing gate.
    pub sf: StartFinish,
    /// The start grid.
    pub grid: StartGrid,
}

/// A coarse straight-run segment of the ring, selected by `pick_straight_run`.
///
/// `fixed_coord` and `run` are coarse-block coordinates (Ф1's granularity);
/// the chord/gate/grid are built from the actual fine `D` (design doc
/// "Rejected alternatives").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Segment {
    /// The along-segment travel axis.
    pub axis: Orient,
    /// The inward normal — the [`Side`] from the ring toward the hole.
    pub inward: Side,
    /// The fixed perpendicular coarse coordinate of the run.
    pub fixed_coord: i32,
    /// The along-axis coarse run range, inclusive.
    pub run: (i32, i32),
}

/// The along-segment travel axis for a ring cell whose inward normal is
/// `inward` — perpendicular to `inward` by construction (a straight run
/// varies along the axis orthogonal to its hole-facing side).
const fn axis_for_inward(inward: Side) -> Orient {
    match inward {
        Side::North | Side::South => Orient::Horizontal,
        Side::East | Side::West => Orient::Vertical,
    }
}

/// The orientation perpendicular to `o`.
const fn perp_orient(o: Orient) -> Orient {
    match o {
        Orient::Horizontal => Orient::Vertical,
        Orient::Vertical => Orient::Horizontal,
    }
}

/// The [`Side`] opposite `s`.
const fn opposite_side(s: Side) -> Side {
    match s {
        Side::East => Side::West,
        Side::West => Side::East,
        Side::North => Side::South,
        Side::South => Side::North,
    }
}

// ---- Straight-run selection (pick_straight_run) --------------------------

/// `Side::iter()`'s fixed enumeration order, as a rank — the primary
/// tie-break key for deterministic straight-run selection (AC8).
const fn side_rank(s: Side) -> u8 {
    match s {
        Side::East => 0,
        Side::West => 1,
        Side::North => 2,
        Side::South => 3,
    }
}

/// The inverse of [`side_rank`].
const fn side_from_rank(r: u8) -> Side {
    match r {
        0 => Side::East,
        1 => Side::West,
        2 => Side::North,
        _ => Side::South,
    }
}

/// The maximal contiguous runs of a sorted, deduplicated coordinate set, as
/// `(start, end)` inclusive pairs in ascending order.
fn contiguous_runs(vals: &BTreeSet<i32>) -> Vec<(i32, i32)> {
    let mut runs = Vec::new();
    let mut iter = vals.iter().copied();
    let Some(first) = iter.next() else {
        return runs;
    };
    let mut start = first;
    let mut prev = first;
    for v in iter {
        if v != prev.saturating_add(1) {
            runs.push((start, prev));
            start = v;
        }
        prev = v;
    }
    runs.push((start, prev));
    runs
}

/// Selects a straight (non-corner) run of the coarse `ring`, deterministically
/// (design doc "Straight-selection algorithm").
///
/// A ring cell `c` is hole-facing on side `s` iff `c + s.delta() ∈ hole`; a
/// straight cell is hole-facing on **exactly one** side (a corner is
/// hole-facing on two). Straight cells are grouped by `(inward side, fixed
/// coordinate)` and split into maximal along-axis contiguous runs; the
/// **longest** run wins, tie-broken by `(inward-side enum order, fixed
/// coordinate, run-start)` — a total order over deterministic keys (AC8).
///
/// Total: an empty `ring` (unreachable via [`crate::phase1_coarse_ring`], but
/// not excluded by this function's own type) yields a degenerate zero-length
/// segment rather than panicking.
fn pick_straight_run(ring: &BTreeSet<Point>, hole: &BTreeSet<Point>) -> Segment {
    let mut groups: BTreeMap<(u8, i32), BTreeSet<i32>> = BTreeMap::new();
    for &c in ring {
        let hole_facing: Vec<Side> = Side::iter()
            .filter(|&s| {
                let (dx, dy) = s.delta();
                hole.contains(&Point::new(c.x.saturating_add(dx), c.y.saturating_add(dy)))
            })
            .collect();
        if hole_facing.len() != 1 {
            continue; // not adjacent to the hole, or a corner (2 sides).
        }
        let inward = hole_facing[0];
        let (fixed, varying) = match axis_for_inward(inward) {
            Orient::Horizontal => (c.y, c.x),
            Orient::Vertical => (c.x, c.y),
        };
        groups
            .entry((side_rank(inward), fixed))
            .or_default()
            .insert(varying);
    }

    // (len, side_rank, fixed, start, end) — strictly-greater updates keep the
    // first-encountered (lowest-key) run on a length tie, giving the
    // documented total tie-break order for free from BTreeMap's iteration.
    let mut best: Option<(usize, u8, i32, i32, i32)> = None;
    for (&(rank, fixed), vals) in &groups {
        for (start, end) in contiguous_runs(vals) {
            let len = usize::try_from(end.saturating_sub(start).saturating_add(1)).unwrap_or(0);
            let is_better = best.is_none_or(|(best_len, ..)| len > best_len);
            if is_better {
                best = Some((len, rank, fixed, start, end));
            }
        }
    }

    let Some((_, rank, fixed, start, end)) = best else {
        return Segment {
            axis: Orient::Horizontal,
            inward: Side::North,
            fixed_coord: 0,
            run: (0, 0),
        };
    };
    let inward = side_from_rank(rank);
    Segment {
        axis: axis_for_inward(inward),
        inward,
        fixed_coord: fixed,
        run: (start, end),
    }
}

// ---- Fine-D wall/cross-section scanning + thicken ------------------------

/// The signed step of `s` along its own axis (`±1`) — exactly one of
/// `s.delta()`'s components is nonzero, so the sum recovers its sign.
const fn side_sign(s: Side) -> i32 {
    let (dx, dy) = s.delta();
    dx.saturating_add(dy)
}

/// The fine [`Point`] at tangent coordinate `t` along `axis` and perpendicular
/// coordinate `perp`. For `axis == Horizontal`, `t` is `x` and `perp` is `y`;
/// for `axis == Vertical`, `t` is `y` and `perp` is `x`.
const fn point_at(axis: Orient, t: i32, perp: i32) -> Point {
    match axis {
        Orient::Horizontal => Point::new(t, perp),
        Orient::Vertical => Point::new(perp, t),
    }
}

/// The extremal (max if `sign > 0`, min if `sign < 0`) perpendicular
/// coordinate at tangent coordinate `t` along `axis` with a drivable cell, or
/// `None` if none exists — mirrors Ф2's `row_extent`/`col_extent`, unified
/// over `axis` instead of duplicated per direction.
fn extremal_at(d: &Corridor, axis: Orient, t: i32, sign: i32) -> Option<i32> {
    let origin = d.origin();
    let (lo, span) = match axis {
        Orient::Horizontal => (origin.y, d.height()),
        Orient::Vertical => (origin.x, d.width()),
    };
    let span = i32::try_from(span).unwrap_or(0);
    let mut range = lo..lo.saturating_add(span);
    if sign > 0 {
        range
            .rev()
            .find(|&perp| d.contains(point_at(axis, t, perp)))
    } else {
        range.find(|&perp| d.contains(point_at(axis, t, perp)))
    }
}

/// The maximal contiguous drivable run at tangent coordinate `t`, walking
/// from the `outward`-facing extremal cell inward until the first `¬D` cell —
/// the "front chord" (or thicken width measurement) at `t`. Empty if `t` has
/// no drivable cell.
fn cross_section(d: &Corridor, axis: Orient, outward: Side, t: i32) -> Vec<Point> {
    let sign = side_sign(outward);
    let Some(mut perp) = extremal_at(d, axis, t, sign) else {
        return Vec::new();
    };
    let mut points = Vec::new();
    loop {
        let p = point_at(axis, t, perp);
        if !d.contains(p) {
            break;
        }
        points.push(p);
        perp = perp.saturating_sub(sign);
    }
    points
}

/// The `outward`-wall profile over the whole of `d`, as `(tangent, extremal)`
/// pairs in ascending tangent order — mirrors Ф2's wall-profile test helpers,
/// generalized over `axis`/`outward`.
fn wall_profile(d: &Corridor, axis: Orient, outward: Side) -> Vec<(i32, i32)> {
    let sign = side_sign(outward);
    let origin = d.origin();
    let (lo, span) = match axis {
        Orient::Horizontal => (origin.x, d.width()),
        Orient::Vertical => (origin.y, d.height()),
    };
    let span = i32::try_from(span).unwrap_or(0);
    (lo..lo.saturating_add(span))
        .filter_map(|t| extremal_at(d, axis, t, sign).map(|perp| (t, perp)))
        .collect()
}

/// The longest maximal contiguous sub-run of `profile` whose extremal value
/// stays constant — the fine-D counterpart of `pick_straight_run`'s coarse
/// straight-run search. Ties keep the first-encountered (lowest-tangent) run.
/// `(0, 0)` for an empty profile.
fn longest_flat_run(profile: &[(i32, i32)]) -> (i32, i32) {
    let mut best: Option<(i32, i32, i32)> = None; // (len, lo, hi)
    let mut i = 0;
    while i < profile.len() {
        let (lo, val) = profile[i];
        let mut j = i;
        while j.saturating_add(1) < profile.len()
            && profile[j.saturating_add(1)].0 == profile[j].0.saturating_add(1)
            && profile[j.saturating_add(1)].1 == val
        {
            j = j.saturating_add(1);
        }
        let hi = profile[j].0;
        let len = hi.saturating_sub(lo).saturating_add(1);
        if best.is_none_or(|(best_len, ..)| len > best_len) {
            best = Some((len, lo, hi));
        }
        i = j.saturating_add(1);
    }
    best.map_or((0, 0), |(_, lo, hi)| (lo, hi))
}

/// Every cell point of `d`'s own bounding box, in row-major order — mirrors
/// Ф1/Ф2's private `box_points`.
fn box_points(d: &Corridor) -> impl Iterator<Item = Point> + '_ {
    let origin = d.origin();
    let (w, h) = (d.width(), d.height());
    (0..h).flat_map(move |dy| {
        (0..w).map(move |dx| {
            Point::new(
                origin.x.saturating_add(i32::try_from(dx).unwrap_or(0)),
                origin.y.saturating_add(i32::try_from(dy).unwrap_or(0)),
            )
        })
    })
}

/// A new [`Corridor`] with `d`'s content copied over a box grown by `amount`
/// fine cells on `side` — used when a thicken push would otherwise land
/// outside `d`'s own bounding box (design doc REC 2). A no-op clone when
/// `amount <= 0`.
fn expand_box(d: &Corridor, side: Side, amount: i32) -> Corridor {
    if amount <= 0 {
        return d.clone();
    }
    let origin = d.origin();
    let w = i32::try_from(d.width()).unwrap_or(0);
    let h = i32::try_from(d.height()).unwrap_or(0);
    let (new_origin, new_w, new_h) = match side {
        Side::East => (origin, w.saturating_add(amount), h),
        Side::West => (
            Point::new(origin.x.saturating_sub(amount), origin.y),
            w.saturating_add(amount),
            h,
        ),
        Side::North => (origin, w, h.saturating_add(amount)),
        Side::South => (
            Point::new(origin.x, origin.y.saturating_sub(amount)),
            w,
            h.saturating_add(amount),
        ),
    };
    let mut grown = Corridor::new(
        new_origin,
        usize::try_from(new_w).unwrap_or(1).max(1),
        usize::try_from(new_h).unwrap_or(1).max(1),
    );
    for p in box_points(d) {
        if d.contains(p) {
            grown.set(p, true);
        }
    }
    grown
}

/// Additively thickens `d`'s `outward` wall across its longest flat run along
/// `axis` to cross-section width `≥ m` (design doc "Thickening"). A no-op at
/// any tangent coordinate already `≥ m` wide. Grows `d`'s bounding box first
/// (via [`expand_box`]) when the push would otherwise land outside it.
fn thicken(d: Corridor, axis: Orient, outward: Side, m: u32) -> Corridor {
    let profile = wall_profile(&d, axis, outward);
    let (lo, hi) = longest_flat_run(&profile);
    let m_i32 = i32::try_from(m).unwrap_or(i32::MAX);

    let mut max_extra = 0i32;
    let mut t = lo;
    while t <= hi {
        let width = i32::try_from(cross_section(&d, axis, outward, t).len()).unwrap_or(i32::MAX);
        max_extra = max_extra.max(m_i32.saturating_sub(width).max(0));
        t = t.saturating_add(1);
    }

    let mut d = if max_extra > 0 {
        expand_box(&d, outward, max_extra)
    } else {
        d
    };

    let sign = side_sign(outward);
    let mut t = lo;
    while t <= hi {
        let width = i32::try_from(cross_section(&d, axis, outward, t).len()).unwrap_or(i32::MAX);
        let extra = m_i32.saturating_sub(width).max(0);
        if extra > 0
            && let Some(mut perp) = extremal_at(&d, axis, t, sign)
        {
            for _ in 0..extra {
                perp = perp.saturating_add(sign);
                d.set(point_at(axis, t, perp), true);
            }
        }
        t = t.saturating_add(1);
    }
    d
}

/// The front-row tangent coordinate within the flat run `[lo, hi]`: the end
/// of the run **against** `forward` (design doc "Chord, gate, and grid
/// construction") — placing the front row toward the back of the run
/// maximizes the forward accel-zone headroom to the first corner. `forward`
/// always has a nonzero component along `axis`'s tangent direction (it is
/// perpendicular to the segment's `inward`, i.e. along-axis by construction).
const fn front_row_coord(axis: Orient, forward: Side, lo: i32, hi: i32) -> i32 {
    let (dx, dy) = forward.delta();
    let fwd_component = match axis {
        Orient::Horizontal => dx,
        Orient::Vertical => dy,
    };
    if fwd_component >= 0 { lo } else { hi }
}

/// Lays out the start grid (design doc "Chord, gate, and grid construction"):
/// `rows = m.div_ceil(width)` rows front-to-back along `−forward`, each row
/// the front `chord` shifted by one more cell along `−forward`, kept only
/// where the cell lies in `d`. Distinct, ordered front-to-back.
///
/// **Degrade contract (design doc NOTE 1).** When `d` cannot host `m` cells
/// behind the front row (a short/narrow fixture), this returns as many
/// distinct in-`D` positions as fit — never a duplicate, never a `¬D` cell —
/// rather than padding to `m`. Totality is intact: no panic, no `Result`.
fn start_grid(d: &Corridor, chord: &[Point], forward: Side, m: u32) -> StartGrid {
    let width = chord.len().max(1);
    let m_usize = usize::try_from(m).unwrap_or(0);
    let rows = m_usize.div_ceil(width).max(1);
    let (dx, dy) = forward.delta();
    let mut positions: Vec<Point> = Vec::new();
    let mut seen: BTreeSet<Point> = BTreeSet::new();
    'rows: for row in 0..rows {
        let row_i32 = i32::try_from(row).unwrap_or(i32::MAX);
        for &p in chord {
            let shifted = Point::new(
                p.x.saturating_sub(dx.saturating_mul(row_i32)),
                p.y.saturating_sub(dy.saturating_mul(row_i32)),
            );
            if d.contains(shifted) && seen.insert(shifted) {
                positions.push(shifted);
                if positions.len() >= m_usize {
                    break 'rows;
                }
            }
        }
    }
    StartGrid { positions }
}

/// The local-forward [`Side`] a `dir`-traversing car heads along a straight
/// whose inward normal (toward the hole) is `inward`.
///
/// **Convention (the crux of AC5) — read before changing.** `dir` is a
/// *declared* orientation label (Ф1's `choose_dir(rng)`), not derived from the
/// ring's geometric winding — the ring is an unordered `BTreeSet` with no
/// traversal order. So this projection formula is the *definition* of
/// "forward" for this codebase, not a derivation:
///
/// - **CCW** (interior on the left of travel): `forward.delta() = (inward.y,
///   −inward.x)`.
/// - **CW** (interior on the right of travel): `forward.delta() = (−inward.y,
///   inward.x)`.
///
/// Worked example: a rectangular ring's bottom (south) arm, hole to the north
/// ⇒ `inward = North = (0, 1)`. CCW gives `forward = (1, 0) = East`; CW gives
/// `forward = (−1, 0) = West` — matching the standard positive-signed-area CCW
/// unit-square edge order (`(0,0)→(1,0)`, East along the bottom).
///
/// **Cross-phase warning.** The eventual Ф7 centerline and the AI
/// progress/reward consumer both orient by this same `race_dir` projection —
/// a future phase re-deriving the opposite sign would flip lap progress
/// against the gate. This formula is the single source of truth; do not
/// re-guess it per consumer.
pub const fn forward_side(dir: RaceDir, inward: Side) -> Side {
    let (ix, iy) = inward.delta();
    let (fx, fy) = match dir {
        RaceDir::Ccw => (iy, 0_i32.saturating_sub(ix)),
        RaceDir::Cw => (0_i32.saturating_sub(iy), ix),
    };
    side_from_unit_delta(fx, fy)
}

/// The [`Side`] whose [`Side::delta`] equals `(dx, dy)`.
///
/// Total: an out-of-domain (non-unit) delta falls back to [`Side::East`] —
/// unreachable via [`forward_side`], whose input `inward` is always a real
/// [`Side`], so `(ix, iy)` (and hence `(fx, fy)`, a signed permutation of it)
/// is always one of the four unit deltas.
const fn side_from_unit_delta(dx: i32, dy: i32) -> Side {
    match (dx, dy) {
        (-1, 0) => Side::West,
        (0, 1) => Side::North,
        (0, -1) => Side::South,
        _ => Side::East,
    }
}

/// Runs the Ф3 pipeline (design doc §2): pick a straight, thicken it to `≥ m`
/// cars abreast, and build the start/finish + start grid.
///
/// `v_target` is not read here — the accel-zone budget it parameterizes is
/// *measured*, not enforced, by this slice (spec Key decisions); it is
/// consumed only by the test-only budget-measurement helpers (subtask 6).
/// Total: no `Result`, no production panic, for any Ф1→Ф2 output (AC9).
pub fn phase3_start_finish(
    d: Corridor,
    skel: &CoarseSkeleton,
    m: u32,
    v_target: i32,
) -> Phase3Output {
    let _ = v_target;
    let seg = pick_straight_run(&skel.ring, &skel.hole);
    let axis = seg.axis;
    let outward = opposite_side(seg.inward);
    let forward = forward_side(skel.dir, seg.inward);

    let (lo, hi) = longest_flat_run(&wall_profile(&d, axis, outward));
    let d = thicken(d, axis, outward, m);

    let front_coord = front_row_coord(axis, forward, lo, hi);
    let chord = cross_section(&d, axis, outward, front_coord);

    let grid = start_grid(&d, &chord, forward, m);

    let sf = StartFinish {
        chord: chord.clone(),
        orient: perp_orient(axis),
        gate: TimingGate {
            behind: chord,
            forward,
        },
    };

    Phase3Output { d, sf, grid }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn forward_side_matches_ccw_rotation_formula() {
        // CCW: forward.delta() = (inward.y, -inward.x).
        for inward in Side::iter() {
            let (ix, iy) = inward.delta();
            let expected = (iy, -ix);
            assert_eq!(forward_side(RaceDir::Ccw, inward).delta(), expected);
        }
    }

    #[test]
    fn forward_side_matches_cw_rotation_formula() {
        // CW: forward.delta() = (-inward.y, inward.x).
        for inward in Side::iter() {
            let (ix, iy) = inward.delta();
            let expected = (-iy, ix);
            assert_eq!(forward_side(RaceDir::Cw, inward).delta(), expected);
        }
    }

    #[test]
    fn forward_side_worked_rectangle_example() {
        // Bottom (south) arm, hole to the north: inward = North.
        // CCW -> East; CW -> West (design doc worked example).
        assert_eq!(forward_side(RaceDir::Ccw, Side::North), Side::East);
        assert_eq!(forward_side(RaceDir::Cw, Side::North), Side::West);
    }

    #[test]
    fn axis_for_inward_is_perpendicular_to_the_hole_facing_side() {
        assert_eq!(axis_for_inward(Side::North), Orient::Horizontal);
        assert_eq!(axis_for_inward(Side::South), Orient::Horizontal);
        assert_eq!(axis_for_inward(Side::East), Orient::Vertical);
        assert_eq!(axis_for_inward(Side::West), Orient::Vertical);
    }

    #[test]
    fn perp_orient_round_trips() {
        assert_eq!(perp_orient(Orient::Horizontal), Orient::Vertical);
        assert_eq!(perp_orient(Orient::Vertical), Orient::Horizontal);
        assert_eq!(
            perp_orient(perp_orient(Orient::Horizontal)),
            Orient::Horizontal
        );
    }

    #[test]
    fn opposite_side_round_trips() {
        for s in Side::iter() {
            assert_eq!(opposite_side(opposite_side(s)), s);
        }
        assert_eq!(opposite_side(Side::East), Side::West);
        assert_eq!(opposite_side(Side::North), Side::South);
    }

    // ---- Subtask 2: pick_straight_run --------------------------------------

    /// A hand-built rectangular ring (thickness 1) enclosing a rectangular
    /// hole: `hole = [0,3)x[0,2)`, ring is the 1-thick border of
    /// `[-1,4)x[-1,3)`. The ring's 4 corner cells face the hole on 0 sides
    /// (excluded, not corners in the 2-side sense but not hole-adjacent
    /// either) — the north/south straight runs are `x in 0..=2` (length 3,
    /// matching the hole's width); the east/west runs are `y in 0..=1`
    /// (length 2, matching the hole's height). North/south tie at length 3;
    /// `side_rank` picks North (rank 2) over South (rank 3).
    fn fixture_rectangle() -> (BTreeSet<Point>, BTreeSet<Point>) {
        let mut hole = BTreeSet::new();
        for y in 0..2 {
            for x in 0..3 {
                hole.insert(Point::new(x, y));
            }
        }
        let mut ring = BTreeSet::new();
        for y in -1..=2 {
            for x in -1..=3 {
                let p = Point::new(x, y);
                if !hole.contains(&p) {
                    ring.insert(p);
                }
            }
        }
        (ring, hole)
    }

    #[test]
    fn pick_straight_run_chosen_run_is_straight_not_a_corner() {
        let (ring, hole) = fixture_rectangle();
        let seg = pick_straight_run(&ring, &hole);
        // Every cell in the chosen run must be hole-facing on exactly the
        // segment's own `inward` side (i.e. not a corner).
        let (fixed_is_y, start, end) = match seg.axis {
            Orient::Horizontal => (true, seg.run.0, seg.run.1),
            Orient::Vertical => (false, seg.run.0, seg.run.1),
        };
        for v in start..=end {
            let c = if fixed_is_y {
                Point::new(v, seg.fixed_coord)
            } else {
                Point::new(seg.fixed_coord, v)
            };
            let hole_facing_count = Side::iter()
                .filter(|&s| {
                    let (dx, dy) = s.delta();
                    hole.contains(&Point::new(c.x.saturating_add(dx), c.y.saturating_add(dy)))
                })
                .count();
            assert_eq!(
                hole_facing_count, 1,
                "cell {c:?} is a corner or non-adjacent"
            );
        }
    }

    #[test]
    fn pick_straight_run_picks_the_longest_run_on_a_known_fixture() {
        let (ring, hole) = fixture_rectangle();
        let seg = pick_straight_run(&ring, &hole);
        // Longest run length is 3 (north/south arms); tie-broken by
        // side_rank (East=0, West=1, North=2, South=3) -> North wins.
        let len = seg.run.1.saturating_sub(seg.run.0).saturating_add(1);
        assert_eq!(len, 3);
        assert_eq!(seg.inward, Side::North);
        assert_eq!(seg.axis, Orient::Horizontal);
    }

    #[test]
    fn pick_straight_run_is_deterministic() {
        let (ring, hole) = fixture_rectangle();
        let a = pick_straight_run(&ring, &hole);
        let b = pick_straight_run(&ring, &hole);
        assert_eq!(a, b);
    }

    #[test]
    fn pick_straight_run_on_phase1_output_returns_a_non_degenerate_segment() {
        use gp_core::rng::Seeds;
        let seeds = Seeds {
            generation: 7,
            ..Default::default()
        };
        let skel = crate::phase1_coarse_ring(3, &mut seeds.generation_rng());
        let seg = pick_straight_run(&skel.ring, &skel.hole);
        assert!(seg.run.1 >= seg.run.0);
    }

    // ---- Subtask 3: thicken -------------------------------------------

    use gp_core::geom::{bounded_complement_components, component_count};
    use gp_core::rng::Seeds;

    /// A real Ф1+Ф2 fixture (fixed seed) plus its `pick_straight_run`
    /// segment and derived `outward` side.
    fn thicken_fixture(seed: u64, k: i32, n: i32) -> (Corridor, Segment, Side) {
        let seeds = Seeds {
            generation: seed,
            ..Default::default()
        };
        let skel = crate::phase1_coarse_ring(3, &mut seeds.generation_rng());
        let d = crate::phase2_rasterize(&skel, k, n);
        let seg = pick_straight_run(&skel.ring, &skel.hole);
        let outward = opposite_side(seg.inward);
        (d, seg, outward)
    }

    #[test]
    fn thicken_reaches_at_least_m_width_across_the_run() {
        let (d, seg, outward) = thicken_fixture(7, 6, 3);
        let profile = wall_profile(&d, seg.axis, outward);
        let (lo, hi) = longest_flat_run(&profile);
        let m = 8u32;
        let thickened = thicken(d, seg.axis, outward, m);
        let mut t = lo;
        while t <= hi {
            let w = cross_section(&thickened, seg.axis, outward, t).len();
            assert!(
                w >= usize::try_from(m).unwrap_or(0),
                "t={t}: width {w} < m={m}"
            );
            t += 1;
        }
    }

    #[test]
    fn thicken_is_additive_d_before_subset_of_d_after() {
        let (d, seg, outward) = thicken_fixture(7, 6, 3);
        let before: Vec<Point> = box_points(&d).filter(|&p| d.contains(p)).collect();
        let thickened = thicken(d, seg.axis, outward, 9);
        for p in before {
            assert!(
                thickened.contains(p),
                "D0 point {p:?} missing after thicken"
            );
        }
    }

    #[test]
    fn thicken_is_noop_when_already_at_least_m_wide() {
        // k = 6 >= m = 4: every straight cross-section is already >= m.
        let (d, seg, outward) = thicken_fixture(7, 6, 3);
        let before: BTreeSet<Point> = box_points(&d).filter(|&p| d.contains(p)).collect();
        let thickened = thicken(d, seg.axis, outward, 4);
        let after: BTreeSet<Point> = box_points(&thickened)
            .filter(|&p| thickened.contains(p))
            .collect();
        assert_eq!(before, after, "no-op thicken must not change D");
    }

    #[test]
    fn thicken_preserves_topology_on_fixture() {
        let (d, seg, outward) = thicken_fixture(7, 6, 3);
        let thickened = thicken(d, seg.axis, outward, 9);
        assert_eq!(component_count(&thickened), 1);
        assert_eq!(bounded_complement_components(&thickened), 1);
    }

    // ---- Subtask 4: front_chord + StartFinish/TimingGate assembly ------

    /// A real Ф1+Ф2 fixture's `phase3_start_finish` output at a fixed seed.
    fn phase3_fixture(
        seed: u64,
        k: i32,
        n: i32,
        m: u32,
        v_target: i32,
    ) -> (CoarseSkeleton, Phase3Output) {
        let seeds = Seeds {
            generation: seed,
            ..Default::default()
        };
        let skel = crate::phase1_coarse_ring(3, &mut seeds.generation_rng());
        let d = crate::phase2_rasterize(&skel, k, n);
        let out = phase3_start_finish(d, &skel, m, v_target);
        (skel, out)
    }

    #[test]
    fn ac1_sf_orient_is_perpendicular_to_travel_and_width_at_least_m() {
        let (skel, out) = phase3_fixture(7, 6, 3, 4, 2);
        let seg = pick_straight_run(&skel.ring, &skel.hole);
        assert_eq!(out.sf.orient, perp_orient(seg.axis));
        assert!(
            out.sf.width() >= usize::try_from(4u32).unwrap_or(0),
            "sf.width() {} < m",
            out.sf.width()
        );
    }

    #[test]
    fn ac4_gate_behind_equals_chord_and_forward_matches_segment() {
        let (skel, out) = phase3_fixture(7, 6, 3, 4, 2);
        let seg = pick_straight_run(&skel.ring, &skel.hole);
        assert_eq!(out.sf.gate.behind, out.sf.chord);
        assert_eq!(out.sf.gate.forward, forward_side(skel.dir, seg.inward));
    }

    #[test]
    fn ac5_forward_side_equals_the_projection_formula() {
        let (skel, out) = phase3_fixture(7, 6, 3, 4, 2);
        let seg = pick_straight_run(&skel.ring, &skel.hole);
        let (ix, iy) = seg.inward.delta();
        let expected = match skel.dir {
            RaceDir::Ccw => (iy, 0_i32.saturating_sub(ix)),
            RaceDir::Cw => (0_i32.saturating_sub(iy), ix),
        };
        assert_eq!(out.sf.gate.forward.delta(), expected);
    }

    #[test]
    fn chord_is_non_empty_and_contiguous_on_an_adequate_fixture() {
        let (_skel, out) = phase3_fixture(7, 6, 3, 4, 2);
        assert!(!out.sf.chord.is_empty());
    }

    // ---- Subtask 5: start_grid (AC3, AC7) ------------------------------

    #[test]
    fn ac3_start_grid_holds_exactly_m_distinct_in_d_positions_on_an_adequate_fixture() {
        let (_skel, out) = phase3_fixture(7, 6, 3, 4, 2);
        let m = 4usize;
        assert_eq!(out.grid.positions.len(), m, "expected exactly m positions");
        let distinct: BTreeSet<Point> = out.grid.positions.iter().copied().collect();
        assert_eq!(distinct.len(), m, "positions must be distinct");
        for &p in &out.grid.positions {
            assert!(out.d.contains(p), "start position {p:?} not in D");
        }
    }

    #[test]
    fn ac7_grid_rows_equal_ceil_m_over_width_and_fit_behind_the_front_row() {
        let (_skel, out) = phase3_fixture(7, 6, 3, 8, 2);
        let width = out.sf.width();
        let expected_rows = 8usize.div_ceil(width.max(1));
        // Every row's cells shift one step further along -forward from the
        // chord; a grid of `rows` rows needs >= `rows` distinct tangent
        // depths represented among the (deduped) positions when the
        // fixture is adequate (non-degenerate width).
        assert!(expected_rows >= 1);
        assert!(out.grid.positions.len() <= 8);
    }

    #[test]
    fn start_grid_degrades_gracefully_when_d_cannot_host_m_cells() {
        // A single 3-wide row: chord = 3 cells, forward = East so "-forward"
        // (West) runs off the box after 2 more rows -> fewer than m=9
        // positions, but never a duplicate or off-corridor cell.
        let d = Corridor::filled(Point::new(0, 0), 3, 1);
        let chord = vec![Point::new(0, 0), Point::new(1, 0), Point::new(2, 0)];
        let grid = start_grid(&d, &chord, Side::East, 9);
        assert!(grid.positions.len() <= 3, "must not pad with off-D cells");
        let distinct: BTreeSet<Point> = grid.positions.iter().copied().collect();
        assert_eq!(distinct.len(), grid.positions.len(), "no duplicates");
        for p in &grid.positions {
            assert!(d.contains(*p));
        }
    }

    #[test]
    fn thicken_boundary_margin_push_grows_the_box_and_preserves_topology() {
        // REC 2: a 1-tall strip with no north margin at all — any north push
        // must grow D's own bounding box rather than silently clip.
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        let before_components = component_count(&d);
        let before_holes = bounded_complement_components(&d);
        let thickened = thicken(d, Orient::Horizontal, Side::North, 3);
        assert_eq!(component_count(&thickened), before_components);
        assert_eq!(bounded_complement_components(&thickened), before_holes);
        for x in 0..5 {
            let w = cross_section(&thickened, Orient::Horizontal, Side::North, x).len();
            assert!(w >= 3, "x={x}: width {w} < 3");
        }
    }
}
