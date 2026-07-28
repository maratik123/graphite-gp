//! The strict supercover predicate (design doc §3 C4).
//!
//! [`supercover`] is the single geometric primitive behind both the runtime
//! legality rule (`legal_move`) and the passability oracle's graph edge, so it is
//! the most correctness-critical routine in the crate — and the hottest.
//!
//! **This is the pre-#171 bounding-box scan, restored.** PRs #171/#172 replaced it
//! with a per-row interval solver that is asymptotically better (`O(cells yielded)`
//! rather than `O(|dx| · |dy|)`), and `benches/supercover.rs` measured that
//! asymptote as unreachable in this product: `--v-target` caps at 10 and Ф5b's
//! deepening reaches `V_ceil = 16`, so a real chord's box is at most ~17×17 ≈ 289
//! cells, which this scan walks as a tight counted loop LLVM strength-reduces and
//! vectorizes. Measured on the two shapes production actually calls, the interval
//! walk was **1.03–1.29× slower** on `legal_move` and **1.08–1.69× slower** on
//! `respawn_cell`; a follow-up Bresenham variant that removed the per-row division
//! landed within 10% of it and did not close the gap. Reverting also restores
//! gp-core's zero-production-panics invariant, which the interval walk's
//! `i32::try_from(..).expect(..)` bounds had broken.
//!
//! Before optimizing this again, run `benches/supercover.rs` — it still carries the
//! rejected interval walk as a baseline — and `supercover_equivalence` below, which
//! turns into a live differential test the moment the two implementations diverge.
//!
//! Pure, deterministic, integer-only, std-only — no floating point anywhere.

use super::Point;

