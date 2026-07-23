//! Ф5b — full Vmax passability oracle (iterative deepening) + speed metrics +
//! `break_points` (design doc §2/§3, spec
//! `ai-docs/plans/2026-07-23-gp-gen-phase5b-full-oracle.spec.md`,
//! design `ai-docs/plans/2026-07-23-gp-gen-phase5b-full-oracle.design.md`).
//!
//! Composes the Ф5a substrate ([`crate::forward_reachable`] /
//! [`crate::backward_reachable`] / `within_v_ceil`, `phase5.rs`) into
//! [`phase5_full_oracle`] — an iterative-deepening driver that never
//! reimplements the flood edge (core's `legal_move`) or the crossing test
//! (core's `LapCounter::register_move`) (design § Approach; AC5).

use std::collections::HashSet;

use gp_core::geom::{Corridor, Point};
use gp_core::sim::{Action, CarState, LapCounter, legal_move, step};
use gp_core::track::{StartFinish, TrackMetrics};
use strum::IntoEnumIterator;

use crate::phase5::within_v_ceil;

/// The result of running [`phase5_full_oracle`] (design § Approach (3)).
///
/// `Lappable` populates the existing [`TrackMetrics`] fields (the exported
/// artifact contract, `gp-core::track`); `NotLappable` carries the raw
/// reachability-stall diagnostic `break_points` — a gen-internal input to
/// Ф6's `map_frontier_gap_to_edge` (out of scope here), not part of the
/// exported contract type.
#[derive(Clone, Debug)]
pub enum OracleResult {
    /// A closed lap exists; carries the populated speed metrics.
    Lappable(TrackMetrics),
    /// No closed lap exists in `live`; carries the frontier-gap diagnostic
    /// (design `[N3]`) — non-empty by the generator-guarantee dependency
    /// (design § Approach (3), AC3).
    NotLappable {
        /// The raw reachability-stall frontier between `R` and the
        /// lap-close goal.
        break_points: Vec<Point>,
    },
}

/// Whether the move `from → to` is a **forward** crossing of `sf`'s
/// start/finish gate (design § Approach (1)) — reuses core's
/// [`LapCounter::register_move`] directly (one crossing code path, AC5),
/// rather than reimplementing the sign/span test.
///
/// A scratch [`LapCounter`] is used purely as a crossing-sign detector: its
/// `raw()` increases by exactly `1` on a forward crossing, decreases by `1`
/// on a reverse crossing, and is unchanged otherwise (`register_move`'s
/// documented at-most-one-event contract) — so comparing `raw()` before and
/// after pins the answer without depending on the counter's absolute value.
#[allow(
    dead_code,
    reason = "not yet wired into phase5_full_oracle (subtask 6); exercised \
              directly by this subtask's own tests in the interim"
)]
pub(crate) fn crosses_sf_forward(sf: &StartFinish, from: Point, to: Point) -> bool {
    let mut counter = LapCounter::new();
    let before = counter.raw();
    counter.register_move(sf, from, to);
    counter.raw() > before
}

