//! Ф5a — passability reachability substrate + V=1 liveness oracle (design doc
//! §2/§3, spec `ai-docs/plans/2026-07-23-gp-gen-phase5a-reachability.spec.md`).
//!
//! Three finite, integer-only, deterministic floods over the 4D car state
//! `(x, y, vx, vy)` = [`CarState`], reusing core's own [`legal_move`] as the
//! graph edge and core's own [`LapCounter::register_move`] for the
//! start/finish crossing test — no reimplementation (AC3). `forward_reachable`
//! and `backward_reachable` are the raw `R`/`B` substrate Ф5b's `R ∩ B` will
//! consume; `oracle_liveness_v1` is the standalone binary "a lap exists at
//! `|v| ≤ 1`" certifier used directly by the generation pipeline.

use std::collections::{HashSet, VecDeque};

use gp_core::geom::Corridor;
use gp_core::sim::{Action, CarState, LapCounter, legal_move, step};
use gp_core::track::{RaceDir, StartFinish, StartGrid};
use strum::IntoEnumIterator;

/// The predecessor of `s2` under action `a` — inverts [`step`]: `step`
/// computes `vx' = vx + ax` then `x' = x + vx'`, so undoing it takes
/// `vx = s2.vx − ax` (the pre-accel velocity) and, independently, `x = s2.x −
/// s2.vx` (`s2.vx` is the **post**-accel velocity `step` actually added to
/// `x`) — same for `y`/`vy`.
///
/// Total via `checked_sub`: returns `None` on `i32` underflow/overflow rather
/// than panicking, so the caller can skip a would-be predecessor outside
/// `i32`'s range. This yields a pure *candidate* only — legality
/// (`legal_move(d, s, a)`, which also proves `step(s, a) == s2`) is the
/// caller's job.
const fn predecessor(s2: CarState, a: Action) -> Option<CarState> {
    let (ax, ay) = a.accel();
    let Some(vx) = s2.vx.checked_sub(ax) else {
        return None;
    };
    let Some(vy) = s2.vy.checked_sub(ay) else {
        return None;
    };
    let Some(x) = s2.x.checked_sub(s2.vx) else {
        return None;
    };
    let Some(y) = s2.y.checked_sub(s2.vy) else {
        return None;
    };
    Some(CarState { x, y, vx, vy })
}

/// Whether `s`'s velocity lies within the L∞ box `|vx| ≤ v_ceil ∧ |vy| ≤
/// v_ceil` — the uniform bound both floods and Ф5b's `V_ceil` deepening share
/// (design's Key-decision default: a superset of the cardinal-only domain, so
/// it can only add genuine `legal_move`-driveable states, never manufacture a
/// spurious one).
///
/// `saturating_neg` (not raw negation) keeps this total even at
/// `v_ceil == i32::MIN`.
pub(crate) const fn within_v_ceil(s: CarState, v_ceil: i32) -> bool {
    let floor = v_ceil.saturating_neg();
    s.vx >= floor && s.vx <= v_ceil && s.vy >= floor && s.vy <= v_ceil
}

/// Forward flood over [`legal_move`] edges from `seeds`, bounded to the L∞
/// box `|vx|, |vy| ≤ v_ceil` (AC1).
///
/// `seeds` outside the bound are dropped (never expanded); every seed inside
/// the bound is a member of the returned set regardless of `d` membership —
/// the bound is purely kinematic, `d` membership is enforced only on
/// transitions via `legal_move`. Deterministic **membership** (AC5): the
/// worklist is a `VecDeque` seeded in `seeds`' argument order, expanding
/// successors in [`Action::iter()`] declaration order; the returned
/// [`HashSet`]'s own iteration order is not meaningful and is never relied on.
pub fn forward_reachable(d: &Corridor, seeds: &[CarState], v_ceil: i32) -> HashSet<CarState> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    for &s in seeds {
        if within_v_ceil(s, v_ceil) && visited.insert(s) {
            queue.push_back(s);
        }
    }
    while let Some(s) = queue.pop_front() {
        for a in Action::iter() {
            if !legal_move(d, s, a) {
                continue;
            }
            let s2 = step(s, a);
            if within_v_ceil(s2, v_ceil) && visited.insert(s2) {
                queue.push_back(s2);
            }
        }
    }
    visited
}

