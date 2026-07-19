//! Regions layer (design doc §4, layer 1): outfield, asphalt, infield.
//!
//! Asphalt is *derived* from the corridor `D` (never authored by hand — design
//! doc §0/§1 duality); the infield is the corridor's complement's one bounded
//! hole, found in-crate via a border-seeded flood over the complement `¬D`,
//! confined to the corridor's bounding box — no gp-core change (design §
//! *Rejected alternatives*).

use super::TrackTransform;
use egui::{Painter, Rect};
use gp_core::geom::{Corridor, Point};
use std::collections::HashSet;

/// Every drivable/non-drivable cell in `d`'s bounding box, classified into
/// the three regions (design doc §4, layer 1). Order within each `Vec` is an
/// implementation detail — callers that need a set compare should collect
/// into a `HashSet`.
#[derive(Clone, Debug, Default)]
pub(crate) struct RegionCells {
    /// Drivable cells (`== D`, AC1).
    pub asphalt: Vec<Point>,
    /// `¬D` cells reachable from the bounding-box border (AC2).
    pub outfield: Vec<Point>,
    /// `¬D` cells **not** reachable from the border — the bounded hole(s)
    /// (AC2).
    pub infield: Vec<Point>,
}

/// Every cell point in the box `[origin, x1) × [origin, y1)`, in row-major
/// (`y`-outer, `x`-inner) order. Mirrors `gp_core::geom::Rect::points`, which
/// is not itself public.
fn bbox_points(origin: Point, x1: i32, y1: i32) -> impl Iterator<Item = Point> + Clone {
    (origin.y..y1).flat_map(move |y| (origin.x..x1).map(move |x| Point::new(x, y)))
}

/// The corridor bounding box's half-open exclusive corner `(x1, y1)`, i.e.
/// `origin + (width, height)`, saturating rather than overflowing — mirrors
/// `gp_core::geom::Rect::points`' own precondition treatment.
fn bbox_exclusive_max(d: &Corridor) -> (i32, i32) {
    let origin = d.origin();
    let x1 = i32::try_from(d.width()).map_or(i32::MAX, |w| origin.x.saturating_add(w));
    let y1 = i32::try_from(d.height()).map_or(i32::MAX, |h| origin.y.saturating_add(h));
    (x1, y1)
}

/// Whether `p` (already known to lie in the bbox) sits on its border.
const fn is_border(p: Point, origin: Point, x1: i32, y1: i32) -> bool {
    p.x == origin.x || p.y == origin.y || p.x == x1.saturating_sub(1) || p.y == y1.saturating_sub(1)
}

/// Whether `p` lies in the half-open box `[origin, (x1, y1))`.
const fn in_bbox(p: Point, origin: Point, x1: i32, y1: i32) -> bool {
    p.x >= origin.x && p.x < x1 && p.y >= origin.y && p.y < y1
}

/// Classifies every bbox cell into asphalt / outfield / infield (AC1/AC2).
///
/// The infield is found by flooding the complement `¬D` from every
/// border cell that is itself `¬D`, confined to the bbox (4-connected, like
/// `gp_core::geom::flood_fill`, but over the complement predicate, which
/// gp-core does not expose). Any `¬D` cell the flood never reaches is a
/// bounded hole — the infield.
pub(crate) fn classify(d: &Corridor) -> RegionCells {
    let origin = d.origin();
    let (x1, y1) = bbox_exclusive_max(d);

    let asphalt: Vec<Point> = bbox_points(origin, x1, y1)
        .filter(|&p| d.contains(p))
        .collect();

    let mut visited: HashSet<Point> = HashSet::new();
    let mut outfield: Vec<Point> = Vec::new();
    let mut stack: Vec<Point> = bbox_points(origin, x1, y1)
        .filter(|&p| !d.contains(p) && is_border(p, origin, x1, y1))
        .collect();
    for &p in &stack {
        visited.insert(p);
    }
    while let Some(p) = stack.pop() {
        outfield.push(p);
        for n in p.neighbors4() {
            if in_bbox(n, origin, x1, y1) && !d.contains(n) && visited.insert(n) {
                stack.push(n);
            }
        }
    }

    let infield: Vec<Point> = bbox_points(origin, x1, y1)
        .filter(|&p| !d.contains(p) && !visited.contains(&p))
        .collect();

    RegionCells {
        asphalt,
        outfield,
        infield,
    }
}

/// The screen-space unit-cell rect centered on lattice point `p`.
fn cell_rect(transform: &TrackTransform, p: Point) -> Rect {
    #[allow(
        clippy::cast_precision_loss,
        reason = "cell coordinates are grid-realistic i32s, far below f32's \
                  exact-integer range; precedent: gp-core track.rs::normalize"
    )]
    let (fx, fy) = (p.x as f32, p.y as f32);
    let a = transform.map((fx - 0.5, fy - 0.5));
    let b = transform.map((fx + 0.5, fy + 0.5));
    Rect::from_two_pos(a, b)
}

