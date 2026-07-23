//! Dual-grid geometry primitives (design doc §1).
//!
//! Core invariant: a [`Point`] is the center of a unit cell (integer
//! coordinates); a [`Wall`] is a dual edge on the boundary of the corridor `D` —
//! anchored to a drivable cell plus the [`Side`] toward its non-drivable
//! neighbor. From this duality, "a wall never passes through a point" and "a car
//! never touches a wall" hold by construction.
//!
//! The corridor-graph helpers ([`Side`], [`flood_fill`], [`component_count`],
//! [`bounded_complement_components`], [`CorridorScratch`], [`geodesic_layers`],
//! [`walls_from_boundary`]) live in a private `graph` submodule, and the
//! distance-transform / medial-axis primitives ([`DistanceTransform`],
//! [`medial_axis`]) live in a private `distance` submodule; both are
//! re-exported here, so every `crate::geom::*` path stays flat.

mod distance;
mod graph;
pub use distance::*;
pub use graph::*;

/// Integer grid coordinate.
pub type Coord = i32;

/// An integer grid point = the center of one unit cell.
///
/// Derives `Ord` (additive to the existing `Eq`/`Hash` set) so `Point` can key
/// a `BTreeSet`/`BTreeMap` for deterministic, cross-platform iteration order
/// (design doc §2, discharges #50) — the derived order is `x` then `y`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Point {
    /// Horizontal cell coordinate (grid column), increasing eastward.
    pub x: Coord,
    /// Vertical cell coordinate (grid row), increasing northward.
    pub y: Coord,
}

impl Point {
    /// The grid point at integer coordinates `(x, y)`.
    #[inline]
    pub const fn new(x: Coord, y: Coord) -> Self {
        Self { x, y }
    }

    /// The 4-connected neighbors. Movement connectivity is 4-conn throughout
    /// (design doc §1): it fixes the Manhattan metric and forbids diagonal
    /// "needle's-eye" slips between two walls.
    #[inline]
    pub const fn neighbors4(self) -> [Self; 4] {
        [
            Self::new(self.x.saturating_add(1), self.y),
            Self::new(self.x.saturating_sub(1), self.y),
            Self::new(self.x, self.y.saturating_add(1)),
            Self::new(self.x, self.y.saturating_sub(1)),
        ]
    }
}

/// A horizontal or vertical orientation on the grid.
///
/// Carried by chords such as the start/finish line
/// ([`StartFinish`](crate::track::StartFinish)); walls instead carry a 4-way
/// [`Side`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Orient {
    /// Horizontal — spanning east–west.
    Horizontal,
    /// Vertical — spanning north–south.
    Vertical,
}

/// A wall = one dual edge on the corridor boundary.
///
/// Anchored to the drivable cell it borders plus which [`Side`] of that cell the
/// edge sits on. Walls are *derived* from the corridor boundary (design doc §1),
/// never authored by hand.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Wall {
    /// The drivable cell this edge borders.
    pub cell: Point,
    /// Which side of `cell` the edge sits on.
    pub side: Side,
}

/// An unsigned grid extent, in cells: a `width` × `height` box size.
///
/// Unsigned fields make a negative dimension unrepresentable — the whole reason
/// [`Corridor::new`] needs no non-negative-dimensions `assert!`. Mirrors [`Point`]'s
/// derive set; a plain [`Copy`] value type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Size {
    /// Extent along `x`, in cells (columns).
    pub width: usize,
    /// Extent along `y`, in cells (rows).
    pub height: usize,
}

