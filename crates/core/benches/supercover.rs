//! Differential benchmark: the optimized [`supercover`] against the
//! bounding-box scan it replaced (PRs #171/#172), measured **through the two
//! shapes production actually calls it in**.
//!
//! # Running
//!
//! Local only — CI never invokes `cargo bench`. `target-cpu=native` is passed on
//! the command line rather than parked in `.cargo/config.toml` on purpose: a
//! config-level `rustflags` would apply to *every* build on this host, including
//! `gp-render`'s float paths, where enabling FMA contraction could shift
//! golden-image pixels.
//!
//! ```text
//! RUSTFLAGS="-C target-cpu=native" cargo bench -p gp-core --bench supercover
//! ```
//!
//! `cargo bench` builds under the `bench` profile, which the workspace pins to
//! `opt-level = 3` (inherited from `release`) plus `lto = "thin"` and
//! `codegen-units = 1`. Those live on `[profile.bench]`, not `[profile.release]`,
//! so benchmarking settings never silently change the shipped binary's codegen.
//! To compare against a saved run:
//!
//! ```text
//! RUSTFLAGS="-C target-cpu=native" cargo bench -p gp-core --bench supercover -- --save-baseline before
//! RUSTFLAGS="-C target-cpu=native" cargo bench -p gp-core --bench supercover -- --baseline before
//! ```
//!
//! # What is measured
//!
//! Not `supercover` in isolation — it returns a lazy iterator, so timing it
//! alone would measure iterator *construction* and nothing else. Each group
//! instead reproduces a real consumer:
//!
//! - **`legal_move`** (`sim/mod.rs`) — `supercover(a, b).all(|c| d.contains(c))`.
//!   The hot path: every legality check and every oracle graph edge. Benched both
//!   fully-inside-`D` (the walk runs to completion) and blocked (the fold
//!   short-circuits), because the two implementations visit cells in different
//!   orders and therefore hit a given wall at different points.
//! - **`respawn_cell`** (`sim/mod.rs`) — collects the walk into a `Vec`, then
//!   takes a min over projections and a `max_by_key`. Runs on every crash.
//!
//! # Reading the numbers
//!
//! At production velocities the win is bounded: `--v-target` is capped at 10 and
//! Ф5b's deepening reaches `V_ceil = 16`, so the bounding box the old scan walked
//! was at most a few hundred cells. The `long_chord` group is deliberately
//! *outside* that domain — it exists to show the asymptotic separation
//! (`O(bbox area)` vs `O(cells yielded)`) that the realistic groups cannot.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gp_core::geom::{Corridor, Point, supercover};
use std::hint::black_box;

/// The pre-#171 `supercover`, copied verbatim from `geom/mod.rs` at `e8620f1^`.
///
/// Deliberately duplicated from the copy in `geom/supercover.rs`'s
/// `supercover_equivalence` test module: a `#[cfg(test)]` item is not compiled
/// for a `benches/` target, and exposing it from the library just to share it
/// would put frozen historical code on the public API. Both copies are the same
/// dead implementation and neither will change again.
///
/// Returns `impl Iterator`, exactly as the shipped original did — collecting into
/// a `Vec` here instead would charge the old implementation an allocation it
/// never paid, and would defeat the short-circuit the `legal_move` shape relies on.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "verbatim copy of the shipped pre-#171 implementation, benched only \
              on bounded chords well inside its documented domain"
)]
fn supercover_reference(a: Point, b: Point) -> impl Iterator<Item = Point> {
    let dx = i64::from(b.x) - i64::from(a.x);
    let dy = i64::from(b.y) - i64::from(a.y);
    let bound = dx.abs() + dy.abs();
    (a.x.min(b.x)..=a.x.max(b.x)).flat_map(move |cx| {
        (a.y.min(b.y)..=a.y.max(b.y)).filter_map(move |cy| {
            let cr = dx * (i64::from(cy) - i64::from(a.y)) - dy * (i64::from(cx) - i64::from(a.x));
            (2 * cr.abs() <= bound).then(|| Point::new(cx, cy))
        })
    })
}

/// Corridor edge, in cells.
///
/// `[measured]` the worst Ф1 coarse-ring extent is 72 blocks and `--block-size`
/// is clamped to `[1, 32]`, so a generated corridor spans at most ~2304 cells per
/// axis; 512 is a comfortable mid-size track that keeps the backing `Vec<bool>`
/// cache-resident enough not to swamp the walk being measured.
const CORRIDOR_EDGE: usize = 512;

/// Where the benched chords start — off the origin, so the absolute coordinates
/// feeding #172's `pre_center` hoist are non-trivial.
const ANCHOR: Point = Point::new(101, 97);

/// Per-move velocities, in the domain production actually produces.
///
/// `--v-target` is clamped to `[3, 10]` (`gp-game` `config::V_TARGET_{MIN,MAX}`)
/// and Ф5b's iterative deepening doubles `V_ceil` through `1, 2, 4, 8, 16`. The
/// spread covers axial, shallow, steep, and exact-diagonal chords, since the
/// implementation picks its loop axis from `|dx| >= |dy|`.
const MOVE_VELOCITIES: [(i32, i32); 7] =
    [(1, 0), (3, 2), (5, -4), (7, 3), (10, 10), (16, 1), (16, 16)];

/// Chord lengths outside the production domain, for the asymptotic comparison.
const LONG_CHORDS: [(i32, i32); 3] = [(64, 64), (128, 96), (200, 200)];

