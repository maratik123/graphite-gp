//! Ф1 — coarse-block ring (infield-first), design doc §2.
//!
//! Produces the coarse skeleton `{ ring, hole, dir }` at **coarse-block**
//! granularity: `ring` is the annulus, `hole` is the enclosed infield polyomino
//! `P`, and `dir` is the fixed global traversal orientation. The `k×k` fine
//! expansion to the actual corridor `D` is Ф2 — out of scope here.

use std::collections::{BTreeSet, HashSet};

use gp_core::geom::{Corridor, Point, Side, walls_from_boundary};
use gp_core::track::RaceDir;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;

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
pub fn phase1_coarse_ring(l_min: i32, rng: &mut ChaCha8Rng) -> CoarseSkeleton {
    let (_p, _l_eff, _base_w) = build_p(l_min, rng);
    todo!("Ф1 pipeline: dilate + widen + check/fallback + orientation (subtask 7)")
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

    /// The maximal straight border run lengths of `d`, grouped by
    /// `(side, fixed coordinate)` into contiguous runs along the varying
    /// axis. A small test-only helper mirroring the design's step-6
    /// verification (used again by later subtasks).
    pub(super) fn max_straight_runs(d: &Corridor) -> Vec<usize> {
        use std::collections::BTreeMap;
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
        for coords in by_key.into_values() {
            let mut coords = coords;
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
}
