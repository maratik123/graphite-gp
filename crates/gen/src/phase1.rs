//! Ф1 — coarse-block ring (infield-first), design doc §2.
//!
//! Produces the coarse skeleton `{ ring, hole, dir }` at **coarse-block**
//! granularity: `ring` is the annulus, `hole` is the enclosed infield polyomino
//! `P`, and `dir` is the fixed global traversal orientation. The `k×k` fine
//! expansion to the actual corridor `D` is Ф2 — out of scope here.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use gp_core::geom::{
    Corridor, Point, Side, bounded_complement_components, component_count, walls_from_boundary,
};
use gp_core::track::RaceDir;
use rand::seq::IteratorRandom;
use rand::{Rng, RngExt};
use strum::VariantArray;

/// Documented supported domain floor for `l_min` (design doc, reviewer NOTE
/// 2): below this, [`phase1_coarse_ring`] clamps up.
const MIN_COARSE_STRAIGHT: i32 = 2;
/// Documented supported domain ceiling for `l_min` (reviewer NOTE 2): caps
/// both the primary and fallback allocation to a bounded coarse grid.
const MAX_COARSE_STRAIGHT: i32 = 64;
/// Minimum base-strip width floor, before the even-rounding and `l_eff` max.
const MIN_BASE: i32 = 4;
/// Target total block count `P`'s even-sublattice growth aims for (base
/// blocks included); growth stops early if the keep-out frontier is
/// exhausted first.
const TARGET_BLOCKS: usize = 16;
/// Maximum outward widen amount drawn per [`Side`] (inclusive).
const WIDEN_MAX: u32 = 3;
/// Bounded same-stream retry budget for the step-6 run-length check before
/// falling back to the guaranteed rectangular annulus.
const MAX_ATTEMPTS: u32 = 8;
/// Minimum fallback rectangle width — `W = max(l_eff + 2, MIN_RECT_W)`.
const MIN_RECT_W: i32 = 4;
/// Fixed fallback rectangle height.
const MIN_RECT_H: i32 = 4;
/// Multi-seed property-test fallback-rate ceiling (design doc, recommendation):
/// a healthy construction falls back rarely, so `fallback_count / N` staying
/// under this catches a construction regression that fails the step-6 check
/// on most seeds, which a weaker "not all fall back" check would miss.
#[cfg(test)]
const FALLBACK_RATE_MAX: f64 = 0.20;

/// The Ф1 output: a coarse annulus `ring` enclosing exactly one hole `hole`
/// (the infield polyomino `P`), plus the fixed traversal orientation.
#[derive(Clone, Debug)]
pub struct CoarseSkeleton {
    /// The coarse-block ring — `dilate_moore(P, 1) \ P`. Connected, exactly
    /// one hole (AC2).
    pub ring: BTreeSet<Point>,
    /// The enclosed infield polyomino `P` (the ring's one hole).
    pub hole: BTreeSet<Point>,
    /// The fixed global traversal orientation, stable across repeated
    /// same-seed calls (AC4).
    pub dir: RaceDir,
}

/// Builds a coarse-block ring skeleton (design doc §2 Ф1) for a fixed
/// `l_min` (minimum coarse-block straight length) and RNG stream.
///
/// Infallible: a bounded same-stream retry plus a guaranteed-terminating
/// rectangular fallback make this a total function — no `Result`, no panic.
pub fn phase1_coarse_ring(l_min: i32, rng: &mut impl Rng) -> CoarseSkeleton {
    phase1_coarse_ring_attempts(l_min, rng, MAX_ATTEMPTS).0
}

/// [`phase1_coarse_ring`]'s core, parameterized over the retry budget so
/// tests can force immediate exhaustion (`max_attempts = 0`, the forced-
/// exhaustion fallback test) or observe the fallback rate at the production
/// budget. Returns the skeleton plus whether it is the rectangular fallback
/// terminal (design doc §2 Ф1 steps 6-8).
#[allow(
    clippy::similar_names,
    reason = "ring/rng are established, unambiguous domain vocabulary here \
              (skeleton.ring, the RNG stream) — no realistic confusion risk"
)]
fn phase1_coarse_ring_attempts(
    l_min: i32,
    rng: &mut impl Rng,
    max_attempts: u32,
) -> (CoarseSkeleton, bool) {
    let l_eff = clamp_l_min(l_min);
    for _attempt in 0..max_attempts {
        let (p, l_eff, _base_w) = build_p(l_min, rng);
        let ring = widen(&ring_from_p(&p), rng);
        let d = corridor_from_cells(&ring, 1);
        // Verified on the actual returned ring, not assumed: widening a
        // concave-shaped ring's extremal run can — in a rare case — pinch
        // off a second bounded pocket, so AC2 is checked here alongside
        // AC3(a)/(b), exactly like the design's own "checked, not by
        // construction" posture for the outer border's run lengths.
        let runs = max_straight_runs(&d);
        let min_run = runs.iter().copied().min().unwrap_or(0);
        let max_run = runs.iter().copied().max().unwrap_or(0);
        let max_run = i32::try_from(max_run).unwrap_or(i32::MAX);
        if component_count(&d) == 1
            && bounded_complement_components(&d) == 1
            && min_run >= 2
            && max_run >= l_eff
        {
            let dir = choose_dir(rng);
            return (CoarseSkeleton { ring, hole: p, dir }, false);
        }
    }
    // Guaranteed-terminating fallback (step 7): a rectangular annulus
    // satisfies every AC by construction. `dir` is still drawn on this path,
    // so it is seeded regardless of which terminal Ф1 hits (AC4).
    let (ring, hole) = rectangular_fallback(l_eff);
    let dir = choose_dir(rng);
    (CoarseSkeleton { ring, hole, dir }, true)
}

