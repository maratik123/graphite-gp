//! Ф6's local-repair task — the run-out model behind `Issue::NoBraking`
//! (design doc §2 `[C3]`; `ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md`
//! § Decision 2).
//!
//! A `v_target`-referenced run-out budget check, never a lap-existence check
//! (#30's executed AC7 result: no track V=1-lappable can be un-lappable at a
//! higher `V_ceil`, so "the corner cannot be taken at all" is never a
//! producible verdict). This module owns the primitives; `phase4.rs`'s
//! `Issue::NoBraking` variant carries the payload, and the detector body
//! (`phase5_runout_checks`) lands in this same module at subtask 6.
#![allow(
    dead_code,
    reason = "no production caller until phase5_runout_checks wires these primitives in \
              at subtask 6 — every item here is already exercised by this module's own \
              tests"
)]

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use gp_core::geom::{Corridor, Point};
use gp_core::sim::{Action, CarState, legal_move, step};
use gp_core::track::TrackMetrics;
use strum::IntoEnumIterator;

use crate::phase4::wall_run;
use crate::phase5::within_v_ceil;
use crate::phase5b::vnorm;

/// Largest `v` with `v·(v+1) ≤ i32::MAX` (`46_340·46_341 = 2_147_441_940`).
pub(crate) const MAX_BRAKE_SPEED: i32 = 46_340;

/// Exact `v·(v+1)/2` — the cell count to decelerate from `v` to rest under
/// cardinal `±1`-per-turn deceleration (design doc's own §2 Ф3 budget is the
/// approximate `V_target²/2`; this module pins the exact form — design.md
/// § Decision 1).
///
/// `v` is clamped to `0..=MAX_BRAKE_SPEED` by an if-chain (`Ord::clamp` is
/// not const-stable), so `v·(v+1)` cannot overflow `i32` and the division by
/// the non-zero literal `2` is exact (`v·(v+1)` is always even).
#[allow(
    clippy::arithmetic_side_effects,
    reason = "v is clamped to 0..=MAX_BRAKE_SPEED immediately above, so v*(v+1) <= i32::MAX; \
              the divisor is the non-zero literal 2"
)]
pub(crate) const fn triangular(v: i32) -> i32 {
    let v = if v < 0 {
        0
    } else if v > MAX_BRAKE_SPEED {
        MAX_BRAKE_SPEED
    } else {
        v
    };
    v * (v + 1) / 2
}

/// Braking-cell budget to decelerate from speed `from` to speed `to` — `0`
/// when `to >= from` (no braking needed, design doc § Decision 1).
pub(crate) const fn braking_cells(from: i32, to: i32) -> i32 {
    if to >= from {
        0
    } else {
        triangular(from).saturating_sub(triangular(to))
    }
}

/// `heatmap`'s recorded value at `p`, or `None` — `heatmap` is sorted
/// ascending by `Point` ([`crate::phase5b::speed_heatmap`]'s own contract),
/// so this is a binary search, not a linear scan.
fn heatmap_at(heatmap: &[(Point, i32)], p: Point) -> Option<i32> {
    heatmap
        .binary_search_by_key(&p, |&(pt, _)| pt)
        .ok()
        .map(|i| heatmap[i].1)
}

/// The speed-sink path indices of `metrics.fastest_lap` — `{ i :
/// heatmap(path[i]) ≤ 1 } ∪ {0}` (design doc § Decision 2). Index `0` is
/// always included when the path is non-empty: `path[0]` is a start-grid
/// position and the race-start seeds are at rest, so a flood seeded there
/// reproduces the global race-start flood — a conservative superset, never a
/// stale window (spec Amendment A2; the "sink set non-empty by heatmap
/// alone" argument is unsound, since [`crate::phase5b::speed_heatmap`] is a
/// per-point max over *all* live states, so a start cell traversed fast on a
/// later lap has heatmap `> 1`).
pub(crate) fn sink_indices(metrics: &TrackMetrics) -> BTreeSet<usize> {
    let mut sinks = BTreeSet::new();
    if !metrics.fastest_lap.is_empty() {
        sinks.insert(0);
    }
    for (i, &p) in metrics.fastest_lap.iter().enumerate() {
        if heatmap_at(&metrics.speed_heatmap, p).is_some_and(|v| v <= 1) {
            sinks.insert(i);
        }
    }
    sinks
}

