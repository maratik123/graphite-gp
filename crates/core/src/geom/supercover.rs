//! The strict supercover predicate (design doc §3 C4) and its fast-path solver.
//!
//! [`supercover`] is the single geometric primitive behind both the runtime
//! legality rule (`legal_move`) and the passability oracle's graph edge, so it is
//! the most correctness-critical routine in the crate — and the hottest. The
//! interval solver it delegates to ([`InnerRange`] → [`InnerRangePreEval`]) and the
//! two rounding helpers below are private implementation detail; only
//! [`supercover`] is re-exported flat at [`crate::geom`].
//!
//! Pure, deterministic, integer-only, std-only — no floating point anywhere.

use super::{Coord, Point};

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
/// - **Order-independent, duplicate-free.** The result is the exact cell set: as a
///   set `supercover(a, b) == supercover(b, a)`, each cell is yielded exactly once
///   (the outer loop visits each minor-axis coordinate once, and the inner range it
///   yields is a contiguous major-axis interval), and both endpoint cells are always
///   present (a degenerate `a == b` yields exactly `{a}`).
///
/// **Cost.** The walk does **not** scan the endpoints' bounding box. It loops over
/// the *minor* axis and, per outer coordinate, solves the membership predicate for
/// the exact major-axis interval that satisfies it (`InnerRangePreEval::evaluate`),
/// so the work is proportional to the number of cells actually yielded rather than
/// to `|dx| · |dy|`.
///
/// **Overflow precondition:** endpoints are assumed separated by a bounded chord —
/// one move's velocity, with `|v| ≪ 1.5×10⁹`. Within that domain the widened `i64`
/// cross product never overflows (its operands are taken relative to `a`, so
/// `|cr| ≤ 2·|dx|·|dy|`). Adversarial full-range `i32` endpoints lie outside the
/// documented domain and are not supported.
///
/// # Panics
///
/// Panics — when the returned iterator is **advanced**, not when it is built — if
/// an inner-interval bound falls outside `i32` while narrowing it back from the
/// widened `i64` arithmetic. That requires violating the bounded-chord precondition
/// above: within it, each bound is clamped against `inner_min` / `inner_max`, which
/// are the endpoints' own `i32` coordinates, so both always fit. The two sites are
/// catalogued in `ai-docs/panic-index.md`.
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

    let ax = i64::from(a.x);
    let ay = i64::from(a.y);

    if dx.abs() >= dy.abs() {
        // `x` is the major axis, so it becomes the inner loop and `y` the outer one.
        // Along the inner axis the segment advances by `dy` per outer step; `dx` is
        // the across-component that shifts the interval.
        let x_min = i64::from(a.x.min(b.x));
        let x_max = i64::from(a.x.max(b.x));

        let inner_range = InnerRange {
            outer_origin: ay,
            across_delta: dx,
            inner_origin: ax,
            along_delta: dy,
            inner_min: x_min,
            inner_max: x_max,
            bound,
        }
        .pre_eval();

        EitherIter::Left((a.y.min(b.y)..=a.y.max(b.y)).flat_map(move |cy| {
            let (lo, hi) = inner_range.evaluate(cy);
            (lo..=hi).map(move |cx| Point::new(cx, cy))
        }))
    } else {
        // Mirror of the branch above with the axes swapped: `y` is the major axis,
        // so it becomes the inner loop and `x` the outer one.
        let y_min = i64::from(a.y.min(b.y));
        let y_max = i64::from(a.y.max(b.y));

        let inner_range = InnerRange {
            outer_origin: ax,
            across_delta: dy,
            inner_origin: ay,
            along_delta: dx,
            inner_min: y_min,
            inner_max: y_max,
            bound,
        }
        .pre_eval();

        EitherIter::Right((a.x.min(b.x)..=a.x.max(b.x)).flat_map(move |cx| {
            let (lo, hi) = inner_range.evaluate(cx);
            (lo..=hi).map(move |cy| Point::new(cx, cy))
        }))
    }
}

