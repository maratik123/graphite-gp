//! Ф2 — rasterize the coarse-block ring to the fine lattice corridor `D` with
//! width taper (design doc §2, `phase2_rasterize`).
//!
//! Ф2 turns Ф1's [`CoarseSkeleton`] into `D` in three deterministic stages:
//! Stage 1 expands each coarse `ring` cell into a solid `k×k` fine block
//! (baseline `D0`), Stage 2 tapers every abrupt outfield wall step into a
//! 1-Lipschitz ramp, and Stage 3 (test-only) certifies the by-construction
//! topology/width invariants. No RNG, integer-only, panic-free — mirroring
//! Ф1's `saturating_*` / `try_from(..).unwrap_or(..)` discipline.

use std::collections::{BTreeSet, HashSet};

use gp_core::geom::{Corridor, Point, Side};
use strum::IntoEnumIterator;

use crate::CoarseSkeleton;

/// Fine-cell padding around `D0`'s bounding box, on every side — gives every
/// `D0` boundary cell an in-box `¬D` neighbor for `walls_from_boundary` /
/// complement-flood, mirroring Ф1's `corridor_from_cells(.., 1)`. The taper
/// needs no extra margin (design doc Note 4): it only fills cells already
/// inside `D0`'s bounding box.
const BBOX_PAD: i32 = 1;

/// The 1-Lipschitz taper slope: an outfield wall may advance by at most this
/// many fine points per column/row (design doc Key Decisions — meets AC4's
/// hard `≤1 point/column` invariant exactly).
const TAPER_SLOPE: i32 = 1;

/// Rasterizes `skel`'s coarse ring/hole into the fine lattice corridor `D`
/// (design doc §2 Ф2): Stage 1 baseline `k×k` expansion, then Stage 2 taper.
///
/// `n` (the width floor) is not read by production geometry (round-1 has no
/// narrow carve, design doc Key Decisions) — it is retained in the signature
/// and guarded by `debug_assert!(n <= k)`, documenting the invariant the
/// round-1 carve would consume.
pub fn phase2_rasterize(skel: &CoarseSkeleton, k: i32, n: i32) -> Corridor {
    debug_assert!(n <= k, "n (min width) must not exceed k (block size)");
    let (mut d, h) = stage1_baseline(&skel.ring, &skel.hole, k);
    stage2_taper(&mut d, &h);
    stage2b_absorb_pockets(&mut d, &h);
    d
}

// ---- Stage 1: baseline k×k expansion (D0) + expanded hole mask (H) -------

/// The fine-point origin of coarse block `c`'s `k×k` patch — `(c.x·k, c.y·k)`.
const fn block_origin(c: Point, k: i32) -> Point {
    Point::new(c.x.saturating_mul(k), c.y.saturating_mul(k))
}

/// Every fine point of coarse block `c`'s `k×k` patch, row-major.
fn block_points(c: Point, k: i32) -> impl Iterator<Item = Point> {
    let origin = block_origin(c, k);
    (0..k).flat_map(move |dy| {
        (0..k).map(move |dx| Point::new(origin.x.saturating_add(dx), origin.y.saturating_add(dy)))
    })
}

/// An empty [`Corridor`] over `ring`'s coarse bounding box, scaled by `k` and
/// padded by [`BBOX_PAD`] fine cells on every side. An empty `ring` yields a
/// `1×1` corridor at the origin — total, no panic (mirrors Ф1's
/// `corridor_from_cells`).
fn corridor_for_ring(ring: &BTreeSet<Point>, k: i32) -> Corridor {
    let Some(ring_min_x) = ring.iter().map(|p| p.x).min() else {
        return Corridor::new(Point::new(0, 0), 1, 1);
    };
    let ring_max_x = ring.iter().map(|p| p.x).max().unwrap_or(ring_min_x);
    let ring_min_y = ring.iter().map(|p| p.y).min().unwrap_or(ring_min_x);
    let ring_max_y = ring.iter().map(|p| p.y).max().unwrap_or(ring_min_y);

    let fine_min_x = ring_min_x.saturating_mul(k).saturating_sub(BBOX_PAD);
    let fine_min_y = ring_min_y.saturating_mul(k).saturating_sub(BBOX_PAD);
    let fine_max_x = ring_max_x
        .saturating_add(1)
        .saturating_mul(k)
        .saturating_add(BBOX_PAD);
    let fine_max_y = ring_max_y
        .saturating_add(1)
        .saturating_mul(k)
        .saturating_add(BBOX_PAD);

    let width = usize::try_from(fine_max_x.saturating_sub(fine_min_x)).unwrap_or(1);
    let height = usize::try_from(fine_max_y.saturating_sub(fine_min_y)).unwrap_or(1);

    Corridor::new(
        Point::new(fine_min_x, fine_min_y),
        width.max(1),
        height.max(1),
    )
}

/// Stage 1 (design doc §2 Ф2): the baseline `k×k` expansion `D0` (every
/// `ring` cell mapped to a solid `k×k` fine block) plus the expanded hole
/// mask `H` (every `hole` cell mapped the same way) that Stage 2 uses to
/// protect the infield. Satisfies AC1 (no ring cell dropped) and AC2-baseline
/// (an adjacent-block union is `≥2k` wide) by construction.
fn stage1_baseline(
    ring: &BTreeSet<Point>,
    hole: &BTreeSet<Point>,
    k: i32,
) -> (Corridor, BTreeSet<Point>) {
    let mut d = corridor_for_ring(ring, k);
    for &c in ring {
        for p in block_points(c, k) {
            d.set(p, true);
        }
    }
    let h: BTreeSet<Point> = hole.iter().flat_map(|&c| block_points(c, k)).collect();
    (d, h)
}

