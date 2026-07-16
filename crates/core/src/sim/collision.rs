//! Same-final-cell collision resolution (design doc §3, product-owner
//! amendment 2026-07-16) — a layer *outside* movement physics.

use crate::geom::{Corridor, CorridorScratch, Point};
use crate::sim::CarState;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

/// Resolves cars that end a turn on the **same final cell** (`A.pos == B.pos`).
///
/// This is the only conflict predicate. Cars whose move-segments swap, thread
/// mid-segment, or cross orthogonally but **end on distinct cells** are left
/// **unchanged**; there is no swap/pass-through detector (design doc §3
/// [D1]/[N2], product-owner amendment 2026-07-16).
///
/// # Algorithm
///
/// 1. **Group by final cell.** Bucket car indices into a
///    `HashMap<Point, Vec<usize>>` by iterating `cars` in index order, so each
///    group's `Vec` is ascending-index-ordered and `group[0]` is the group's
///    min (first-appearance) car index.
/// 2. **Winner.** For every group, one car stays at its cell (the winner);
///    every other car in the group (a loser) is displaced to the nearest free
///    cell by in-`D` geodesic BFS (4-connected). Displacement is a teleport:
///    velocity is retained (`vx`/`vy` untouched — zeroing it would revive the
///    "ram the pack to brake" abuse), no supercover check, no lap-counter
///    change.
/// 3. **Exhaustion.** If BFS returns `None` (the car's cell is outside `D`, or
///    the reachable component is fully packed), the car stays at its current
///    cell, unchanged.
///
/// # Determinism contract (AC3/AC7)
///
/// Given the same `d`, `cars`, and `seed`, the output is byte-identical
/// (including across 32-/64-bit targets). This holds because of two fixed
/// rules that MUST NOT drift:
///
/// - **(a) Canonical pre-shuffle group order.** `HashMap` iteration order is
///   *not* used for anything RNG-sensitive: the buckets are materialized into
///   a `Vec<Vec<usize>>` and sorted with
///   `groups.sort_unstable_by_key(|g| g[0])` — the unique min car index —
///   *before* any shuffle, giving a canonical, RNG-independent group order.
/// - **(b) Fixed RNG-consumption order.** A single
///   `ChaCha8Rng::seed_from_u64(seed)` is built, then consumed in exactly
///   this sequence: `groups.shuffle(&mut rng)` (picks which group is
///   processed first), then for each group in that shuffled order,
///   `group.shuffle(&mut rng)` (picks the winner = post-shuffle index 0, and
///   the displacement order of the rest), then for each loser (in that
///   order) a `u32` tie draw via `rng.random_range` — **only** when the
///   nearest-free BFS layer has more than one candidate cell
///   (`free.len() > 1`).
///
/// The tie index is drawn as `u32` (matching `rand`'s own slice-index
/// policy for `shuffle`), so the pick is reproducible across 32-/64-bit
/// targets.
///
/// # Panics
///
/// Panics if a single BFS layer's free-cell count exceeds `u32::MAX` — not
/// reachable in practice, since a layer is bounded by `d`'s cell count.
pub fn resolve_collisions(d: &Corridor, cars: &mut [CarState], seed: u64) {
    let mut buckets: HashMap<Point, Vec<usize>> = HashMap::new();
    for (i, car) in cars.iter().enumerate() {
        buckets.entry(car.pos()).or_default().push(i);
    }
    let mut groups: Vec<Vec<usize>> = buckets.into_values().collect();
    groups.sort_unstable_by_key(|g| g[0]);

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    groups.shuffle(&mut rng);
    for group in &mut groups {
        group.shuffle(&mut rng);
    }

    // Phase 1 — winners (no RNG): the post-shuffle first car of every group
    // stays; its cell is occupied.
    let mut occupied: HashSet<Point> = HashSet::new();
    for group in &groups {
        occupied.insert(cars[group[0]].pos());
    }

    // Phase 2 — losers (single linear pass): nearest free cell by geodesic
    // BFS, reusing one scratch buffer across every query.
    let mut scratch = CorridorScratch::new(d);
    for group in &groups {
        for &i in &group[1..] {
            let pos_i = cars[i].pos();
            let target = scratch.geodesic_bfs(d, pos_i, |_dist, layer| {
                let free: Vec<Point> = layer
                    .iter()
                    .copied()
                    .filter(|c| !occupied.contains(c))
                    .collect();
                if free.is_empty() {
                    ControlFlow::Continue(())
                } else {
                    ControlFlow::Break(free)
                }
            });
            if let Some(free) = target {
                let chosen = if free.len() == 1 {
                    free[0]
                } else {
                    let idx = rng.random_range(
                        0..u32::try_from(free.len()).expect("layer <= area fits u32"),
                    );
                    free[idx as usize]
                };
                cars[i].x = chosen.x;
                cars[i].y = chosen.y;
                occupied.insert(chosen);
            }
            // Exhaustion (`None`): car stays at `pos_i`, unchanged.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car at rest at `(x, y)` with velocity `(vx, vy)`.
    fn car(x: i32, y: i32, vx: i32, vy: i32) -> CarState {
        CarState { x, y, vx, vy }
    }

    /// A lint-clean fully-drivable rectangle `w × h` at origin `(0,0)`.
    fn filled(w: usize, h: usize) -> Corridor {
        let mut d = Corridor::new(Point::new(0, 0), w, h);
        for y in 0..i32::try_from(h).unwrap() {
            for x in 0..i32::try_from(w).unwrap() {
                d.set(Point::new(x, y), true);
            }
        }
        d
    }

    #[test]
    fn ac1_singleton_unchanged() {
        let d = filled(5, 5);
        let mut cars = [car(2, 2, 1, 0)];
        let before = cars;
        resolve_collisions(&d, &mut cars, 7);
        assert_eq!(cars, before);
    }

    #[test]
    fn ac8_three_into_one_cell_resolved_distinctly() {
        let d = filled(5, 5);
        let mut cars = [car(2, 2, 1, 0), car(2, 2, 0, 1), car(2, 2, -1, 0)];
        resolve_collisions(&d, &mut cars, 7);
        // Exactly one car remains at (2,2); the other two are on distinct
        // free cells.
        let positions: Vec<Point> = cars.iter().map(|c| c.pos()).collect();
        let unique: HashSet<Point> = positions.iter().copied().collect();
        assert_eq!(
            unique.len(),
            3,
            "all three cars must land on distinct cells: {positions:?}"
        );
        assert!(positions.contains(&Point::new(2, 2)));
        // Velocities are untouched by the teleport (AC4).
        assert_eq!(cars[0].vx, 1);
        assert_eq!(cars[0].vy, 0);
        assert_eq!(cars[1].vx, 0);
        assert_eq!(cars[1].vy, 1);
        assert_eq!(cars[2].vx, -1);
        assert_eq!(cars[2].vy, 0);
    }

    #[test]
    fn ac2_displaced_into_occupied_ring_lands_on_first_free_layer() {
        let d = filled(5, 5);
        // Two cars collide at (2,2); its immediate 4-conn ring is fully
        // occupied by other (non-colliding) cars, so the loser must reach
        // past the ring to the first free layer.
        let mut cars = [
            car(2, 2, 0, 0),
            car(2, 2, 0, 0),
            car(1, 2, 0, 0),
            car(3, 2, 0, 0),
            car(2, 1, 0, 0),
            car(2, 3, 0, 0),
        ];
        resolve_collisions(&d, &mut cars, 11);
        let ring = [
            Point::new(1, 2),
            Point::new(3, 2),
            Point::new(2, 1),
            Point::new(2, 3),
        ];
        let positions: Vec<Point> = cars.iter().map(|c| c.pos()).collect();
        let unique: HashSet<Point> = positions.iter().copied().collect();
        assert_eq!(
            unique.len(),
            6,
            "all six cars must land on distinct cells: {positions:?}"
        );
        let collider_positions = [positions[0], positions[1]];
        assert!(collider_positions.contains(&Point::new(2, 2)));
        for p in collider_positions {
            if p != Point::new(2, 2) {
                assert!(
                    !ring.contains(&p),
                    "displaced car must skip the fully-occupied ring, got {p:?}"
                );
            }
        }
    }

    #[test]
    fn ac5_swap_ending_apart_left_unchanged() {
        let d = filled(5, 5);
        let mut cars = [car(1, 2, 1, 0), car(2, 2, -1, 0)];
        let before = cars;
        resolve_collisions(&d, &mut cars, 3);
        assert_eq!(cars, before);
    }

    #[test]
    fn ac5_thread_ending_apart_left_unchanged() {
        let d = filled(5, 5);
        // Opposing multi-cell moves with overlapping supercovers but
        // distinct final cells: A ends at (2,2), B ends at (0,2).
        let mut cars = [car(0, 2, 2, 0), car(2, 2, -2, 0)];
        let before = cars;
        resolve_collisions(&d, &mut cars, 3);
        assert_eq!(cars, before);
    }

    #[test]
    fn ac7_repeated_calls_are_byte_identical() {
        let d = filled(5, 5);
        let mut cars_a = [car(2, 2, 1, 0), car(2, 2, 0, 1), car(2, 2, -1, 0)];
        let mut cars_b = cars_a;
        resolve_collisions(&d, &mut cars_a, 99);
        resolve_collisions(&d, &mut cars_b, 99);
        assert_eq!(cars_a, cars_b);
    }

    #[test]
    fn ac3_equidistant_seeded_pick_is_exact_and_stable() {
        // A symmetric fixture: two colliding cars at the center of a 5x5
        // filled corridor, both immediate ring cells free and equidistant —
        // a genuine intra-layer tie the seeded RNG must break reproducibly.
        let d = filled(5, 5);
        let mut cars = [car(2, 2, 0, 0), car(2, 2, 0, 0)];
        resolve_collisions(&d, &mut cars, 42);
        // Seed 42 deterministically picks (2,3) among the equidistant free
        // ring cells {(1,2), (3,2), (2,1), (2,3)}.
        assert_eq!(cars, [car(2, 2, 0, 0), car(2, 3, 0, 0)]);

        // A second independent run with the same seed reproduces exactly.
        let mut cars2 = [car(2, 2, 0, 0), car(2, 2, 0, 0)];
        resolve_collisions(&d, &mut cars2, 42);
        assert_eq!(cars, cars2, "same seed must reproduce the same tie pick");
    }
}