/// A fully drivable corridor big enough to hold every chord benched here.
fn filled_corridor() -> Corridor {
    Corridor::filled(Point::new(0, 0), CORRIDOR_EDGE, CORRIDOR_EDGE)
}

/// `legal_move`'s fold (`sim/mod.rs:107`), parameterised by the walk.
///
/// Takes the iterator **by value and generically** so each variant monomorphises
/// and inlines exactly as it does in production. An earlier cut passed a
/// `Box<dyn Iterator>` to unify the two walk types behind one closure; that
/// charged both sides a dynamic dispatch per cell and suppressed the inlining
/// the reference's nested `flat_map`/`filter_map` depends on, making the two
/// benchmark groups incomparable. Do not reintroduce the boxing.
fn legal_move_shape(d: &Corridor, mut cover: impl Iterator<Item = Point>) -> bool {
    cover.all(|c| d.contains(c))
}

/// `respawn_cell`'s body (`sim/mod.rs:504-521`), parameterised by the walk.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "mirrors respawn_cell's own documented bounded-chord precondition — \
              the projection vx*(c.x-a.x) + vy*(c.y-a.y) cannot overflow i64 on \
              the chords benched here"
)]
fn respawn_shape(d: &Corridor, a: Point, b: Point, cover: Vec<Point>) -> Point {
    let vx = i64::from(b.x) - i64::from(a.x);
    let vy = i64::from(b.y) - i64::from(a.y);
    let proj =
        |c: Point| vx * (i64::from(c.x) - i64::from(a.x)) + vy * (i64::from(c.y) - i64::from(a.y));
    let t_block = cover
        .iter()
        .filter(|&&c| !d.contains(c))
        .map(|&c| proj(c))
        .min()
        .unwrap_or(i64::MAX);
    cover
        .into_iter()
        .filter(|&c| proj(c) < t_block)
        .max_by_key(|&c| (proj(c), c.x, c.y))
        .unwrap_or(a)
}

/// `ANCHOR` displaced by `(dx, dy)`.
const fn target(dx: i32, dy: i32) -> Point {
    Point::new(ANCHOR.x.saturating_add(dx), ANCHOR.y.saturating_add(dy))
}

/// `legal_move` over a corridor with no walls — the fold never short-circuits, so
/// this times the complete walk. The common case: most polled moves are legal.
fn bench_legal_move_full(c: &mut Criterion) {
    let d = filled_corridor();
    let mut g = c.benchmark_group("legal_move/no_short_circuit");
    for (dx, dy) in MOVE_VELOCITIES {
        let b = target(dx, dy);
        let id = format!("v=({dx},{dy})");
        g.bench_with_input(BenchmarkId::new("optimized", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("reference", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_reference(ANCHOR, black_box(b))));
        });
    }
    g.finish();
}

/// `legal_move` against a wall planted one cell off the chord's midpoint.
///
/// The two implementations emit cells in different orders, so they reach that
/// wall at different points in the fold — this is the group that says whether the
/// rewrite helped or hurt the *rejecting* path, which the no-wall group cannot.
fn bench_legal_move_blocked(c: &mut Criterion) {
    let mut g = c.benchmark_group("legal_move/short_circuit_on_wall");
    for (dx, dy) in MOVE_VELOCITIES {
        let b = target(dx, dy);
        let mut d = filled_corridor();
        // A cell the chord genuinely touches, so both walks must reject.
        let blocker = supercover(ANCHOR, b)
            .nth(usize::try_from(dx.abs().max(dy.abs())).unwrap_or(0) / 2)
            .unwrap_or(b);
        d.set(blocker, false);

        let id = format!("v=({dx},{dy})");
        g.bench_with_input(BenchmarkId::new("optimized", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("reference", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_reference(ANCHOR, black_box(b))));
        });
    }
    g.finish();
}

/// `respawn_cell` — collect, project, min/max. Runs once per crash, and unlike
/// `legal_move` it materialises the whole walk, so allocation volume matters.
fn bench_respawn_cell(c: &mut Criterion) {
    let d = filled_corridor();
    let mut g = c.benchmark_group("respawn_cell/collect_and_project");
    for (dx, dy) in MOVE_VELOCITIES {
        let b = target(dx, dy);
        let id = format!("v=({dx},{dy})");
        g.bench_with_input(BenchmarkId::new("optimized", &id), &b, |bench, &b| {
            bench.iter(|| respawn_shape(&d, ANCHOR, b, supercover(ANCHOR, black_box(b)).collect()));
        });
        g.bench_with_input(BenchmarkId::new("reference", &id), &b, |bench, &b| {
            bench.iter(|| {
                respawn_shape(
                    &d,
                    ANCHOR,
                    b,
                    supercover_reference(ANCHOR, black_box(b)).collect(),
                )
            });
        });
    }
    g.finish();
}

/// Chords longer than any legal move — outside the production domain, included
/// only to expose the `O(bbox area)` vs `O(cells yielded)` separation that the
/// realistic groups are too small to show. Do not read these as a product win.
fn bench_long_chords(c: &mut Criterion) {
    let d = filled_corridor();
    let mut g = c.benchmark_group("long_chord/not_production_reachable");
    for (dx, dy) in LONG_CHORDS {
        let b = target(dx, dy);
        let id = format!("d=({dx},{dy})");
        g.bench_with_input(BenchmarkId::new("optimized", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("reference", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_reference(ANCHOR, black_box(b))));
        });
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_legal_move_full,
    bench_legal_move_blocked,
    bench_respawn_cell,
    bench_long_chords
);
criterion_main!(benches);
