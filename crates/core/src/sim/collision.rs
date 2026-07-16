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
