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

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};

use gp_core::geom::{Corridor, Point};
use gp_core::sim::{Action, CarState, LapCounter, legal_move, step};
use gp_core::track::{StartFinish, TrackMetrics};
use strum::IntoEnumIterator;

use crate::phase5::within_v_ceil;

/// The result of running `phase5_full_oracle` (subtask 6; design §
/// Approach (3)).
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
    /// (design `[N3]`, § Approach (3)) — the **goal-aware** outer 4-frontier
    /// of the phase-0 reachable region `P0` (post-race-start, pre-lap-close
    /// cells, emitted by `fastest_lap_through_live`, subtask 5) within
    /// `proj(R)`. Its
    /// non-emptiness (AC3) is guaranteed **unconditionally** by the driver's
    /// seed-cell fallback, which fires whenever this frontier is empty — both
    /// when `P0 == ∅` (no forward crossing reachable at all) and when `P0 ==
    /// proj(R)` (the phase-0 region already covers the whole drivable
    /// component, leaving no outer frontier); `grid.positions` is non-empty
    /// by generator contract, so the fallback is always non-empty. In the
    /// normal (non-degenerate) `NotLappable` case (`∅ ⊊ P0 ⊊ proj(R)`) the
    /// P0-frontier itself is the meaningful diagnostic, localizing the
    /// reachability stall for Ф6's `map_frontier_gap_to_edge`.
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

/// The goal-aware reachability-stall frontier (design `[N3]`, § Approach (3),
/// design amendment): the **outer 4-frontier of `p0_cells` within
/// `r_cells`** — cells `c ∈ r_cells`, `c ∉ p0_cells`, with a 4-neighbor `∈
/// p0_cells`.
///
/// **Pure** point-set helper — this replaces the earlier committed
/// `frontier_gap(d, &R)` (drivable-vs-`R`), which was **provably always
/// empty** for a real `forward_reachable` flood (design § Risks: any
/// 4-adjacent drivable neighbor of a reached cell is itself always reachable,
/// by construction of `legal_move`'s unit-distance supercover). The
/// goal-aware form is instead evaluated against `proj(R)` (drivable cells any
/// live state occupies) and `P0` (the phase-0 reachable region emitted by
/// `fastest_lap_through_live`, subtask 5) — the phase distinction encodes
/// the lap-close-vs-race-start awareness a plain drivability boundary cannot.
/// Full driver wiring (`r_cells = proj(R)`, `p0_cells = P0`, plus the
/// seed-cell fallback when this frontier is empty) lands in subtask 6.
///
/// Sorted by [`Point`] for deterministic output (AC6).
#[allow(
    dead_code,
    reason = "not yet wired into phase5_full_oracle (subtask 6); exercised \
              directly by this subtask's own tests in the interim"
)]
pub(crate) fn frontier_gap(r_cells: &HashSet<Point>, p0_cells: &HashSet<Point>) -> Vec<Point> {
    let mut frontier: Vec<Point> = r_cells
        .iter()
        .filter(|p| !p0_cells.contains(p))
        .filter(|p| p.neighbors4().into_iter().any(|q| p0_cells.contains(&q)))
        .copied()
        .collect();
    frontier.sort();
    frontier
}

