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

use std::collections::{HashMap, HashSet};

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

/// The L∞ (Chebyshev) speed norm `max(|vx|, |vy|)` of `s` (design §
/// Approach (4)/`tempo`; Key-decisions), matching Ф5a's `within_v_ceil` box
/// bound.
///
/// `const fn`, forced by `missing_const_for_fn` (nursery, deny): only a
/// **branchless** body is const-callable on stable — neither `Ord::max` nor
/// `try_from` is const-stable (E0658, rust-lang/rust#143874, verified by
/// compile) — see design § Risks. `saturating_abs` (not plain `i32::abs`)
/// keeps the body clear of `arithmetic_side_effects` (also deny): plain
/// `abs` overflows at `i32::MIN`, so this stays total even there.
#[allow(
    dead_code,
    reason = "not yet wired into phase5_full_oracle (subtask 6); exercised \
              directly by this subtask's own tests in the interim"
)]
pub(crate) const fn vnorm(s: CarState) -> i32 {
    let a = s.vx.saturating_abs();
    let b = s.vy.saturating_abs();
    if a >= b { a } else { b }
}

/// Per-corridor-point max [`vnorm`] over `live`'s states at that point
/// (design § Approach (3), `TrackMetrics::speed_heatmap`) — the "where's
/// fast/slow" diagnostic. Sorted ascending by [`Point`] (`x` then `y`) for
/// deterministic output (AC6) regardless of `live`'s `HashSet` iteration
/// order.
#[allow(
    dead_code,
    reason = "not yet wired into phase5_full_oracle (subtask 6); exercised \
              directly by this subtask's own tests in the interim"
)]
pub(crate) fn speed_heatmap(live: &HashSet<CarState>) -> Vec<(Point, i32)> {
    let mut peak: HashMap<Point, i32> = HashMap::new();
    for &s in live {
        peak.entry(s.pos())
            .and_modify(|v| *v = (*v).max(vnorm(s)))
            .or_insert_with(|| vnorm(s));
    }
    let mut out: Vec<(Point, i32)> = peak.into_iter().collect();
    out.sort_by_key(|&(p, _)| p);
    out
}

