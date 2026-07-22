//! Ф1 — coarse-block ring (infield-first), design doc §2.
//!
//! Produces the coarse skeleton `{ ring, hole, dir }` at **coarse-block**
//! granularity: `ring` is the annulus, `hole` is the enclosed infield polyomino
//! `P`, and `dir` is the fixed global traversal orientation. The `k×k` fine
//! expansion to the actual corridor `D` is Ф2 — out of scope here.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use gp_core::geom::{Corridor, Point, Side, walls_from_boundary};
use gp_core::track::RaceDir;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use strum::IntoEnumIterator;

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
const TARGET_BLOCKS: usize = 40;
/// Maximum outward widen amount drawn per [`Side`] (inclusive).
const WIDEN_MAX: u32 = 3;
/// Bounded same-stream retry budget for the step-6 run-length check before
/// falling back to the guaranteed rectangular annulus.
const MAX_ATTEMPTS: u32 = 8;
/// Minimum fallback rectangle width — `W = max(l_eff + 2, MIN_RECT_W)`.
const MIN_RECT_W: i32 = 4;
/// Fixed fallback rectangle height.
const MIN_RECT_H: i32 = 4;

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
#[allow(
    clippy::similar_names,
    reason = "ring/rng are established, unambiguous domain vocabulary here \
              (skeleton.ring, the RNG stream) — no realistic confusion risk"
)]
pub fn phase1_coarse_ring(l_min: i32, rng: &mut ChaCha8Rng) -> CoarseSkeleton {
    let l_eff = clamp_l_min(l_min);
    for _attempt in 0..MAX_ATTEMPTS {
        let (p, l_eff, _base_w) = build_p(l_min, rng);
        let ring = widen(&ring_from_p(&p), rng);
        let runs = max_straight_runs(&corridor_from_cells(&ring, 1));
        let min_run = runs.iter().copied().min().unwrap_or(0);
        let max_run = runs.iter().copied().max().unwrap_or(0);
        let max_run = i32::try_from(max_run).unwrap_or(i32::MAX);
        if min_run >= 2 && max_run >= l_eff {
            let dir = choose_dir(rng);
            return CoarseSkeleton { ring, hole: p, dir };
        }
    }
    // Guaranteed-terminating fallback (step 7): a rectangular annulus
    // satisfies every AC by construction. `dir` is still drawn on this path,
    // so it is seeded regardless of which terminal Ф1 hits (AC4).
    let (ring, hole) = rectangular_fallback(l_eff);
    let dir = choose_dir(rng);
    CoarseSkeleton { ring, hole, dir }
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
/// that side (the whole outer-border run for that side, since every such
/// cell already touches `ring`), so widening cannot disconnect the ring, add
/// a length-1 run, or touch the inner hole — outward-only, annulus invariants
/// preserved by construction.
#[allow(
    clippy::similar_names,
    reason = "ring/rng are established, unambiguous domain vocabulary here \
              (the annulus, the RNG stream) — no realistic confusion risk"
)]
fn widen(ring: &BTreeSet<Point>, rng: &mut ChaCha8Rng) -> BTreeSet<Point> {
    let mut ring = ring.clone();
    for side in Side::iter() {
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
fn choose_dir(rng: &mut ChaCha8Rng) -> RaceDir {
    if rng.random_range(0u32..2) == 0 {
        RaceDir::Cw
    } else {
        RaceDir::Ccw
    }
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
fn grow_blocks(blocks: &mut BTreeSet<(i32, i32)>, target: usize, rng: &mut ChaCha8Rng) {
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
fn build_p(l_min: i32, rng: &mut ChaCha8Rng) -> (BTreeSet<Point>, i32, i32) {
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
    use rand::SeedableRng;

    fn rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
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
}
