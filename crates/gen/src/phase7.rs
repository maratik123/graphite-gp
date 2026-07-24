//! Ф7: render-only racing centerline producer (design doc §2 line 191:
//! `centerline = racing_line(medial_axis(D))`).
//!
//! [`medial_axis`] now arrives connected and thin (DT-ordered anchored
//! thinning, `ai-docs/plans/2026-07-24-gp-gen-generate-pipeline.design.md`
//! § A1) — for a corridor with exactly one bounded hole it is already one
//! 4-connected loop, no bridging needed. [`racing_line`] still runs the same
//! pipeline for the residual cases (a genuinely disconnected medial set, or
//! spur branches on infield-finger / hairpin tracks): bridge cross-component
//! gaps only when the medial set is disconnected (a minimal 4-connected
//! rectilinear path) → prune degree-1 spurs → walk a straightest-continuation
//! cycle anchored at `gate`'s forward face → orient by integer shoelace
//! winding vs `race_dir` → resample by arc length → wraparound unit tangents.
//! Every failure path (empty medial axis, an unbridgeable gap, an empty
//! post-prune core, or a walk that cannot close) returns
//! [`Centerline::default()`] — render-only, `Centerline::at` already degrades
//! gracefully on an empty centerline; this producer never panics.

use std::collections::BTreeSet;

use gp_core::geom::{Corridor, DistanceTransform, Point, medial_axis};
use gp_core::track::{Centerline, CenterlineSample, RaceDir, TimingGate};

/// Maximum Manhattan gap (in cells) [`bridge_gaps`] will bridge between two
/// cross-component medial-axis cells; a wider minimal gap abandons bridging
/// (fallback to [`Centerline::default()`]). Sized against the hand-built
/// 4-strip unit-test fixture (`bridge_gaps_joins_annulus_corner_gaps_into_one_component`)
/// — its nearest cross-strip corner pair (e.g. `(3, 1)` to `(1, 3)`) is
/// Manhattan `4`; this is that value plus a small margin. (The real
/// `medial_axis` of that annulus corridor is no longer 4 disjoint strips —
/// see [`medial_axis`]'s own rustdoc — so this constant now guards only the
/// residual genuinely-disconnected case, not the common one.)
const MAX_BRIDGE_GAP: i32 = 6;

/// The Manhattan distance between `a` and `b`, widened to avoid any overflow
/// concern regardless of the (bounded, grid-realistic) input magnitudes.
fn manhattan(a: Point, b: Point) -> i64 {
    i64::from(a.x.abs_diff(b.x)).saturating_add(i64::from(a.y.abs_diff(b.y)))
}

/// A minimal 4-connected rectilinear path from `a` to `b`, inclusive of both
/// endpoints (`manhattan(a, b) + 1` cells): every `x` step first when
/// `x_first`, else every `y` step first.
fn rectilinear_path(a: Point, b: Point, x_first: bool) -> Vec<Point> {
    let mut path = Vec::with_capacity(1);
    let mut cur = a;
    path.push(cur);
    let step_x = |cur: &mut Point, path: &mut Vec<Point>| {
        while cur.x != b.x {
            cur.x = if b.x > cur.x {
                cur.x.saturating_add(1)
            } else {
                cur.x.saturating_sub(1)
            };
            path.push(*cur);
        }
    };
    let step_y = |cur: &mut Point, path: &mut Vec<Point>| {
        while cur.y != b.y {
            cur.y = if b.y > cur.y {
                cur.y.saturating_add(1)
            } else {
                cur.y.saturating_sub(1)
            };
            path.push(*cur);
        }
    };
    if x_first {
        step_x(&mut cur, &mut path);
        step_y(&mut cur, &mut path);
    } else {
        step_y(&mut cur, &mut path);
        step_x(&mut cur, &mut path);
    }
    path
}

