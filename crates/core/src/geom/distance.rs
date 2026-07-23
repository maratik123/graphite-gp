//! Distance transform + medial axis over the corridor `D` (design doc §2, Ф4).
//!
//! Two pure, integer-only, deterministic primitives that Ф4's width check reads
//! directly and the future Ф7 centerline (`racing_line(medial_axis(D))`,
//! `docs/design.md` §2 line 191) will consume: a multi-source 4-connected BFS
//! wall-distance field ([`DistanceTransform`]) and its strict axis-wise ridge
//! ([`medial_axis`]). Both route through [`Corridor`]'s private index/box
//! helpers, exactly like [`super::component_count`] and
//! [`super::bounded_complement_components`].

use std::collections::{BTreeSet, VecDeque};

use super::{Corridor, Point, Rect};

/// The 4-connected wall-distance field of a corridor `D`.
///
/// `at(p)` is the Manhattan (4-conn step-count) distance from `p` to the nearest
/// `¬D` cell — `0` for any `p ∉ D` (including out-of-box points), `≥ 1` for every
/// drivable cell. A `D` cell on the box border is `¬D`-adjacent by construction
/// (out-of-box is `¬D`), so it always has `at == 1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistanceTransform {
    rect: Rect,
    dist: Vec<u32>,
}

impl DistanceTransform {
    /// Computes the wall-distance field of `d`.
    ///
    /// Multi-source 4-connected BFS: every `D` cell with a `¬D` 4-neighbor (a
    /// wall-adjacent cell, including the box border) seeds at distance `1`; BFS
    /// then relaxes outward through `D`, one layer at a time. `¬D` cells (and
    /// every out-of-box point) stay at the sentinel `0`, which never collides
    /// with a real distance since every reached `D` cell's distance is `≥ 1`.
    /// Deterministic (design doc §3a — integer-only, no floats).
    pub fn compute(d: &Corridor) -> Self {
        let rect = d.rect;
        let mut dist = vec![0u32; d.area()];
        let mut queue = VecDeque::new();

        // Seed pass: every D cell with a ¬D 4-neighbor starts at distance 1.
        for p in d.box_points() {
            let Some(idx) = d.index(p) else {
                continue;
            };
            if !d.contains(p) {
                continue;
            }
            if p.neighbors4().into_iter().any(|n| !d.contains(n)) {
                dist[idx] = 1;
                queue.push_back(p);
            }
        }

        // BFS relaxation: each popped cell's unvisited D neighbors get dist+1.
        while let Some(p) = queue.pop_front() {
            let Some(idx) = d.index(p) else {
                continue;
            };
            let next = dist[idx].saturating_add(1);
            for n in p.neighbors4() {
                let Some(ni) = d.index(n) else {
                    continue;
                };
                if d.contains(n) && dist[ni] == 0 {
                    dist[ni] = next;
                    queue.push_back(n);
                }
            }
        }

        Self { rect, dist }
    }

    /// The wall distance at `p` — `0` for any `p ∉ D` (out-of-box included).
    #[inline]
    pub fn at(&self, p: Point) -> u32 {
        self.rect.index(p).map_or(0, |i| self.dist[i])
    }

    /// The bounding box this field was computed over.
    #[inline]
    pub const fn rect(&self) -> Rect {
        self.rect
    }
}

