//! Dual-grid geometry primitives (design doc §1).
//!
//! Core invariant: a [`Point`] is the center of a unit cell (integer
//! coordinates); a [`Wall`] is a dual edge on the boundary of the corridor `D` —
//! anchored to a drivable cell plus the [`Side`] toward its non-drivable
//! neighbour. From this duality, "a wall never passes through a point" and "a car
//! never touches a wall" hold by construction.

/// Integer grid coordinate.
pub type Coord = i32;

/// An integer grid point = the center of one unit cell.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Point {
    /// Horizontal cell coordinate (grid column), increasing eastward.
    pub x: Coord,
    /// Vertical cell coordinate (grid row), increasing northward.
    pub y: Coord,
}

impl Point {
    /// The grid point at integer coordinates `(x, y)`.
    pub const fn new(x: Coord, y: Coord) -> Self {
        Self { x, y }
    }

    /// The 4-connected neighbours. Movement connectivity is 4-conn throughout
    /// (design doc §1): it fixes the Manhattan metric and forbids diagonal
    /// "needle's-eye" slips between two walls.
    pub const fn neighbors4(self) -> [Self; 4] {
        [
            Self::new(self.x + 1, self.y),
            Self::new(self.x - 1, self.y),
            Self::new(self.x, self.y + 1),
            Self::new(self.x, self.y - 1),
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

/// One of a cell's four axis-aligned sides.
///
/// The outward direction from a drivable cell toward a non-drivable neighbour
/// (design doc §1). Variant order mirrors [`Point::neighbors4`]: east, west,
/// north, south.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Side {
    /// The +x side — toward the eastern neighbour.
    East,
    /// The -x side — toward the western neighbour.
    West,
    /// The +y side — toward the northern neighbour (`y` increases northward).
    North,
    /// The -y side — toward the southern neighbour.
    South,
}

impl Side {
    /// All four sides, in [`Point::neighbors4`] order: east, west, north, south.
    pub const ALL: [Self; 4] = [Self::East, Self::West, Self::North, Self::South];

    /// The unit step `(dx, dy)` from a cell across this side to its neighbour.
    pub const fn delta(self) -> (Coord, Coord) {
        match self {
            Self::East => (1, 0),
            Self::West => (-1, 0),
            Self::North => (0, 1),
            Self::South => (0, -1),
        }
    }
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

/// The corridor `D` — the set of drivable points/cells (design doc §1).
///
/// Backed by a dense bitmap over a bounding box (`origin` + `width`×`height`) for
/// O(1) membership plus cheap flood-fill and distance-transform (design doc §2,
/// Ф4). Points outside the box are, by definition, not in `D`.
#[derive(Clone, Debug, Default)]
pub struct Corridor {
    origin: Point,
    width: i32,
    height: i32,
    cells: Vec<bool>,
}

impl Corridor {
    /// A new, empty corridor over the box `[origin, origin + (width, height))`.
    ///
    /// # Panics
    ///
    /// Panics if `width` or `height` is negative — corridor dimensions must be
    /// non-negative.
    pub fn new(origin: Point, width: i32, height: i32) -> Self {
        assert!(
            width >= 0 && height >= 0,
            "corridor dimensions must be non-negative"
        );
        // `width`/`height` are asserted `>= 0` immediately above, so the product
        // is non-negative and the `usize` cast cannot lose sign.
        #[allow(clippy::cast_sign_loss)]
        let cell_count = (width * height) as usize;
        Self {
            origin,
            width,
            height,
            cells: vec![false; cell_count],
        }
    }

    /// The bounding-box origin — its minimum-coordinate corner.
    pub const fn origin(&self) -> Point {
        self.origin
    }
    /// The bounding-box width, in cells (columns); always `>= 0`.
    pub const fn width(&self) -> i32 {
        self.width
    }
    /// The bounding-box height, in cells (rows); always `>= 0`.
    pub const fn height(&self) -> i32 {
        self.height
    }

    /// Is `p` a drivable point of `D`?
    pub fn contains(&self, p: Point) -> bool {
        self.index(p).is_some_and(|i| self.cells[i])
    }

    /// Mark `p` drivable / not drivable. No-op if `p` is outside the box.
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

    const fn index(&self, p: Point) -> Option<usize> {
        let (dx, dy) = (p.x - self.origin.x, p.y - self.origin.y);
        if dx < 0 || dy < 0 || dx >= self.width || dy >= self.height {
            return None;
        }
        // `dx`,`dy` lie in `[0, width) × [0, height)` by the guard above, so the
        // flat index is non-negative and the `usize` cast cannot lose sign.
        #[allow(clippy::cast_sign_loss)]
        let idx = (dy * self.width + dx) as usize;
        Some(idx)
    }

    /// The number of cells in the bounding box (`width × height`).
    // `width`/`height` are non-negative (asserted in `new`), so the product is
    // non-negative and the `usize` cast cannot lose sign.
    #[allow(clippy::cast_sign_loss)]
    const fn area(&self) -> usize {
        (self.width * self.height) as usize
    }

    /// Every cell point in the bounding box, in row-major (`y`-outer) order.
    fn box_points(&self) -> impl Iterator<Item = Point> {
        let origin = self.origin;
        let (width, height) = (self.width, self.height);
        (0..height)
            .flat_map(move |dy| (0..width).map(move |dx| Point::new(origin.x + dx, origin.y + dy)))
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
/// # Contract
///
/// - **Closed squares.** A boundary or single-corner touch counts. When the
///   segment crosses a dual vertex `(i + ½, j + ½)`, all four sharing cells are
///   returned — the correctness-critical tie (e.g. `(0,0)→(1,1)` yields all of
///   `(0,0),(1,0),(0,1),(1,1)`).
/// - **Exact & integer.** Membership is the integer test `2·|cr| ≤ |dx| + |dy|`
///   with `cr = dx·(c.y − a.y) − dy·(c.x − a.x)`, evaluated with no floating point
///   anywhere (design doc §3a).
/// - **Order-independent, duplicate-free.** The result is the exact cell set: as a
///   set `supercover(a, b) == supercover(b, a)`, each cell is pushed exactly once
///   (the bounding-box scan visits every cell once), and both endpoint cells are
///   always present (a degenerate `a == b` yields exactly `{a}`).
///
/// # Overflow precondition
///
/// Endpoints are assumed separated by a bounded chord — one move's velocity, with
/// `|v| ≪ 1.5×10⁹`. Within that domain the widened `i64` cross product never
/// overflows (its operands are taken relative to `a`, so `|cr| ≤ 2·|dx|·|dy|`).
/// Adversarial full-range `i32` endpoints lie outside the documented domain and
/// are not supported.
pub fn supercover(a: Point, b: Point) -> Vec<Point> {
    let dx = i64::from(b.x) - i64::from(a.x);
    let dy = i64::from(b.y) - i64::from(a.y);
    let bound = dx.abs() + dy.abs();
    let mut cover = Vec::new();
    for cx in a.x.min(b.x)..=a.x.max(b.x) {
        for cy in a.y.min(b.y)..=a.y.max(b.y) {
            let cr = dx * (i64::from(cy) - i64::from(a.y)) - dy * (i64::from(cx) - i64::from(a.x));
            if 2 * cr.abs() <= bound {
                cover.push(Point::new(cx, cy));
            }
        }
    }
    cover
}

/// 4-connected flood from `seed` over cells satisfying `in_set`, confined to the
/// bounding box.
///
/// Marks every reached cell in `visited` (indexed by [`Corridor::index`]) and
/// pushes it to `out`. Returns whether the component touches the box border
/// (`dx ∈ {0, w-1}` or `dy ∈ {0, h-1}`) — the "unbounded" flag the complement
/// counter reads. A no-op returning `false` when `seed` is outside the box, is
/// already visited, or fails `in_set`.
fn flood_component(
    d: &Corridor,
    in_set: impl Fn(Point) -> bool,
    visited: &mut [bool],
    seed: Point,
    out: &mut Vec<Point>,
) -> bool {
    let Some(seed_idx) = d.index(seed) else {
        return false;
    };
    if visited[seed_idx] || !in_set(seed) {
        return false;
    }
    visited[seed_idx] = true;
    let mut touches_boundary = false;
    let mut stack = vec![seed];
    while let Some(p) = stack.pop() {
        out.push(p);
        let (dx, dy) = (p.x - d.origin().x, p.y - d.origin().y);
        if dx == 0 || dy == 0 || dx == d.width() - 1 || dy == d.height() - 1 {
            touches_boundary = true;
        }
        for n in p.neighbors4() {
            if let Some(i) = d.index(n)
                && !visited[i]
                && in_set(n)
            {
                visited[i] = true;
                stack.push(n);
            }
        }
    }
    touches_boundary
}

/// The 4-connected component of the corridor `D` reachable from `seed`.
///
/// Returns the drivable cells 4-reachable from `seed` without leaving `D` (the
/// seed itself included when drivable); empty when `seed ∉ D`. Deterministic: for
/// a given corridor and seed the returned order is fixed (design doc §1, §3a).
pub fn flood_fill(d: &Corridor, seed: Point) -> Vec<Point> {
    let mut visited = vec![false; d.area()];
    let mut out = Vec::new();
    flood_component(d, |p| d.contains(p), &mut visited, seed, &mut out);
    out
}

/// The number of 4-connected components of the corridor `D`.
///
/// Counts maximal 4-connected clusters of drivable cells over the bounding box;
/// `0` for an empty corridor. Deterministic (design doc §1, §3a).
pub fn component_count(d: &Corridor) -> usize {
    let mut visited = vec![false; d.area()];
    let mut out = Vec::new();
    let mut count = 0;
    for p in d.box_points() {
        out.clear();
        flood_component(d, |q| d.contains(q), &mut visited, p, &mut out);
        if !out.is_empty() {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Collect a supercover into a `HashSet` so comparisons ignore iteration
    /// order (spec §4: the result is defined up to set equality).
    fn cover_set(a: Point, b: Point) -> HashSet<Point> {
        supercover(a, b).into_iter().collect()
    }

    /// Build an expected cell set from `(x, y)` literals.
    fn cells(pts: &[(Coord, Coord)]) -> HashSet<Point> {
        pts.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    /// Build a corridor over the box `[origin, origin + (w, h))` with the given
    /// `(x, y)` cells marked drivable.
    fn corridor(
        origin: (Coord, Coord),
        w: Coord,
        h: Coord,
        drivable: &[(Coord, Coord)],
    ) -> Corridor {
        let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
        for &(x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    /// Collect a `flood_fill` result into a `HashSet` (order is an impl detail).
    fn flood_set(d: &Corridor, seed: Point) -> HashSet<Point> {
        flood_fill(d, seed).into_iter().collect()
    }

    #[test]
    fn flood_fill_single_block_is_whole_block() {
        // A solid 2×2 block: flood from one cell returns exactly the block (AC1).
        let d = corridor((0, 0), 5, 5, &[(1, 1), (2, 1), (1, 2), (2, 2)]);
        assert_eq!(
            flood_set(&d, Point::new(1, 1)),
            cells(&[(1, 1), (2, 1), (1, 2), (2, 2)]),
        );
    }

    #[test]
    fn component_count_single_block_is_one() {
        // One 4-connected cluster → count 1 (AC1).
        let d = corridor((0, 0), 5, 5, &[(1, 1), (2, 1), (1, 2), (2, 2)]);
        assert_eq!(component_count(&d), 1);
    }

    #[test]
    fn two_disjoint_blocks_count_two_and_flood_isolates() {
        // Two clusters with no 4-adjacency → count 2; flood from one returns only
        // that cluster (AC1).
        let d = corridor((0, 0), 5, 5, &[(0, 0), (1, 0), (3, 3), (4, 3)]);
        assert_eq!(component_count(&d), 2);
        assert_eq!(flood_set(&d, Point::new(0, 0)), cells(&[(0, 0), (1, 0)]));
        assert_eq!(flood_set(&d, Point::new(3, 3)), cells(&[(3, 3), (4, 3)]));
    }

    #[test]
    fn flood_fill_empty_when_seed_not_in_d() {
        // Seed outside D → empty (AC1).
        let d = corridor((0, 0), 5, 5, &[(1, 1)]);
        assert!(flood_fill(&d, Point::new(3, 3)).is_empty());
    }

    #[test]
    fn empty_corridor_has_no_components() {
        // No drivable cells → count 0, flood empty (AC1).
        let d = corridor((0, 0), 5, 5, &[]);
        assert_eq!(component_count(&d), 0);
        assert!(flood_fill(&d, Point::new(2, 2)).is_empty());
    }

    #[test]
    fn axial_horizontal_no_diagonal() {
        // (0,0)→(3,0): the straight run only, no diagonal neighbours (AC3).
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
        // double-push, so assert the raw Vec length equals its deduped length.
        let v = supercover(Point::new(0, 0), Point::new(1, 1));
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