// ---- Stage 2: outer-wall taper --------------------------------------------

/// The minimal 1-Lipschitz field `≥ tops` (two linear passes, design doc
/// §"Taper"): `env[i] = max(tops[i], env[i∓1] − TAPER_SLOPE)` forward then
/// backward. Entries with no local `top` (`None`) contribute nothing of
/// their own — they still receive propagation from Lipschitz-adjacent
/// entries, which is exactly the desired "no local constraint" behavior.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "the forward loop ranges 1..env.len() so i - 1 never underflows, and the \
              backward loop ranges 0..env.len()-1 (rev) so i + 1 stays < env.len() — \
              both index offsets are bounded by the loop ranges immediately enclosing them"
)]
fn one_lipschitz_envelope(tops: &[Option<i32>]) -> Vec<i32> {
    /// Sentinel for "no local top" — far below any real coordinate, safe
    /// under `saturating_sub` by [`TAPER_SLOPE`] without wrapping toward
    /// [`i32::MAX`].
    const NEG_INF: i32 = i32::MIN / 2;
    let mut env: Vec<i32> = tops.iter().map(|t| t.unwrap_or(NEG_INF)).collect();
    for i in 1..env.len() {
        env[i] = env[i].max(env[i - 1].saturating_sub(TAPER_SLOPE));
    }
    for i in (0..env.len().saturating_sub(1)).rev() {
        env[i] = env[i].max(env[i + 1].saturating_sub(TAPER_SLOPE));
    }
    env
}

/// The extremal (max if `sign > 0`, min if `sign < 0`) `x` with `d.contains((x,
/// y))`, or `None` if the row has no drivable cell.
fn row_extent(d: &Corridor, y: i32, sign: i32) -> Option<i32> {
    let origin = d.origin();
    let w = i32::try_from(d.width()).unwrap_or(0);
    if sign > 0 {
        (origin.x..origin.x.saturating_add(w))
            .rev()
            .find(|&x| d.contains(Point::new(x, y)))
    } else {
        (origin.x..origin.x.saturating_add(w)).find(|&x| d.contains(Point::new(x, y)))
    }
}

/// The extremal (max if `sign > 0`, min if `sign < 0`) `y` with `d.contains((x,
/// y))`, or `None` if the column has no drivable cell.
fn col_extent(d: &Corridor, x: i32, sign: i32) -> Option<i32> {
    let origin = d.origin();
    let hgt = i32::try_from(d.height()).unwrap_or(0);
    if sign > 0 {
        (origin.y..origin.y.saturating_add(hgt))
            .rev()
            .find(|&y| d.contains(Point::new(x, y)))
    } else {
        (origin.y..origin.y.saturating_add(hgt)).find(|&y| d.contains(Point::new(x, y)))
    }
}

/// Fills `d` outward from `start` (exclusive) to `target` (inclusive) along
/// `sign`'s direction on the varying-perpendicular-fixed coordinate, skipping
/// (and stopping at) any cell in `h` — additive, hole-safe either way. `make`
/// builds the candidate [`Point`] from the walking coordinate.
fn fill_outward(
    d: &mut Corridor,
    h: &BTreeSet<Point>,
    start: i32,
    target: i32,
    sign: i32,
    make: impl Fn(i32) -> Point,
) {
    let mut v = start.saturating_add(sign);
    loop {
        if (sign > 0 && v > target) || (sign < 0 && v < target) {
            break;
        }
        let p = make(v);
        if h.contains(&p) {
            break;
        }
        d.set(p, true);
        v = v.saturating_add(sign);
    }
}

/// One directional taper pass (design doc §"Taper"): tapers `d`'s
/// outfield-facing wall on `side` into a 1-Lipschitz ramp, additively, never
/// touching `h`.
fn taper_pass(d: &mut Corridor, h: &BTreeSet<Point>, side: Side) {
    let (dx, dy) = side.delta();
    let origin = d.origin();
    match side {
        Side::East | Side::West => {
            let sign = dx;
            let rows = d.height();
            let mut existing: Vec<Option<i32>> = Vec::with_capacity(rows);
            let mut tops: Vec<Option<i32>> = Vec::with_capacity(rows);
            for ry in 0..rows {
                let y = origin
                    .y
                    .saturating_add(i32::try_from(ry).unwrap_or(i32::MAX));
                let ex = row_extent(d, y, sign);
                existing.push(ex);
                let top = ex.and_then(|x| {
                    let neighbor = Point::new(x.saturating_add(sign), y);
                    (!h.contains(&neighbor)).then_some(x.saturating_mul(sign))
                });
                tops.push(top);
            }
            let env = one_lipschitz_envelope(&tops);
            for (ry, ex) in existing.into_iter().enumerate() {
                let Some(ex) = ex else { continue };
                let y = origin
                    .y
                    .saturating_add(i32::try_from(ry).unwrap_or(i32::MAX));
                let target = env[ry].saturating_mul(sign);
                fill_outward(d, h, ex, target, sign, move |x| Point::new(x, y));
            }
        }
        Side::North | Side::South => {
            let sign = dy;
            let cols = d.width();
            let mut existing: Vec<Option<i32>> = Vec::with_capacity(cols);
            let mut tops: Vec<Option<i32>> = Vec::with_capacity(cols);
            for rx in 0..cols {
                let x = origin
                    .x
                    .saturating_add(i32::try_from(rx).unwrap_or(i32::MAX));
                let ex = col_extent(d, x, sign);
                existing.push(ex);
                let top = ex.and_then(|y| {
                    let neighbor = Point::new(x, y.saturating_add(sign));
                    (!h.contains(&neighbor)).then_some(y.saturating_mul(sign))
                });
                tops.push(top);
            }
            let env = one_lipschitz_envelope(&tops);
            for (rx, ex) in existing.into_iter().enumerate() {
                let Some(ex) = ex else { continue };
                let x = origin
                    .x
                    .saturating_add(i32::try_from(rx).unwrap_or(i32::MAX));
                let target = env[rx].saturating_mul(sign);
                fill_outward(d, h, ex, target, sign, move |y| Point::new(x, y));
            }
        }
    }
}

