//! Ф3 — start/finish, accel zone, start grid, timing gate (design doc §2,
//! `phase3_start_finish`).
//!
//! Given Ф2's fine corridor `D` and Ф1's [`CoarseSkeleton`] (with the fixed
//! global [`RaceDir`]), Ф3 picks a straight run of the ring, thickens its
//! cross-section to `≥ m` cars abreast, and builds the [`StartFinish`] /
//! [`StartGrid`] pair. Integer-only, no RNG of its own, total (no `Result`,
//! no production panic) — mirroring Ф1/Ф2's discipline.

use gp_core::geom::{Corridor, Orient, Side};
use gp_core::track::{RaceDir, StartFinish, StartGrid, TimingGate};

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
    // Placeholder segment selection (subtask 1 scaffold) — `pick_straight_run`
    // (subtask 2) replaces this with the actual coarse-ring straight search.
    let inward = Side::North;
    let axis = axis_for_inward(inward);
    let outward = opposite_side(inward);
    let _ = outward;
    let forward = forward_side(skel.dir, inward);
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
}