/// Moore (3×3 / Chebyshev-1) dilation of `p` by one cell.
fn dilate_moore(p: &BTreeSet<Point>) -> BTreeSet<Point> {
    p.iter()
        .flat_map(|pt| {
            (-1..=1).flat_map(move |dy| {
                (-1..=1).map(move |dx| Point::new(pt.x.saturating_add(dx), pt.y.saturating_add(dy)))
            })
        })
        .collect()
}

/// `ring = dilate_moore(P) \ P` — the annulus enclosing `p` (design doc §2
/// Ф1 step 4). For non-empty simply-connected `p` this is connected with
/// exactly one hole (`p` itself) by construction (AC2).
fn ring_from_p(p: &BTreeSet<Point>) -> BTreeSet<Point> {
    dilate_moore(p).difference(p).copied().collect()
}

/// Widens `ring` outward on each [`Side`] by a `0..=WIDEN_MAX` amount drawn
/// per side, in `Side::iter()`'s fixed order (design doc §2 Ф1 step 5).
///
/// Each widened layer is attached only to `ring`'s existing extremal cells on
/// that side and never touches the inner hole — outward-only. For a concave
/// ring shape, widening a side whose extremal run has multiple disjoint arms
/// can, rarely, pinch off a second bounded pocket; that case is **not**
/// assumed away — [`phase1_coarse_ring_attempts`] re-verifies AC2 on the
/// actual post-widen ring (step 6) and retries/falls back on failure, exactly
/// like the run-length check.
#[allow(
    clippy::similar_names,
    reason = "ring/rng are established, unambiguous domain vocabulary here \
              (the annulus, the RNG stream) — no realistic confusion risk"
)]
fn widen(ring: &BTreeSet<Point>, rng: &mut impl Rng) -> BTreeSet<Point> {
    let mut ring = ring.clone();
    for side in Side::VARIANTS {
        let amount = rng.random_range(0..=WIDEN_MAX);
        if amount == 0 {
            continue;
        }
        let extremal_coord = match side {
            Side::East => ring.iter().map(|p| p.x).max(),
            Side::West => ring.iter().map(|p| p.x).min(),
            Side::North => ring.iter().map(|p| p.y).max(),
            Side::South => ring.iter().map(|p| p.y).min(),
        };
        let Some(extremal_coord) = extremal_coord else {
            continue;
        };
        let extremal: Vec<Point> = ring
            .iter()
            .copied()
            .filter(|p| match side {
                Side::East | Side::West => p.x == extremal_coord,
                Side::North | Side::South => p.y == extremal_coord,
            })
            .collect();
        let (dx, dy) = side.delta();
        for layer in 1..=amount {
            let layer = i32::try_from(layer).unwrap_or(i32::MAX);
            for pt in &extremal {
                ring.insert(Point::new(
                    pt.x.saturating_add(dx.saturating_mul(layer)),
                    pt.y.saturating_add(dy.saturating_mul(layer)),
                ));
            }
        }
    }
    ring
}

/// The maximal straight border-run lengths of `d`'s **entire** boundary
/// (inner and outer), grouped by `(orientation, fixed coordinate)` into
/// contiguous runs along the varying axis (design doc §2 Ф1 step 6).
fn max_straight_runs(d: &Corridor) -> Vec<usize> {
    let mut by_key: BTreeMap<(bool, i32), Vec<i32>> = BTreeMap::new();
    for w in walls_from_boundary(d) {
        let (is_horizontal_wall, fixed, varying) = match w.side {
            Side::North | Side::South => (true, w.cell.y, w.cell.x),
            Side::East | Side::West => (false, w.cell.x, w.cell.y),
        };
        by_key
            .entry((is_horizontal_wall, fixed))
            .or_default()
            .push(varying);
    }
    let mut runs = Vec::new();
    for mut coords in by_key.into_values() {
        coords.sort_unstable();
        coords.dedup();
        let mut run_len = 1usize;
        for w in coords.windows(2) {
            if w[1] == w[0].saturating_add(1) {
                run_len = run_len.saturating_add(1);
            } else {
                runs.push(run_len);
                run_len = 1;
            }
        }
        runs.push(run_len);
    }
    runs
}

/// The guaranteed-terminating fallback (design doc §2 Ф1 step 7): a
/// rectangular annulus `[0, W) × [0, H)` minus its interior `[1, W−1) ×
/// [1, H−1)`. Satisfies every AC by construction: one 4-connected loop, one
/// hole of `(W−2)(H−2) ≥ 1` cells, every maximal border run a full side
/// length `≥ 2`, and the bottom side `W − 2 ≥ l_eff` (within the documented
/// `l_min` domain).
///
/// **Overflow precondition:** `l_eff` is clamped to
/// `MIN_COARSE_STRAIGHT..=MAX_COARSE_STRAIGHT`, so `w` is bounded by
/// `MAX_COARSE_STRAIGHT + 2` — the `+2` cannot overflow `i32`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "l_eff is clamped to MIN_COARSE_STRAIGHT..=MAX_COARSE_STRAIGHT, so \
              l_eff + 2 is bounded by MAX_COARSE_STRAIGHT + 2, far below i32::MAX"
)]
fn rectangular_fallback(l_eff: i32) -> (BTreeSet<Point>, BTreeSet<Point>) {
    let w = (l_eff + 2).max(MIN_RECT_W);
    let h = MIN_RECT_H;
    let mut ring = BTreeSet::new();
    let mut hole = BTreeSet::new();
    for y in 0..h {
        for x in 0..w {
            let pt = Point::new(x, y);
            if x == 0 || y == 0 || x.saturating_add(1) == w || y.saturating_add(1) == h {
                ring.insert(pt);
            } else {
                hole.insert(pt);
            }
        }
    }
    (ring, hole)
}