/// The strict axis-wise distance-transform ridge of `dt` (design doc §D2, "гребень
/// distance-transform").
///
/// A `D` cell `p` is a medial cell iff it is a **strict** local maximum of the
/// distance transform along at least one axis: `dt(p) > dt(p ± x̂)` (both
/// horizontal neighbors) **or** `dt(p) > dt(p ± ŷ)` (both vertical neighbors),
/// reading `dt == 0` for any `¬D` / out-of-box neighbor. Strict inequality is
/// load-bearing: the along-flow axis of a straight corridor is a distance-transform
/// **plateau** (constant `dt`), so a non-strict `≥` would admit every cell and
/// collapse the ridge to the whole corridor.
///
/// A **neck is always on this ridge**: at a narrow cross-section's center cell the
/// two perpendicular walls are close, so `dt` is a strict local max *across* the
/// neck — the ridge stays 4-connected through a constriction rather than leaving a
/// gap there (unlike a pure local-maximum definition, which a neck's along-flow
/// DT-valley would exclude).
///
/// Returns a [`BTreeSet`] for deterministic, cross-platform iteration order
/// ([`Point`]'s derived `Ord`, `x`-then-`y`).
///
/// This primitive ships the two responsibilities the future Ф7 centerline
/// (`racing_line`, `docs/design.md` §2 line 191) still owns: thinning an
/// even-width 2-cell ridge band to a single strand, and bridging a residual
/// 1-cell diagonal gap at a rectilinear corner. Both are out of scope here.
pub fn medial_axis(dt: &DistanceTransform) -> BTreeSet<Point> {
    let rect = dt.rect();
    let mut out = BTreeSet::new();
    for p in rect.points() {
        let dp = dt.at(p);
        if dp == 0 {
            continue;
        }
        let east = Point::new(p.x.saturating_add(1), p.y);
        let west = Point::new(p.x.saturating_sub(1), p.y);
        let north = Point::new(p.x, p.y.saturating_add(1));
        let south = Point::new(p.x, p.y.saturating_sub(1));
        let x_ridge = dp > dt.at(east) && dp > dt.at(west);
        let y_ridge = dp > dt.at(north) && dp > dt.at(south);
        if x_ridge || y_ridge {
            out.insert(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::common::cells;
    use crate::geom::{Coord, component_count};

    /// Build a corridor over `[origin, origin + (w, h))` with the given `(x, y)`
    /// cells marked drivable.
    fn corridor(
        origin: (Coord, Coord),
        w: usize,
        h: usize,
        drivable: &[(Coord, Coord)],
    ) -> Corridor {
        let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
        for &(x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    #[test]
    fn dt_straight_band_is_distance_to_nearest_wall() {
        // A fully-filled 5x3 box: DT along the 3-row height is 1,2,1 pattern, and
        // the middle row peaks at 2 for interior columns, dropping to 1 at the
        // left/right box edges (design doc's "1,2,...,2,1" fixture).
        let d = Corridor::filled(Point::new(0, 0), 5, 3);
        let dt = DistanceTransform::compute(&d);

        for x in 0..5 {
            assert_eq!(dt.at(Point::new(x, 0)), 1, "bottom row at x={x}");
            assert_eq!(dt.at(Point::new(x, 2)), 1, "top row at x={x}");
        }
        assert_eq!(dt.at(Point::new(0, 1)), 1);
        assert_eq!(dt.at(Point::new(1, 1)), 2);
        assert_eq!(dt.at(Point::new(2, 1)), 2);
        assert_eq!(dt.at(Point::new(3, 1)), 2);
        assert_eq!(dt.at(Point::new(4, 1)), 1);
    }

    #[test]
    fn at_is_zero_outside_d_and_outside_box() {
        let d = corridor((0, 0), 3, 3, &[(1, 1)]);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(dt.at(Point::new(0, 0)), 0, "in-box, not in D");
        assert_eq!(dt.at(Point::new(-1, -1)), 0, "out of box");
        assert_eq!(dt.at(Point::new(1, 1)), 1, "sole D cell, wall-adjacent");
    }

    #[test]
    fn medial_axis_is_thin_centerline_on_straight_band() {
        // Same 5x3 filled band: the medial axis is exactly the interior of the
        // middle row (the cross-flow centerline), excluding the two end columns
        // whose DT ties with a neighbor.
        let d = Corridor::filled(Point::new(0, 0), 5, 3);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(
            medial_axis(&dt),
            cells(&[(1, 1), (2, 1), (3, 1)])
                .into_iter()
                .collect::<BTreeSet<_>>(),
        );
    }

    #[test]
    fn medial_axis_includes_neck_and_is_connected_across_it() {
        // A wide 3-row-tall corridor pinched to a single-row neck at x=3: the
        // medial axis must include the neck cell and stay 4-connected through it
        // (the Issue-#1 property a local-maximum-only ridge would break).
        let mut drivable = Vec::new();
        for x in 0..7 {
            if x == 3 {
                drivable.push((x, 2));
            } else {
                for y in 1..4 {
                    drivable.push((x, y));
                }
            }
        }
        let d = corridor((0, 0), 7, 5, &drivable);
        let dt = DistanceTransform::compute(&d);
        let medial = medial_axis(&dt);

        for p in [Point::new(2, 2), Point::new(3, 2), Point::new(4, 2)] {
            assert!(medial.contains(&p), "{p:?} should be on the ridge");
        }
    }

    #[test]
    fn medial_axis_forms_four_connected_strips_on_annulus() {
        // An odd-thickness-3 square frame (11x11 outer minus a 5x5 centered
        // hole): each straight stretch's cross-frame DT profile is 1,2,1, giving
        // a strict local max at its middle column/row throughout — unlike an
        // even thickness, whose middle two cells tie and never form a continuous
        // strip. Each side's ridge is one connected strip (4 total, one per
        // side); the 4 strips are NOT joined at the corners — a diagonal gap
        // remains where the along-axis strict-max condition ties across the
        // corner's 2x2 plateau (design doc Open Q2 "AC5 Ф7-consumer
        // reconciliation": bridging that residual corner gap is Ф7's job, not
        // this primitive's).
        let mut d = Corridor::filled(Point::new(0, 0), 11, 11);
        for y in 3..8 {
            for x in 3..8 {
                d.set(Point::new(x, y), false);
            }
        }
        let dt = DistanceTransform::compute(&d);
        let medial = medial_axis(&dt);

        let mut expected = BTreeSet::new();
        for x in 3..8 {
            expected.insert(Point::new(x, 1));
            expected.insert(Point::new(x, 9));
        }
        for y in 3..8 {
            expected.insert(Point::new(1, y));
            expected.insert(Point::new(9, y));
        }
        assert_eq!(medial, expected);

        // Each side's strip is internally 4-connected (a genuine centerline,
        // not scattered points).
        let mut top = Corridor::new(Point::new(0, 0), 11, 11);
        for x in 3..8 {
            top.set(Point::new(x, 1), true);
        }
        assert_eq!(
            component_count(&top),
            1,
            "the top strip must be one connected run"
        );
    }

    #[test]
    fn medial_axis_even_width_band_is_two_cell() {
        // A 4x3 filled box: the cross-width (x, 4 cells) is even, so the two
        // middle columns tie on DT and neither is a strict x-ridge; both still
        // qualify via the y-axis (the length, 3 rows, is short enough to make the
        // single middle row a strict y-local-max) — the documented "2-cell ridge
        // band" Ф7 later thins to a single strand.
        let d = Corridor::filled(Point::new(0, 0), 4, 3);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(
            medial_axis(&dt),
            cells(&[(1, 1), (2, 1)])
                .into_iter()
                .collect::<BTreeSet<_>>(),
        );
    }

    #[test]
    fn empty_corridor_has_zero_dt_and_empty_medial_axis() {
        let d = Corridor::new(Point::new(0, 0), 4, 4);
        let dt = DistanceTransform::compute(&d);
        for p in d.box_points() {
            assert_eq!(dt.at(p), 0);
        }
        assert!(medial_axis(&dt).is_empty());
    }

    #[test]
    fn rect_round_trips_the_box() {
        let d = Corridor::new(Point::new(2, 3), 6, 4);
        let dt = DistanceTransform::compute(&d);
        assert_eq!(
            dt.rect(),
            Rect {
                origin: Point::new(2, 3),
                size: crate::geom::Size::new(6, 4),
            }
        );
    }

    #[test]
    fn compute_and_medial_axis_are_deterministic() {
        let mut drivable = Vec::new();
        for x in 0..7 {
            if x == 3 {
                drivable.push((x, 2));
            } else {
                for y in 1..4 {
                    drivable.push((x, y));
                }
            }
        }
        let d = corridor((0, 0), 7, 5, &drivable);

        let dt1 = DistanceTransform::compute(&d);
        let dt2 = DistanceTransform::compute(&d);
        assert_eq!(dt1, dt2);

        let m1 = medial_axis(&dt1);
        let m2 = medial_axis(&dt2);
        assert_eq!(m1, m2);
    }
}
