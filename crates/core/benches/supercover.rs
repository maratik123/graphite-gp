//! Benchmark guarding [`supercover`] against re-optimization that does not pay.
//!
//! `gp-core` ships the bounding-box scan. PRs #171/#172 replaced it with a per-row
//! interval solver and were reverted after this benchmark measured the swap as a
//! *loss* at every velocity the product can produce. The rejected walk is kept
//! below as `supercover_v172` so the comparison stays one `cargo bench` away —
//! **if you are about to optimize `supercover`, this file is the acceptance test.**
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
//!
//! # What is measured
//!
//! Not `supercover` in isolation — it returns a lazy iterator, so timing it alone
//! would measure iterator *construction*. Each group reproduces a real consumer:
//!
//! - **`legal_move`** (`sim/mod.rs`) — `supercover(a, b).all(|c| d.contains(c))`.
//!   The hot path: every legality check and every oracle graph edge. Benched both
//!   fully-inside-`D` and blocked, because two walks that visit cells in different
//!   orders reach a given wall at different points.
//! - **`respawn_cell`** (`sim/mod.rs`) — collects the walk, then takes a min over
//!   projections and a `max_by_key`. Runs on every crash.
//!
//! # The result that got #171/#172 reverted
//!
//! `--v-target` caps at 10 and Ф5b's deepening reaches `V_ceil = 16`, so a real
//! chord's bounding box is at most ~17×17 ≈ 289 cells — a tight counted loop LLVM
//! strength-reduces. The interval walk was **1.03–1.29× slower** on `legal_move`
//! and **1.08–1.69× slower** on `respawn_cell`, winning only on chords of 64+ cells
//! that no legal move can produce. A Bresenham variant removing the per-row
//! division landed within 10% of it and did not close the gap.
//!
//! # The axial fast path, also measured and also rejected
//!
//! A horizontal chord walked `x`-outer enters an N-step outer loop whose inner
//! range is a single cell — the worst possible `flat_map` shape, and `[measured]`
//! ~2× off what a direct run costs. Two ways to fix it were tried; both regress
//! the general chord, so neither shipped:
//!
//! | variant | axial | general |
//! |---|---|---|
//! | `enum_axial_xy` (below) — `AxialX`/`AxialY`/`Box` enum, branch-free arms | **2.0–2.7× faster** | **0.53–0.94×** |
//! | loop-order swap, one type, captured `horizontal` flag | 1.0–1.7× faster | 0.56–0.81× |
//!
//! The cause is the same for both, and it is worth not rediscovering: wrapping
//! `FlatMap` in an enum forwards only `next()`, which **costs `FlatMap`'s internal
//! `try_fold` specialisation** — exactly what `legal_move`'s `.all()` and
//! `respawn_cell`'s `.collect()` drive. `try_fold` cannot be overridden on stable
//! (its `R: Try` bound is unstable), so an enum union cannot give it back;
//! `Cover3` below forwards `fold` to show that is not enough. The single-type
//! variant avoids the enum but pays a per-cell branch in the inner closure, which
//! costs about as much.
//!
//! **Do not benchmark through a `Box<dyn Iterator>`.** An early cut unified the two
//! walk types that way; the per-cell dynamic dispatch suppressed inlining and
//! *inverted* the verdict, showing the interval walk 1.2–1.8× faster on the same
//! hardware. Both shapes take their iterator generically, by value.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gp_core::geom::{Corridor, Point, supercover};
use std::hint::black_box;

// ---------------------------------------------------------------------------
// The interval walk, in its best known form, as the baseline to beat.
//
// This is NOT PR #172 verbatim. It is that approach plus every improvement the
// review of PR #171/#172 and of the follow-up Bresenham branch identified, so the
// comparison below is against the strongest version of the idea rather than its
// first draft:
//
//   * Bresenham-style `FloorTracker` — the per-row division is gone, replaced by
//     an add/compare/branch that tracks `floor((x0 + i*step) / m)` incrementally.
//   * `along_delta == 0` is a THIRD enum variant, not an `Option` checked once per
//     row. That case is always a single outer step (a purely axial chord), so it
//     needs no tracker at all — which also deletes the two `.expect()` unwraps the
//     `Option` form required.
//   * The remaining `i64 -> i32` narrowings are total (`unwrap_or`), so this
//     version would not have cost gp-core its zero-production-panics invariant.
//     An out-of-range bound can only mean an empty row, and both sentinels encode
//     exactly that.
//   * No `DoubleEndedIterator`. The trackers make the walk stateful, so a reverse
//     traversal would silently desync; `impl Iterator` hides the impl today, which
//     makes it a trap rather than a bug. Removed instead of documented.
// ---------------------------------------------------------------------------

