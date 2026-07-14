//! Corridor-graph helpers over the dense-bitmap [`Corridor`] (design doc §1–§3).
//!
//! Flood-fill and 4-connected component counting of `D` and its complement, an
//! in-`D` geodesic BFS (reusable-scratch + eager forms), and boundary-wall
//! derivation. All are pure, deterministic, integer-only, and std-only. Every
//! path routes through [`Corridor`]'s private index/box helpers, so cells outside
//! the box are `¬D` by construction. Re-exported flat at [`crate::geom`].

use std::ops::ControlFlow;

use super::{Coord, Corridor, Point, Wall};

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

/// The number of **bounded** 4-connected components of the complement `¬D`.
///
/// Counts holes of `¬D` (cells not in `D`) over the bounding box that do **not**
/// touch the box border — infield holes fully enclosed by `D`, each of ≥1 cell. A
/// complement component is *unbounded* (the outfield) iff any of its cells lies on
/// the box border, since everything outside the box is one connected non-drivable
/// region (design doc §2, Ф4 — "exactly one bounded hole" is then `== 1`). Works
/// regardless of box margin; deterministic; `0` for an empty or fully-drivable
/// corridor.
pub fn bounded_complement_components(d: &Corridor) -> usize {
    let mut visited = vec![false; d.area()];
    let mut out = Vec::new();
    let mut count = 0;
    for p in d.box_points() {
        out.clear();
        let touches_boundary = flood_component(d, |q| !d.contains(q), &mut visited, p, &mut out);
        if !out.is_empty() && !touches_boundary {
            count += 1;
        }
    }
    count
}

/// Reusable scratch buffers for repeated in-`D` geodesic BFS queries.
///
/// Owns a generation-stamped visited buffer plus two frontier buffers, all sized
/// once to a corridor's bounding box, so successive [`geodesic_bfs`] queries reset
/// in O(1) (bump the generation) instead of reallocating a full-box buffer per
/// query (design doc §3; AC6). Bind one scratch to the single corridor it queries
/// — [`geodesic_bfs`] `debug_assert!`s a matching box.
///
/// [`geodesic_bfs`]: CorridorScratch::geodesic_bfs
#[derive(Clone, Debug)]
pub struct CorridorScratch {
    width: i32,
    height: i32,
    /// `stamp[i] == generation` ⟺ cell `i` was visited by the current query.
    stamp: Vec<u32>,
    /// The current query's stamp value; bumped once per [`geodesic_bfs`] call.
    ///
    /// [`geodesic_bfs`]: CorridorScratch::geodesic_bfs
    generation: u32,
    /// The frontier being visited (cells at the current geodesic distance).
    frontier: Vec<Point>,
    /// Scratch for the next frontier being built (double-buffered with `frontier`).
    next_frontier: Vec<Point>,
}

impl CorridorScratch {
    /// A scratch sized to `d`'s bounding box, ready to query `d`.
    ///
    /// Bind the returned scratch to `d`; reusing it against a differently-sized
    /// corridor trips a `debug_assert!` in [`geodesic_bfs`].
    ///
    /// [`geodesic_bfs`]: CorridorScratch::geodesic_bfs
    pub fn new(d: &Corridor) -> Self {
        Self {
            width: d.width(),
            height: d.height(),
            stamp: vec![0; d.area()],
            generation: 0,
            frontier: Vec::new(),
            next_frontier: Vec::new(),
        }
    }

    /// Advance to a fresh generation, clearing the stamp on `u32` wrap.
    ///
    /// The reset is O(1) on the common path; the `O(area)` fill happens at most
    /// once per ~4·10⁹ queries (`checked_add` wrap → refill, restart at `1`).
    fn bump_generation(&mut self) -> u32 {
        let next = self.generation.checked_add(1).unwrap_or_else(|| {
            self.stamp.fill(0);
            1
        });
        self.generation = next;
        next
    }