impl Size {
    /// A grid extent of `width` × `height` cells.
    #[inline]
    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    /// The number of cells in the box (`width × height`).
    ///
    /// A raw `usize` multiply: real grid dimensions are ≪ `usize::MAX`, so a
    /// product large enough to overflow is unreachable in the grid domain — a
    /// [`Corridor`] of that cell count could not allocate its backing `Vec<bool>`
    /// first. This is the same bounded-domain treatment as [`supercover`]'s
    /// overflow precondition (`docs/design.md` §3 C4); not a panic-index entry.
    ///
    /// **Overflow precondition:** holds for grid-realistic, [`Corridor`]-backed
    /// (allocatable) dimensions. `Size` is a public, standalone-constructible
    /// type — a `Size { width: usize::MAX, .. }` built directly by struct literal,
    /// bypassing [`Corridor::new`], is representable but lies outside this
    /// documented domain and is not supported (its `area()` would overflow).
    #[inline]
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "bounded by allocatability: a Corridor of this many cells must \
                  first allocate a Vec<bool> of that length, so a usize-overflowing \
                  product is unreachable via Corridor::new — see the doc precondition \
                  for the (unsupported) direct-struct-literal exception"
    )]
    pub const fn area(self) -> usize {
        self.width * self.height
    }

    /// Whether the box has zero area (either dimension is `0`).
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// An axis-aligned integer-grid box: a [`Size`] extent anchored at `origin`.
///
/// Owns the flat-index and bounds arithmetic over the half-open cell rectangle
/// `[origin, origin + (width, height))`. Every query is total and panic-free for
/// any [`Point`] — out-of-box, negative-delta, and coordinate-overflowing inputs
/// all resolve without a cast or an `#[allow]`. Mirrors [`Point`]'s derive set.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Rect {
    /// The minimum-coordinate corner (the box's lower-left cell).
    pub origin: Point,
    /// The box extent, in cells.
    pub size: Size,
}

impl Rect {
    /// The non-negative `(dx, dy)` cell offset of `p` from `origin`, or `None`.
    ///
    /// `checked_sub` folds the i32 subtraction-overflow case (adversarial coords,
    /// e.g. `i32::MAX - (-10)`) to `None`; `usize::try_from` folds a negative delta
    /// (`p` left of / below `origin`) to `None`. Together this replaces both a
    /// `dx < 0 || dy < 0` guard and a `cast_sign_loss` `#[allow]`, with no `as`.
    fn offset(&self, p: Point) -> Option<(usize, usize)> {
        Some((
            usize::try_from(p.x.checked_sub(self.origin.x)?).ok()?,
            usize::try_from(p.y.checked_sub(self.origin.y)?).ok()?,
        ))
    }

    /// The row-major (`y`-outer) flat index of `p`, or `None` when `p` is outside
    /// the box.
    ///
    /// Total and panic-free for every [`Point`]: out-of-box, negative-delta, and
    /// coordinate-overflowing inputs all yield `None`. Widening happens only in the
    /// checked `offset` conversion — there is no `as` cast in the index path.
    ///
    /// **Overflow precondition:** the final `dy * width + dx` multiply-add is
    /// guarded by the immediately preceding `dx < width && dy < height`, so the
    /// result is always strictly `< width * height` (== [`Size::area`]) — the same
    /// grid-realistic, allocatable-dimensions domain as `area`'s. `Rect` is a
    /// public, standalone-constructible type — a `Rect` built by struct literal
    /// with adversarially large, unallocatable `width`/`height` near `usize::MAX`,
    /// bypassing [`Corridor::new`], lies outside this documented domain and is
    /// not supported.
    #[inline]
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "dx < width && dy < height (checked immediately above) bounds the \
                  product strictly below width*height == Size::area, itself bounded \
                  by allocatability (see Size::area's doc precondition)"
    )]
    pub fn index(&self, p: Point) -> Option<usize> {
        let (dx, dy) = self.offset(p)?;
        (dx < self.size.width && dy < self.size.height).then(|| dy * self.size.width + dx)
    }

    /// Whether `p` lies inside the box.
    #[inline]
    pub fn contains(&self, p: Point) -> bool {
        self.index(p).is_some()
    }

    /// Every cell point in the box, in row-major (`y`-outer, `x`-inner) order.
    ///
    /// Empty when either dimension is `0`. The endpoints saturate at `i32::MAX`
    /// rather than overflow, so iteration is total; for every in-domain grid the
    /// order and contents are identical to a direct `origin.x + dx` walk.
    pub fn points(&self) -> impl Iterator<Item = Point> {
        let origin = self.origin;
        let x1 = i32::try_from(self.size.width).map_or(i32::MAX, |w| origin.x.saturating_add(w));
        let y1 = i32::try_from(self.size.height).map_or(i32::MAX, |h| origin.y.saturating_add(h));
        (origin.y..y1).flat_map(move |y| (origin.x..x1).map(move |x| Point::new(x, y)))
    }

    /// Whether `p` lies on the box's border (any of its four edges).
    ///
    /// `false` for any out-of-box point. Uses the `dx + 1 == width` form (never
    /// `width - 1`), so it is correct and underflow-free at `width`/`height` of `0`
    /// (a zero-dim box has no border cells).
    ///
    /// **Overflow precondition:** `dx + 1` / `dy + 1` are guarded by the preceding
    /// `dx < w && dy < h`, so `dx ≤ w − 1` (resp. `dy ≤ h − 1`) and the sum cannot
    /// overflow `usize` for any grid-realistic, allocatable box — the same domain
    /// as [`Size::area`].
    #[inline]
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "dx < w && dy < h (checked immediately above) bounds dx+1/dy+1 \
                  strictly below w+1/h+1, so the add cannot overflow in the \
                  allocatable-dimensions domain (see Size::area's doc precondition)"
    )]
    pub fn on_border(&self, p: Point) -> bool {
        let Some((dx, dy)) = self.offset(p) else {
            return false;
        };
        let (w, h) = (self.size.width, self.size.height);
        dx < w && dy < h && (dx == 0 || dy == 0 || dx + 1 == w || dy + 1 == h)
    }
}