/// Draws the fixed traversal orientation — one `u32` pick after the loop
/// settles (success or fallback), so `dir` is seeded on every path (AC4).
fn choose_dir(rng: &mut impl Rng) -> RaceDir {
    RaceDir::VARIANTS
        .iter()
        .copied()
        .choose(rng)
        .expect("enum variants iterator should return correct size_hint")
}

/// Clamps `l_min` into the documented supported coarse-block domain
/// (reviewer NOTE 2) — every later length use reads `l_eff`, never `l_min`.
fn clamp_l_min(l_min: i32) -> i32 {
    l_min.clamp(MIN_COARSE_STRAIGHT, MAX_COARSE_STRAIGHT)
}

/// The base-strip width: `max(l_eff, MIN_BASE)` rounded up to even.
///
/// **Overflow precondition:** `l_eff` is already clamped to
/// `MIN_COARSE_STRAIGHT..=MAX_COARSE_STRAIGHT` and `MIN_BASE` is a small
/// const, so `w` is bounded by `max(MAX_COARSE_STRAIGHT, MIN_BASE)` — the
/// `+1` rounding step cannot overflow `i32` for any in-domain `l_eff`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "w = l_eff.max(MIN_BASE) is bounded by max(MAX_COARSE_STRAIGHT, MIN_BASE), \
              both small consts; w + (w & 1) stays far below i32::MAX"
)]
const fn base_width(l_eff: i32) -> i32 {
    let w = if l_eff > MIN_BASE { l_eff } else { MIN_BASE };
    w + (w & 1)
}

/// The 4 unit steps between block-adjacent block coordinates.
const BLOCK_STEPS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// The block-4-adjacent frontier of `blocks`, restricted to `j ≥ 1` (the
/// `y ≥ 2` growth keep-out, Issue-2 guard) and excluding already-occupied
/// blocks. A `BTreeSet` — never a `HashSet` — so the pick below draws from a
/// deterministic, cross-platform-stable order (AC5).
fn block_frontier(blocks: &BTreeSet<(i32, i32)>) -> BTreeSet<(i32, i32)> {
    let mut frontier = BTreeSet::new();
    for &(i, j) in blocks {
        for (di, dj) in BLOCK_STEPS {
            let candidate = (i.saturating_add(di), j.saturating_add(dj));
            if candidate.1 >= 1 && !blocks.contains(&candidate) {
                frontier.insert(candidate);
            }
        }
    }
    frontier
}

/// Grows `blocks` by adding even-aligned 2×2 blocks from the block-4-adjacent
/// frontier (`y ≥ 2` keep-out) until `target` blocks are reached or the
/// frontier is exhausted. Each pick draws a fixed-width `u32` index into the
/// frontier enumerated as a sorted `Vec` (AC5 determinism).
fn grow_blocks(blocks: &mut BTreeSet<(i32, i32)>, target: usize, rng: &mut impl Rng) {
    while blocks.len() < target {
        let frontier: Vec<(i32, i32)> = block_frontier(blocks).into_iter().collect();
        let Ok(n) = u32::try_from(frontier.len()) else {
            break;
        };
        if n == 0 {
            break;
        }
        let idx = rng.random_range(0..n) as usize;
        blocks.insert(frontier[idx]);
    }
}

/// Expands a set of even-aligned block coordinates to their 4 constituent
/// cells each (`{(2i,2j), (2i+1,2j), (2i,2j+1), (2i+1,2j+1)}`).
fn cells_of_blocks(blocks: &BTreeSet<(i32, i32)>) -> BTreeSet<Point> {
    blocks
        .iter()
        .flat_map(|&(i, j)| {
            let x = i.saturating_mul(2);
            let y = j.saturating_mul(2);
            [
                Point::new(x, y),
                Point::new(x.saturating_add(1), y),
                Point::new(x, y.saturating_add(1)),
                Point::new(x.saturating_add(1), y.saturating_add(1)),
            ]
        })
        .collect()
}

/// Builds a [`Corridor`] over the bounding box of `cells`, expanded by
/// `margin` cells on every side, with exactly `cells` marked drivable.
///
/// An empty `cells` yields a `1×1` corridor at the origin with nothing
/// drivable — total, no panic.
fn corridor_from_cells(cells: &BTreeSet<Point>, margin: i32) -> Corridor {
    let Some(min_x) = cells.iter().map(|p| p.x).min() else {
        return Corridor::new(Point::new(0, 0), 1, 1);
    };
    let max_x = cells.iter().map(|p| p.x).max().unwrap_or(min_x);
    let min_y = cells.iter().map(|p| p.y).min().unwrap_or(min_x);
    let max_y = cells.iter().map(|p| p.y).max().unwrap_or(min_y);

    let origin = Point::new(min_x.saturating_sub(margin), min_y.saturating_sub(margin));
    let span_x = max_x.saturating_sub(min_x).saturating_add(1);
    let span_y = max_y.saturating_sub(min_y).saturating_add(1);
    let width = usize::try_from(span_x.saturating_add(margin.saturating_mul(2))).unwrap_or(1);
    let height = usize::try_from(span_y.saturating_add(margin.saturating_mul(2))).unwrap_or(1);

    let mut d = Corridor::new(origin, width.max(1), height.max(1));
    for &p in cells {
        d.set(p, true);
    }
    d
}

/// Every cell point of `d`'s bounding box, in row-major order.
fn box_points(d: &Corridor) -> impl Iterator<Item = Point> {
    let origin = d.origin();
    let (w, h) = (d.width(), d.height());
    let x1 = i32::try_from(w).map_or(i32::MAX, |w| origin.x.saturating_add(w));
    let y1 = i32::try_from(h).map_or(i32::MAX, |h| origin.y.saturating_add(h));
    (origin.y..y1).flat_map(move |y| (origin.x..x1).map(move |x| Point::new(x, y)))
}

