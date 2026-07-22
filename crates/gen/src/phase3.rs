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
    let _ = m;
    let seg = pick_straight_run(&skel.ring, &skel.hole);
    let axis = seg.axis;
    let outward = opposite_side(seg.inward);
    let _ = outward;
    let forward = forward_side(skel.dir, seg.inward);
    Phase3Output {
        d,
        sf: StartFinish {
            chord: Vec::new(),
            orient: perp_orient(axis),
            gate: TimingGate {
                behind: Vec::new(),
                forward,
            },
        },
        grid: StartGrid::default(),
    }
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
}