/// The enum fast path with `Axial` split into `AxialX`/`AxialY`.
///
/// Answers a specific design question: does giving each orientation its own
/// variant — so the per-element `if horizontal` before `Point::new` disappears —
/// pay for the enum? Both axial arms become branch-free `Map<RangeInclusive<_>>`.
/// Measured against `scan_plain` and the shipped loop-order swap below.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "same bounded-chord precondition as the shipped supercover"
)]
fn supercover_enum_axial_xy(a: Point, b: Point) -> impl Iterator<Item = Point> {
    let dx = i64::from(b.x) - i64::from(a.x);
    let dy = i64::from(b.y) - i64::from(a.y);

    if dy == 0 {
        let fixed = a.y;
        return Cover3::AxialX((a.x.min(b.x)..=a.x.max(b.x)).map(move |i| Point::new(i, fixed)));
    }
    if dx == 0 {
        let fixed = a.x;
        return Cover3::AxialY((a.y.min(b.y)..=a.y.max(b.y)).map(move |i| Point::new(fixed, i)));
    }

    let bound = dx.abs() + dy.abs();
    Cover3::Box((a.x.min(b.x)..=a.x.max(b.x)).flat_map(move |cx| {
        (a.y.min(b.y)..=a.y.max(b.y)).filter_map(move |cy| {
            let cr = dx * (i64::from(cy) - i64::from(a.y)) - dy * (i64::from(cx) - i64::from(a.x));
            (2 * cr.abs() <= bound).then(|| Point::new(cx, cy))
        })
    }))
}

/// Three-variant union for [`supercover_enum_axial_xy`].
///
/// Forwards `fold` as well as `next`, so the comparison is not unfairly rigged by
/// the *absence* of delegation. `try_fold` — which `.all()` actually drives —
/// cannot be overridden on stable, and that is the point the measurement makes.
enum Cover3<A, B, C> {
    /// Horizontal chord.
    AxialX(A),
    /// Vertical chord.
    AxialY(B),
    /// General chord.
    Box(C),
}

impl<A, B, C, T> Iterator for Cover3<A, B, C>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
    C: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            Self::AxialX(i) => i.next(),
            Self::AxialY(i) => i.next(),
            Self::Box(i) => i.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::AxialX(i) => i.size_hint(),
            Self::AxialY(i) => i.size_hint(),
            Self::Box(i) => i.size_hint(),
        }
    }

    fn fold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, T) -> Acc,
    {
        match self {
            Self::AxialX(i) => i.fold(init, f),
            Self::AxialY(i) => i.fold(init, f),
            Self::Box(i) => i.fold(init, f),
        }
    }
}

/// The interval walk: one interval solve per outer row, no per-row division.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "same bounded-chord precondition as the shipped supercover — the \
              i64-widened deltas cannot overflow on the chords benched here"
)]
fn supercover_interval(a: Point, b: Point) -> impl Iterator<Item = Point> {
    let dx = i64::from(b.x) - i64::from(a.x);
    let dy = i64::from(b.y) - i64::from(a.y);
    let bound = dx.abs() + dy.abs();
    let (ax, ay) = (i64::from(a.x), i64::from(a.y));

    // A purely axial chord spans exactly one outer coordinate, so it needs no
    // interval machinery — this is the third-variant case.
    let axial = |fixed: i32, horizontal: bool, lo: i32, hi: i32| {
        (lo..=hi).map(move |i| {
            if horizontal {
                Point::new(i, fixed)
            } else {
                Point::new(fixed, i)
            }
        })
    };

    if dx.abs() >= dy.abs() {
        let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
        if dy == 0 {
            return Either3::Axial(axial(a.y, true, lo, hi));
        }
        let mut st = IntervalState::new(
            ay,
            dx,
            ax,
            dy,
            i64::from(lo),
            i64::from(hi),
            bound,
            i64::from(a.y.min(b.y)),
        );
        Either3::MajorX((a.y.min(b.y)..=a.y.max(b.y)).flat_map(move |cy| {
            let (lo, hi) = st.step();
            (lo..=hi).map(move |cx| Point::new(cx, cy))
        }))
    } else {
        let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
        if dx == 0 {
            return Either3::Axial(axial(a.x, false, lo, hi));
        }
        let mut st = IntervalState::new(
            ax,
            dy,
            ay,
            dx,
            i64::from(lo),
            i64::from(hi),
            bound,
            i64::from(a.x.min(b.x)),
        );
        Either3::MajorY((a.x.min(b.x)..=a.x.max(b.x)).flat_map(move |cx| {
            let (lo, hi) = st.step();
            (lo..=hi).map(move |cy| Point::new(cx, cy))
        }))
    }
}