/// The bridging path [`bridge_gaps`] inserts between leaf cells `a` and `b`.
///
/// Tries the `x`-then-`y` [`rectilinear_path`] first and the `y`-then-`x`
/// path second, preferring whichever lies **entirely** within `d`
/// (deterministic: `x`-then-`y` wins a tie). Falls back to the `x`-then-`y`
/// path (its cells still individually filtered by `d.contains` at the call
/// site) if neither lies entirely within `d` — this only under- rather than
/// over-connects, never introduces a cell outside `d`.
///
/// **Why not [`gp_core::geom::supercover`]:** `supercover`'s closed-square
/// touch test is exactly right for a moving car's chord (design doc §3 C4),
/// but over a corridor **thicker** than one cell (the realistic case —
/// generated tracks are `>= n` cells wide, design doc §1) a diagonal corner
/// gap's `supercover(a, b)` touches a small *blob* of cells, not a thin path,
/// and that blob can touch the existing medial ridge at more than one point —
/// creating a degree-3/4 junction that [`prune_spurs`] (degree-`< 2`-only)
/// cannot remove and that can dead-end [`walk_cycle`]'s non-backtracking
/// search (found via this module's own AC7 annulus test: `supercover`-
/// bridging left 12 degree-`> 2` cells around the ring's 4 corners, and the
/// walk failed to close). A minimal single-width rectilinear path only ever
/// adds one new neighbor to each of its interior cells, so it cannot
/// introduce that branching — **why try both axis orders:** a single fixed
/// order (e.g. always `x`-then-`y`) can route through a `¬D` cell (e.g. the
/// annulus's own centre hole) that the *other* order would have gone around,
/// even though a valid in-`D` 4-connected path exists (also found via the
/// AC7 annulus test).
fn bridge_path(d: &Corridor, a: Point, b: Point) -> Vec<Point> {
    let x_first = rectilinear_path(a, b, true);
    if x_first.iter().all(|&p| d.contains(p)) {
        return x_first;
    }
    let y_first = rectilinear_path(a, b, false);
    if y_first.iter().all(|&p| d.contains(p)) {
        return y_first;
    }
    x_first
}

/// The 4-connected components of `cells`, as a `Vec` of disjoint `BTreeSet`s.
///
/// Deterministic: `cells` is walked in `BTreeSet` (`x`-then-`y`) order, so
/// repeated calls on the same input yield components in the same order with
/// the same membership.
fn components(cells: &BTreeSet<Point>) -> Vec<BTreeSet<Point>> {
    let mut remaining = cells.clone();
    let mut comps = Vec::new();
    while let Some(&start) = remaining.iter().next() {
        let mut comp = BTreeSet::new();
        let mut stack = vec![start];
        remaining.remove(&start);
        while let Some(p) = stack.pop() {
            comp.insert(p);
            for n in p.neighbors4() {
                if remaining.remove(&n) {
                    stack.push(n);
                }
            }
        }
        comps.push(comp);
    }
    comps
}

/// Bridges gaps in the medial cell set `medial` (design doc § "The loop-trim
/// / resample algorithm", step 2).
///
/// Repeatedly finds the minimal-Manhattan-distance pair of degree-`< 2`
/// ("leaf") cells `(a, b)` that are not already 4-adjacent, preferring a
/// cross-component pair over a same-component one whenever both exist, and
/// inserts every cell [`bridge_path`] returns for `(d, a, b)` that lies in
/// `d` into the set (deterministic tie-break: minimal `(a, b)` by `Point`'s
/// derived `Ord`,
/// `a <= b`). Stops when fewer than 2 leaves remain.
///
/// **Why same-component pairs too, not only cross-component:** merely
/// reaching one 4-connected component does not imply a *closed* loop — a
/// ring's last corner gap can bridge its two flanking strips into the same
/// component (via the other 3 corners) while leaving that last gap's own two
/// leaf cells unconnected, cross-component-blind bridging would then declare
/// victory one gap early. Preferring cross- over same-component candidates
/// each round still closes the "easy" gaps between genuinely separate
/// pieces first, before ever touching a same-component pair.
///
/// Returns `None` (fallback) if `medial` is empty, or if the smallest
/// candidate gap ever exceeds [`MAX_BRIDGE_GAP`]. Also stops (without
/// panicking or looping) if a chosen candidate's bridge inserts no new cell —
/// a leftover open path (not a ring) has no more available progress.
fn bridge_gaps(d: &Corridor, medial: BTreeSet<Point>) -> Option<BTreeSet<Point>> {
    if medial.is_empty() {
        return None;
    }
    let mut cells = medial;
    loop {
        let leaves: Vec<Point> = cells
            .iter()
            .copied()
            .filter(|&p| degree(&cells, p) < 2)
            .collect();
        if leaves.len() < 2 {
            return Some(cells);
        }
        let comps = components(&cells);
        let comp_of = |p: Point| comps.iter().position(|c| c.contains(&p));

        let mut cross: Option<(i64, Point, Point)> = None;
        let mut same: Option<(i64, Point, Point)> = None;
        for i in 0..leaves.len() {
            for j in (i.saturating_add(1))..leaves.len() {
                let (a, b) = (leaves[i], leaves[j]);
                if a.neighbors4().into_iter().any(|n| n == b) {
                    continue; // already directly connected; nothing to bridge
                }
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                let cand = (manhattan(a, b), lo, hi);
                let slot = if comp_of(a) == comp_of(b) {
                    &mut same
                } else {
                    &mut cross
                };
                if slot.is_none_or(|cur| cand < cur) {
                    *slot = Some(cand);
                }
            }
        }
        let Some((dist, a, b)) = cross.or(same) else {
            return Some(cells); // no viable leaf pair left to bridge
        };
        if dist > i64::from(MAX_BRIDGE_GAP) {
            return None;
        }
        let before = cells.len();
        for p in bridge_path(d, a, b) {
            if d.contains(p) {
                cells.insert(p);
            }
        }
        if cells.len() == before {
            return Some(cells); // this pair's bridge added nothing new; stop
        }
    }
}