/// Stage 2 (design doc §2 Ф2): four directional outfield passes, in
/// [`Side::iter()`] order (East, West, North, South) for determinism (AC6).
/// Additive and hole-safe by construction (design doc Risks) — satisfies AC4.
fn stage2_taper(d: &mut Corridor, h: &BTreeSet<Point>) {
    for side in Side::iter() {
        taper_pass(d, h, side);
    }
}

// ---- Stage 2b: pocket absorption (topology-safe closure) -----------------

/// Whether `p` lies within `d`'s own bounding box (fine coordinates).
fn in_box(d: &Corridor, p: Point) -> bool {
    let origin = d.origin();
    let w = i32::try_from(d.width()).unwrap_or(0);
    let hgt = i32::try_from(d.height()).unwrap_or(0);
    p.x >= origin.x
        && p.x < origin.x.saturating_add(w)
        && p.y >= origin.y
        && p.y < origin.y.saturating_add(hgt)
}

/// Whether `p` lies on `d`'s bounding-box border (mirrors Ф1's `on_border`,
/// `phase1.rs:388`) — a component touching the border is, by definition, the
/// unbounded outfield, never absorbed.
fn on_box_border(d: &Corridor, p: Point) -> bool {
    let origin = d.origin();
    let w = i32::try_from(d.width()).unwrap_or(0);
    let hgt = i32::try_from(d.height()).unwrap_or(0);
    p.x == origin.x
        || p.y == origin.y
        || p.x.saturating_add(1) == origin.x.saturating_add(w)
        || p.y.saturating_add(1) == origin.y.saturating_add(hgt)
}