/// The corridor `D` — the set of drivable points/cells (design doc §1).
///
/// Backed by a dense bitmap over a bounding box (`origin` + `width`×`height`) for
/// O(1) membership plus cheap flood-fill and distance-transform (design doc §2,
/// Ф4). Points outside the box are, by definition, not in `D`.
#[derive(Clone, Debug, Default)]
pub struct Corridor {
    rect: Rect,
    cells: Vec<bool>,
}

impl Corridor {
    /// A new, empty corridor over the box `[origin, origin + (width, height))`.
    ///
    /// Unsigned `width`/`height` make a negative dimension unrepresentable, so no
    /// validation is needed and the constructor is infallible.
    pub fn new(origin: Point, width: usize, height: usize) -> Self {
        let rect = Rect {
            origin,
            size: Size::new(width, height),
        };
        Self {
            cells: vec![false; rect.size.area()],
            rect,
        }
    }

    /// A new corridor over `[origin, origin + (width, height))` with **every**
    /// cell drivable — a fully-drivable rectangle (the dual of [`new`]'s empty
    /// box).
    ///
    /// [`new`]: Self::new
    pub fn filled(origin: Point, width: usize, height: usize) -> Self {
        let rect = Rect {
            origin,
            size: Size::new(width, height),
        };
        Self {
            cells: vec![true; rect.size.area()],
            rect,
        }
    }

    /// The bounding-box origin — its minimum-coordinate corner.
    #[inline]
    pub const fn origin(&self) -> Point {
        self.rect.origin
    }
    /// The bounding-box width, in cells (columns).
    #[inline]
    pub const fn width(&self) -> usize {
        self.rect.size.width
    }
    /// The bounding-box height, in cells (rows).
    #[inline]
    pub const fn height(&self) -> usize {
        self.rect.size.height
    }

    /// Whether `p` is a drivable point of `D`.
    #[inline]
    pub fn contains(&self, p: Point) -> bool {
        // Delegates only the *index* to `rect`; in-box ≠ drivable, so this is not
        // `Rect::contains`.
        self.rect.index(p).is_some_and(|i| self.cells[i])
    }

    /// Marks `p` drivable / not drivable. No-op if `p` is outside the box.
    #[inline]
    pub fn set(&mut self, p: Point, drivable: bool) {
        if let Some(i) = self.index(p) {
            self.cells[i] = drivable;
        }
    }