/// Per-row interval state. `along_delta != 0` is a constructor precondition, which
/// is what lets the two trackers be unconditional rather than `Option`s.
struct IntervalState {
    /// Tracks `ceil(low / m)` across rows.
    lo: FloorTracker,
    /// Tracks `floor(high / m)` across rows.
    hi: FloorTracker,
    /// Inner-axis floor.
    inner_min: i64,
    /// Inner-axis ceiling.
    inner_max: i64,
}

impl IntervalState {
    /// Seeds both trackers at the first outer coordinate actually iterated.
    #[allow(clippy::arithmetic_side_effects, reason = "bounded chords only")]
    #[allow(
        clippy::too_many_arguments,
        reason = "bench-local, mirrors the shape under test"
    )]
    fn new(
        outer_origin: i64,
        across_delta: i64,
        inner_origin: i64,
        along_delta: i64,
        inner_min: i64,
        inner_max: i64,
        bound: i64,
        outer_start: i64,
    ) -> Self {
        debug_assert!(along_delta != 0, "axial chords take the third variant");
        // Normalise to a positive divisor: negating both deltas maps
        // (low, high, m) -> (-high, -low, -m), which is the old m < 0 swap.
        let (across_delta, along_delta) = if along_delta < 0 {
            (-across_delta, -along_delta)
        } else {
            (across_delta, along_delta)
        };
        let m = 2 * along_delta;
        let step = 2 * across_delta;
        let center0 =
            2 * (across_delta * (outer_start - outer_origin) + along_delta * inner_origin);
        Self {
            // ceil(x/m) == floor((x + m - 1)/m) for m > 0.
            lo: FloorTracker::new(center0 - bound + m - 1, step, m),
            hi: FloorTracker::new(center0 + bound, step, m),
            inner_min,
            inner_max,
        }
    }

    /// The interval for the current row, then advance. Exactly once per row, in order.
    fn step(&mut self) -> (i32, i32) {
        let lo = self.lo.q.max(self.inner_min);
        let hi = self.hi.q.min(self.inner_max);
        self.lo.advance();
        self.hi.advance();
        // Total by construction: `lo >= inner_min` and `hi <= inner_max` are both
        // i32-derived, so only the far side can escape i32 — and either sentinel
        // yields an empty `lo..=hi`, which is the correct answer for such a row.
        (
            i32::try_from(lo).unwrap_or(i32::MAX),
            i32::try_from(hi).unwrap_or(i32::MIN),
        )
    }
}

/// Tracks `floor((x0 + i * step) / m)` for `i = 0, 1, 2, …` without dividing.
struct FloorTracker {
    /// Current quotient.
    q: i64,
    /// Current remainder, always in `[0, m)`.
    r: i64,
    /// Integer part of one step's increment.
    step_q: i64,
    /// Remainder part of one step's increment, in `[0, m)`.
    step_r: i64,
    /// The (positive) divisor.
    m: i64,
}

impl FloorTracker {
    /// Requires `m > 0`.
    fn new(x0: i64, step: i64, m: i64) -> Self {
        debug_assert!(m > 0);
        Self {
            q: x0.div_euclid(m),
            r: x0.rem_euclid(m),
            step_q: step.div_euclid(m),
            step_r: step.rem_euclid(m),
            m,
        }
    }