/// The raw reachability-stall frontier between `r` and the (unreached)
/// lap-close goal (design `[N3]`, § Approach (3)): drivable cells `∉
/// proj(r)` with a 4-neighbor `∈ proj(r)`, where `proj(r)` is the set of
/// cell positions any state in `r` occupies.
///
/// Mirrors `gp_core::geom::Rect::points`' saturating box-walk (rather than
/// adding a new public iterator to `Corridor`, a `gp-core` change out of
/// scope here — spec Key-decisions/Out-of-scope) so the scan stays total
/// even at extreme `origin`/`width`/`height`. Sorted by [`Point`] for
/// deterministic output (AC6).
#[allow(
    dead_code,
    reason = "not yet wired into phase5_full_oracle (subtask 6); exercised \
              directly by this subtask's own tests in the interim"
)]
pub(crate) fn frontier_gap(d: &Corridor, r: &HashSet<CarState>) -> Vec<Point> {
    let reached: HashSet<Point> = r.iter().map(|s| s.pos()).collect();
    let origin = d.origin();
    let x1 = i32::try_from(d.width()).map_or(i32::MAX, |w| origin.x.saturating_add(w));
    let y1 = i32::try_from(d.height()).map_or(i32::MAX, |h| origin.y.saturating_add(h));
    let mut frontier: Vec<Point> = (origin.y..y1)
        .flat_map(|y| (origin.x..x1).map(move |x| Point::new(x, y)))
        .filter(|&p| d.contains(p) && !reached.contains(&p))
        .filter(|&p| p.neighbors4().into_iter().any(|q| reached.contains(&q)))
        .collect();
    frontier.sort();
    frontier
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

    // ---- vnorm / speed_heatmap / frontier_gap (subtask 4) ----

    #[test]
    fn vnorm_is_the_l_infinity_norm() {
        assert_eq!(vnorm(car(0, 0, 2, -3)), 3);
        assert_eq!(vnorm(car(0, 0, -5, 1)), 5);
        assert_eq!(vnorm(car(0, 0, 0, 0)), 0);
    }

    #[test]
    fn vnorm_is_total_at_i32_min() {
        // saturating_abs(i32::MIN) == i32::MAX, not a panic.
        assert_eq!(vnorm(car(0, 0, i32::MIN, 0)), i32::MAX);
        assert_eq!(vnorm(car(0, 0, 0, i32::MIN)), i32::MAX);
    }

    #[test]
    fn speed_heatmap_is_per_point_max_and_sorted_by_point() {
        let live: HashSet<CarState> = [
            car(1, 0, 1, 0), // vnorm 1 at (1, 0)
            car(1, 0, 0, 2), // vnorm 2 at (1, 0) -- same point, higher speed
            car(0, 0, 1, 1), // vnorm 1 at (0, 0)
        ]
        .into_iter()
        .collect();

        let heatmap = speed_heatmap(&live);
        assert_eq!(heatmap, vec![(Point::new(0, 0), 1), (Point::new(1, 0), 2)]);
    }

    #[test]
    fn frontier_gap_is_empty_on_the_phase5a_broken_ring_fixture_via_a_real_flood() {
        // IMPORTANT (see progress-file Decisions log / subtask-4 finding):
        // this fixture is Ф5a's broken-ring (`d.set((4,2), false)`), used
        // via a REAL `forward_reachable` flood -- deliberately asserting
        // the EMPTY result here, not the non-empty result design §
        // Approach (3)/subtask 7 originally expected. Removing one cell
        // from a width-1 ring's border cycle leaves a single connected
        // PATH (not two disjoint components): a 4-adjacent cell pair both
        // in `D` is always connected by a legal move (a unit-distance
        // chord's supercover is exactly its 2 endpoints, so it is never
        // wall-clipped), so `forward_reachable`'s BFS -- unbounded in move
        // count -- eventually reaches every cell in the seed's connected
        // component. `frontier_gap`'s literal definition (drivable ∉
        // proj(R) with a 4-neighbor ∈ proj(R)) can therefore never be
        // non-empty for a REAL flood `R`: any such neighbor would, by the
        // same reachability, already be `∈ proj(R)`. This is the
        // "degenerate" case design § Approach (3) itself names (all of `D`
        // reachable, no forward lap-close) -- it is NOT a stall in `R`,
        // it is a lap-closure failure, invisible to `frontier_gap(d, &R)`.
        let mut d = ring_corridor();
        d.set(Point::new(4, 2), false);
        let seed = car(2, 0, 0, 0);
        let r = crate::forward_reachable(&d, &[seed], 1);

        let frontier = frontier_gap(&d, &r);
        assert!(frontier.is_empty());
    }

    #[test]
    fn frontier_gap_lists_drivable_cells_adjacent_to_r_but_not_in_r() {
        let d = ring_corridor();
        // R covers only the gate cell; its East and North drivable
        // neighbors are frontier (drivable, not in R, 4-adjacent to it).
        // West of (2,0) is (1,0) -- also drivable and not in R.
        let r: HashSet<CarState> = std::iter::once(car(2, 0, 0, 0)).collect();

        let frontier = frontier_gap(&d, &r);
        assert!(frontier.contains(&Point::new(3, 0)));
        assert!(frontier.contains(&Point::new(1, 0)));
        // R's own cell is never in the frontier.
        assert!(!frontier.contains(&Point::new(2, 0)));
        // Not drivable, so never a frontier member even though 4-adjacent.
        assert!(!frontier.contains(&Point::new(2, 1)));
        // Sorted ascending by Point.
        let mut sorted = frontier.clone();
        sorted.sort();
        assert_eq!(frontier, sorted);
    }

    #[test]
    fn frontier_gap_is_empty_when_r_covers_all_of_d() {
        let d = ring_corridor();
        let r: HashSet<CarState> = (0..5)
            .flat_map(|y| (0..5).map(move |x| Point::new(x, y)))
            .filter(|&p| d.contains(p))
            .map(|p| car(p.x, p.y, 0, 0))
            .collect();

        let frontier = frontier_gap(&d, &r);
        assert!(frontier.is_empty());
    }
}