/// The 4-connected degree of `p` within `cells` — the number of `p`'s 4
/// neighbors that are themselves members of `cells`.
fn degree(cells: &BTreeSet<Point>, p: Point) -> usize {
    p.neighbors4()
        .into_iter()
        .filter(|n| cells.contains(n))
        .count()
}

/// Prunes spur (degree-1) branches from the bridged medial set `cells`
/// (design doc § "The loop-trim / resample algorithm", step 3).
///
/// Iterative degree-1 removal (synchronous rounds: each round's degrees are
/// computed against the round's starting set, then every degree-`< 2` cell is
/// removed together) until every surviving cell has 4-connected degree `>= 2`
/// — the graph's 2-core. A clean, already-thinned loop is all-degree-2 (the
/// pass is then a no-op); infield-finger / hairpin spurs are trees hanging
/// off the ring and peel away round by round. Returns `None` (fallback) if
/// the 2-core is empty (nothing survives, e.g. `cells` was itself a tree with
/// no cycle).
fn prune_spurs(cells: &BTreeSet<Point>) -> Option<BTreeSet<Point>> {
    let mut cur = cells.clone();
    loop {
        let leaves: Vec<Point> = cur
            .iter()
            .copied()
            .filter(|&p| degree(&cur, p) < 2)
            .collect();
        if leaves.is_empty() {
            break;
        }
        for p in leaves {
            cur.remove(&p);
        }
    }
    if cur.is_empty() { None } else { Some(cur) }
}

/// The `core` cell nearest (Manhattan) to the centroid of `gate`'s forward
/// face, tie-broken by minimal `Point` (design doc § "The loop-trim /
/// resample algorithm", step 4). `None` if either `core` or the forward face
/// is empty.
#[allow(
    clippy::cast_precision_loss,
    reason = "face.len() is a small, grid-realistic cell count, exactly \
              representable in f64"
)]
fn anchor(core: &BTreeSet<Point>, gate: &TimingGate) -> Option<Point> {
    let face: Vec<Point> = gate.forward_face().collect();
    if face.is_empty() || core.is_empty() {
        return None;
    }
    let n = face.len() as f64;
    let (sx, sy) = face.iter().fold((0.0f64, 0.0f64), |(sx, sy), p| {
        (sx + f64::from(p.x), sy + f64::from(p.y))
    });
    let (cx, cy) = (sx / n, sy / n);
    let dist_to_centroid = |p: Point| (f64::from(p.x) - cx).abs() + (f64::from(p.y) - cy).abs();

    let mut ranked: Vec<(f64, Point)> = core.iter().map(|&p| (dist_to_centroid(p), p)).collect();
    ranked.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    ranked.into_iter().next().map(|(_, p)| p)
}

/// The `(dx, dy)` step from `from` to a 4-connected neighbor `to` — bounded
/// to `{-1, 0, 1}` per axis in every caller here, since `to` is always drawn
/// from `from.neighbors4()`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "to is always one of from.neighbors4() in every call site below, \
              so each axis delta is bounded to {-1, 0, 1} and cannot overflow"
)]
const fn step_delta(from: Point, to: Point) -> (i32, i32) {
    (to.x - from.x, to.y - from.y)
}

/// The straightest-continuation priority rank of step direction `dir`
/// relative to `heading` (design doc step 4): `0` = straight ahead, `1` = a
/// turn, `2` = a U-turn, and `3` when there is no established `heading` yet
/// (every direction ties, so only the `Point`-order tie-break in
/// [`walk_cycle`] discriminates).
#[allow(
    clippy::arithmetic_side_effects,
    reason = "heading and dir are always {-1, 0, 1}-per-axis unit step deltas \
              (see step_delta), so the dot product is bounded to {-1, 0, 1} \
              and cannot overflow"
)]
const fn turn_rank(heading: Option<(i32, i32)>, dir: (i32, i32)) -> u8 {
    let Some(h) = heading else { return 3 };
    match h.0 * dir.0 + h.1 * dir.1 {
        1 => 0,
        0 => 1,
        _ => 2,
    }
}