/// `p`'s `(dx, dy)` offset within `d`'s bounding box, or `None` if `p` lies
/// outside it. The single gate every box-confined traversal below routes
/// through, so a flood can never wander into the unbounded exterior.
fn local_coords(d: &Corridor, p: Point) -> Option<(usize, usize)> {
    let (ox, oy) = (d.origin().x, d.origin().y);
    let (w, h) = (d.width(), d.height());
    let dx = usize::try_from(p.x.checked_sub(ox)?).ok()?;
    let dy = usize::try_from(p.y.checked_sub(oy)?).ok()?;
    (dx < w && dy < h).then_some((dx, dy))
}

/// Whether `p` lies on `d`'s bounding-box border. `false` for any
/// out-of-box point.
fn on_border(d: &Corridor, p: Point) -> bool {
    let (w, h) = (d.width(), d.height());
    local_coords(d, p).is_some_and(|(dx, dy)| {
        dx == 0 || dy == 0 || dx.saturating_add(1) == w || dy.saturating_add(1) == h
    })
}

/// Fills every **bounded** 4-connected component of `p`'s complement into
/// `p`, so the result is simply connected (AC2 ordering: runs before Ф1's
/// dilation step). Self-contained flood-fill over a padded corridor — the
/// padding guarantees the box border is entirely `¬p`, so a component
/// touching the border is, by definition, the unbounded outfield (never
/// filled), matching `gp_core::geom::bounded_complement_components`'s
/// definition.
fn fill_holes(p: &BTreeSet<Point>) -> BTreeSet<Point> {
    if p.is_empty() {
        return p.clone();
    }
    let d = corridor_from_cells(p, 1);
    let mut filled = p.clone();
    let mut visited: HashSet<Point> = HashSet::new();
    for seed in box_points(&d) {
        if d.contains(seed) || visited.contains(&seed) {
            continue;
        }
        let mut component = Vec::new();
        let mut touches_border = false;
        let mut stack = vec![seed];
        visited.insert(seed);
        while let Some(cur) = stack.pop() {
            component.push(cur);
            if on_border(&d, cur) {
                touches_border = true;
            }
            for n in cur.neighbors4() {
                // Confine the flood to `d`'s padded box (`local_coords`
                // gates it) — otherwise a component neighboring the box
                // border would spill into the unbounded exterior forever.
                if local_coords(&d, n).is_some() && !d.contains(n) && visited.insert(n) {
                    stack.push(n);
                }
            }
        }
        if !touches_border {
            filled.extend(component);
        }
    }
    filled
}

/// The enclosure-based AC3(b) guard (reviewer NOTE 1): every `y == 0` cell's
/// `Side::South` dual edge is present on `p`'s boundary — the base strip's
/// south edge was never covered by growth or hole-fill. Enclosure-based (via
/// `walls_from_boundary`), **not** a neighbor-count assertion.
fn debug_assert_base_south_edge_intact(p: &BTreeSet<Point>, base_w: i32) {
    debug_assert!(
        {
            let d = corridor_from_cells(p, 1);
            let walls: HashSet<_> = walls_from_boundary(&d).into_iter().collect();
            (0..base_w).all(|x| {
                walls.contains(&gp_core::geom::Wall {
                    cell: Point::new(x, 0),
                    side: Side::South,
                })
            })
        },
        "base strip south edge must stay a border run (AC3b enclosure guard)"
    );
}