/// Strict supercover of the segment `a → b`: every cell whose **closed** unit
/// square `[c.x ± ½] × [c.y ± ½]` the closed segment touches — corner- and
/// edge-grazes included (design doc §3 C4).
///
/// This strictness is what stops a fast chord from jumping a wall or squeezing
/// through a dual vertex pinched between two walls. It is used identically as the
/// runtime legality rule (`legal_move`) and as the passability-oracle graph edge —
/// one implementation, two callers.
///
/// The predicate guarantees:
///
/// - **Closed squares.** A boundary or single-corner touch counts. When the
///   segment crosses a dual vertex `(i + ½, j + ½)`, all four sharing cells are
///   returned — the correctness-critical tie (e.g. `(0,0)→(1,1)` yields all of
///   `(0,0),(1,0),(0,1),(1,1)`).
/// - **Exact & integer.** Membership is the integer test `2·|cr| ≤ |dx| + |dy|`
///   with `cr = dx·(c.y − a.y) − dy·(c.x − a.x)`, evaluated with no floating point
///   anywhere (design doc §3a).
/// - **Endpoint-order-independent, duplicate-free.** The result is the exact cell
///   set: as a set `supercover(a, b) == supercover(b, a)`, each cell is yielded
///   exactly once (the bounding-box scan visits every cell once), and both endpoint
///   cells are always present (a degenerate `a == b` yields exactly `{a}`).
///
/// **The order cells are yielded in is unspecified.** Only the *set* is
/// contractual — treat the sequence as implementation-defined and free to change.
/// It has changed before: the #171/#172 interval walk emitted shallow chords
/// transposed relative to this scan, and reverting changed it back. Consume the
/// result with an order-insensitive fold, or impose your own total order;
/// `respawn_cell` (`sim/mod.rs`) is the in-tree example of the latter, picking its
/// cell by `max_by_key(|c| (proj(c), c.x, c.y))` rather than by stream position.
///
/// **Cost.** `O(|dx| · |dy|)` — the endpoints' whole bounding box is scanned and
/// filtered. Deliberate: see this module's header for the measurement that chose
/// this over an asymptotically better per-row interval solver.
///
/// **Overflow precondition:** endpoints are assumed separated by a bounded chord —
/// one move's velocity, with `|v| ≪ 1.5×10⁹`. Within that domain the widened `i64`
/// cross product never overflows (its operands are taken relative to `a`, so
/// `|cr| ≤ 2·|dx|·|dy|`). Adversarial full-range `i32` endpoints lie outside the
/// documented domain and are not supported.
///
/// # Examples
///
/// ```
/// use gp_core::geom::{supercover, Point};
///
/// // A straight horizontal edge covers each cell it spans, endpoints included.
/// let cells = supercover(Point::new(0, 0), Point::new(2, 0));
/// assert_eq!(cells.count(), 3);
/// ```
#[allow(
    clippy::arithmetic_side_effects,
    reason = "bounded-chord precondition above: |v| << 1.5e9 per move, so the \
              i64-widened cross product (|cr| <= 2*|dx|*|dy|) cannot overflow \
              within the documented domain; a giant-chord overflow test is \
              infeasible (~1e18-cell scan)"
)]
pub fn supercover(a: Point, b: Point) -> impl Iterator<Item = Point> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::common::*;
    use std::collections::HashSet;

    /// Collect a supercover into a `HashSet` so comparisons ignore iteration
    /// order (spec §4: the result is defined up to set equality).
    fn cover_set(a: Point, b: Point) -> HashSet<Point> {
        supercover(a, b).collect()
    }

    #[test]
    fn axial_horizontal_no_diagonal() {
        // (0,0)→(3,0): the straight run only, no diagonal neighbors (AC3).
        assert_eq!(
            cover_set(Point::new(0, 0), Point::new(3, 0)),
            cells(&[(0, 0), (1, 0), (2, 0), (3, 0)]),
        );
    }

    #[test]
    fn axial_vertical_no_diagonal() {
        // (0,0)→(0,3): the straight vertical run only (AC3).
        assert_eq!(
            cover_set(Point::new(0, 0), Point::new(0, 3)),
            cells(&[(0, 0), (0, 1), (0, 2), (0, 3)]),
        );
    }

    #[test]
    fn dual_vertex_diagonal_all_four() {
        // (0,0)→(1,1) passes through the dual vertex (½,½): all 4 sharing cells (AC2).
        assert_eq!(
            cover_set(Point::new(0, 0), Point::new(1, 1)),
            cells(&[(0, 0), (1, 0), (0, 1), (1, 1)]),
        );
    }

    #[test]
    fn dual_vertex_symmetric() {
        // Reversed endpoints yield the same four cells (order-independence, AC1/AC2).
        assert_eq!(
            cover_set(Point::new(1, 1), Point::new(0, 0)),
            cells(&[(0, 0), (1, 0), (0, 1), (1, 1)]),
        );
    }

    #[test]
    fn primitive_slope_gcd1() {
        // (0,0)→(2,1), gcd(2,1)=1: crosses interiors/edges without hitting a dual
        // vertex; excludes the off-line corners (2,0) and (0,1) (AC1).
        assert_eq!(
            cover_set(Point::new(0, 0), Point::new(2, 1)),
            cells(&[(0, 0), (1, 0), (1, 1), (2, 1)]),
        );
    }

    #[test]
    fn collinear_dual_vertices_gcd2() {
        // (0,0)→(2,2), gcd=2: through collinear dual vertices (½,½) and (1½,1½);
        // each contributes its full 4-cell tie. Excludes (2,0) and (0,2) (AC1/AC2).
        assert_eq!(
            cover_set(Point::new(0, 0), Point::new(2, 2)),
            cells(&[(0, 0), (1, 0), (0, 1), (1, 1), (2, 1), (1, 2), (2, 2)]),
        );
    }

    #[test]
    fn single_corner_graze() {
        // (1,0)→(0,1) grazes the dual vertex (½,½): the 4 cells around it (AC1).
        assert_eq!(
            cover_set(Point::new(1, 0), Point::new(0, 1)),
            cells(&[(0, 0), (1, 0), (0, 1), (1, 1)]),
        );
    }

    #[test]
    fn long_diagonal_three_vertices() {
        // (0,0)→(3,3) passes through three collinear dual vertices (AC1 reinforcement).
        assert_eq!(
            cover_set(Point::new(0, 0), Point::new(3, 3)),
            cells(&[
                (0, 0),
                (0, 1),
                (1, 0),
                (1, 1),
                (1, 2),
                (2, 1),
                (2, 2),
                (2, 3),
                (3, 2),
                (3, 3),
            ]),
        );
    }

    #[test]
    fn degenerate_single_cell() {
        // a == b returns exactly {a} (AC5).
        assert_eq!(
            cover_set(Point::new(2, 2), Point::new(2, 2)),
            cells(&[(2, 2)])
        );
    }

    #[test]
    fn no_duplicate_cells() {
        // AC6: each cell appears exactly once. A HashSet compare alone would hide a
        // double-yield, so assert the raw Vec length equals its deduped length.
        let v: Vec<Point> = supercover(Point::new(0, 0), Point::new(1, 1)).collect();
        assert_eq!(v.len(), v.iter().collect::<HashSet<_>>().len());
    }

    #[test]
    fn includes_both_endpoints() {
        // AC5: both endpoint cells are always present, even on a long chord.
        let (a, b) = (Point::new(0, 0), Point::new(3, 3));
        let set = cover_set(a, b);
        assert!(set.contains(&a) && set.contains(&b));
    }

    #[test]
    fn order_independent_symmetry() {
        // AC1: as a set, supercover(a,b) == supercover(b,a) across several chords.
        for (a, b) in [
            (Point::new(0, 0), Point::new(2, 1)),
            (Point::new(0, 0), Point::new(2, 2)),
            (Point::new(0, 0), Point::new(3, 3)),
            (Point::new(1, 0), Point::new(0, 1)),
        ] {
            assert_eq!(cover_set(a, b), cover_set(b, a));
        }
    }
}