/// The dominant-axis sign of `path`'s step at index `idx` (tie → `x`) —
/// `dir(c)` in design doc § Decision 2. Prefers the forward step
/// (`path[idx] → path[idx + 1]`); falls back to the backward step
/// (`path[idx - 1] → path[idx]`) at the path's last index; `(0, 0)` on a
/// single-point or out-of-range path (total, never panics).
pub(crate) fn travel_dir(path: &[Point], idx: usize) -> (i32, i32) {
    let Some(&cur) = path.get(idx) else {
        return (0, 0);
    };
    let (dx, dy) = if let Some(&next) = path.get(idx.saturating_add(1)) {
        (next.x.saturating_sub(cur.x), next.y.saturating_sub(cur.y))
    } else if idx > 0 {
        let prev = path[idx.saturating_sub(1)];
        (cur.x.saturating_sub(prev.x), cur.y.saturating_sub(prev.y))
    } else {
        (0, 0)
    };
    if dx.saturating_abs() >= dy.saturating_abs() {
        (dx.signum(), 0)
    } else {
        (0, dy.signum())
    }
}

/// The last drivable cell walking from `c` along `dir`, `c` included — the
/// wall the braking ray hits (`end(D,c)`, design doc § Decision 2). `c`
/// itself when `c ∉ D` or `dir == (0, 0)` (total, never loops: a `(0, 0)`
/// direction or a saturated non-advancing step both terminate immediately).
pub(crate) fn end_of_ray(d: &Corridor, c: Point, dir: (i32, i32)) -> Point {
    if dir == (0, 0) || !d.contains(c) {
        return c;
    }
    let (dx, dy) = dir;
    let mut cur = c;
    loop {
        let next = Point::new(cur.x.saturating_add(dx), cur.y.saturating_add(dy));
        if next == cur || !d.contains(next) {
            return cur;
        }
        cur = next;
    }
}

/// The geometric run-out room at `c` along `dir` — `wall_run(D, c, dir) − 1`
/// (design doc § Decision 2; `wall_run` counts `c` itself, so subtracting `1`
/// is correct). This is `lengthen_straight`'s lever: it grows when the
/// add-edit extends the braking ray.
pub(crate) fn runout_room(d: &Corridor, c: Point, dir: (i32, i32)) -> i32 {
    let run = wall_run(d, c, dir);
    i32::try_from(run).unwrap_or(i32::MAX).saturating_sub(1)
}

/// One forward flood: the states it reached, plus the per-cell max `vnorm`
/// over those states — what [`window_speed`] returns and [`deficit_at`]
/// reads.
pub(crate) struct WindowFlood {
    /// Every state reached, barrier arrivals included (recorded, never
    /// expanded).
    pub(crate) states: HashSet<CarState>,
    /// Per-cell max `vnorm` over `states` — what `attainable` reads.
    pub(crate) peak: HashMap<Point, i32>,
}

/// Records `s` into `states`/`peak` if not already present; returns whether
/// it was newly recorded (mirrors the `HashSet::insert` return convention).
fn record(states: &mut HashSet<CarState>, peak: &mut HashMap<Point, i32>, s: CarState) -> bool {
    if !states.insert(s) {
        return false;
    }
    peak.entry(s.pos())
        .and_modify(|v| *v = (*v).max(vnorm(s)))
        .or_insert_with(|| vnorm(s));
    true
}