/// Paints the three regions back-to-front (design doc §4, layer 1): the
/// whole `rect` filled `PAPER_1` (outfield background), then each infield
/// cell (`SURFACE_INFIELD`), then each asphalt cell (`SURFACE_ASPHALT`).
pub(crate) fn paint(
    painter: &Painter,
    rect: Rect,
    transform: &TrackTransform,
    cells: &RegionCells,
) {
    painter.rect_filled(rect, 0, crate::tokens::color::SURFACE_PAGE);
    for &p in &cells.infield {
        painter.rect_filled(
            cell_rect(transform, p),
            0,
            crate::tokens::color::SURFACE_INFIELD,
        );
    }
    for &p in &cells.asphalt {
        painter.rect_filled(
            cell_rect(transform, p),
            0,
            crate::tokens::color::SURFACE_ASPHALT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::classify;
    use gp_core::geom::{Corridor, Point};
    use std::collections::HashSet;

    /// Builds a corridor over `[origin, origin + (w, h))` with the given
    /// `(x, y)` cells drivable — mirrors `gp_core::geom::graph::tests::corridor`.
    fn corridor(origin: (i32, i32), w: usize, h: usize, drivable: &[(i32, i32)]) -> Corridor {
        let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
        for &(x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    fn set(points: &[Point]) -> HashSet<Point> {
        points.iter().copied().collect()
    }

    /// A 3×3 ring around one hole cell, over a 5×5 bbox with a 1-cell
    /// outfield margin (mirrors `geom::graph::tests`' ring fixture).
    fn ring_3x3() -> Corridor {
        let cells: Vec<(i32, i32)> = [
            (1, 1),
            (2, 1),
            (3, 1),
            (1, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
        ]
        .to_vec();
        corridor((0, 0), 5, 5, &cells)
    }

    /// AC1 — the classified asphalt cell set equals `{p : corridor.contains(p)}`
    /// exactly, on a hand-built ring.
    #[test]
    fn asphalt_equals_corridor_contains() {
        let d = ring_3x3();
        let regions = classify(&d);
        let want: HashSet<Point> = [
            (1, 1),
            (2, 1),
            (3, 1),
            (1, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
        ]
        .into_iter()
        .map(|(x, y)| Point::new(x, y))
        .collect();
        assert_eq!(set(&regions.asphalt), want);
    }

    /// AC2 — the ring's center cell is the bounded infield hole; every other
    /// `¬D` cell (the 1-cell outfield margin) is border-reachable outfield;
    /// the two sets are disjoint from each other and from asphalt.
    #[test]
    fn infield_hole_and_outfield_are_disjoint() {
        let d = ring_3x3();
        let regions = classify(&d);

        assert_eq!(set(&regions.infield), set(&[Point::new(2, 2)]));
        assert!(!regions.outfield.contains(&Point::new(2, 2)));
        assert_eq!(regions.outfield.len(), 25 - 8 - 1);

        let asphalt = set(&regions.asphalt);
        let infield = set(&regions.infield);
        let outfield = set(&regions.outfield);
        assert!(asphalt.is_disjoint(&infield));
        assert!(asphalt.is_disjoint(&outfield));
        assert!(infield.is_disjoint(&outfield));
    }

    /// Edge — a solid block (no hole) has an empty infield; every `¬D` cell
    /// is border-reachable outfield.
    #[test]
    fn solid_block_has_empty_infield() {
        let cells: Vec<(i32, i32)> = (0..3).flat_map(|y| (0..3).map(move |x| (x, y))).collect();
        let d = corridor((0, 0), 5, 5, &cells);
        let regions = classify(&d);
        assert!(regions.infield.is_empty());
        assert_eq!(regions.outfield.len(), 25 - 9);
    }

    /// Edge — a ring flush against the bbox edge (no outfield margin) still
    /// yields its bounded hole (mirrors `bounded_complement_components`'s
    /// flush-to-edge fixture).
    #[test]
    fn flush_to_edge_ring_still_has_bounded_hole() {
        let cells: Vec<(i32, i32)> = [
            (0, 0),
            (1, 0),
            (2, 0),
            (0, 1),
            (2, 1),
            (0, 2),
            (1, 2),
            (2, 2),
        ]
        .to_vec();
        let d = corridor((0, 0), 3, 3, &cells);
        let regions = classify(&d);
        assert_eq!(set(&regions.infield), set(&[Point::new(1, 1)]));
        assert!(regions.outfield.is_empty());
    }
}