/// Differential tests pinning the optimized [`supercover`] to the pre-#171
/// bounding-box implementation it replaced.
///
/// The rewrite (PRs [#171]/[#172]) changed the *algorithm* while claiming an
/// unchanged *result set*. These tests hold the old implementation as a
/// behavioural oracle and assert that claim directly, rather than re-deriving
/// expectations by hand as `super::tests` does.
///
/// [#171]: https://github.com/maratik123/graphite-gp/pull/171
/// [#172]: https://github.com/maratik123/graphite-gp/pull/172
#[cfg(test)]
mod supercover_equivalence {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    /// Half-extent of the coordinate window anchors are drawn from.
    ///
    /// Sized to a real track, not to `i32`. `[measured]` over 64 master seeds ×
    /// `l_min ∈ {2, 4, 8, 16, 64}`, the worst Ф1 coarse-ring extent is **72
    /// blocks**; Ф2 expands each block to `k × k`, and `k = --block-size` is
    /// clamped to `[1, 32]`, so the widest fine corridor a generated track can
    /// span is ~`72 × 32 = 2304` cells per axis (432 at the default `k = 6`).
    /// `4096` clears that with margin while keeping the reference's box
    /// affordable — the point of item 4 in the request that prompted this module.
    const MAP_HALF_EXTENT: i32 = 4_096;

    /// Half-extent of one move's chord — the only delta `supercover` ever sees in
    /// production, since every caller passes `(pos, pos + v)`.
    ///
    /// `--v-target` is clamped to `[3, 10]` (`gp-game` `config::V_TARGET_{MIN,MAX}`)
    /// and Ф5b's iterative deepening doubles `V_ceil` through `1, 2, 4, 8, 16`, so
    /// this is comfortably above the largest per-axis component the product asks
    /// for.
    const MOVE_HALF_EXTENT: i32 = 32;

    /// Major-axis half-extent for the shallow-chord strategy.
    const LONG_HALF_EXTENT: i32 = 4_096;

    /// Minor-axis half-extent for the shallow-chord strategy. Kept tiny so the
    /// reference's `O(|dx| · |dy|)` box stays affordable at a long major axis.
    const SHALLOW_HALF_EXTENT: i32 = 6;

    /// Half-extent of the free-endpoint window.
    const COMPACT_HALF_EXTENT: i32 = 48;