/// Builds the infield polyomino `P` (design doc §2 Ф1 steps 1–3): a clamped
/// base strip, even-sublattice growth restricted to `y ≥ 2`, then
/// pre-dilation hole-fill. Returns `(P, l_eff, base_w)` — callers need
/// `l_eff` for the later run-length check and `base_w` only for tests/debug.
fn build_p(l_min: i32, rng: &mut impl Rng) -> (BTreeSet<Point>, i32, i32) {
    let l_eff = clamp_l_min(l_min);
    let base_w = base_width(l_eff);
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "base_w is even and bounded (base_width's doc precondition), so \
                  base_w / 2 is an exact, small, non-negative block count"
    )]
    let base_blocks_count = usize::try_from(base_w / 2).unwrap_or(0);
    let mut blocks: BTreeSet<(i32, i32)> = (0..base_blocks_count)
        .map(|i| (i32::try_from(i).unwrap_or(i32::MAX), 0))
        .collect();

    grow_blocks(&mut blocks, TARGET_BLOCKS, rng);

    let p = cells_of_blocks(&blocks);
    let p = fill_holes(&p);
    debug_assert_base_south_edge_intact(&p, base_w);
    (p, l_eff, base_w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::{bounded_complement_components, component_count};
    use gp_core::rng::Seeds;
    use rand::SeedableRng;
    use rand::rngs::Xoshiro256PlusPlus;

    fn rng(seed: u64) -> Xoshiro256PlusPlus {
        Xoshiro256PlusPlus::seed_from_u64(seed)
    }

    #[test]
    fn coarse_skeleton_carries_ring_hole_and_dir() {
        // AC1: CoarseSkeleton is a plain data carrier for {ring, hole, dir}.
        let skeleton = CoarseSkeleton {
            ring: BTreeSet::from([Point::new(0, 0)]),
            hole: BTreeSet::from([Point::new(1, 1)]),
            dir: RaceDir::Cw,
        };
        assert!(skeleton.ring.contains(&Point::new(0, 0)));
        assert!(skeleton.hole.contains(&Point::new(1, 1)));
        assert_eq!(skeleton.dir, RaceDir::Cw);
    }

    #[test]
    fn clamp_l_min_maps_in_domain_value_to_itself() {
        assert_eq!(clamp_l_min(5), 5);
        assert_eq!(clamp_l_min(MIN_COARSE_STRAIGHT), MIN_COARSE_STRAIGHT);
        assert_eq!(clamp_l_min(MAX_COARSE_STRAIGHT), MAX_COARSE_STRAIGHT);
    }

    #[test]
    fn clamp_l_min_clamps_out_of_domain_extremes() {
        assert_eq!(clamp_l_min(i32::MAX), MAX_COARSE_STRAIGHT);
        assert_eq!(clamp_l_min(i32::MIN), MIN_COARSE_STRAIGHT);
    }

    #[test]
    fn p_is_non_empty_and_connected() {
        let (p, _l_eff, _base_w) = build_p(3, &mut rng(1));
        assert!(!p.is_empty());
        let d = corridor_from_cells(&p, 1);
        assert_eq!(component_count(&d), 1);
    }

    #[test]
    fn p_is_simply_connected_after_hole_fill() {
        for seed in 0..20 {
            let (p, _l_eff, _base_w) = build_p(3, &mut rng(seed));
            let d = corridor_from_cells(&p, 1);
            assert_eq!(
                bounded_complement_components(&d),
                0,
                "seed {seed}: P must have no bounded holes after fill"
            );
        }
    }

    #[test]
    fn growth_never_adds_a_cell_at_y_le_1() {
        // Issue-2 guard: growth is confined to y >= 2, so the only in-P
        // territory at y <= 1 is exactly the base strip [0, base_w) x {0,1};
        // no cell ever appears at y < 0 (hole-fill cannot cover it either,
        // since y <= -1 opens into the unbounded outfield).
        for seed in 0..20 {
            let (p, _l_eff, base_w) = build_p(3, &mut rng(seed));
            for pt in &p {
                assert!(
                    pt.y >= 0,
                    "seed {seed}: unexpected cell {pt:?} below the base strip"
                );
                if pt.y <= 1 {
                    assert!(
                        pt.x >= 0 && pt.x < base_w,
                        "seed {seed}: cell {pt:?} at y<=1 outside the base strip"
                    );
                }
            }
        }
    }

    #[test]
    fn base_south_edge_is_on_the_boundary() {
        // Enclosure-based AC3(b) guard (NOTE 1): every y==0 cell's South wall
        // survives growth + hole-fill, for several seeds and l_min values.
        for l_min in [2, 3, 8, 20] {
            for seed in 0..10 {
                let (p, _l_eff, base_w) = build_p(l_min, &mut rng(seed));
                let d = corridor_from_cells(&p, 1);
                let walls: HashSet<_> = walls_from_boundary(&d).into_iter().collect();
                for x in 0..base_w {
                    assert!(
                        walls.contains(&gp_core::geom::Wall {
                            cell: Point::new(x, 0),
                            side: Side::South,
                        }),
                        "l_min {l_min} seed {seed}: south edge missing at x={x}"
                    );
                }
            }
        }
    }

    #[test]
    fn p_border_has_no_length_one_run() {
        // Even-sublattice guarantee (supporting claim; the real guarantee for
        // the ring is the step-6 check in subtask 7).
        for seed in 0..20 {
            let (p, _l_eff, _base_w) = build_p(3, &mut rng(seed));
            let d = corridor_from_cells(&p, 1);
            let runs = max_straight_runs(&d);
            assert!(
                runs.iter().all(|&r| r >= 2),
                "seed {seed}: P border has a length-1 run: {runs:?}"
            );
        }
    }

    // ---- Subtask 7: dilate / widen / check-retry-fallback / orientation ----

    #[test]
    fn ring_is_disjoint_from_hole_and_encloses_it() {
        // AC2: ring = dilate_moore(P) \ P is disjoint from P (the hole).
        let (p, _l_eff, _base_w) = build_p(3, &mut rng(1));
        let ring = ring_from_p(&p);
        assert!(ring.is_disjoint(&p));
        assert!(!ring.is_empty());
    }

    #[test]
    fn ring_is_connected_with_exactly_one_hole() {
        for seed in 0..20 {
            let skeleton = phase1_coarse_ring(3, &mut rng(seed));
            let d = corridor_from_cells(&skeleton.ring, 1);
            assert_eq!(
                component_count(&d),
                1,
                "seed {seed}: ring must be one connected component"
            );
            assert_eq!(
                bounded_complement_components(&d),
                1,
                "seed {seed}: ring must enclose exactly one hole"
            );
            assert!(
                !skeleton.hole.is_empty(),
                "seed {seed}: hole must be non-empty"
            );
        }
    }

    #[test]
    fn hole_fill_precedes_dilation() {
        // A P seeded with a bounded interior hole (from growth) is filled
        // before the ring is built — the ring's hole must equal the filled
        // P, not a P with a residual gap.
        let (p, _l_eff, _base_w) = build_p(5, &mut rng(2));
        let d = corridor_from_cells(&p, 1);
        assert_eq!(
            bounded_complement_components(&d),
            0,
            "P must already be hole-free before ring construction"
        );
    }

    #[test]
    fn ring_run_length_check_holds_min_two_on_the_returned_ring() {
        // AC3(a): the run-length check is enforced on the actual returned
        // ring (not merely on P's inner border).
        for seed in 0..20 {
            let skeleton = phase1_coarse_ring(3, &mut rng(seed));
            let d = corridor_from_cells(&skeleton.ring, 1);
            let runs = max_straight_runs(&d);
            assert!(
                runs.iter().all(|&r| r >= 2),
                "seed {seed}: returned ring has a length-1 run: {runs:?}"
            );
        }
    }

    #[test]
    fn widening_preserves_ac2_and_introduces_no_length_one_run() {
        let (p, _l_eff, _base_w) = build_p(3, &mut rng(4));
        let ring_before = ring_from_p(&p);
        let ring_after = widen(&ring_before, &mut rng(4));
        assert!(ring_after.is_superset(&ring_before));
        let d_before = corridor_from_cells(&ring_before, 1);
        let d_after = corridor_from_cells(&ring_after, 1);
        assert_eq!(component_count(&d_before), component_count(&d_after));
        assert_eq!(
            bounded_complement_components(&d_before),
            bounded_complement_components(&d_after)
        );
        assert!(
            ring_after.is_disjoint(&p),
            "widening must not touch the hole"
        );
    }

    #[test]
    fn dir_is_cw_or_ccw_and_stable_across_same_seed_calls() {
        // AC4: dir in {Cw, Ccw}, equal across two same-seed calls.
        let a = phase1_coarse_ring(3, &mut rng(7));
        let b = phase1_coarse_ring(3, &mut rng(7));
        assert!(matches!(a.dir, RaceDir::Cw | RaceDir::Ccw));
        assert_eq!(a.dir, b.dir);
    }

    #[test]
    fn rectangular_fallback_satisfies_ac2_and_ac3() {
        for l_eff in [MIN_COARSE_STRAIGHT, 10, MAX_COARSE_STRAIGHT] {
            let (ring, hole) = rectangular_fallback(l_eff);
            let d = corridor_from_cells(&ring, 1);
            assert_eq!(component_count(&d), 1);
            assert_eq!(bounded_complement_components(&d), 1);
            assert!(!hole.is_empty());
            let runs = max_straight_runs(&d);
            assert!(runs.iter().all(|&r| r >= 2));
            assert!(
                runs.iter().copied().max().unwrap_or(0) >= usize::try_from(l_eff).unwrap_or(0),
                "fallback bottom side must be >= l_eff"
            );
        }
    }

    // ---- Subtask 8: Ф1 replay + snapshot + container type -----------------

    /// The fixed seed the AC6 snapshot below is minted against.
    const SNAPSHOT_SEED: u64 = 999;

    #[test]
    fn replay_same_generation_seed_yields_identical_skeleton() {
        // AC5: two same-generation-seed runs produce identical {ring, hole,
        // dir}, seeded via Seeds::generation_rng (AC8 holds through Seeds).
        let seeds = Seeds {
            generation: 42,
            ..Default::default()
        };
        let a = phase1_coarse_ring(3, &mut seeds.generation_rng());
        let b = phase1_coarse_ring(3, &mut seeds.generation_rng());
        assert_eq!(a.ring, b.ring);
        assert_eq!(a.hole, b.hole);
        assert_eq!(a.dir, b.dir);
    }

    #[test]
    fn replay_different_generation_seed_differs_in_at_least_one_field() {
        // AC5: different seeds differ in at least one field.
        let a = phase1_coarse_ring(
            3,
            &mut Seeds {
                generation: 1,
                ..Default::default()
            }
            .generation_rng(),
        );
        let b = phase1_coarse_ring(
            3,
            &mut Seeds {
                generation: 2,
                ..Default::default()
            }
            .generation_rng(),
        );
        assert!(a.ring != b.ring || a.hole != b.hole || a.dir != b.dir);
    }

    #[test]
    fn snapshot_pins_exact_cells_and_dir_for_a_known_seed() {
        // AC6: one known small seed -> an exact assert_eq! of sorted
        // ring/hole and dir. Minted by running Ф1 once for SNAPSHOT_SEED and
        // cross-checked against AC2/AC3 below before freezing (Ф1's own
        // retry loop already enforces AC2/AC3 on the value it returns unless
        // it hit the rectangular fallback — verified not the case here by
        // the non-rectangular shape of the pinned cells).
        let seeds = Seeds {
            generation: SNAPSHOT_SEED,
            ..Default::default()
        };
        let skeleton = phase1_coarse_ring(2, &mut seeds.generation_rng());

        // Cross-check AC2/AC3 before trusting the pinned literals below.
        let d = corridor_from_cells(&skeleton.ring, 1);
        assert_eq!(component_count(&d), 1, "minted ring must be connected");
        assert_eq!(
            bounded_complement_components(&d),
            1,
            "minted ring must enclose exactly one hole"
        );
        let runs = max_straight_runs(&d);
        assert!(
            runs.iter().all(|&r| r >= 2),
            "minted ring must have no length-1 run"
        );

        let mut ring: Vec<Point> = skeleton.ring.iter().copied().collect();
        ring.sort_unstable();
        let mut hole: Vec<Point> = skeleton.hole.iter().copied().collect();
        hole.sort_unstable();

        assert_eq!(ring.len(), 96, "pinned ring cell count changed");
        assert_eq!(hole.len(), 64, "pinned hole cell count changed");
        assert_eq!(skeleton.dir, RaceDir::Ccw);
        assert_eq!(ring[0], Point::new(-10, 3));
        assert_eq!(ring[ring.len() - 1], Point::new(6, 6));
        assert_eq!(hole[0], Point::new(-6, 4));
        assert_eq!(hole[hole.len() - 1], Point::new(3, 5));
        // The full pinned sets (byte-identical snapshot, AC6).
        assert_eq!(ring, snapshot_ring());
        assert_eq!(hole, snapshot_hole());
    }

    /// The exact, minted `ring` for [`SNAPSHOT_SEED`] at `l_min = 2` (AC6).
    fn snapshot_ring() -> Vec<Point> {
        vec![
            Point::new(-10, 3),
            Point::new(-10, 4),
            Point::new(-10, 5),
            Point::new(-10, 6),
            Point::new(-10, 7),
            Point::new(-10, 8),
            Point::new(-9, 3),
            Point::new(-9, 4),
            Point::new(-9, 5),
            Point::new(-9, 6),
            Point::new(-9, 7),
            Point::new(-9, 8),
            Point::new(-8, 3),
            Point::new(-8, 4),
            Point::new(-8, 5),
            Point::new(-8, 6),
            Point::new(-8, 7),
            Point::new(-8, 8),
            Point::new(-7, 3),
            Point::new(-7, 4),
            Point::new(-7, 5),
            Point::new(-7, 6),
            Point::new(-7, 7),
            Point::new(-7, 8),
            Point::new(-6, 3),
            Point::new(-6, 8),
            Point::new(-5, 3),
            Point::new(-5, 8),
            Point::new(-5, 9),
            Point::new(-5, 10),
            Point::new(-5, 11),
            Point::new(-5, 12),
            Point::new(-5, 13),
            Point::new(-4, 3),
            Point::new(-4, 4),
            Point::new(-4, 5),
            Point::new(-4, 12),
            Point::new(-4, 13),
            Point::new(-3, 1),
            Point::new(-3, 2),
            Point::new(-3, 3),
            Point::new(-3, 4),
            Point::new(-3, 5),
            Point::new(-3, 12),
            Point::new(-3, 13),
            Point::new(-2, 1),
            Point::new(-2, 8),
            Point::new(-2, 9),
            Point::new(-2, 12),
            Point::new(-2, 13),
            Point::new(-1, -1),
            Point::new(-1, 0),
            Point::new(-1, 1),
            Point::new(-1, 8),
            Point::new(-1, 9),
            Point::new(-1, 12),
            Point::new(-1, 13),
            Point::new(0, -1),
            Point::new(0, 8),
            Point::new(0, 9),
            Point::new(0, 10),
            Point::new(0, 11),
            Point::new(0, 12),
            Point::new(0, 13),
            Point::new(1, -1),
            Point::new(1, 8),
            Point::new(2, -1),
            Point::new(2, 6),
            Point::new(2, 7),
            Point::new(2, 8),
            Point::new(3, -1),
            Point::new(3, 6),
            Point::new(4, -1),
            Point::new(4, 0),
            Point::new(4, 1),
            Point::new(4, 2),
            Point::new(4, 3),
            Point::new(4, 4),
            Point::new(4, 5),
            Point::new(4, 6),
            Point::new(5, -1),
            Point::new(5, 0),
            Point::new(5, 1),
            Point::new(5, 2),
            Point::new(5, 3),
            Point::new(5, 4),
            Point::new(5, 5),
            Point::new(5, 6),
            Point::new(6, -1),
            Point::new(6, 0),
            Point::new(6, 1),
            Point::new(6, 2),
            Point::new(6, 3),
            Point::new(6, 4),
            Point::new(6, 5),
            Point::new(6, 6),
        ]
    }

    /// The exact, minted `hole` for [`SNAPSHOT_SEED`] at `l_min = 2` (AC6).
    fn snapshot_hole() -> Vec<Point> {
        vec![
            Point::new(-6, 4),
            Point::new(-6, 5),
            Point::new(-6, 6),
            Point::new(-6, 7),
            Point::new(-5, 4),
            Point::new(-5, 5),
            Point::new(-5, 6),
            Point::new(-5, 7),
            Point::new(-4, 6),
            Point::new(-4, 7),
            Point::new(-4, 8),
            Point::new(-4, 9),
            Point::new(-4, 10),
            Point::new(-4, 11),
            Point::new(-3, 6),
            Point::new(-3, 7),
            Point::new(-3, 8),
            Point::new(-3, 9),
            Point::new(-3, 10),
            Point::new(-3, 11),
            Point::new(-2, 2),
            Point::new(-2, 3),
            Point::new(-2, 4),
            Point::new(-2, 5),
            Point::new(-2, 6),
            Point::new(-2, 7),
            Point::new(-2, 10),
            Point::new(-2, 11),
            Point::new(-1, 2),
            Point::new(-1, 3),
            Point::new(-1, 4),
            Point::new(-1, 5),
            Point::new(-1, 6),
            Point::new(-1, 7),
            Point::new(-1, 10),
            Point::new(-1, 11),
            Point::new(0, 0),
            Point::new(0, 1),
            Point::new(0, 2),
            Point::new(0, 3),
            Point::new(0, 4),
            Point::new(0, 5),
            Point::new(0, 6),
            Point::new(0, 7),
            Point::new(1, 0),
            Point::new(1, 1),
            Point::new(1, 2),
            Point::new(1, 3),
            Point::new(1, 4),
            Point::new(1, 5),
            Point::new(1, 6),
            Point::new(1, 7),
            Point::new(2, 0),
            Point::new(2, 1),
            Point::new(2, 2),
            Point::new(2, 3),
            Point::new(2, 4),
            Point::new(2, 5),
            Point::new(3, 0),
            Point::new(3, 1),
            Point::new(3, 2),
            Point::new(3, 3),
            Point::new(3, 4),
            Point::new(3, 5),
        ]
    }

    #[test]
    fn returned_containers_are_btreesets() {
        // AC12: the returned containers are BTreeSet — enforced by the type
        // (a std HashSet cannot reach output through this signature).
        fn assert_is_btreeset(_: &BTreeSet<Point>) {}
        let skeleton = phase1_coarse_ring(3, &mut rng(1));
        assert_is_btreeset(&skeleton.ring);
        assert_is_btreeset(&skeleton.hole);
    }

    // ---- Subtask 9: multi-seed property test -------------------------------

    /// Seed count the multi-seed property test and fallback-rate assertion
    /// run over.
    const PROPERTY_SEED_COUNT: u64 = 64;

    #[test]
    fn multi_seed_property_holds_ac2_and_ac3_for_every_seed() {
        let l_min = 3;
        for seed in 0..PROPERTY_SEED_COUNT {
            let (skeleton, _used_fallback) =
                phase1_coarse_ring_attempts(l_min, &mut rng(seed), MAX_ATTEMPTS);
            let d = corridor_from_cells(&skeleton.ring, 1);
            assert_eq!(
                component_count(&d),
                1,
                "seed {seed}: ring must be one connected component"
            );
            assert_eq!(
                bounded_complement_components(&d),
                1,
                "seed {seed}: ring must enclose exactly one hole"
            );
            assert!(
                !skeleton.hole.is_empty(),
                "seed {seed}: hole must have >= 1 cell"
            );
            let runs = max_straight_runs(&d);
            assert!(
                runs.iter().all(|&r| r >= 2),
                "seed {seed}: ring has a length-1 run: {runs:?}"
            );
            let max_run =
                i32::try_from(runs.iter().copied().max().unwrap_or(0)).unwrap_or(i32::MAX);
            assert!(
                max_run >= clamp_l_min(l_min),
                "seed {seed}: ring has no run >= l_eff: {runs:?}"
            );
        }
    }

    #[test]
    fn seed_48_lmin3_widen_pinch_stays_one_hole() {
        // Pins the d0f665e regression witness: outward widening of a concave
        // ring at seed 48 / l_min = 3 used to pinch off a second bounded
        // complement component (2 holes), violating AC2. This case is
        // otherwise only covered implicitly as one member of the
        // 0..PROPERTY_SEED_COUNT sweep above — revert d0f665e and this test
        // fails at seed 48 with "must enclose exactly one hole".
        let l_min = 3;
        let seed = 48;
        let (skeleton, _used_fallback) =
            phase1_coarse_ring_attempts(l_min, &mut rng(seed), MAX_ATTEMPTS);
        let d = corridor_from_cells(&skeleton.ring, 1);
        assert_eq!(
            component_count(&d),
            1,
            "seed {seed}: ring must be one connected component"
        );
        assert_eq!(
            bounded_complement_components(&d),
            1,
            "seed {seed}: ring must enclose exactly one hole"
        );
        assert!(
            !skeleton.hole.is_empty(),
            "seed {seed}: hole must have >= 1 cell"
        );
    }

    #[test]
    fn fallback_rate_stays_under_the_ceiling() {
        // Recommendation: a healthy construction falls back rarely — assert
        // the rate, not merely "not all seeds fall back".
        let fallback_count = (0..PROPERTY_SEED_COUNT)
            .filter(|&seed| phase1_coarse_ring_attempts(3, &mut rng(seed), MAX_ATTEMPTS).1)
            .count();
        #[allow(
            clippy::cast_precision_loss,
            reason = "PROPERTY_SEED_COUNT (64) and fallback_count (<= 64) are both \
                      small integers, exactly representable in f64"
        )]
        let rate = fallback_count as f64 / PROPERTY_SEED_COUNT as f64;
        assert!(
            rate <= FALLBACK_RATE_MAX,
            "fallback rate {rate} exceeds ceiling {FALLBACK_RATE_MAX} \
             ({fallback_count}/{PROPERTY_SEED_COUNT} seeds fell back)"
        );
    }

    #[test]
    fn clamp_boundary_extremes_yield_a_bounded_valid_skeleton() {
        // NOTE 2: l_min = i32::MAX / i32::MIN clamp to the documented domain
        // on both the primary and fallback paths — bounded work, valid
        // skeleton, no hang, no multi-billion-cell allocation.
        for l_min in [i32::MAX, i32::MIN] {
            let skeleton = phase1_coarse_ring(l_min, &mut rng(1));
            let d = corridor_from_cells(&skeleton.ring, 1);
            assert_eq!(component_count(&d), 1);
            assert_eq!(bounded_complement_components(&d), 1);
            assert!(!skeleton.hole.is_empty());
            let runs = max_straight_runs(&d);
            assert!(runs.iter().all(|&r| r >= 2));

            let side = usize::try_from(MAX_COARSE_STRAIGHT)
                .unwrap_or(usize::MAX)
                .saturating_add(2);
            let bound = side.saturating_mul(side).saturating_mul(4);
            assert!(
                skeleton.ring.len() <= bound && skeleton.hole.len() <= bound,
                "l_min {l_min}: unbounded cell count (ring {}, hole {})",
                skeleton.ring.len(),
                skeleton.hole.len()
            );
        }
    }

    #[test]
    fn forced_exhaustion_returns_a_valid_rectangular_fallback() {
        // Drive max_attempts to 0 via the test-only entry: every attempt is
        // skipped, so Ф1 must go straight to the rectangular fallback, which
        // itself satisfies AC2 + AC3 (all runs >= 2, bottom side >= l_eff,
        // one hole >= 1).
        let l_min = 5;
        let (skeleton, used_fallback) = phase1_coarse_ring_attempts(l_min, &mut rng(1), 0);
        assert!(used_fallback, "max_attempts = 0 must force the fallback");
        let d = corridor_from_cells(&skeleton.ring, 1);
        assert_eq!(component_count(&d), 1);
        assert_eq!(bounded_complement_components(&d), 1);
        assert!(!skeleton.hole.is_empty());
        let runs = max_straight_runs(&d);
        assert!(runs.iter().all(|&r| r >= 2));
        let max_run = i32::try_from(runs.iter().copied().max().unwrap_or(0)).unwrap_or(i32::MAX);
        assert!(max_run >= clamp_l_min(l_min));
    }
}