    /// 4-connected geodesic BFS over `D` from `seed`, one distance layer at a time.
    ///
    /// Confined to `D` — it never steps to a `¬D` cell, so it provably never
    /// crosses a wall (design doc §3) — it calls `visit(distance, &layer)` for each
    /// layer of `D`-cells at strictly increasing 4-conn geodesic distance from
    /// `seed`; every equal-distance cell shares one layer (ties grouped, for the
    /// caller's seeded pick — design doc §3). Returns `Some(b)` when `visit` breaks
    /// via [`ControlFlow::Break`] carrying `b` — an early stop, e.g. the first layer
    /// holding a free cell — or `None` when BFS exhausts the component or
    /// `seed ∉ D`. Reuses
    /// `self`'s buffers via an O(1) generation-stamp reset (AC6). Intra-layer order
    /// is fixed and reproducible (AC5), but callers must treat a layer only as an
    /// unordered tie set.
    pub fn geodesic_bfs<B>(
        &mut self,
        d: &Corridor,
        seed: Point,
        mut visit: impl FnMut(usize, &[Point]) -> ControlFlow<B>,
    ) -> Option<B> {
        debug_assert!(
            self.width == d.width() && self.height == d.height(),
            "CorridorScratch is bound to a differently-sized corridor"
        );
        let seed_idx = d.index(seed)?;
        if !d.contains(seed) {
            return None;
        }
        let generation = self.bump_generation();
        self.frontier.clear();
        self.next_frontier.clear();
        self.stamp[seed_idx] = generation;
        self.frontier.push(seed);
        let mut distance = 0;
        while !self.frontier.is_empty() {
            if let ControlFlow::Break(b) = visit(distance, &self.frontier) {
                return Some(b);
            }
            self.next_frontier.clear();
            for &p in &self.frontier {
                for n in p.neighbors4() {
                    if let Some(i) = d.index(n)
                        && self.stamp[i] != generation
                        && d.contains(n)
                    {
                        self.stamp[i] = generation;
                        self.next_frontier.push(n);
                    }
                }
            }
            std::mem::swap(&mut self.frontier, &mut self.next_frontier);
            distance += 1;
        }
        None
    }
}

/// All 4-connected geodesic distance layers of `D` from `seed`, eagerly collected.
///
/// A convenience wrapper over [`CorridorScratch::geodesic_bfs`] that allocates its
/// own scratch and materializes every layer: element `k` is the set of `D`-cells at
/// 4-conn geodesic distance `k` from `seed` (empty when `seed ∉ D`). For repeated
/// queries prefer one reused [`CorridorScratch`] (AC6). Deterministic (design §3a).
pub fn geodesic_layers(d: &Corridor, seed: Point) -> Vec<Vec<Point>> {
    let mut scratch = CorridorScratch::new(d);
    let mut layers = Vec::new();
    scratch.geodesic_bfs(d, seed, |_distance, layer| {
        layers.push(layer.to_vec());
        ControlFlow::<()>::Continue(())
    });
    layers
}