    /// Number of drivable points in `D`.
    pub fn len(&self) -> usize {
        self.cells.iter().filter(|&&c| c).count()
    }

    /// Whether `D` has no drivable points.
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|&c| !c)
    }

    fn index(&self, p: Point) -> Option<usize> {
        self.rect.index(p)
    }

    /// The number of cells in the bounding box (`width × height`).
    const fn area(&self) -> usize {
        self.rect.size.area()
    }

    /// Every cell point in the bounding box, in row-major (`y`-outer) order.
    fn box_points(&self) -> impl Iterator<Item = Point> {
        self.rect.points()
    }

    /// Whether `p` lies on the bounding-box border.
    fn on_border(&self, p: Point) -> bool {
        self.rect.on_border(p)
    }
}

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
///   (the bounding-box scan visits every cell once), and both endpoint cells are
///   always present (a degenerate `a == b` yields exactly `{a}`).
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
pub(crate) mod common {
    use super::*;
    use std::collections::HashSet;

    /// Build an expected cell set from `(x, y)` literals.
    pub(crate) fn cells(pts: &[(Coord, Coord)]) -> HashSet<Point> {
        pts.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::common::*;
    use super::*;
    use std::collections::HashSet;

    /// Collect a supercover into a `HashSet` so comparisons ignore iteration
    /// order (spec §4: the result is defined up to set equality).
    fn cover_set(a: Point, b: Point) -> HashSet<Point> {
        supercover(a, b).collect()
    }

    #[test]
    fn corridor_filled_marks_every_cell_drivable() {
        let d = Corridor::filled(Point::new(0, 0), 3, 2);
        assert_eq!(d.len(), 6, "every cell of the 3×2 box is drivable");
        for y in 0..2 {
            for x in 0..3 {
                assert!(d.contains(Point::new(x, y)), "({x}, {y}) not drivable");
            }
        }
        assert!(
            !d.contains(Point::new(3, 0)),
            "outside the box is not drivable"
        );

        // The origin is respected — cells are offset, not always at (0, 0).
        let off = Corridor::filled(Point::new(5, 5), 2, 2);
        assert!(off.contains(Point::new(6, 6)));
        assert!(!off.contains(Point::new(0, 0)));
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

    #[test]
    fn neighbors4_saturates_at_i32_max_without_panic() {
        // AC4: the east/north neighbors of an i32::MAX-cornered point saturate
        // rather than overflow; the non-saturating axis is a const-operand
        // literal (i32::MAX - 1), not flagged by arithmetic_side_effects.
        let p = Point::new(i32::MAX, i32::MAX);
        assert_eq!(
            p.neighbors4(),
            [
                Point::new(i32::MAX, i32::MAX),     // east: saturated self
                Point::new(i32::MAX - 1, i32::MAX), // west: in-domain
                Point::new(i32::MAX, i32::MAX),     // north: saturated self
                Point::new(i32::MAX, i32::MAX - 1), // south: in-domain
            ]
        );
    }

    #[test]
    fn neighbors4_saturates_at_i32_min_without_panic() {
        // AC4: the west/south neighbors of an i32::MIN-cornered point saturate
        // rather than underflow; the non-saturating axis is a const-operand
        // literal (i32::MIN + 1), not flagged by arithmetic_side_effects.
        let p = Point::new(i32::MIN, i32::MIN);
        assert_eq!(
            p.neighbors4(),
            [
                Point::new(i32::MIN + 1, i32::MIN), // east: in-domain
                Point::new(i32::MIN, i32::MIN),     // west: saturated self
                Point::new(i32::MIN, i32::MIN + 1), // north: in-domain
                Point::new(i32::MIN, i32::MIN),     // south: saturated self
            ]
        );
    }

    #[test]
    fn point_ord_orders_by_x_then_y() {
        // AC12: derived Ord orders by x then y — the ordering a BTreeSet<Point>
        // relies on for deterministic iteration.
        assert!(Point::new(0, 5) < Point::new(1, 0));
        assert!(Point::new(1, 0) < Point::new(1, 1));
        assert_eq!(
            Point::new(2, 3).cmp(&Point::new(2, 3)),
            std::cmp::Ordering::Equal
        );

        let mut pts = vec![
            Point::new(1, 1),
            Point::new(0, 5),
            Point::new(1, 0),
            Point::new(0, 0),
        ];
        pts.sort();
        assert_eq!(
            pts,
            vec![
                Point::new(0, 0),
                Point::new(0, 5),
                Point::new(1, 0),
                Point::new(1, 1),
            ]
        );
    }

    #[test]
    fn size_area_is_width_times_height() {
        // AC1/AC10: area of a normal box is the product of its dimensions.
        assert_eq!(Size::new(3, 4).area(), 12);
        assert_eq!(Size::new(1, 1).area(), 1);
    }

    #[test]
    fn size_area_large_in_domain_dims() {
        // AC3: a large-but-in-domain product (50_000 x 50_000) exercises the
        // documented overflow-precondition bound well beyond the tiny 3x4
        // fixture, while staying portable on 32-bit targets too
        // (usize::MAX >= 4_294_967_295 > 2_500_000_000).
        assert_eq!(Size::new(50_000, 50_000).area(), 2_500_000_000);
    }

    #[test]
    fn size_zero_dimension_is_empty_with_zero_area() {
        // AC1/AC10: a zero in either dimension → empty box, zero area.
        for s in [Size::new(0, 5), Size::new(5, 0), Size::new(0, 0)] {
            assert!(s.is_empty());
            assert_eq!(s.area(), 0);
        }
    }

    #[test]
    fn size_nonzero_dimensions_are_not_empty() {
        // AC1/AC10: both dimensions positive → not empty.
        assert!(!Size::new(1, 1).is_empty());
        assert!(!Size::new(3, 4).is_empty());
    }

    #[test]
    fn size_default_is_empty_zero_area() {
        // AC1/AC10: `Size::default()` is the empty `{0, 0}` box.
        let s = Size::default();
        assert_eq!(s, Size::new(0, 0));
        assert!(s.is_empty());
        assert_eq!(s.area(), 0);
    }

    /// Build a `Rect` at `(ox, oy)` with a `w × h` extent, by struct literal
    /// (AC2 forbids a speculative `Rect::new`).
    fn rect_at(ox: Coord, oy: Coord, w: usize, h: usize) -> Rect {
        Rect {
            origin: Point::new(ox, oy),
            size: Size::new(w, h),
        }
    }

    #[test]
    fn rect_index_in_box_is_row_major() {
        // AC2/AC3/AC10: exact row-major flat index for cells of an off-origin box
        // (origin (2,3), size 4×5): index = dy * width + dx.
        let r = rect_at(2, 3, 4, 5);
        assert_eq!(r.index(Point::new(2, 3)), Some(0)); // dx=0, dy=0
        assert_eq!(r.index(Point::new(3, 3)), Some(1)); // dx=1, dy=0
        assert_eq!(r.index(Point::new(2, 4)), Some(4)); // dx=0, dy=1
        assert_eq!(r.index(Point::new(3, 4)), Some(5)); // dx=1, dy=1
        assert_eq!(r.index(Point::new(5, 7)), Some(19)); // dx=3, dy=4 (last cell)
    }

    #[test]
    fn rect_index_large_in_domain_dims() {
        // AC3: a large-but-in-domain box (50_000 x 50_000) drives a large
        // dy*width product through the documented overflow-precondition bound
        // (49_999*50_000 = 2_499_950_000, + 12_345), plus a small in-box sanity
        // cell.
        let r = rect_at(0, 0, 50_000, 50_000);
        assert_eq!(r.index(Point::new(12_345, 49_999)), Some(2_499_962_345));
        assert_eq!(r.index(Point::new(1, 1)), Some(50_001));
    }

    #[test]
    fn rect_index_out_of_box_is_none() {
        // AC3/AC10: out-of-box points → None with no explicit sign guard.
        let r = rect_at(2, 3, 4, 5);
        assert_eq!(r.index(Point::new(1, 3)), None); // dx < 0 (left of origin)
        assert_eq!(r.index(Point::new(2, 2)), None); // dy < 0 (below origin)
        assert_eq!(r.index(Point::new(6, 3)), None); // dx == width
        assert_eq!(r.index(Point::new(2, 8)), None); // dy == height
    }

    #[test]
    fn rect_index_overflowing_point_is_none_without_panic() {
        // AC3/AC10: a coordinate-overflowing delta (i32::MAX - negative origin)
        // resolves to None via `checked_sub`, never panicking.
        let r = rect_at(-10, -10, 4, 5);
        assert_eq!(r.index(Point::new(i32::MAX, 0)), None);
        assert_eq!(r.index(Point::new(0, i32::MAX)), None);
    }

    #[test]
    fn rect_contains_mirrors_index_is_some() {
        // AC2/AC10: contains is exactly index(..).is_some().
        let r = rect_at(2, 3, 4, 5);
        for p in [
            Point::new(2, 3),
            Point::new(5, 7),
            Point::new(1, 3),
            Point::new(6, 8),
            Point::new(i32::MAX, 0),
        ] {
            assert_eq!(r.contains(p), r.index(p).is_some());
        }
    }

    #[test]
    fn rect_points_are_row_major() {
        // AC2/AC10: points() walks y-outer / x-inner over the box.
        assert_eq!(
            rect_at(0, 0, 2, 2).points().collect::<Vec<_>>(),
            vec![
                Point::new(0, 0),
                Point::new(1, 0),
                Point::new(0, 1),
                Point::new(1, 1),
            ],
        );
        // Off-origin box: absolute coords, same row-major order.
        assert_eq!(
            rect_at(2, 3, 2, 2).points().collect::<Vec<_>>(),
            vec![
                Point::new(2, 3),
                Point::new(3, 3),
                Point::new(2, 4),
                Point::new(3, 4),
            ],
        );
    }

    #[test]
    fn rect_points_empty_for_zero_dim() {
        // AC2/AC10: a zero in either dimension yields no points.
        assert!(rect_at(0, 0, 0, 5).points().next().is_none());
        assert!(rect_at(0, 0, 5, 0).points().next().is_none());
        assert!(rect_at(0, 0, 0, 0).points().next().is_none());
    }

    #[test]
    fn rect_on_border_zero_dim_has_no_border() {
        // AC4/AC10: a zero-dim box has no border cells (dx + 1 == width form, no
        // width - 1 underflow).
        assert!(!rect_at(0, 0, 0, 0).on_border(Point::new(0, 0)));
        assert!(!rect_at(0, 0, 3, 0).on_border(Point::new(0, 0)));
        assert!(!rect_at(0, 0, 0, 3).on_border(Point::new(0, 0)));
    }

    #[test]
    fn rect_on_border_single_cell_is_border() {
        // AC4/AC10: the sole cell of a 1×1 box is on the border.
        assert!(rect_at(4, 4, 1, 1).on_border(Point::new(4, 4)));
    }

    #[test]
    fn rect_on_border_edges_and_corners_true_interior_false() {
        // AC4/AC10: every non-center cell of a 3×3 box is on the border; the
        // center is not; an out-of-box point is not.
        let r = rect_at(0, 0, 3, 3);
        assert!(r.on_border(Point::new(0, 0))); // corner
        assert!(r.on_border(Point::new(2, 2))); // corner
        assert!(r.on_border(Point::new(1, 0))); // edge
        assert!(r.on_border(Point::new(0, 1))); // edge
        assert!(r.on_border(Point::new(2, 1))); // edge (dx + 1 == width)
        assert!(!r.on_border(Point::new(1, 1))); // strict interior
        assert!(!r.on_border(Point::new(5, 5))); // out of box
        assert!(!r.on_border(Point::new(-1, 0))); // negative delta → None
    }
}