/// Backward flood over the *reversed* [`legal_move`] edges from `goals`,
/// bounded to the L∞ box `|vx|, |vy| ≤ v_ceil` (AC2).
///
/// A candidate predecessor `s` of `s2` under `a` (from `predecessor`) is
/// only admitted when `legal_move(d, s, a)` holds — re-validating
/// `supercover ⊆ D` and `p1 ∈ D`, and guaranteeing `step(s, a) == s2` — the
/// **same** edge relation `forward_reachable` walks, just enumerated in
/// reverse (AC3): no parallel legality rule. Deterministic membership (AC5)
/// for the same reason as `forward_reachable`.
pub fn backward_reachable(d: &Corridor, goals: &[CarState], v_ceil: i32) -> HashSet<CarState> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    for &s in goals {
        if within_v_ceil(s, v_ceil) && visited.insert(s) {
            queue.push_back(s);
        }
    }
    while let Some(s2) = queue.pop_front() {
        for a in Action::iter() {
            let Some(s) = predecessor(s2, a) else {
                continue;
            };
            if within_v_ceil(s, v_ceil) && legal_move(d, s, a) && visited.insert(s) {
                queue.push_back(s);
            }
        }
    }
    visited
}

/// The V=1 velocity ceiling `oracle_liveness_v1` floods within (design doc §2
/// Ф5a scaffolding: `|v| ≤ 1`).
const ORACLE_V1_CEIL: i32 = 1;