/// The exact set of dual boundary edges (walls) of the corridor `D`.
///
/// For every drivable cell and every [`Side`] whose neighbour is not in `D`,
/// emits one [`Wall`] anchored to that cell and side. Because a `D ↔ ¬D` adjacency
/// has exactly one drivable side, each boundary edge is emitted **exactly once**,
/// and no edge lies between two `D` cells (design doc §1 duality). Feeds
/// [`TrackArtifact`](crate::track::TrackArtifact)'s walls; deterministic (design §3a).
pub fn walls_from_boundary(d: &Corridor) -> Vec<Wall> {
    let mut walls = Vec::new();
    for cell in d.box_points() {
        if !d.contains(cell) {
            continue;
        }
        for side in Side::ALL {
            let (dx, dy) = side.delta();
            let neighbour = Point::new(cell.x + dx, cell.y + dy);
            if !d.contains(neighbour) {
                walls.push(Wall { cell, side });
            }
        }
    }
    walls
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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

    /// All `(x, y)` in the inclusive rectangle `[x0..=x1] × [y0..=y1]`.
    fn rect(x0: Coord, x1: Coord, y0: Coord, y1: Coord) -> Vec<(Coord, Coord)> {
        (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .collect()
    }

    /// The 3×3 ring `rect(1,3,1,3)` minus its center `(2,2)` — a rectangular
    /// annulus reused across the geodesic and wall tests.
    fn ring_3x3() -> Vec<(Coord, Coord)> {
        rect(1, 3, 1, 3)
            .into_iter()
            .filter(|&p| p != (2, 2))
            .collect()
    }

    /// Collect the geodesic layers as ordered per-layer `HashSet`s (distance order
    /// is contractual; intra-layer order is an impl detail).
    fn layer_sets(d: &Corridor, seed: Point) -> Vec<HashSet<Point>> {
        geodesic_layers(d, seed)
            .into_iter()
            .map(|layer| layer.into_iter().collect())
            .collect()
    }

    /// A boundary `Wall` at `(x, y)` on `side`.
    fn wall(x: Coord, y: Coord, side: Side) -> Wall {
        Wall {
            cell: Point::new(x, y),
            side,
        }
    }

    /// Collect `walls_from_boundary` into a `HashSet` (edge order is an impl detail).
    fn wall_set(d: &Corridor) -> HashSet<Wall> {
        walls_from_boundary(d).into_iter().collect()
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
    fn solid_rectangle_has_no_bounded_holes() {
        // A solid filled block: ¬D is one region touching the border → 0 (AC2).
        let d = corridor((0, 0), 5, 5, &rect(1, 3, 1, 3));
        assert_eq!(bounded_complement_components(&d), 0);
    }

    #[test]
    fn annulus_has_one_bounded_hole() {
        // 3×3 ring (block minus its center) → the enclosed center is 1 hole (AC2).
        let ring: Vec<_> = rect(1, 3, 1, 3)
            .into_iter()
            .filter(|&p| p != (2, 2))
            .collect();
        let d = corridor((0, 0), 5, 5, &ring);
        assert_eq!(bounded_complement_components(&d), 1);
    }

    #[test]
    fn two_hole_shape_has_two_bounded_holes() {
        // A 5×3 block with two separate enclosed cells → 2 holes (AC2).
        let body: Vec<_> = rect(1, 5, 1, 3)
            .into_iter()
            .filter(|&p| p != (2, 2) && p != (4, 2))
            .collect();
        let d = corridor((0, 0), 7, 5, &body);
        assert_eq!(bounded_complement_components(&d), 2);
    }

    #[test]
    fn empty_corridor_has_no_bounded_holes() {
        // Whole box is the unbounded outfield (touches the border) → 0 (AC2).
        let d = corridor((0, 0), 5, 5, &[]);
        assert_eq!(bounded_complement_components(&d), 0);
    }

    #[test]
    fn ring_flush_to_box_edges_has_one_bounded_hole() {
        // A ring flush to all four box edges still encloses a bounded interior
        // hole (unbounded ⟺ touches border, so a flush ring's hole is bounded).
        let perimeter: Vec<_> = rect(0, 4, 0, 4)
            .into_iter()
            .filter(|&(x, y)| x == 0 || x == 4 || y == 0 || y == 4)
            .collect();
        let d = corridor((0, 0), 5, 5, &perimeter);
        assert_eq!(bounded_complement_components(&d), 1);
    }

    #[test]
    fn geodesic_layers_straight_corridor_are_distance_bands() {
        // A straight 1-wide corridor: each layer is exactly one cell, the next step
        // along the run (AC3 — strictly increasing distance bands).
        let d = corridor((0, 0), 6, 3, &[(0, 1), (1, 1), (2, 1), (3, 1)]);
        assert_eq!(
            layer_sets(&d, Point::new(0, 1)),
            vec![
                cells(&[(0, 1)]),
                cells(&[(1, 1)]),
                cells(&[(2, 1)]),
                cells(&[(3, 1)]),
            ],
        );
    }

    #[test]
    fn geodesic_layers_annulus_ties_share_a_layer() {
        // Seed at the bottom-mid of the 3×3 ring: the two arms climb at equal
        // distance, so each of layers 1–3 holds two equidistant cells (AC3 tie),
        // and the opposite midpoint (2,3) is reached from both arms at distance 4.
        let d = corridor((0, 0), 5, 5, &ring_3x3());
        let layers = layer_sets(&d, Point::new(2, 1));
        assert_eq!(
            layers,
            vec![
                cells(&[(2, 1)]),
                cells(&[(1, 1), (3, 1)]),
                cells(&[(1, 2), (3, 2)]),
                cells(&[(1, 3), (3, 3)]),
                cells(&[(2, 3)]),
            ],
        );
        // The equal-distance tie: both arms' cells share one layer.
        assert_eq!(layers[1].len(), 2);
    }

    #[test]
    fn geodesic_layers_empty_when_seed_not_in_d() {
        // Seed outside D → no layers (AC3 boundary case).
        let d = corridor((0, 0), 5, 5, &[(1, 1)]);
        assert!(geodesic_layers(&d, Point::new(3, 3)).is_empty());
        let mut scratch = CorridorScratch::new(&d);
        let out: Option<()> =
            scratch.geodesic_bfs(&d, Point::new(3, 3), |_, _| ControlFlow::Break(()));
        assert_eq!(out, None);
    }

    #[test]
    fn geodesic_layers_deterministic() {
        // Identical input → byte-identical layers, order included (AC5).
        let d = corridor((0, 0), 5, 5, &ring_3x3());
        let seed = Point::new(2, 1);
        assert_eq!(geodesic_layers(&d, seed), geodesic_layers(&d, seed));
    }

    #[test]
    fn geodesic_bfs_break_stops_and_returns_payload() {
        // ControlFlow::Break at distance 2 returns Some(payload) and visits no
        // further layer (AC3 early-stop path). Straight corridor → one cell/layer.
        let d = corridor((0, 0), 6, 3, &[(0, 1), (1, 1), (2, 1), (3, 1)]);
        let mut scratch = CorridorScratch::new(&d);
        let mut layers_seen = 0usize;
        let result = scratch.geodesic_bfs(&d, Point::new(0, 1), |distance, layer| {
            layers_seen += 1;
            if distance == 2 {
                ControlFlow::Break(layer[0])
            } else {
                ControlFlow::Continue(())
            }
        });
        assert_eq!(result, Some(Point::new(2, 1)));
        assert_eq!(layers_seen, 3); // layers 0, 1, 2 visited, then broke
    }

    #[test]
    fn scratch_reuse_yields_identical_layers() {
        // AC6: two successive queries on one reused scratch give identical layers —
        // the first query's stamps do not pollute the second.
        let d = corridor((0, 0), 5, 5, &ring_3x3());
        let seed = Point::new(2, 1);
        let mut scratch = CorridorScratch::new(&d);
        let mut first = Vec::new();
        scratch.geodesic_bfs(&d, seed, |_, layer| {
            first.push(layer.to_vec());
            ControlFlow::<()>::Continue(())
        });
        let mut second = Vec::new();
        scratch.geodesic_bfs(&d, seed, |_, layer| {
            second.push(layer.to_vec());
            ControlFlow::<()>::Continue(())
        });
        assert_eq!(first, second);
        assert_eq!(first, geodesic_layers(&d, seed));
    }

    #[test]
    fn scratch_reuse_second_seed_unpolluted() {
        // AC6: after a query from one seed, a query from a different seed on the
        // same scratch matches a fresh computation (no stale-stamp pollution).
        let d = corridor((0, 0), 5, 5, &ring_3x3());
        let mut scratch = CorridorScratch::new(&d);
        // Drive a full first query to stamp the buffer; its output is irrelevant.
        scratch.geodesic_bfs(&d, Point::new(2, 1), |_, _| ControlFlow::<()>::Continue(()));
        let mut second = Vec::new();
        scratch.geodesic_bfs(&d, Point::new(1, 3), |_, layer| {
            second.push(layer.to_vec());
            ControlFlow::<()>::Continue(())
        });
        assert_eq!(second, geodesic_layers(&d, Point::new(1, 3)));
    }

    #[test]
    fn walls_of_solid_2x2_block_are_eight_outward_edges() {
        // A solid 2×2 block: each cell contributes its two outward sides — the
        // square's 8-edge perimeter, no interior edges between two D cells (AC4).
        let d = corridor((0, 0), 5, 5, &[(1, 1), (2, 1), (1, 2), (2, 2)]);
        let expected: HashSet<Wall> = [
            wall(1, 1, Side::West),
            wall(1, 1, Side::South),
            wall(2, 1, Side::East),
            wall(2, 1, Side::South),
            wall(1, 2, Side::West),
            wall(1, 2, Side::North),
            wall(2, 2, Side::East),
            wall(2, 2, Side::North),
        ]
        .into_iter()
        .collect();
        assert_eq!(wall_set(&d), expected);
        // Each edge exactly once: raw Vec length equals its deduped length
        // (mirrors the `no_duplicate_cells` supercover test).
        let v = walls_from_boundary(&d);
        assert_eq!(v.len(), v.iter().collect::<HashSet<_>>().len());
        assert_eq!(v.len(), 8);
    }

    #[test]
    fn walls_of_ring_are_outer_and_inner_edges_each_once() {
        // The 3×3 ring: outer perimeter edges plus the four inner edges facing the
        // enclosed hole (2,2) — each D↔¬D pair once, none between two D cells (AC4).
        let d = corridor((0, 0), 5, 5, &ring_3x3());
        let expected: HashSet<Wall> = [
            // bottom row
            wall(1, 1, Side::West),
            wall(1, 1, Side::South),
            wall(2, 1, Side::North), // inner (faces the hole)
            wall(2, 1, Side::South),
            wall(3, 1, Side::East),
            wall(3, 1, Side::South),
            // middle row (both cells straddle the hole)
            wall(1, 2, Side::East), // inner
            wall(1, 2, Side::West),
            wall(3, 2, Side::East),
            wall(3, 2, Side::West), // inner
            // top row
            wall(1, 3, Side::West),
            wall(1, 3, Side::North),
            wall(2, 3, Side::North),
            wall(2, 3, Side::South), // inner
            wall(3, 3, Side::East),
            wall(3, 3, Side::North),
        ]
        .into_iter()
        .collect();
        assert_eq!(wall_set(&d), expected);
        let v = walls_from_boundary(&d);
        assert_eq!(v.len(), v.iter().collect::<HashSet<_>>().len());
        assert_eq!(v.len(), 16);
    }

    #[test]
    fn all_helpers_are_deterministic() {
        // AC5: every helper returns byte-identical output (Vec order included) for
        // identical input across runs — one fixture exercised by each entry point.
        let d = corridor((0, 0), 5, 5, &ring_3x3());
        let seed = Point::new(2, 1);
        assert_eq!(flood_fill(&d, seed), flood_fill(&d, seed));
        assert_eq!(component_count(&d), component_count(&d));
        assert_eq!(
            bounded_complement_components(&d),
            bounded_complement_components(&d),
        );
        assert_eq!(geodesic_layers(&d, seed), geodesic_layers(&d, seed));
        assert_eq!(walls_from_boundary(&d), walls_from_boundary(&d));
    }
}