/// Walks `core` into an ordered, closed cycle starting at [`anchor`] (design
/// doc § "The loop-trim / resample algorithm", step 4).
///
/// At each step, picks the best straightest-continuation candidate among
/// `core` neighbors that are either unvisited or (once at least 4 cells have
/// been walked, the minimum possible 4-connected grid cycle length) the
/// `start` cell itself — closing the loop. Ties are broken by minimal
/// `Point`. Returns `None` (fallback) if `core`/the forward face is empty, if
/// a step dead-ends (no viable candidate), or if the walk exhausts `core`
/// without ever closing.
fn walk_cycle(core: &BTreeSet<Point>, gate: &TimingGate) -> Option<Vec<Point>> {
    let start = anchor(core, gate)?;
    let mut order = vec![start];
    let mut visited = BTreeSet::from([start]);
    let mut heading: Option<(i32, i32)> = None;

    loop {
        let current = *order.last().expect("order is never empty");
        let mut candidates: Vec<Point> = current
            .neighbors4()
            .into_iter()
            .filter(|n| core.contains(n))
            .filter(|&n| !visited.contains(&n) || (n == start && order.len() >= 4))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by_key(|&cand| {
            let dir = step_delta(current, cand);
            (turn_rank(heading, dir), cand)
        });
        let next = candidates[0];
        if next == start {
            break;
        }
        heading = Some(step_delta(current, next));
        visited.insert(next);
        order.push(next);
        if order.len() > core.len() {
            return None; // safety: cannot exceed core's own size without closing
        }
    }

    if order.len() < 4 { None } else { Some(order) }
}

/// Arc-length spacing (in cells) between resampled `CenterlineSample`s —
/// approximately one cell, per the design's "even spacing" requirement (AC2).
const RESAMPLE_STEP: f32 = 1.0;

/// The integer shoelace signed area `Σ(xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ)` of the closed
/// polygon `order` (design doc step 5). This grid is x-east/y-north
/// right-handed, so the standard convention holds: `> 0` for a
/// counter-clockwise ring, `< 0` for clockwise (pinned by
/// `shoelace_sign_matches_ccw_convention` below). All-`saturating` integer
/// arithmetic — no raw `+`/`-`/`*` — so no `arithmetic_side_effects` allow is
/// needed.
fn shoelace(order: &[Point]) -> i64 {
    order
        .iter()
        .zip(order.iter().skip(1).chain(order.iter().take(1)))
        .fold(0i64, |sum, (&a, &b)| {
            let term = i64::from(a.x)
                .saturating_mul(i64::from(b.y))
                .saturating_sub(i64::from(b.x).saturating_mul(i64::from(a.y)));
            sum.saturating_add(term)
        })
}

/// Orients the closed cycle `order` to match `race_dir`'s winding sense
/// (design doc step 5): reverses `order` iff its shoelace sign disagrees with
/// `race_dir` (`Ccw` ⇔ `> 0`, `Cw` ⇔ `< 0`).
fn orient(mut order: Vec<Point>, race_dir: RaceDir) -> Vec<Point> {
    let is_ccw = shoelace(&order) > 0;
    let wants_ccw = matches!(race_dir, RaceDir::Ccw);
    if is_ccw != wants_ccw {
        order.reverse();
    }
    order
}

/// The sub-cell `(f32, f32)` position of cell center `p`.
#[allow(
    clippy::cast_precision_loss,
    reason = "grid coordinates are bounded by corridor dimensions, far below \
              f32's 24-bit exact-integer range"
)]
const fn point_pos(p: Point) -> (f32, f32) {
    (p.x as f32, p.y as f32)
}

/// The next index after `i` in a `n`-length wraparound cycle.
const fn next_index(i: usize, n: usize) -> usize {
    let j = i.saturating_add(1);
    if j < n { j } else { 0 }
}

/// The index before `i` in a `n`-length wraparound cycle.
const fn prev_index(i: usize, n: usize) -> usize {
    if i == 0 {
        n.saturating_sub(1)
    } else {
        i.saturating_sub(1)
    }
}