/// Confined augmented `(CarState, LapCounter)` BFS from `seeds` (start-grid
/// positions, expanded at rest, `v = (0, 0)`) through `live`, returning the
/// fewest-move path to the first lap-close (`raw() >= 1`) transition —
/// `None` if no lap exists — together with the phase-0 reachable cell set
/// `P0` (design § Approach (1)/(3)): `P0 = { s.pos() : a visited augmented
/// state (s, φ) has φ == 0 }`, the post-race-start, pre-lap-close region
/// `frontier_gap` consumes (subtask 6).
///
/// Reuses the identical `legal_move` / `step` / [`LapCounter::register_move`]
/// triple `oracle_liveness_v1` (`phase5.rs`) uses (AC5) — this is a distinct
/// product-graph traversal, not a reimplementation of `forward_reachable`.
/// Expansion is confined to successors `s2 ∈ live`: a real lap-close path is
/// never dropped by this confinement, since every state on it is reachable
/// from a seed (`∈ R`) and can reach a forward crossing (`∈ B`), hence `∈
/// live` (design § Approach (1)).
///
/// The visited key clamps the counter to `{-1, 0}` (mirroring
/// `oracle_liveness_v1`) — the only values reachable before the `>= 1`
/// short-circuit, since every seed starts strictly behind a full-chord gate.
/// BFS (FIFO) order guarantees the first `raw() >= 1` transition found is a
/// fewest-move path.
#[allow(
    dead_code,
    reason = "not yet wired into phase5_full_oracle (subtask 6); exercised \
              directly by this subtask's own tests in the interim"
)]
pub(crate) fn fastest_lap_through_live(
    d: &Corridor,
    seeds: &[Point],
    sf: &StartFinish,
    live: &HashSet<CarState>,
    v_ceil: i32,
) -> (Option<Vec<Point>>, HashSet<Point>) {
    type Key = (CarState, i32);

    let mut parent: HashMap<Key, Option<Key>> = HashMap::new();
    let mut queue: VecDeque<(CarState, LapCounter)> = VecDeque::new();
    let mut p0: HashSet<Point> = HashSet::new();

    for &p in seeds {
        let s = CarState {
            x: p.x,
            y: p.y,
            vx: 0,
            vy: 0,
        };
        if !within_v_ceil(s, v_ceil) {
            continue;
        }
        let counter = LapCounter::new();
        let key: Key = (s, counter.raw().clamp(-1, 0));
        if let Entry::Vacant(e) = parent.entry(key) {
            e.insert(None);
            queue.push_back((s, counter));
        }
    }

    while let Some((s, counter)) = queue.pop_front() {
        let s_key: Key = (s, counter.raw().clamp(-1, 0));
        for a in Action::iter() {
            if !legal_move(d, s, a) {
                continue;
            }
            let s2 = step(s, a);
            if !within_v_ceil(s2, v_ceil) || !live.contains(&s2) {
                continue;
            }
            let mut counter2 = counter;
            counter2.register_move(sf, s.pos(), s2.pos());
            if counter2.raw() >= 1 {
                let mut path = vec![s2.pos()];
                let mut cur = Some(s_key);
                while let Some(key) = cur {
                    path.push(key.0.pos());
                    cur = parent.get(&key).copied().flatten();
                }
                path.reverse();
                return (Some(path), p0);
            }
            let key2: Key = (s2, counter2.raw().clamp(-1, 0));
            if let Entry::Vacant(e) = parent.entry(key2) {
                e.insert(Some(s_key));
                if key2.1 == 0 {
                    p0.insert(s2.pos());
                }
                queue.push_back((s2, counter2));
            }
        }
    }

    (None, p0)
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
    fn frontier_gap_lists_r_cells_adjacent_to_a_proper_p0_but_not_in_p0() {
        // Hand-built, connected `r`: a 4-cell straight line (0,0)-(3,0).
        // `p0` is the proper subset `{(1,0)}` in its interior.
        let r: HashSet<Point> = [
            Point::new(0, 0),
            Point::new(1, 0),
            Point::new(2, 0),
            Point::new(3, 0),
        ]
        .into_iter()
        .collect();
        let p0: HashSet<Point> = std::iter::once(Point::new(1, 0)).collect();

        let frontier = frontier_gap(&r, &p0);

        // (0,0) and (2,0) are 4-adjacent to (1,0), in r, not in p0.
        assert!(frontier.contains(&Point::new(0, 0)));
        assert!(frontier.contains(&Point::new(2, 0)));
        // (3,0) is in r but not 4-adjacent to (1,0) -- excluded.
        assert!(!frontier.contains(&Point::new(3, 0)));
        // p0's own cell is never in the frontier.
        assert!(!frontier.contains(&Point::new(1, 0)));
        // Sorted ascending by Point.
        let mut sorted = frontier.clone();
        sorted.sort();
        assert_eq!(frontier, sorted);
    }

    #[test]
    fn frontier_gap_is_empty_when_p0_equals_r() {
        let r: HashSet<Point> = [Point::new(0, 0), Point::new(1, 0)].into_iter().collect();
        let p0 = r.clone();

        assert!(frontier_gap(&r, &p0).is_empty());
    }

    #[test]
    fn frontier_gap_is_empty_when_p0_is_empty() {
        // The driver (subtask 6), not this pure helper, supplies the
        // seed-cell fallback for this degenerate case.
        let r: HashSet<Point> = [Point::new(0, 0), Point::new(1, 0)].into_iter().collect();
        let p0: HashSet<Point> = HashSet::new();

        assert!(frontier_gap(&r, &p0).is_empty());
    }

    // ---- fastest_lap_through_live (subtask 5) ----

    /// Computes `live = R ∩ B` the way the driver (subtask 6) will, for a
    /// test fixture: `forward_reachable` from `seeds`, `lap_close_goals`
    /// over `R`, `backward_reachable` from those goals, intersected with `R`.
    fn live_for(
        d: &Corridor,
        sf: &StartFinish,
        seeds: &[CarState],
        v_ceil: i32,
    ) -> HashSet<CarState> {
        let r = crate::forward_reachable(d, seeds, v_ceil);
        let goals = lap_close_goals(d, sf, &r, v_ceil);
        let b = crate::backward_reachable(d, &goals, v_ceil);
        r.intersection(&b).copied().collect()
    }

    #[test]
    fn fastest_lap_through_live_finds_the_fewest_move_lap_on_a_valid_ring() {
        let d = ring_corridor();
        let sf = ring_sf();
        let grid = ring_grid();
        let seed_states: Vec<CarState> = grid
            .positions
            .iter()
            .map(|&p| car(p.x, p.y, 0, 0))
            .collect();
        let live = live_for(&d, &sf, &seed_states, 1);

        let (path, p0) = fastest_lap_through_live(&d, &grid.positions, &sf, &live, 1);

        let path = path.expect("a valid ring has a closed lap at V=1");
        // Starts at the start-grid seed, ends at the lap-close crossing (the
        // gate's ahead cell, reached a second time after the full loop).
        assert_eq!(path.first(), Some(&Point::new(2, 0)));
        assert_eq!(path.last(), Some(&Point::new(3, 0)));
        assert!(path.len() > 1);
        assert!(!p0.is_empty());
        // NOT asserted here (design § Approach (3) step 2, scoped to
        // non-loopable topologies only): a full loop re-enters (2, 0) at
        // phase 0 via the bounded-chord far-wall exclusion, so the
        // behind-gate-seed-excluded-from-P0 invariant is FALSE on this
        // fixture -- see the broken-ring / dead-end tests below instead.
    }

    #[test]
    fn fastest_lap_through_live_returns_none_on_the_broken_ring() {
        let mut d = ring_corridor();
        d.set(Point::new(4, 2), false); // Ф5a's broken-ring fixture.
        let sf = ring_sf();
        let grid = ring_grid();
        let seed_states: Vec<CarState> = grid
            .positions
            .iter()
            .map(|&p| car(p.x, p.y, 0, 0))
            .collect();
        let live = live_for(&d, &sf, &seed_states, 1);

        let (path, p0) = fastest_lap_through_live(&d, &grid.positions, &sf, &live, 1);

        assert!(path.is_none());
        assert!(!p0.is_empty());
        // Non-loopable topology: no re-crossing loops back to the
        // behind-gate seed cell at phase 0 (design § Approach (3) step 2).
        assert!(!p0.contains(&Point::new(2, 0)));
        // The phase-0 arc reaches at least the immediate post-crossing cell.
        assert!(p0.contains(&Point::new(3, 0)));
    }

    #[test]
    fn fastest_lap_through_live_returns_none_on_a_lone_race_start_dead_end() {
        // A lone race-start (-1 -> 0) crossing must not be mistaken for a
        // lap: the fixture dead-ends right after it, no return path.
        let (d, sf, grid) = dead_end_corridor();
        let seed_states: Vec<CarState> = grid
            .positions
            .iter()
            .map(|&p| car(p.x, p.y, 0, 0))
            .collect();
        let live = live_for(&d, &sf, &seed_states, 1);

        let (path, p0) = fastest_lap_through_live(&d, &grid.positions, &sf, &live, 1);

        assert!(path.is_none());
        assert!(!p0.is_empty());
        assert!(!p0.contains(&Point::new(2, 0))); // non-loopable, § Approach (3) step 2
        assert!(p0.contains(&Point::new(3, 0)));
    }
}