/// [`supercover`]'s per-row inner-interval solver, in axis-agnostic terms.
///
/// "Outer" is the minor axis (the `flat_map` loop variable), "inner" the major axis
/// (the contiguous range yielded per outer step). Both `supercover` branches build
/// this with their axes swapped, so the arithmetic below is written once. Construct
/// it, then call [`InnerRange::pre_eval`] — [`InnerRangePreEval`] is the only form
/// that can [`evaluate`](InnerRangePreEval::evaluate).
struct InnerRange {
    /// The segment's start coordinate on the outer (minor) axis.
    outer_origin: i64,
    /// Segment delta *across* the inner axis — what shifts the interval per outer step.
    across_delta: i64,
    /// The segment's start coordinate on the inner (major) axis.
    inner_origin: i64,
    /// Segment delta *along* the inner axis; `0` means the interval never narrows.
    along_delta: i64,
    /// Inclusive inner-axis floor — the lower of the two endpoints' coordinates.
    inner_min: i64,
    /// Inclusive inner-axis ceiling — the higher of the two endpoints' coordinates.
    inner_max: i64,
    /// The membership predicate's right-hand side, `|dx| + |dy|`.
    bound: i64,
}

/// An [`InnerRange`] with its loop-invariant product hoisted out of the per-row path.
///
/// Identical to [`InnerRange`] except that `inner_origin` has been folded into
/// [`pre_center`](Self::pre_center), which is the only term of the interval formula
/// that does not depend on the outer coordinate.
struct InnerRangePreEval {
    /// The segment's start coordinate on the outer (minor) axis.
    outer_origin: i64,
    /// Segment delta *across* the inner axis — what shifts the interval per outer step.
    across_delta: i64,
    /// `along_delta * inner_origin`, the outer-invariant half of the interval centre.
    pre_center: i64,
    /// Segment delta *along* the inner axis; `0` means the interval never narrows.
    along_delta: i64,
    /// Inclusive inner-axis floor — the lower of the two endpoints' coordinates.
    inner_min: i64,
    /// Inclusive inner-axis ceiling — the higher of the two endpoints' coordinates.
    inner_max: i64,
    /// The membership predicate's right-hand side, `|dx| + |dy|`.
    bound: i64,
}

impl InnerRange {
    /// Hoists the outer-invariant `along_delta * inner_origin` product out of the
    /// per-row path, yielding the only form that can `evaluate`.
    ///
    /// Called once per [`supercover`] call, never inside the iteration.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "every field is i64-widened from an i32 coordinate, so the single \
                  product here is bounded by 2^62 and cannot overflow i64 under \
                  supercover's documented bounded-chord precondition"
    )]
    const fn pre_eval(self) -> InnerRangePreEval {
        let Self {
            outer_origin,
            across_delta,
            inner_origin,
            along_delta,
            inner_min,
            inner_max,
            bound,
        } = self;
        InnerRangePreEval {
            outer_origin,
            across_delta,
            pre_center: along_delta * inner_origin,
            along_delta,
            inner_min,
            inner_max,
            bound,
        }
    }
}

impl InnerRangePreEval {
    /// The inclusive inner-axis interval `[lo, hi]` whose cells satisfy the
    /// membership predicate at outer coordinate `outer`.
    ///
    /// Solves `2·|cr| ≤ bound` for the inner coordinate instead of testing each
    /// candidate: the admissible inner values are exactly those in
    /// `[⌈low / m⌉, ⌊high / m⌋]`, clamped to the endpoints' own span. A returned
    /// `lo > hi` denotes an empty row and yields no cells.
    ///
    /// # Panics
    ///
    /// Panics if either bound falls outside `i32`, which requires violating
    /// [`supercover`]'s bounded-chord precondition — see that function's
    /// `# Panics` section.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "same bounded-chord precondition as supercover: every operand is \
                  i64-widened from an i32 coordinate and the only multipliers are \
                  the constant 2 and the chord deltas, so |center| <= 2^63 holds \
                  within the documented domain; `m` is non-zero on this arm because \
                  the along_delta == 0 case returns above"
    )]
    fn evaluate(&self, outer: Coord) -> (i32, i32) {
        let outer = i64::from(outer);
        let (lo, hi) = if self.along_delta == 0 {
            (self.inner_min, self.inner_max)
        } else {
            let (m, low, high) = {
                let cross = self.across_delta * (outer - self.outer_origin);
                let center = 2 * (cross + self.pre_center);
                let m = 2 * self.along_delta;
                let low = center - self.bound;
                let high = center + self.bound;
                (m, low, high)
            };

            let (low, high) = if m > 0 { (low, high) } else { (high, low) };

            let (lo, hi) = (div_ceil(low, m), div_floor(high, m));

            (lo.max(self.inner_min), hi.min(self.inner_max))
        };
        (
            i32::try_from(lo).expect("lo clamped within i32-derived inner_min..=inner_max"),
            i32::try_from(hi).expect("hi clamped within i32-derived inner_min..=inner_max"),
        )
    }
}

