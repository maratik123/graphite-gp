//! Dual-grid geometry primitives (design doc §1).
//!
//! Core invariant: a [`Point`] is the center of a unit cell (integer
//! coordinates); a [`Wall`] is a dual edge on the half-grid — the shared boundary
//! between two 4-adjacent cells where one is drivable and one is not. From this
//! duality, "a wall never passes through a point" and "a car never touches a
//! wall" hold by construction.

/// Integer grid coordinate.
pub type Coord = i32;

/// An integer grid point = the center of one unit cell.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Point {
    pub x: Coord,
    pub y: Coord,
}

impl Point {
    pub const fn new(x: Coord, y: Coord) -> Self {
        Self { x, y }
    }

    /// The 4-connected neighbours. Movement connectivity is 4-conn throughout
    /// (design doc §1): it fixes the Manhattan metric and forbids diagonal
    /// "needle's-eye" slips between two walls.
    pub const fn neighbors4(self) -> [Point; 4] {
        [
            Point::new(self.x + 1, self.y),
            Point::new(self.x - 1, self.y),
            Point::new(self.x, self.y + 1),
            Point::new(self.x, self.y - 1),
        ]
    }
}

/// Orientation of a dual edge (a wall) on the half-grid.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Orient {
    /// A horizontal edge — the boundary between a cell and its N/S neighbour.
    Horizontal,
    /// A vertical edge — the boundary between a cell and its E/W neighbour.
    Vertical,
}

/// A wall = one dual edge on the half-grid, anchored to the cell it borders plus
/// which side. Walls are *derived* from the corridor boundary (design doc §1),
/// never authored by hand.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Wall {
    /// The drivable cell this edge borders.
    pub cell: Point,
    /// Which side of `cell` the edge sits on.
    pub orient: Orient,
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
    pub fn new(origin: Point, width: i32, height: i32) -> Self {
        assert!(
            width >= 0 && height >= 0,
            "corridor dimensions must be non-negative"
        );
        Self {
            origin,
            width,
            height,
            cells: vec![false; (width * height) as usize],
        }
    }

    pub fn origin(&self) -> Point {
        self.origin
    }
    pub fn width(&self) -> i32 {
        self.width
    }
    pub fn height(&self) -> i32 {
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

    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|&c| !c)
    }

    fn index(&self, p: Point) -> Option<usize> {
        let (dx, dy) = (p.x - self.origin.x, p.y - self.origin.y);
        if dx < 0 || dy < 0 || dx >= self.width || dy >= self.height {
            return None;
        }
        Some((dy * self.width + dx) as usize)
    }
}

/// Strict supercover of the segment `a → b`: every cell the segment touches,
/// **including corner-clipped cells** (design doc §3, `legal_move`).
///
/// This is what makes a fast chord unable to jump a wall or squeeze through a
/// dual vertex pinched between two walls. It is used identically as the runtime
/// legality rule and as the passability-oracle graph edge — one implementation,
/// two callers.
///
/// TODO(3a): implement the strict, corner-aware supercover.
pub fn supercover(_a: Point, _b: Point) -> Vec<Point> {
    todo!("strict supercover (design doc §3)")
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