/// A unit `(f32, f32)` vector along `v`, or the flat fallback `(1.0, 0.0)`
/// when `v` is degenerate (zero length).
fn normalize_vec(v: (f32, f32)) -> (f32, f32) {
    let len = v.0.hypot(v.1);
    if len > 0.0 {
        (v.0 / len, v.1 / len)
    } else {
        (1.0, 0.0)
    }
}

/// Resamples the ordered, closed cycle `order` by arc length (design doc
/// steps 6-7): emits a `CenterlineSample` every [`RESAMPLE_STEP`] of
/// accumulated perimeter, seeding `samples[0].s == 0`, then fills each
/// sample's unit tangent from its wraparound neighbors' positions. `None`
/// (fallback) if `order` is empty or its perimeter is non-positive/non-finite
/// (a degenerate, zero-area cycle).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the sample count is length/RESAMPLE_STEP floored and clamped to \
              >= 1: a small, grid-realistic, non-negative cell count, exactly \
              representable in f32 and well within usize range"
)]
fn resample(order: &[Point]) -> Option<Centerline> {
    let positions: Vec<(f32, f32)> = order.iter().map(|&p| point_pos(p)).collect();
    let n = positions.len();
    if n == 0 {
        return None;
    }
    let seg_lengths: Vec<f32> = positions
        .iter()
        .zip(positions.iter().skip(1).chain(positions.iter().take(1)))
        .map(|(&(ax, ay), &(bx, by))| (bx - ax).hypot(by - ay))
        .collect();
    let length: f32 = seg_lengths.iter().copied().sum();
    if length.is_sign_negative() || length == 0.0 || !length.is_finite() {
        return None;
    }
    let mut cum = vec![0.0f32; n];
    for i in 1..n {
        cum[i] = cum[prev_index(i, n)] + seg_lengths[prev_index(i, n)];
    }

    let step_count = (length / RESAMPLE_STEP).floor().max(1.0) as usize;
    let mut samples = Vec::with_capacity(step_count);
    for i in 0..step_count {
        let s = i as f32 * RESAMPLE_STEP;
        let idx = cum
            .iter()
            .rposition(|&c| c <= s)
            .unwrap_or(0)
            .min(n.saturating_sub(1));
        let (ax, ay) = positions[idx];
        let (bx, by) = positions[next_index(idx, n)];
        let seg = seg_lengths[idx];
        let t = if seg > 0.0 { (s - cum[idx]) / seg } else { 0.0 };
        let pos = ((bx - ax).mul_add(t, ax), (by - ay).mul_add(t, ay));
        samples.push(CenterlineSample {
            s,
            pos,
            tangent: (0.0, 0.0), // filled below, once every sample position exists
        });
    }

    let m = samples.len();
    let sample_positions: Vec<(f32, f32)> = samples.iter().map(|sm| sm.pos).collect();
    for (i, sample) in samples.iter_mut().enumerate() {
        let (px, py) = sample_positions[prev_index(i, m)];
        let (nx, ny) = sample_positions[next_index(i, m)];
        sample.tangent = normalize_vec((nx - px, ny - py));
    }

    Some(Centerline { samples, length })
}

/// Produces the render-only racing centerline for corridor `d` (design doc §2
/// line 191).
///
/// Computes the distance transform + medial axis internally, bridges the
/// medial set only when it is genuinely disconnected (`components(&medial)`
/// has more than one component — the common case, a corridor with exactly
/// one bounded hole, is already one connected loop out of `medial_axis`),
/// trims and orders the result into a single closed loop anchored at `gate`'s
/// forward face and oriented along `race_dir`, then resamples it by arc
/// length. Never panics: every failure path (empty medial axis, an
/// unbridgeable gap, an empty post-prune core, or a walk that cannot close)
/// returns [`Centerline::default()`], which degrades gracefully under
/// [`Centerline::at`].
pub fn racing_line(d: &Corridor, gate: &TimingGate, race_dir: RaceDir) -> Centerline {
    let dt = DistanceTransform::compute(d);
    let medial = medial_axis(&dt);
    if medial.is_empty() {
        return Centerline::default();
    }
    let bridged = if components(&medial).len() > 1 {
        bridge_gaps(d, medial)
    } else {
        Some(medial)
    };
    let Some(bridged) = bridged else {
        return Centerline::default();
    };
    let Some(core) = prune_spurs(&bridged) else {
        return Centerline::default();
    };
    let Some(cycle) = walk_cycle(&core, gate) else {
        return Centerline::default();
    };
    let oriented = orient(cycle, race_dir);
    resample(&oriented).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    include!("phase7_tests.rs");
}