/// Unifies two iterator types of the same `Item` behind one concrete type.
///
/// [`supercover`] picks its inner axis at runtime, so its two `flat_map` chains have
/// different (unnameable) types that a single `impl Iterator` return cannot cover.
/// Boxing would allocate on every legality check; this enum forwards by `match`
/// instead, preserving [`ExactSizeIterator`] / [`DoubleEndedIterator`] where both
/// sides provide them.
enum EitherIter<A, B> {
    /// The first alternative.
    Left(A),
    /// The second alternative.
    Right(B),
}

impl<A, B, T> Iterator for EitherIter<A, B>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            Self::Left(a) => a.next(),
            Self::Right(b) => b.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Left(a) => a.size_hint(),
            Self::Right(b) => b.size_hint(),
        }
    }
}

impl<A, B, T> ExactSizeIterator for EitherIter<A, B>
where
    A: ExactSizeIterator<Item = T>,
    B: ExactSizeIterator<Item = T>,
{
    fn len(&self) -> usize {
        match self {
            Self::Left(a) => a.len(),
            Self::Right(b) => b.len(),
        }
    }
}

impl<A, B, T> DoubleEndedIterator for EitherIter<A, B>
where
    A: DoubleEndedIterator<Item = T>,
    B: DoubleEndedIterator<Item = T>,
{
    fn next_back(&mut self) -> Option<T> {
        match self {
            Self::Left(a) => a.next_back(),
            Self::Right(b) => b.next_back(),
        }
    }
}

/// `⌊a / b⌋` — division rounding toward negative infinity, for any sign of `b`.
///
/// `i64::div_euclid` rounds toward `-∞` only for positive divisors; for a negative
/// one it rounds *up*, so the quotient is corrected by one on a non-exact division.
/// Not `i64::div_floor` — that method is still unstable.
///
/// # Panics
///
/// Panics if `b == 0`, inheriting `i64::div_euclid`'s precondition.
/// [`InnerRangePreEval::evaluate`] is the only caller and passes `2 * along_delta`
/// on the arm where `along_delta != 0` is already established, so `b` is non-zero —
/// and, being even, can never be the `-1` that would make `div_euclid` overflow on
/// `i64::MIN`.
const fn div_floor(a: i64, b: i64) -> i64 {
    let q = a.div_euclid(b);
    if b < 0 && a.rem_euclid(b) != 0 {
        q.wrapping_sub(1)
    } else {
        q
    }
}

/// `⌈a / b⌉` — division rounding toward positive infinity, for any sign of `b`.
///
/// Mirror of [`div_floor`]: `i64::div_euclid` already rounds up for a negative
/// divisor, so only the positive-divisor case needs the non-exact correction.
/// Not `i64::div_ceil` — that method is still unstable.
///
/// # Panics
///
/// Panics if `b == 0`, under the same caller guarantee as [`div_floor`].
const fn div_ceil(a: i64, b: i64) -> i64 {
    let q = a.div_euclid(b);
    if b > 0 && a.rem_euclid(b) != 0 {
        q.wrapping_add(1)
    } else {
        q
    }
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