/// Stage 2b (design doc §"Stage 2b — pocket absorption"): enumerates every
/// **bounded** 4-connected component of `d`'s complement inside `d`'s own
/// bounding box (mirrors Ф1's `fill_holes` flood, `phase1.rs:402`) and
/// absorbs (sets drivable) every one **disjoint from `h`** — i.e. every
/// component that is not the legitimate hole. `H`'s own component is never
/// absorbed because it shares at least one cell with `h` by construction (2a
/// never fills `H`, so it survives 2a intact). Fixed row-major seed order —
/// deterministic, no RNG. Additive only (only ever sets cells drivable) and
/// `O(cells)` (one flood + one `h`-membership check per component cell).
fn stage2b_absorb_pockets(d: &mut Corridor, h: &BTreeSet<Point>) {
    let origin = d.origin();
    let w = i32::try_from(d.width()).unwrap_or(0);
    let hgt = i32::try_from(d.height()).unwrap_or(0);
    let mut visited: HashSet<Point> = HashSet::new();
    for by in 0..hgt {
        for bx in 0..w {
            let seed = Point::new(origin.x.saturating_add(bx), origin.y.saturating_add(by));
            if d.contains(seed) || !visited.insert(seed) {
                continue;
            }
            let mut component = Vec::new();
            let mut touches_border = false;
            let mut touches_h = false;
            let mut stack = vec![seed];
            while let Some(cur) = stack.pop() {
                component.push(cur);
                if on_box_border(d, cur) {
                    touches_border = true;
                }
                if h.contains(&cur) {
                    touches_h = true;
                }
                for n in cur.neighbors4() {
                    if in_box(d, n) && !d.contains(n) && visited.insert(n) {
                        stack.push(n);
                    }
                }
            }
            if !touches_border && !touches_h {
                for p in component {
                    d.set(p, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::{Orient, bounded_complement_components, component_count, supercover};
    use gp_core::track::RaceDir;
    use rand::SeedableRng;
    use rand_xoshiro::Xoshiro256PlusPlus;

    fn rng(seed: u64) -> Xoshiro256PlusPlus {
        Xoshiro256PlusPlus::seed_from_u64(seed)
    }

    /// The block-size the fixture uses (`k ≥ n`).
    const FIXTURE_K: i32 = 3;
    /// The min-width the fixture uses (`n = ⌈m/2⌉`, `m = 4`).
    const FIXTURE_N: i32 = 2;

    /// A hand-built [`CoarseSkeleton`] with a known widen-jog: a 3×3-coarse
    /// hole surrounded by a 1-thick ring, widened by one extra coarse cell on
    /// the East side for `y ∈ {0, 1}` only — a deterministic `k→2k`
    /// transition on part of the East wall's run (design doc Test Design).
    fn fixture_jog() -> CoarseSkeleton {
        let mut hole = BTreeSet::new();
        for y in 0..3 {
            for x in 0..3 {
                hole.insert(Point::new(x, y));
            }
        }
        let mut ring = BTreeSet::new();
        for y in -1..=3 {
            for x in -1..=3 {
                let p = Point::new(x, y);
                if !hole.contains(&p) {
                    ring.insert(p);
                }
            }
        }
        // Partial widen: East side, one extra coarse cell, for y in {0, 1}.
        ring.insert(Point::new(4, 0));
        ring.insert(Point::new(4, 1));
        CoarseSkeleton {
            ring,
            hole,
            dir: RaceDir::Cw,
        }
    }

    #[test]
    fn ac1_every_ring_cell_becomes_a_solid_k_by_k_block() {
        let skel = fixture_jog();
        let (d, _h) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        for &c in &skel.ring {
            for p in block_points(c, FIXTURE_K) {
                assert!(
                    d.contains(p),
                    "ring cell {c:?}'s block point {p:?} not drivable"
                );
            }
        }
    }

    #[test]
    fn ac1_drivable_count_matches_ring_cell_count_times_k_squared() {
        // No ring cell dropped: pre-taper drivable count == |ring| * k^2
        // (blocks never overlap for a 1-thick-or-wider annulus ring; no two
        // distinct coarse cells share a fine point).
        let skel = fixture_jog();
        let (d, _h) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        let expected = skel.ring.len() * usize::try_from(FIXTURE_K).unwrap_or(0).pow(2);
        assert_eq!(d.len(), expected);
    }

    #[test]
    fn ac2_baseline_two_coarse_cell_thick_section_is_at_least_2k_wide() {
        let skel = fixture_jog();
        let (d, _h) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        // Row y=1 (coarse) sits inside the widened section: ring cells at
        // cx = 3 and cx = 4, contiguous, so the fine row through it is >= 2k.
        let y = FIXTURE_K; // first fine row of coarse row y=1
        let run = row_run_length(&d, y, 9); // scan starting near the east arm
        assert!(run >= 2 * FIXTURE_K as usize, "run {run} < 2k");
    }

    #[test]
    fn ac6_baseline_two_calls_on_identical_input_are_byte_identical() {
        let skel = fixture_jog();
        let (d1, h1) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        let (d2, h2) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        assert_eq!(h1, h2);
        assert_eq!(drivable_points(&d1), drivable_points(&d2));
    }

    #[test]
    fn phase2_rasterize_debug_asserts_n_le_k() {
        // n <= k holds for the fixture; a direct call should not panic.
        let skel = fixture_jog();
        let _ = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
    }

    // ---- Subtask 2: Stage-2 outer-wall taper -------------------------------

    /// The documented "several columns" floor for a taper run (design doc Key
    /// Decisions) — test-only threshold.
    const MIN_TAPER_RUN: i32 = 3;

    /// The nominal (unwidened) East-wall fine `x` for [`fixture_jog`]: the
    /// ring's unwidened East coarse column is `3`, so the fine block edge is
    /// `(3 + 1) * k - 1`.
    const FIXTURE_NOMINAL_EAST_X: i32 = 4 * FIXTURE_K - 1;

    #[test]
    fn ac4_tapered_east_wall_advances_at_most_one_point_per_row() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        let profile = east_wall_profile(&d);
        assert!(
            profile.len() >= 2,
            "expected a multi-row east wall profile, got {profile:?}"
        );
        for w in profile.windows(2) {
            let (y0, x0) = w[0];
            let (y1, x1) = w[1];
            assert_eq!(
                y1,
                y0.saturating_add(1),
                "profile must be over contiguous rows"
            );
            let step = x1.abs_diff(x0);
            assert!(
                step <= 1,
                "east wall advanced by {step} between rows {y0} and {y1} (x {x0} -> {x1})"
            );
        }
    }

    #[test]
    fn ac4_widen_jog_delta_spans_at_least_min_taper_run_rows() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        let profile = east_wall_profile(&d);
        let min_x = profile.iter().map(|&(_, x)| x).min().unwrap_or(0);
        let max_x = profile.iter().map(|&(_, x)| x).max().unwrap_or(0);
        // The widen jog is a real Δ=k step (not erased/flattened): the
        // 1-Lipschitz property (previous test) already forces the walk from
        // min_x to max_x to span >= delta row-steps.
        let delta = max_x.abs_diff(min_x);
        let fixture_k = u32::try_from(FIXTURE_K).unwrap_or(0);
        let min_taper_run = u32::try_from(MIN_TAPER_RUN).unwrap_or(0);
        assert!(delta >= fixture_k, "expected a >= k jog, got delta {delta}");
        assert!(
            delta >= min_taper_run,
            "jog delta {delta} smaller than the MIN_TAPER_RUN floor"
        );
    }

    #[test]
    fn ac4_nominal_rows_far_from_the_jog_keep_width_k() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        // The extreme south content row (first fine row of coarse ring row
        // -1) and the extreme north content row (last fine row of coarse
        // ring row 3) sit at the corridor's box edges — no taper pass can
        // reach past D0's own bounding box (design doc Note 4) — so both
        // stay at the unwidened nominal East wall position.
        let south_y = (-1i32).saturating_mul(FIXTURE_K);
        let north_y = 4i32.saturating_mul(FIXTURE_K).saturating_sub(1);
        assert_eq!(row_extent(&d, south_y, 1), Some(FIXTURE_NOMINAL_EAST_X));
        assert_eq!(row_extent(&d, north_y, 1), Some(FIXTURE_NOMINAL_EAST_X));
    }

    #[test]
    fn taper_is_additive_d0_subset_of_final_d() {
        let skel = fixture_jog();
        let (d0, _h) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        let d_final = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        for p in drivable_points(&d0) {
            assert!(d_final.contains(p), "D0 point {p:?} missing from final D");
        }
    }

    #[test]
    fn taper_never_makes_a_hole_cell_drivable() {
        let skel = fixture_jog();
        let (_d0, h) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        let d_final = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        for &p in &h {
            assert!(!d_final.contains(p), "hole cell {p:?} became drivable");
        }
    }

    #[test]
    fn taper_is_deterministic_across_repeated_calls() {
        let skel = fixture_jog();
        let a = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        let b = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        assert_eq!(drivable_points(&a), drivable_points(&b));
    }

    // ---- Subtask 2b: Stage-2b pocket absorption ----------------------------

    /// `l_min` the [`fn@crate::phase1_coarse_ring`] witness below runs at.
    const POCKET_WITNESS_L_MIN: i32 = 3;
    /// `k` the pocket-absorption witness runs at.
    const POCKET_WITNESS_K: i32 = 6;
    /// `n` the pocket-absorption witness runs at.
    const POCKET_WITNESS_N: i32 = 3;

    /// The design doc's Stage-2b witness (design doc §"Stage 2b" / Risks):
    /// `phase1_coarse_ring(3, rng(0))` at `k=6, n=3` produces a Stage-2a
    /// output with a spurious 12-cell pocket (`bounded_complement_components
    /// == 2`); Stage 2b must restore `== 1`. Mirrors Ф1's
    /// `seed_48_lmin3_widen_pinch_stays_one_hole` regression-pin style
    /// (`phase1.rs`).
    #[test]
    fn seed_0_lmin3_stage2b_absorbs_the_spurious_pocket() {
        let skel = crate::phase1_coarse_ring(POCKET_WITNESS_L_MIN, &mut rng(0));
        let d = phase2_rasterize(&skel, POCKET_WITNESS_K, POCKET_WITNESS_N);
        assert_eq!(
            bounded_complement_components(&d),
            1,
            "stage 2b must absorb every bounded complement component except H"
        );
    }

    #[test]
    fn stage2b_absorption_is_additive_over_stage2a_output() {
        let skel = crate::phase1_coarse_ring(POCKET_WITNESS_L_MIN, &mut rng(0));
        let (mut envelope_only, h) = stage1_baseline(&skel.ring, &skel.hole, POCKET_WITNESS_K);
        stage2_taper(&mut envelope_only, &h);
        let envelope_points = drivable_points(&envelope_only);

        let mut absorbed = envelope_only.clone();
        stage2b_absorb_pockets(&mut absorbed, &h);

        for p in envelope_points {
            assert!(
                absorbed.contains(p),
                "2a point {p:?} missing after 2b absorption"
            );
        }
    }

    #[test]
    fn stage2b_never_absorbs_a_hole_cell() {
        let skel = crate::phase1_coarse_ring(POCKET_WITNESS_L_MIN, &mut rng(0));
        let (mut d, h) = stage1_baseline(&skel.ring, &skel.hole, POCKET_WITNESS_K);
        stage2_taper(&mut d, &h);
        stage2b_absorb_pockets(&mut d, &h);
        for &p in &h {
            assert!(!d.contains(p), "hole cell {p:?} became drivable via 2b");
        }
    }

    #[test]
    fn stage2b_absorption_is_deterministic_across_repeated_calls() {
        let skel = crate::phase1_coarse_ring(POCKET_WITNESS_L_MIN, &mut rng(0));
        let a = phase2_rasterize(&skel, POCKET_WITNESS_K, POCKET_WITNESS_N);
        let b = phase2_rasterize(&skel, POCKET_WITNESS_K, POCKET_WITNESS_N);
        assert_eq!(drivable_points(&a), drivable_points(&b));
    }

    // ---- Subtask 3: Stage-3 certification + test scaffolding ---------------

    /// The plausible-entry-speed bound for the AC5 Part B supercover check —
    /// derived from the fixture's own `k` (design doc Test Design), not
    /// hand-tuned.
    const V_ENTRY_CHECK: i32 = FIXTURE_K;

    #[test]
    fn stage3_topology_holds_on_fixture() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        assert_eq!(component_count(&d), 1, "D must be one connected component");
        assert_eq!(
            bounded_complement_components(&d),
            1,
            "D must enclose exactly one hole"
        );
    }

    #[test]
    fn ac3_and_carve_vacuity_measured_sections_are_at_least_n_and_min_is_k() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        let n = usize::try_from(FIXTURE_N).unwrap_or(0);
        let k = usize::try_from(FIXTURE_K).unwrap_or(0);
        // West arm: never widened, at a hole-adjacent row (bounded by the
        // hole on one side, so the run measures the arm, not the whole ring).
        let west = cross_section_width(&d, Point::new(-2, 1), Orient::Horizontal);
        // Widened East arm.
        let east_wide = cross_section_width(&d, Point::new(12, 1), Orient::Horizontal);
        for w in [west, east_wide] {
            assert!(w >= n, "cross-section {w} < n={n}");
        }
        // Carve-vacuity (design doc Key Decisions): round-1 never carves, so
        // the *minimum* measured cross-section is exactly k — never below k,
        // and in particular never narrowed down to n.
        assert_eq!(
            west.min(east_wide),
            k,
            "unwidened arm must measure exactly k"
        );
    }

    #[test]
    fn ac7_cross_sections_match_the_intended_profile() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        let k = usize::try_from(FIXTURE_K).unwrap_or(0);
        let west = cross_section_width(&d, Point::new(-2, 1), Orient::Horizontal);
        assert_eq!(west, k, "unwidened arm must be exactly k (nominal)");
        let east_wide = cross_section_width(&d, Point::new(12, 1), Orient::Horizontal);
        assert!(
            east_wide >= 2 * k,
            "widened arm must be >= 2k, got {east_wide}"
        );
    }

    #[test]
    fn ac5_part_a_every_wall_direction_is_one_lipschitz_on_fixture() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        assert_wall_profile_lipschitz(&east_wall_profile(&d), "east");
        assert_wall_profile_lipschitz(&west_wall_profile(&d), "west");
        assert_wall_profile_lipschitz(&north_wall_profile(&d), "north");
        assert_wall_profile_lipschitz(&south_wall_profile(&d), "south");
    }

    #[test]
    fn ac5_part_b_supercover_never_exits_d_across_the_ramp() {
        let skel = fixture_jog();
        let d = phase2_rasterize(&skel, FIXTURE_K, FIXTURE_N);
        let (_d0, h) = stage1_baseline(&skel.ring, &skel.hole, FIXTURE_K);
        let mut in_family_count = 0usize;
        // The ramp bounding box: the East wall / taper region (design doc:
        // the sharpest surviving concave corner is a unit step of the ramp).
        for x in 8..=15 {
            for y in -4..=9 {
                let a = Point::new(x, y);
                if !d.contains(a) {
                    continue;
                }
                for dx in -V_ENTRY_CHECK..=V_ENTRY_CHECK {
                    for dy in -V_ENTRY_CHECK..=V_ENTRY_CHECK {
                        let speed = dx.abs().max(dy.abs());
                        if !(1..=V_ENTRY_CHECK).contains(&speed) {
                            continue;
                        }
                        // Along-ramp-axis (design doc Test Design AC5 Part
                        // B): the East wall's run-axis is vertical (`y`), so
                        // keep only chords whose run-axis delta strictly
                        // dominates the perpendicular (`x`) delta —
                        // necessary but not sufficient (see the H-clearance
                        // filter below). Strict, not `≥`: an exact-diagonal
                        // tie (`|dx| == |dy|`) runs at exactly the ramp's
                        // own 1-Lipschitz slope, so `supercover` always
                        // touches BOTH cells straddling every staircase
                        // corner (a mathematical property of an exact 45°
                        // chord through a unit step, independent of how
                        // "solid" the ramp fill is) — such a chord is not a
                        // wall-grazing entry, it is the wall's own boundary
                        // line, and is excluded from the family.
                        if dy.abs() <= dx.abs() {
                            continue;
                        }
                        let b = Point::new(a.x.saturating_add(dx), a.y.saturating_add(dy));
                        if !d.contains(b) {
                            continue;
                        }
                        let cover: Vec<Point> = supercover(a, b).collect();
                        // H-clearance (design doc Issue 1 correction): skip
                        // any chord whose supercover touches a hole cell —
                        // that is a hole-facing inner-corner cut (a normal
                        // annulus turn), exempt per Part A's outfield-vs-
                        // hole-facing classification.
                        if cover.iter().any(|c| h.contains(c)) {
                            continue;
                        }
                        in_family_count = in_family_count.saturating_add(1);
                        for cell in cover {
                            assert!(
                                d.contains(cell),
                                "supercover({a:?}, {b:?}) exits D at {cell:?}"
                            );
                        }
                    }
                }
            }
        }
        assert!(
            in_family_count > 0,
            "chord family must be non-vacuous (design doc AC5 Part B non-vacuity)"
        );
    }

    /// Seed count the multi-seed property sweep runs over — matches Ф1's own
    /// sweep count (`crates/gen/src/phase1.rs`, `PROPERTY_SEED_COUNT`).
    const PROPERTY_SEED_COUNT: u64 = 64;
    /// Block size / min-width the property sweep runs Ф2 at (`k >= n`).
    const PROPERTY_K: i32 = 6;
    const PROPERTY_N: i32 = 3;

    #[test]
    fn property_sweep_holds_ac1_topology_and_ac5_part_a_across_phase1_seeds() {
        let l_min = 3;
        for seed in 0..PROPERTY_SEED_COUNT {
            let skel = crate::phase1_coarse_ring(l_min, &mut rng(seed));
            let (d0, h) = stage1_baseline(&skel.ring, &skel.hole, PROPERTY_K);
            let d = phase2_rasterize(&skel, PROPERTY_K, PROPERTY_N);

            // AC1: D0 subset of the final D (additive taper).
            for p in drivable_points(&d0) {
                assert!(d.contains(p), "seed {seed}: D0 point {p:?} missing from D");
            }
            // Hole-safety.
            for &p in &h {
                assert!(
                    !d.contains(p),
                    "seed {seed}: hole cell {p:?} became drivable"
                );
            }
            // Topology (design doc §"no production self-check" — test-only
            // certification of the by-construction argument).
            assert_eq!(
                component_count(&d),
                1,
                "seed {seed}: D must be one connected component"
            );
            assert_eq!(
                bounded_complement_components(&d),
                1,
                "seed {seed}: D must enclose exactly one hole"
            );
            // AC5 Part A: every outfield-facing wall direction is 1-Lipschitz.
            assert_wall_profile_lipschitz(&east_wall_profile(&d), "east");
            assert_wall_profile_lipschitz(&west_wall_profile(&d), "west");
            assert_wall_profile_lipschitz(&north_wall_profile(&d), "north");
            assert_wall_profile_lipschitz(&south_wall_profile(&d), "south");
            // AC3 (local-width proxy, design doc §"AC3" Note 4): the proxy
            // `local_width` can false-dip below the true perpendicular
            // cross-section at a 45° ramp tip, so it is asserted only on
            // straight-run cells — those already present in `D0` (the
            // uniform k×k baseline, never a ramp/absorption addition). The
            // ramp/tip region is covered instead by the additive property
            // (`D0 ⊆ D`, already asserted above) + `D0`'s own uniform-`k`
            // width, per the design doc's slack argument (proxy <= true, so
            // this restriction only avoids false failures, never masks a
            // real narrowing).
            for p in drivable_points(&d0) {
                let w = local_width(&d, p);
                assert!(
                    w >= usize::try_from(PROPERTY_N).unwrap_or(0),
                    "seed {seed}: local width {w} at straight-run cell {p:?} < n"
                );
            }
        }
    }

    /// The fixed seed the AC6 snapshot below is minted against.
    const SNAPSHOT_SEED: u64 = 12345;

    #[test]
    fn ac6_snapshot_pins_an_exact_d_for_a_known_seed() {
        // AC6: a known small skeleton (via Seeds::generation_rng, mirroring
        // Ф1's own snapshot test) -> an exact assert_eq! of the sorted
        // drivable-point set. Cross-checked against AC1/AC3/AC4/AC5 (via the
        // shared taper machinery already exercised above) before freezing.
        let seeds = gp_core::rng::Seeds {
            generation: SNAPSHOT_SEED,
            ..Default::default()
        };
        let skel = crate::phase1_coarse_ring(3, &mut seeds.generation_rng());
        let d = phase2_rasterize(&skel, PROPERTY_K, PROPERTY_N);

        assert_eq!(component_count(&d), 1, "minted D must be one component");
        assert_eq!(
            bounded_complement_components(&d),
            1,
            "minted D must enclose exactly one hole"
        );
        assert_wall_profile_lipschitz(&east_wall_profile(&d), "east");
        assert_wall_profile_lipschitz(&west_wall_profile(&d), "west");
        assert_wall_profile_lipschitz(&north_wall_profile(&d), "north");
        assert_wall_profile_lipschitz(&south_wall_profile(&d), "south");

        assert_eq!(d.origin(), snapshot_origin());
        assert_eq!(d.width(), snapshot_width());
        assert_eq!(d.height(), snapshot_height());
        // The exact, byte-identical point set (AC6) — pinned compactly via
        // count + boundary extremes + a deterministic content hash rather
        // than a literal ~4000-entry `Vec<Point>` (at `PROPERTY_K = 6` on
        // real Ф1 output, embedding the full set would blow phase2.rs far
        // past the file-size cap, AGENTS.md § Code Style / design doc
        // Risks). `fnv1a_hash_points` is a fixed, non-randomized hash (never
        // `std::collections::HashSet`'s per-process-random `RandomState`,
        // which would make a persisted literal non-reproducible) — any
        // change to the exact point set changes the hash with overwhelming
        // probability, so this still catches a byte-level regression.
        let points = drivable_points(&d);
        assert_eq!(
            points.len(),
            snapshot_point_count(),
            "pinned point count changed"
        );
        assert_eq!(
            points[0],
            snapshot_first_point(),
            "pinned first point changed"
        );
        assert_eq!(
            points[points.len() - 1],
            snapshot_last_point(),
            "pinned last point changed"
        );
        assert_eq!(
            fnv1a_hash_points(&points),
            snapshot_points_hash(),
            "pinned exact point-set hash changed"
        );
    }

    /// The exact, minted [`Corridor::origin`] for [`SNAPSHOT_SEED`] (AC6).
    fn snapshot_origin() -> Point {
        Point::new(-61, -19)
    }

    /// The exact, minted [`Corridor::width`] for [`SNAPSHOT_SEED`] (AC6).
    fn snapshot_width() -> usize {
        116
    }

    /// The exact, minted [`Corridor::height`] for [`SNAPSHOT_SEED`] (AC6).
    fn snapshot_height() -> usize {
        92
    }

    /// The exact, minted drivable-point count for [`SNAPSHOT_SEED`] (AC6).
    fn snapshot_point_count() -> usize {
        6087
    }

    /// The first point of [`drivable_points`]'s row-major traversal for
    /// [`SNAPSHOT_SEED`] (AC6) — mirrors Ф1's `ring[0]` boundary-literal
    /// cross-check style.
    fn snapshot_first_point() -> Point {
        Point::new(-24, -18)
    }

    /// The last point of [`drivable_points`]'s row-major traversal for
    /// [`SNAPSHOT_SEED`] (AC6).
    fn snapshot_last_point() -> Point {
        Point::new(11, 71)
    }

    /// The exact, minted [`fnv1a_hash_points`] digest for [`SNAPSHOT_SEED`]
    /// (AC6).
    fn snapshot_points_hash() -> u64 {
        668_417_556_869_487_235
    }

    /// A fixed (non-randomized) FNV-1a 64-bit hash of `pts`'s content, in
    /// traversal order — used to pin the AC6 snapshot's exact point set
    /// compactly (see the doc comment at its call site). Deliberately not
    /// `std::collections::HashSet`'s default `RandomState`, which reseeds
    /// per process and would make a persisted literal irreproducible.
    fn fnv1a_hash_points(pts: &[Point]) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = FNV_OFFSET;
        for p in pts {
            for byte in p.x.to_le_bytes().into_iter().chain(p.y.to_le_bytes()) {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
        hash
    }

    /// `(y, x_max)` for every row of `d`'s bounding box with at least one
    /// drivable cell, in increasing-`y` order — the East-wall profile AC4
    /// walks.
    fn east_wall_profile(d: &Corridor) -> Vec<(i32, i32)> {
        let origin = d.origin();
        let hgt = i32::try_from(d.height()).unwrap_or(0);
        (origin.y..origin.y.saturating_add(hgt))
            .filter_map(|y| row_extent(d, y, 1).map(|x| (y, x)))
            .collect()
    }

    /// `(y, x_min)` for every row with a drivable cell — the West-wall
    /// profile.
    fn west_wall_profile(d: &Corridor) -> Vec<(i32, i32)> {
        let origin = d.origin();
        let hgt = i32::try_from(d.height()).unwrap_or(0);
        (origin.y..origin.y.saturating_add(hgt))
            .filter_map(|y| row_extent(d, y, -1).map(|x| (y, x)))
            .collect()
    }

    /// `(x, y_max)` for every column with a drivable cell — the North-wall
    /// profile.
    fn north_wall_profile(d: &Corridor) -> Vec<(i32, i32)> {
        let origin = d.origin();
        let w = i32::try_from(d.width()).unwrap_or(0);
        (origin.x..origin.x.saturating_add(w))
            .filter_map(|x| col_extent(d, x, 1).map(|y| (x, y)))
            .collect()
    }

    /// `(x, y_min)` for every column with a drivable cell — the South-wall
    /// profile.
    fn south_wall_profile(d: &Corridor) -> Vec<(i32, i32)> {
        let origin = d.origin();
        let w = i32::try_from(d.width()).unwrap_or(0);
        (origin.x..origin.x.saturating_add(w))
            .filter_map(|x| col_extent(d, x, -1).map(|y| (x, y)))
            .collect()
    }

    /// Asserts `profile` (a `(coordinate, extent)` series) is 1-Lipschitz —
    /// no coordinate-adjacent pair (`c1 == c0 + 1`) advances `extent` by more
    /// than 1 (AC4/AC5 Part A). Non-adjacent pairs (a gap in the profile) are
    /// not part of any single wall run and are skipped.
    fn assert_wall_profile_lipschitz(profile: &[(i32, i32)], label: &str) {
        for w in profile.windows(2) {
            let (c0, v0) = w[0];
            let (c1, v1) = w[1];
            if c1 == c0.saturating_add(1) {
                let step = v1.abs_diff(v0);
                assert!(
                    step <= 1,
                    "{label}: advanced by {step} between coordinate {c0} and {c1} (extent {v0} -> {v1})"
                );
            }
        }
    }

    /// The maximal contiguous drivable run through `through` along `axis`
    /// (test-only cross-section-width scan; no such helper exists in
    /// gp-core, design doc Technical Constraints). `0` if `through` itself
    /// is not drivable.
    fn cross_section_width(d: &Corridor, through: Point, axis: Orient) -> usize {
        if !d.contains(through) {
            return 0;
        }
        let mut count = 1usize;
        match axis {
            Orient::Horizontal => {
                let mut x = through.x.saturating_add(1);
                while d.contains(Point::new(x, through.y)) {
                    count = count.saturating_add(1);
                    x = x.saturating_add(1);
                }
                let mut x = through.x.saturating_sub(1);
                while d.contains(Point::new(x, through.y)) {
                    count = count.saturating_add(1);
                    x = x.saturating_sub(1);
                }
            }
            Orient::Vertical => {
                let mut y = through.y.saturating_add(1);
                while d.contains(Point::new(through.x, y)) {
                    count = count.saturating_add(1);
                    y = y.saturating_add(1);
                }
                let mut y = through.y.saturating_sub(1);
                while d.contains(Point::new(through.x, y)) {
                    count = count.saturating_add(1);
                    y = y.saturating_sub(1);
                }
            }
        }
        count
    }

    /// A best-effort local corridor width at `p`: the smaller of the
    /// horizontal and vertical contiguous drivable runs through `p`. On a
    /// straight corridor stretch the along-direction run is large and the
    /// perpendicular run is the true local width, so the minimum recovers it
    /// without needing to know the local travel direction.
    fn local_width(d: &Corridor, p: Point) -> usize {
        cross_section_width(d, p, Orient::Horizontal).min(cross_section_width(
            d,
            p,
            Orient::Vertical,
        ))
    }

    /// Every drivable point of `d`, as a sorted `Vec` for equality comparison.
    fn drivable_points(d: &Corridor) -> Vec<Point> {
        let mut pts = Vec::new();
        let origin = d.origin();
        for dy in 0..d.height() {
            for dx in 0..d.width() {
                let p = Point::new(
                    origin.x.saturating_add(i32::try_from(dx).unwrap_or(0)),
                    origin.y.saturating_add(i32::try_from(dy).unwrap_or(0)),
                );
                if d.contains(p) {
                    pts.push(p);
                }
            }
        }
        pts
    }

    /// The contiguous drivable run length starting at `(from_x, y)` and
    /// extending east while drivable (test-only scan helper).
    fn row_run_length(d: &Corridor, y: i32, from_x: i32) -> usize {
        let mut x = from_x;
        let mut count = 0usize;
        while d.contains(Point::new(x, y)) {
            count = count.saturating_add(1);
            x = x.saturating_add(1);
        }
        // Also extend west from from_x - 1 to capture the full contiguous run.
        let mut x = from_x.saturating_sub(1);
        while d.contains(Point::new(x, y)) {
            count = count.saturating_add(1);
            x = x.saturating_sub(1);
        }
        count
    }
}