/// Forward flood over [`legal_move`] edges from `seeds`, bounded to `|vx|,
/// |vy| ≤ v_ceil`, with `barriers` acting as a cut in the state space: a
/// successor landing on a barrier cell is recorded but never expanded
/// (design doc § Decision 2, the sink-to-sink model).
///
/// **Seed exemption (load-bearing in both directions — design.md § Risks
/// R13).** Seed states are **always** expanded, even when their own cell is
/// in `barriers`; only *successors* landing on a barrier are
/// recorded-not-expanded. The sink-seeded detection call depends on this
/// (its seed cell **is** itself a barrier, so without the exemption the
/// flood would be empty); the AC3 counter-scope depends on the barrier half.
pub(crate) fn window_speed(
    d: &Corridor,
    seeds: &[CarState],
    barriers: &HashSet<Point>,
    v_ceil: i32,
) -> WindowFlood {
    let mut states = HashSet::new();
    let mut peak = HashMap::new();
    let mut queue = VecDeque::new();
    for &s in seeds {
        if within_v_ceil(s, v_ceil) && record(&mut states, &mut peak, s) {
            queue.push_back(s);
        }
    }
    while let Some(s) = queue.pop_front() {
        for a in Action::iter() {
            if !legal_move(d, s, a) {
                continue;
            }
            let s2 = step(s, a);
            if !within_v_ceil(s2, v_ceil) || !record(&mut states, &mut peak, s2) {
                continue;
            }
            if !barriers.contains(&s2.pos()) {
                queue.push_back(s2);
            }
        }
    }
    WindowFlood { states, peak }
}

/// Max `vnorm` over `flood.states` at `end` that have `≥ 1` legal successor
/// in `d` — `v_corner(D,c)` (design doc § Decision 2), `widen_corner`'s
/// lever. `0` when no qualifying arrival exists.
pub(crate) fn corner_speed(d: &Corridor, flood: &WindowFlood, end: Point) -> i32 {
    flood
        .states
        .iter()
        .filter(|s| s.pos() == end)
        .filter(|&&s| Action::iter().any(|a| legal_move(d, s, a)))
        .map(|&s| vnorm(s))
        .max()
        .unwrap_or(0)
}