    /// The frozen specification implementation — the design doc's §3 C4 predicate
    /// evaluated over the endpoints' whole bounding box.
    ///
    /// Identical to the shipped [`supercover`] again since the #171/#172 revert.
    /// Kept separate anyway: it is the fixed point the next optimization attempt
    /// gets measured against, and it must not be edited to track a new walk.
    ///
    /// Scans the endpoints' entire bounding box and keeps every cell satisfying
    /// `2·|cr| ≤ |dx| + |dy|`. That is `O(|dx| · |dy|)`, which is why every
    /// strategy below bounds the **box area** rather than the coordinates alone —
    /// a free pair over `±10_000` would make one case scan up to 4·10⁸ cells.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "verbatim copy of the shipped pre-#171 implementation, under the \
                  same bounded-chord precondition every strategy here respects; \
                  rewriting its arithmetic would stop it being an oracle"
    )]
    fn supercover_reference(a: Point, b: Point) -> Vec<Point> {
        let dx = i64::from(b.x) - i64::from(a.x);
        let dy = i64::from(b.y) - i64::from(a.y);
        let bound = dx.abs() + dy.abs();
        (a.x.min(b.x)..=a.x.max(b.x))
            .flat_map(move |cx| {
                (a.y.min(b.y)..=a.y.max(b.y)).filter_map(move |cy| {
                    let cr = dx * (i64::from(cy) - i64::from(a.y))
                        - dy * (i64::from(cx) - i64::from(a.x));
                    (2 * cr.abs() <= bound).then(|| Point::new(cx, cy))
                })
            })
            .collect()
    }

    /// Asserts every contractual property of [`supercover`] for the chord `a → b`.
    ///
    /// **Four checks, and only the last one is differential.** Since the #171/#172
    /// revert, [`supercover`] and `supercover_reference` are the *same* algorithm,
    /// so the oracle comparison is tautological **today** — it is a ratchet, armed
    /// for the next rewrite, and it costs nothing while disarmed. The first three
    /// are live properties of whatever implementation is compiled:
    ///
    /// 1. **Duplicate-free** — the documented "each cell exactly once". A bare
    ///    `HashSet` comparison would absorb a repeat; comparing `Vec::len()` to
    ///    `HashSet::len()` catches it.
    /// 2. **Both endpoints present** — the guarantee `legal_move` leans on, since a
    ///    walk that dropped `b` would call an into-wall move legal.
    /// 3. **Endpoint symmetry** — `supercover(a, b) == supercover(b, a)` as sets.
    ///    This is the check that actually bit during the rewrite: the interval walk
    ///    picks its loop axis from `|dx| >= |dy|`, which is symmetric, but its
    ///    rounding was not obviously so.
    /// 4. **Matches the frozen reference** — the differential ratchet.
    ///
    /// **Sets, never iteration order.** Order is not contractual and no production
    /// caller depends on it: `legal_move` folds with `.all()`, and `respawn_cell`
    /// imposes its own `max_by_key(|c| (proj(c), c.x, c.y))` precisely so it does
    /// not lean on the iterator; the third call site is `#[cfg(test)]`.
    fn check_same_cells(a: Point, b: Point) -> Result<(), TestCaseError> {
        let got: Vec<Point> = supercover(a, b).collect();
        let want: Vec<Point> = supercover_reference(a, b);
        let got_set: HashSet<Point> = got.iter().copied().collect();
        let want_set: HashSet<Point> = want.iter().copied().collect();

        // (1) duplicate-free, subject and oracle alike.
        prop_assert_eq!(
            got.len(),
            got_set.len(),
            "supercover({:?}, {:?}) yielded a duplicate cell",
            a,
            b
        );
        prop_assert_eq!(
            want.len(),
            want_set.len(),
            "reference({:?}, {:?}) yielded a duplicate cell — the oracle is broken",
            a,
            b
        );

        // (2) both endpoint cells present.
        prop_assert!(
            got_set.contains(&a) && got_set.contains(&b),
            "supercover({a:?}, {b:?}) dropped an endpoint cell"
        );

        // (3) endpoint symmetry, as sets.
        let swapped: HashSet<Point> = supercover(b, a).collect();
        prop_assert_eq!(
            &got_set,
            &swapped,
            "supercover({:?}, {:?}) is not symmetric in its endpoints",
            a,
            b
        );

        // (4) the differential ratchet.
        if got_set != want_set {
            let mut missing: Vec<Point> = want_set.difference(&got_set).copied().collect();
            let mut extra: Vec<Point> = got_set.difference(&want_set).copied().collect();
            missing.sort_unstable();
            extra.sort_unstable();
            return Err(TestCaseError::fail(format!(
                "supercover({a:?}, {b:?}) disagrees with the reference: \
                 missing {missing:?}, extra {extra:?}"
            )));
        }
        Ok(())
    }

    /// `a` offset by `(dx, dy)`, saturating — the offsets used here are orders of
    /// magnitude below `i32::MAX`, so saturation never actually engages; it is
    /// simply the total form the workspace's `arithmetic_side_effects` deny wants.
    fn offset(a: Point, dx: i32, dy: i32) -> Point {
        Point::new(a.x.saturating_add(dx), a.y.saturating_add(dy))
    }

    proptest! {
        /// The real call shape: a position anywhere on a realistic map plus one
        /// move's velocity.
        ///
        /// A large *anchor* is not redundant with a small one — #172 hoists
        /// `along_delta * inner_origin` (the **absolute** inner-axis origin) into
        /// `pre_center`, so the magnitude of the anchor, not just the delta, feeds
        /// the interval arithmetic and its `div_ceil`/`div_floor` rounding.
        #[test]
        #[cfg_attr(
            miri,
            ignore = "cost + entropy: proptest seeds its RNG from the OS and runs \
                      256 cases, each scanning the reference's whole bounding box"
        )]
        fn matches_reference_on_realistic_moves(
            ax in -MAP_HALF_EXTENT..=MAP_HALF_EXTENT,
            ay in -MAP_HALF_EXTENT..=MAP_HALF_EXTENT,
            dx in -MOVE_HALF_EXTENT..=MOVE_HALF_EXTENT,
            dy in -MOVE_HALF_EXTENT..=MOVE_HALF_EXTENT,
        ) {
            let a = Point::new(ax, ay);
            check_same_cells(a, offset(a, dx, dy))?;
        }

        /// Long, nearly-axial chords — the aspect ratio at which the two
        /// implementations diverge most structurally, and the one that exercises
        /// both arms of the `dx.abs() >= dy.abs()` branch pick via `swap_axes`.
        ///
        /// Affordable despite the long axis precisely because the reference's box
        /// is `|dx| · |dy|`: a shallow chord has a thin box however long it is.
        #[test]
        #[cfg_attr(
            miri,
            ignore = "cost + entropy: proptest seeds its RNG from the OS and runs \
                      256 cases, each scanning the reference's whole bounding box"
        )]
        fn matches_reference_on_shallow_chords(
            ax in -MAP_HALF_EXTENT..=MAP_HALF_EXTENT,
            ay in -MAP_HALF_EXTENT..=MAP_HALF_EXTENT,
            long in -LONG_HALF_EXTENT..=LONG_HALF_EXTENT,
            shallow in -SHALLOW_HALF_EXTENT..=SHALLOW_HALF_EXTENT,
            swap_axes in any::<bool>(),
        ) {
            let (dx, dy) = if swap_axes { (shallow, long) } else { (long, shallow) };
            let a = Point::new(ax, ay);
            check_same_cells(a, offset(a, dx, dy))?;
        }

        /// Free endpoint pairs in a compact window — dense arbitrary geometry at a
        /// box size the `O(area)` reference can afford. This is the strategy that
        /// hits non-primitive slopes, near-diagonals, and both endpoint orders
        /// without being told to.
        #[test]
        #[cfg_attr(
            miri,
            ignore = "cost + entropy: proptest seeds its RNG from the OS and runs \
                      256 cases, each scanning the reference's whole bounding box"
        )]
        fn matches_reference_on_compact_arbitrary_pairs(
            ax in -COMPACT_HALF_EXTENT..=COMPACT_HALF_EXTENT,
            ay in -COMPACT_HALF_EXTENT..=COMPACT_HALF_EXTENT,
            bx in -COMPACT_HALF_EXTENT..=COMPACT_HALF_EXTENT,
            by in -COMPACT_HALF_EXTENT..=COMPACT_HALF_EXTENT,
        ) {
            check_same_cells(Point::new(ax, ay), Point::new(bx, by))?;
        }
    }

    /// Required edge case 1 — `ax == bx`, a pure vertical chord.
    ///
    /// Enumerated rather than left to proptest: an exact axis hit is a measure-zero
    /// target for a uniform sampler, and it is the arm where the optimized walk
    /// takes `along_delta == dx == 0` and returns the full `inner_min..=inner_max`
    /// span without dividing at all.
    #[test]
    fn vertical_chords_match_reference() {
        for x in [-MAP_HALF_EXTENT, -1, 0, 1, MAP_HALF_EXTENT] {
            for (y0, y1) in [(0, 0), (0, 1), (0, 17), (-9, 9), (5, -5), (-31, 32)] {
                check_same_cells(Point::new(x, y0), Point::new(x, y1))
                    .unwrap_or_else(|e| panic!("vertical chord x={x}, y {y0} -> {y1}: {e}"));
            }
        }
    }

    /// Required edge case 1 (mirror) — `ay == by`, a pure horizontal chord.
    #[test]
    fn horizontal_chords_match_reference() {
        for y in [-MAP_HALF_EXTENT, -1, 0, 1, MAP_HALF_EXTENT] {
            for (x0, x1) in [(0, 0), (0, 1), (0, 17), (-9, 9), (5, -5), (-31, 32)] {
                check_same_cells(Point::new(x0, y), Point::new(x1, y))
                    .unwrap_or_else(|e| panic!("horizontal chord y={y}, x {x0} -> {x1}: {e}"));
            }
        }
    }

    /// Required edge case 2 — `a == b`, the degenerate zero-length chord.
    ///
    /// Also re-asserts the documented `{a}` singleton directly, so a regression
    /// that broke *both* implementations identically would still be caught here.
    #[test]
    fn degenerate_zero_length_chords_match_reference() {
        for p in [
            Point::new(0, 0),
            Point::new(1, -1),
            Point::new(-MAP_HALF_EXTENT, MAP_HALF_EXTENT),
            Point::new(MAP_HALF_EXTENT, -MAP_HALF_EXTENT),
        ] {
            check_same_cells(p, p).unwrap_or_else(|e| panic!("degenerate chord at {p:?}: {e}"));
            let got: Vec<Point> = supercover(p, p).collect();
            assert_eq!(got, vec![p], "a == b must yield exactly {{a}}");
        }
    }

    /// Required edge case 3 — `|dx| == |dy|`, the exact diagonals.
    ///
    /// The dual-vertex tie is the correctness-critical case of the whole predicate
    /// (design doc §3 C4): a diagonal crosses `(i + ½, j + ½)`, where all four
    /// sharing cells must be returned. All four sign combinations are covered
    /// because a rounding-based walk's floor/ceil swap roles with the sign of the
    /// along-axis delta — the defect class the #171 sign swap existed to handle.
    #[test]
    fn exact_diagonals_match_reference() {
        for anchor in [
            Point::new(0, 0),
            Point::new(-MAP_HALF_EXTENT, MAP_HALF_EXTENT),
        ] {
            for len in [1, 2, 3, 4, 8, 33] {
                for (sx, sy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                    let b = offset(anchor, len * sx, len * sy);
                    check_same_cells(anchor, b)
                        .unwrap_or_else(|e| panic!("diagonal {anchor:?} -> {b:?}: {e}"));
                }
            }
        }
    }

    /// Exhaustive over a small window — every chord shape that fits, with no
    /// sampler in the loop.
    ///
    /// Strictly stronger than a property run on this domain, and deterministic, so
    /// a failure reproduces without a proptest seed file. 9⁴ = 6561 ordered pairs,
    /// each with a box of at most 81 cells.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "cost: 6561 ordered pairs, each scanning the reference's whole \
                  bounding box — pure integer arithmetic, so it carries no \
                  production-UB signal to trade for the runtime"
    )]
    fn exhaustive_small_window_matches_reference() {
        const R: i32 = 4;
        for ax in -R..=R {
            for ay in -R..=R {
                for bx in -R..=R {
                    for by in -R..=R {
                        let (a, b) = (Point::new(ax, ay), Point::new(bx, by));
                        check_same_cells(a, b)
                            .unwrap_or_else(|e| panic!("exhaustive {a:?} -> {b:?}: {e}"));
                    }
                }
            }
        }
    }
}