/// Enumerates the lap-close goal states reachable in one legal move from
/// `r` (design § Approach (1)): for each `s ∈ r` and each `a ∈
/// Action::iter()`, if `legal_move(d, s, a)` holds and the swept move `s.pos()
/// → step(s, a).pos()` is a forward S/F crossing ([`crosses_sf_forward`]),
/// the successor `step(s, a)` is a goal — bounded to the same `v_ceil` L∞
/// box the floods enforce ([`within_v_ceil`]).
///
/// May contain duplicate states (multiple `(s, a)` pairs can land on the
/// same successor) — harmless, since [`crate::backward_reachable`] (the
/// sole consumer) de-duplicates via its own visited set.
#[allow(
    dead_code,
    reason = "not yet wired into phase5_full_oracle (subtask 6); exercised \
              directly by this subtask's own tests in the interim"
)]
pub(crate) fn lap_close_goals(
    d: &Corridor,
    sf: &StartFinish,
    r: &HashSet<CarState>,
    v_ceil: i32,
) -> Vec<CarState> {
    let mut goals = Vec::new();
    for &s in r {
        for a in Action::iter() {
            if !legal_move(d, s, a) {
                continue;
            }
            let s2 = step(s, a);
            if within_v_ceil(s2, v_ceil) && crosses_sf_forward(sf, s.pos(), s2.pos()) {
                goals.push(s2);
            }
        }
    }
    goals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testfix::*;

    #[test]
    fn oracle_result_variants_are_constructible_and_clonable() {
        // Subtask-1 compile-smoke test: both variants build, derive
        // Clone/Debug, and match as expected. `OracleResult` deliberately
        // does not derive `PartialEq` (its `TrackMetrics` payload doesn't
        // either, and adding it there is a gp-core change, out of scope) —
        // callers compare field-wise instead (design § Approach (3)).
        let lappable = OracleResult::Lappable(TrackMetrics::default());
        let lappable2 = lappable.clone();
        match (lappable, lappable2) {
            (OracleResult::Lappable(a), OracleResult::Lappable(b)) => {
                assert_eq!(a.vmax_attain, b.vmax_attain);
            }
            _ => panic!("expected both to be Lappable"),
        }

        let not_lappable = OracleResult::NotLappable {
            break_points: vec![Point::new(1, 2)],
        };
        let not_lappable2 = not_lappable.clone();
        match (not_lappable, not_lappable2) {
            (
                OracleResult::NotLappable { break_points: a },
                OracleResult::NotLappable { break_points: b },
            ) => assert_eq!(a, b),
            _ => panic!("expected both to be NotLappable"),
        }
    }

    // ---- crosses_sf_forward / lap_close_goals (subtask 3) ----

    #[test]
    fn crosses_sf_forward_true_for_the_gate_crossing() {
        let sf = ring_sf(); // behind = [(2, 0)], forward = East -> ahead (3, 0)
        assert!(crosses_sf_forward(&sf, Point::new(2, 0), Point::new(3, 0)));
    }

    #[test]
    fn crosses_sf_forward_false_for_the_reverse_crossing() {
        let sf = ring_sf();
        assert!(!crosses_sf_forward(&sf, Point::new(3, 0), Point::new(2, 0)));
    }

    #[test]
    fn crosses_sf_forward_false_for_an_off_gate_move() {
        let sf = ring_sf();
        // Both endpoints stay on the same (ahead) side -- no crossing.
        assert!(!crosses_sf_forward(&sf, Point::new(3, 0), Point::new(4, 0)));
    }

    #[test]
    fn crosses_sf_forward_ac5_pin_matches_direct_register_move() {
        // AC5: the oracle's crossing decision must agree with a direct
        // LapCounter::register_move call on the same from -> to -- the
        // shared core crossing path (a forward crossing takes the fresh -1
        // counter to 0).
        let sf = ring_sf();
        let (from, to) = (Point::new(2, 0), Point::new(3, 0));

        let mut counter = LapCounter::new();
        counter.register_move(&sf, from, to);
        assert_eq!(counter.raw(), 0);

        assert_eq!(crosses_sf_forward(&sf, from, to), counter.raw() == 0);
    }

    #[test]
    fn lap_close_goals_yields_the_post_crossing_state_within_the_box() {
        let d = ring_corridor();
        let sf = ring_sf();
        let seed = car(2, 0, 0, 0);
        let r: HashSet<CarState> = std::iter::once(seed).collect();

        let goals = lap_close_goals(&d, &sf, &r, 1);

        // East from the seed crosses the gate and lands at (3, 0), v=(1, 0).
        assert!(goals.contains(&car(3, 0, 1, 0)));
        // Every goal stays within the v_ceil box.
        assert!(goals.iter().all(|s| within_v_ceil(*s, 1)));
    }

    #[test]
    fn lap_close_goals_is_empty_when_no_state_in_r_crosses_the_gate() {
        let d = ring_corridor();
        let sf = ring_sf();
        // A state already past the gate, moving further away from it: no
        // move from this state re-crosses the gate forward.
        let r: HashSet<CarState> = std::iter::once(car(3, 0, 1, 0)).collect();

        let goals = lap_close_goals(&d, &sf, &r, 1);
        assert!(goals.is_empty());
    }
}