/// The run-out deficit at `c` — `braking_cells(v_entry, v_corner) −
/// runout_room` (design doc § Decision 2), positive iff `c` is under-braked.
///
/// `flood` is the caller's `window_speed` result (sink-to-sink for
/// production detection/recheck; a `#[cfg(test)]`-only fixed-radius counter-
/// scope for AC3) — this function is agnostic to how it was seeded, which is
/// exactly what makes the AC3 discriminating fixture honest rather than a
/// shape assertion (design.md § Test Design).
///
/// **`v_entry`'s `min` is currently a no-op — retained as a defensive
/// clamp.** `window_speed` prunes by `within_v_ceil(·, max(v_target, 1))`
/// (the caller's contract), so `attainable ≤ v_target` always holds in
/// practice and `min(v_target, attainable) == attainable`. The clamp keeps
/// this function correct if a future caller ever passes a `v_ceil` above
/// `v_target`.
pub(crate) fn deficit_at(
    d: &Corridor,
    flood: &WindowFlood,
    c: Point,
    dir: (i32, i32),
    v_target: i32,
) -> i32 {
    let end = end_of_ray(d, c, dir);
    let runout = runout_room(d, c, dir);
    // Never `flood.peak[c]`: `HashMap`'s `Index` panics on a missing key, and
    // `c` is not guaranteed present — a sink cell between the flood's seed
    // and `c` truncates the flood before it reaches `c`. The `0` default
    // under-reports the deficit — the conservative direction.
    let attainable = flood.peak.get(&c).copied().unwrap_or(0);
    let v_entry = v_target.min(attainable);
    let v_corner = corner_speed(d, flood, end);
    braking_cells(v_entry, v_corner).saturating_sub(runout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testfix::car;

    // ---- triangular / braking_cells ----------------------------------------

    #[test]
    fn triangular_matches_hand_values_at_small_v() {
        assert_eq!(triangular(0), 0);
        assert_eq!(triangular(1), 1);
        assert_eq!(triangular(2), 3);
        assert_eq!(triangular(3), 6);
    }

    #[test]
    fn triangular_is_clamped_and_total_at_i32_max() {
        // No panic on overflow-prone input; clamps to MAX_BRAKE_SPEED.
        assert_eq!(triangular(i32::MAX), triangular(MAX_BRAKE_SPEED));
        assert_eq!(triangular(i32::MIN), 0);
    }

    #[test]
    fn braking_cells_is_zero_when_to_is_at_least_from() {
        assert_eq!(braking_cells(2, 2), 0);
        assert_eq!(braking_cells(2, 5), 0);
    }

    #[test]
    fn braking_cells_matches_the_triangular_difference() {
        assert_eq!(braking_cells(3, 1), triangular(3) - triangular(1));
    }

    // ---- sink_indices --------------------------------------------------

    #[test]
    fn sink_indices_always_contains_zero_when_path_non_empty() {
        let metrics = TrackMetrics {
            fastest_lap: vec![Point::new(0, 0), Point::new(1, 0)],
            speed_heatmap: vec![(Point::new(0, 0), 5), (Point::new(1, 0), 5)],
            ..Default::default()
        };
        assert!(sink_indices(&metrics).contains(&0));
    }

    #[test]
    fn sink_indices_is_empty_on_an_empty_path() {
        let metrics = TrackMetrics::default();
        assert!(sink_indices(&metrics).is_empty());
    }

    #[test]
    fn sink_indices_includes_low_heatmap_points() {
        let metrics = TrackMetrics {
            fastest_lap: vec![Point::new(0, 0), Point::new(1, 0), Point::new(2, 0)],
            speed_heatmap: vec![
                (Point::new(0, 0), 5),
                (Point::new(1, 0), 1),
                (Point::new(2, 0), 5),
            ],
            ..Default::default()
        };
        let sinks = sink_indices(&metrics);
        assert!(sinks.contains(&0)); // unconditional
        assert!(sinks.contains(&1)); // heatmap <= 1
        assert!(!sinks.contains(&2));
    }

    // ---- travel_dir ----------------------------------------------------

    #[test]
    fn travel_dir_on_a_diagonal_step_picks_the_dominant_axis_ties_to_x() {
        // Equal |dx| and |dy| -> tie -> x.
        let path = [Point::new(0, 0), Point::new(2, 2)];
        assert_eq!(travel_dir(&path, 0), (1, 0));

        // |dy| > |dx| -> y wins.
        let path = [Point::new(0, 0), Point::new(1, 3)];
        assert_eq!(travel_dir(&path, 0), (0, 1));
    }

    #[test]
    fn travel_dir_at_the_last_index_uses_the_backward_step() {
        let path = [Point::new(0, 0), Point::new(3, 0)];
        assert_eq!(travel_dir(&path, 1), (1, 0));
    }

    #[test]
    fn travel_dir_is_total_on_out_of_range_and_singleton_paths() {
        assert_eq!(travel_dir(&[], 0), (0, 0));
        assert_eq!(travel_dir(&[Point::new(0, 0)], 0), (0, 0));
        assert_eq!(travel_dir(&[Point::new(0, 0)], 5), (0, 0));
    }

    // ---- end_of_ray / runout_room ---------------------------------------

    #[test]
    fn end_of_ray_walks_to_the_last_drivable_cell() {
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        assert_eq!(end_of_ray(&d, Point::new(0, 0), (1, 0)), Point::new(4, 0));
    }

    #[test]
    fn end_of_ray_is_total_on_zero_direction_and_off_d_start() {
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        assert_eq!(end_of_ray(&d, Point::new(2, 0), (0, 0)), Point::new(2, 0));
        assert_eq!(end_of_ray(&d, Point::new(9, 9), (1, 0)), Point::new(9, 9));
    }

    #[test]
    fn runout_room_is_wall_run_minus_one() {
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        // wall_run(d, (0,0), east) = 5 (cells 0..=4) -> runout_room = 4.
        assert_eq!(runout_room(&d, Point::new(0, 0), (1, 0)), 4);
    }

    // ---- window_speed / corner_speed ------------------------------------

    #[test]
    fn window_speed_records_but_does_not_expand_a_barrier_successor() {
        // v_ceil=1 confines every move to a single-cell cardinal step (no
        // multi-cell jump can skip over the barrier cell without landing on
        // it), so this genuinely exercises the "cut" semantics rather than
        // an accel-driven overshoot the barrier check cannot see (only the
        // *landing* cell is checked, not the swept cells in between).
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        let barriers = HashSet::from([Point::new(2, 0)]);
        let flood = window_speed(&d, &[car(0, 0, 0, 0)], &barriers, 1);

        // (2,0) is reached (recorded)...
        assert!(flood.states.iter().any(|s| s.pos() == Point::new(2, 0)));
        // ...but not expanded past: no state at (3,0) or beyond.
        assert!(!flood.states.iter().any(|s| s.pos().x > 2));
    }

    #[test]
    fn window_speed_seed_exemption_expands_a_seed_whose_own_cell_is_a_barrier() {
        // R13: the sink-seeded detection call's seed cell IS itself a
        // barrier; without the exemption the flood would be empty.
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        let seed = car(0, 0, 0, 0);
        let barriers = HashSet::from([Point::new(0, 0)]);
        let flood = window_speed(&d, &[seed], &barriers, 3);

        assert!(flood.states.contains(&seed));
        // Expansion happened: at least one state beyond the seed cell.
        assert!(flood.states.iter().any(|s| s.pos().x > 0));
    }

    #[test]
    fn corner_speed_excludes_an_arrival_with_no_legal_successor() {
        // A 1-wide dead end at x=2: the fastest arrival there (highest vnorm)
        // has no legal successor (every move overruns off-D), but a slow
        // arrival that can brake to rest in place does.
        let d = Corridor::filled(Point::new(0, 0), 3, 1);
        let barriers = HashSet::new();
        let flood = window_speed(&d, &[car(0, 0, 1, 0)], &barriers, 3);
        let end = Point::new(2, 0);

        let fastest_at_end = flood
            .states
            .iter()
            .filter(|s| s.pos() == end)
            .map(|s| vnorm(*s))
            .max()
            .unwrap_or(0);
        let qualifying = corner_speed(&d, &flood, end);
        assert!(
            qualifying <= fastest_at_end,
            "corner_speed must never exceed the raw peak at end"
        );
    }

    #[test]
    fn corner_speed_returns_zero_when_no_state_reaches_end() {
        let d = Corridor::filled(Point::new(0, 0), 3, 1);
        let flood = window_speed(&d, &[car(0, 0, 0, 0)], &HashSet::new(), 3);
        assert_eq!(corner_speed(&d, &flood, Point::new(99, 99)), 0);
    }

    // ---- deficit_at ------------------------------------------------------

    #[test]
    fn deficit_at_never_panics_on_a_cell_absent_from_the_flood() {
        // A barrier truncates the flood before it ever reaches c; peak.get
        // returns None, defaulting attainable to 0 (never `peak[c]`).
        let d = Corridor::filled(Point::new(0, 0), 10, 1);
        let barriers = HashSet::from([Point::new(2, 0)]);
        let flood = window_speed(&d, &[car(0, 0, 1, 0)], &barriers, 3);
        // c = (5,0) is beyond the barrier at (2,0) -> absent from flood.peak.
        let _ = deficit_at(&d, &flood, Point::new(5, 0), (1, 0), 3);
    }
}
