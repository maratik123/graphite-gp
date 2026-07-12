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
        assert!(width >= 0 && height >= 0, "corridor dimensions must be non-negative");
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