/// Binary "a driveable closed lap exists at `|v| ≤ 1`" oracle (AC4).
///
/// Runs an **augmented** `(CarState, LapCounter)` flood seeded at each of
/// `grid.positions` at rest (`v = (0, 0)`) with a fresh [`LapCounter::new()`]
/// (`raw() == -1`, "behind the gate, race not started"). On every legal
/// transition the frontier node's counter is cloned and
/// [`LapCounter::register_move`]d across the swept `from → to`; `raw() >= 1`
/// (the lap-closing `0 → 1` crossing) short-circuits `true` without
/// enqueueing. The visited key clamps the counter to `{-1, 0}`
/// (`raw().clamp(-1, 0)`) — geometrically the only values reachable before the
/// `>= 1` success short-circuit, since the S/F is a full chord and every seed
/// starts behind the gate — so the augmented state space stays finite.
///
/// `race_dir` is accepted for signature fidelity (design `[N4]`, Ф5b, and the
/// `generate()` call site) but unused here: the crossing sign derives from
/// `sf.gate.forward` alone.
pub fn oracle_liveness_v1(
    d: &Corridor,
    grid: &StartGrid,
    sf: &StartFinish,
    race_dir: RaceDir,
) -> bool {
    let _ = race_dir;

    let mut visited: HashSet<(CarState, i32)> = HashSet::new();
    let mut queue: VecDeque<(CarState, LapCounter)> = VecDeque::new();
    for &p in &grid.positions {
        let s = CarState {
            x: p.x,
            y: p.y,
            vx: 0,
            vy: 0,
        };
        if !within_v_ceil(s, ORACLE_V1_CEIL) {
            continue;
        }
        let counter = LapCounter::new();
        if visited.insert((s, counter.raw().clamp(-1, 0))) {
            queue.push_back((s, counter));
        }
    }
    while let Some((s, counter)) = queue.pop_front() {
        for a in Action::iter() {
            if !legal_move(d, s, a) {
                continue;
            }
            let s2 = step(s, a);
            if !within_v_ceil(s2, ORACLE_V1_CEIL) {
                continue;
            }
            let mut counter2 = counter;
            counter2.register_move(sf, s.pos(), s2.pos());
            if counter2.raw() >= 1 {
                return true;
            }
            if visited.insert((s2, counter2.raw().clamp(-1, 0))) {
                queue.push_back((s2, counter2));
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use gp_core::geom::Point;

    use super::*;
    use crate::testfix::*;

    #[test]
    fn predecessor_inverts_step_for_a_legal_transition() {
        // step(car(2,2,1,0), Coast) = car(3,2,1,0); predecessor recovers the
        // pre-move state from the post-move state.
        let s2 = car(3, 2, 1, 0);
        assert_eq!(predecessor(s2, Action::Coast), Some(car(2, 2, 1, 0)));
    }

    #[test]
    fn predecessor_is_total_on_overflowing_input() {
        // s2.x == i32::MIN combined with s2.vx == 1 makes `x = s2.x - s2.vx`
        // underflow; predecessor returns None rather than panicking.
        let s2 = car(i32::MIN, 0, 1, 0);
        assert_eq!(predecessor(s2, Action::Coast), None);
    }

    #[test]
    fn within_v_ceil_bounds_the_l_infinity_box() {
        assert!(within_v_ceil(car(0, 0, 1, -1), 1));
        assert!(!within_v_ceil(car(0, 0, 2, 0), 1));
        assert!(!within_v_ceil(car(0, 0, 0, -2), 1));
        // Total even at v_ceil == i32::MIN (saturating_neg keeps floor finite).
        assert!(!within_v_ceil(car(0, 0, 0, 0), i32::MIN));
    }

    // ---- forward_reachable (subtask 2) ----

    #[test]
    fn forward_reachable_ac1_bounded_flood_from_seed() {
        let d = Corridor::filled(Point::new(0, 0), 5, 5);
        let seed = car(2, 2, 0, 0);
        let set = forward_reachable(&d, &[seed], 1);

        assert!(set.contains(&seed));
        assert!(set.contains(&car(3, 2, 1, 0))); // East
        assert!(set.contains(&car(1, 2, -1, 0))); // West
        assert!(set.contains(&car(2, 3, 0, 1))); // North
        assert!(set.contains(&car(2, 1, 0, -1))); // South

        // Bound: no member exceeds |v| <= 1.
        assert!(set.iter().all(|s| s.vx.abs() <= 1 && s.vy.abs() <= 1));
    }

    #[test]
    fn forward_reachable_ac3_excludes_supercover_illegal_chord() {
        // Same wall-clip fixture as gp_core::sim's legal_move test: a chord
        // whose supercover clips an off-D cell is illegal, so the flood must
        // never reach the state it would otherwise produce.
        let mut d = Corridor::new(Point::new(0, 0), 4, 4);
        d.set(Point::new(0, 0), true);
        d.set(Point::new(1, 0), true);
        d.set(Point::new(2, 0), true);
        d.set(Point::new(1, 1), true);
        // (0, 1) is deliberately left off-D.

        let s_clip = car(0, 0, 0, 1);
        let set = forward_reachable(&d, &[s_clip], 1);

        assert!(set.contains(&s_clip));
        // East from s_clip: v2 = (1, 1), p1 = (1, 1) — in D, but the chord's
        // supercover clips the off-D (0, 1), so legal_move rejects it.
        assert!(d.contains(Point::new(1, 1))); // non-vacuous
        assert!(!set.contains(&car(1, 1, 1, 1)));
    }

    #[test]
    fn forward_reachable_ac5_deterministic() {
        let d = Corridor::filled(Point::new(0, 0), 5, 5);
        let seeds = [car(2, 2, 0, 0)];
        assert_eq!(
            forward_reachable(&d, &seeds, 1),
            forward_reachable(&d, &seeds, 1)
        );
    }

    // ---- backward_reachable (subtask 3) ----

    #[test]
    fn backward_reachable_ac2_known_predecessor_is_member() {
        let d = Corridor::filled(Point::new(0, 0), 5, 5);
        let goal = car(3, 2, 1, 0);
        let set = backward_reachable(&d, &[goal], 1);

        // predecessor(goal, Coast) = (2, 2, 1, 0); legal_move confirms
        // step((2,2,1,0), Coast) == goal.
        let pred = car(2, 2, 1, 0);
        assert!(legal_move(&d, pred, Action::Coast));
        assert_eq!(step(pred, Action::Coast), goal);
        assert!(set.contains(&pred));
        assert!(set.contains(&goal));
    }

    #[test]
    fn backward_reachable_ac5_total_on_extreme_goal() {
        // goal.vx == 1 combined with goal.x == i32::MIN makes every
        // predecessor's `x = s2.x - s2.vx` underflow -> checked_sub returns
        // None for every action, so the search returns promptly with no
        // predecessors (just the goal itself), never panicking.
        let d = Corridor::filled(Point::new(0, 0), 5, 5);
        let goal = car(i32::MIN, 0, 1, 0);
        let set = backward_reachable(&d, &[goal], 1);
        assert_eq!(set.len(), 1);
        assert!(set.contains(&goal));
    }

    #[test]
    fn backward_reachable_ac5_deterministic() {
        let d = Corridor::filled(Point::new(0, 0), 5, 5);
        let goals = [car(3, 2, 1, 0)];
        assert_eq!(
            backward_reachable(&d, &goals, 1),
            backward_reachable(&d, &goals, 1)
        );
    }

    // ---- oracle_liveness_v1 (subtask 4) ----

    #[test]
    fn oracle_liveness_v1_ac4_valid_ring_is_lappable() {
        // AC4: a valid closed ring is lappable at V=1.
        //
        // `LapCounter::register_move` now bounds the crossing test to the
        // gate's along-chord extent (`gp_core::sim`'s `lat_coord`/
        // `crossing_within_span`, the S/F bounded-chord fix — design doc §3),
        // not the gate's infinite supporting line. On this ring the far-wall
        // crossing (the supporting line's *other* intersection with the
        // annulus, on the opposite straight) falls outside the gate's
        // along-chord span (`behind = [(2, 0)]`, so the span is `y = 0` only)
        // and is excluded, so a full CCW loop nets a real `+1` and the
        // augmented flood reaches `raw() >= 1` (a lap), not `{-1, 0}`.
        let d = ring_corridor();
        let sf = ring_sf();
        let grid = ring_grid();
        assert!(oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw));
    }

    #[test]
    fn oracle_liveness_v1_ac6_broken_ring_is_not_lappable() {
        let mut d = ring_corridor();
        d.set(Point::new(4, 2), false); // break the right-side straight
        let sf = ring_sf();
        let grid = ring_grid();
        assert!(!oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw));
    }

    #[test]
    fn oracle_liveness_v1_ac4_distinguishes_race_start_from_lap_close() {
        // A lone race-start (-1 -> 0) forward crossing must NOT be accepted
        // as a lap-close (0 -> 1): the fixture permits the crossing but
        // dead-ends right after it, with no return path.
        let (d, sf, grid) = dead_end_corridor();
        assert!(!oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw));
    }

    // ---- Cross-cutting AC tests (subtask 5) ----

    #[test]
    fn ac3_forward_flood_admits_successor_iff_legal_move() {
        let mut d = Corridor::new(Point::new(0, 0), 4, 4);
        d.set(Point::new(0, 0), true);
        d.set(Point::new(1, 0), true);
        d.set(Point::new(2, 0), true);
        d.set(Point::new(1, 1), true);
        // (0, 1) deliberately off-D, reproducing the wall-clip shape.

        let seed = car(0, 0, 0, 1);
        let set = forward_reachable(&d, &[seed], 1);

        for &s in &set {
            for a in Action::iter() {
                let expects_edge = legal_move(&d, s, a);
                if expects_edge {
                    let s2 = step(s, a);
                    if within_v_ceil(s2, 1) {
                        assert!(
                            set.contains(&s2),
                            "legal_move({s:?}, {a:?}) holds but {s2:?} is absent from the flood"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn ac3_oracle_crossing_matches_direct_lap_counter_register_move() {
        // Pin the shared crossing path: the oracle's first S/F crossing
        // (race start, -1 -> 0) must agree with a direct LapCounter call on
        // the identical `from -> to`, and a lone crossing is not itself a
        // lap.
        let sf = ring_sf();
        let mut counter = LapCounter::new();
        counter.register_move(&sf, Point::new(2, 0), Point::new(3, 0));
        assert_eq!(counter.raw(), 0); // race start only, not yet a lap

        // oracle_liveness_v1 runs the identical register_move path (AC3): the
        // shared path scores this single crossing as race start (raw() 0)
        // and, threaded through the full flood on the valid ring, also
        // reaches a lap (see `oracle_liveness_v1_ac4_valid_ring_is_lappable`).
        let d = ring_corridor();
        let grid = ring_grid();
        assert!(oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw));
    }

    #[test]
    fn ac5_all_three_functions_deterministic_on_ring_fixture() {
        let d = ring_corridor();
        let sf = ring_sf();
        let grid = ring_grid();
        let seeds: Vec<CarState> = grid
            .positions
            .iter()
            .map(|&p| car(p.x, p.y, 0, 0))
            .collect();
        let goal = [car(3, 0, 1, 0)];

        assert_eq!(
            forward_reachable(&d, &seeds, 1),
            forward_reachable(&d, &seeds, 1)
        );
        assert_eq!(
            backward_reachable(&d, &goal, 1),
            backward_reachable(&d, &goal, 1)
        );
        assert_eq!(
            oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw),
            oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw)
        );
    }

    #[test]
    fn ac6_forward_and_backward_reachable_intersect_on_known_state() {
        // The post-gate East-crossing state at (3, 0) is directly reachable
        // forward from the grid seed (one East move) and is itself the
        // backward flood's goal seed — a minimal, hand-verifiable witness
        // that the two substrates compose over the same state space.
        let d = ring_corridor();
        let grid = ring_grid();
        let seeds: Vec<CarState> = grid
            .positions
            .iter()
            .map(|&p| car(p.x, p.y, 0, 0))
            .collect();
        let witness = car(3, 0, 1, 0);
        let goal = [witness];

        let forward = forward_reachable(&d, &seeds, 1);
        let backward = backward_reachable(&d, &goal, 1);
        assert!(forward.contains(&witness));
        assert!(backward.contains(&witness));
    }
}