    /// One step. `r + step_r < 2m`, so at most one carry.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "r and step_r are both in [0, m)"
    )]
    fn advance(&mut self) {
        let r = self.r + self.step_r;
        let carry = r >= self.m;
        self.r = if carry { r - self.m } else { r };
        self.q += self.step_q + i64::from(carry);
    }
}

/// Three-way iterator union: major-x, major-y, and the axial degenerate case.
///
/// Deliberately implements `Iterator` only. `ExactSizeIterator` could never apply
/// (`FlatMap` is not one), and `DoubleEndedIterator` must not — [`IntervalState`]
/// is stateful and advances forward, so a reverse traversal would desync it and
/// silently return the wrong cells.
enum Either3<A, B, C> {
    /// `|dx| >= |dy|`, inner loop over `x`.
    MajorX(A),
    /// `|dy| > |dx|`, inner loop over `y`.
    MajorY(B),
    /// A purely axial chord — one outer step, no interval solve.
    Axial(C),
}

impl<A, B, C, T> Iterator for Either3<A, B, C>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
    C: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            Self::MajorX(i) => i.next(),
            Self::MajorY(i) => i.next(),
            Self::Axial(i) => i.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::MajorX(i) => i.size_hint(),
            Self::MajorY(i) => i.size_hint(),
            Self::Axial(i) => i.size_hint(),
        }
    }
}

/// Panics unless the baseline agrees with the shipped walk over an exhaustive
/// window. A baseline that has drifted makes every number below meaningless, so
/// this runs on every `cargo bench` before any timing is taken.
fn verify_baseline() {
    const R: i32 = 4;
    for ax in -R..=R {
        for ay in -R..=R {
            for bx in -R..=R {
                for by in -R..=R {
                    let (a, b) = (Point::new(ax, ay), Point::new(bx, by));
                    let mut got: Vec<Point> = supercover_interval(a, b).collect();
                    let mut want: Vec<Point> = supercover(a, b).collect();
                    got.sort_unstable();
                    want.sort_unstable();
                    assert_eq!(got, want, "interval baseline disagrees for {a:?} -> {b:?}");
                }
            }
        }
    }
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
const MOVE_VELOCITIES: [(i32, i32); 11] = [
    // Axial — what the fast path targets.
    (1, 0),
    (0, 1),
    (7, 0),
    (0, -10),
    (16, 0),
    // General — where the extra dispatch must not cost anything.
    (3, 2),
    (5, -4),
    (7, 3),
    (10, 10),
    (16, 1),
    (16, 16),
];

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
    verify_baseline();
    let d = filled_corridor();
    let mut g = c.benchmark_group("legal_move/no_short_circuit");
    for (dx, dy) in MOVE_VELOCITIES {
        let b = target(dx, dy);
        let id = format!("v=({dx},{dy})");
        g.bench_with_input(BenchmarkId::new("shipped_scan", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("enum_axial_xy", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_enum_axial_xy(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("interval_walk", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_interval(ANCHOR, black_box(b))));
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
        g.bench_with_input(BenchmarkId::new("shipped_scan", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("enum_axial_xy", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_enum_axial_xy(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("interval_walk", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_interval(ANCHOR, black_box(b))));
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
        g.bench_with_input(BenchmarkId::new("shipped_scan", &id), &b, |bench, &b| {
            bench.iter(|| respawn_shape(&d, ANCHOR, b, supercover(ANCHOR, black_box(b)).collect()));
        });
        g.bench_with_input(BenchmarkId::new("enum_axial_xy", &id), &b, |bench, &b| {
            bench.iter(|| {
                respawn_shape(
                    &d,
                    ANCHOR,
                    b,
                    supercover_enum_axial_xy(ANCHOR, black_box(b)).collect(),
                )
            });
        });
        g.bench_with_input(BenchmarkId::new("interval_walk", &id), &b, |bench, &b| {
            bench.iter(|| {
                respawn_shape(
                    &d,
                    ANCHOR,
                    b,
                    supercover_interval(ANCHOR, black_box(b)).collect(),
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
        g.bench_with_input(BenchmarkId::new("shipped_scan", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("enum_axial_xy", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_enum_axial_xy(ANCHOR, black_box(b))));
        });
        g.bench_with_input(BenchmarkId::new("interval_walk", &id), &b, |bench, &b| {
            bench.iter(|| legal_move_shape(&d, supercover_interval(ANCHOR, black_box(b))));
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
