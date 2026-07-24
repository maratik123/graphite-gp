//! Ф6's local-repair task — two new static Ф4-family defect detectors
//! (`ConcaveChordCut`, `ArmsMerging`) plus their shared width helper.
//!
//! Sited as a sibling to `phase4.rs` rather than inlined there: `phase4.rs`
//! sits at 754 lines against the 800-line incl.-tests soft cap, and every new
//! detector body + fixture + test lands here instead
//! (`ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md` § Approach,
//! § Risks R1).

use std::collections::{BTreeSet, HashSet};

use gp_core::geom::{Corridor, Orient, Point};

use crate::CoarseSkeleton;
use crate::Issue;
use crate::coarse::block_points;
use crate::phase4::{box_points, wall_runs};

/// The corridor's cross-section width at `p`, measured along `axis` — the
/// perpendicular-run length `push_outer_wall_out`'s metric compares between a
/// working and a scratch corridor.
///
/// `axis` names the *narrow chord's own orientation* (the same convention as
/// [`Issue::Narrow`](crate::Issue::Narrow) / [`Issue::NarrowSf`](crate::Issue::NarrowSf)):
/// `Vertical` reads the vertical run (`up + down − 1`), `Horizontal` reads the
/// horizontal run (`left + right − 1`) — mirroring `phase4.rs`'s own
/// `narrow_at` width derivation over the same `wall_runs` primitive.
#[allow(
    dead_code,
    reason = "no production caller until push_outer_wall_out wires it in at subtask 10 \
              (Group B) — sited here at subtask 1 per the design's module-layout decision \
              (design.md § Approach); already exercised by this module's own tests"
)]
pub(crate) fn axis_width(d: &Corridor, p: Point, axis: Orient) -> u32 {
    let (left, right, up, down) = wall_runs(d, p);
    let run = match axis {
        Orient::Vertical => up.saturating_add(down).saturating_sub(1),
        Orient::Horizontal => left.saturating_add(right).saturating_sub(1),
    };
    u32::try_from(run).unwrap_or(u32::MAX)
}

/// Whether `p` lies inside `d`'s own bounding box (in-box, independent of
/// drivability) — `Corridor` exposes no public box-membership test distinct
/// from [`Corridor::contains`] (drivable), so this is derived from the
/// public `origin`/`width`/`height` accessors, mirroring `phase4.rs`'s own
/// `box_points` re-derivation.
#[allow(
    dead_code,
    reason = "no production caller until phase4_static_checks wires the two new detectors \
              in at subtask 4 — already exercised by this module's own tests"
)]
fn in_box(d: &Corridor, p: Point) -> bool {
    let origin = d.origin();
    let w = i32::try_from(d.width()).unwrap_or(i32::MAX);
    let h = i32::try_from(d.height()).unwrap_or(i32::MAX);
    p.x >= origin.x
        && p.x < origin.x.saturating_add(w)
        && p.y >= origin.y
        && p.y < origin.y.saturating_add(h)
}

/// Whether `c` is a `ConcaveChordCut` tooth — a degree-1 non-drivable
/// protrusion (exactly one of its four 4-neighbours is not drivable,
/// out-of-box counting as not drivable) **and** that sole non-drivable
/// neighbour is itself in-box. The in-box clause excludes a border tooth
/// whose complement component is the unbounded outfield
/// (`ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md` § The
/// five arms, `fill_inner_tooth`'s local hole-preservation guard).
#[allow(
    dead_code,
    reason = "no production caller until phase4_static_checks wires the two new detectors \
              in at subtask 4 — already exercised by this module's own tests"
)]
fn is_concave_chord_cut(d: &Corridor, c: Point) -> bool {
    let non_drivable: Vec<Point> = c
        .neighbors4()
        .into_iter()
        .filter(|&n| !d.contains(n))
        .collect();
    let [only] = non_drivable.as_slice() else {
        return false;
    };
    in_box(d, *only)
}

/// Every `ConcaveChordCut` issue over `d`'s own bounding box, in ascending
/// `Point` order (design doc's emission-order pin) — `box_points`'s walk is
/// row-major (`y`-outer), which is *not* `Point`'s derived `x`-then-`y`
/// `Ord`, so the result is explicitly sorted rather than relying on
/// `box_points`'s incidental order.
#[allow(
    dead_code,
    reason = "no production caller until phase4_static_checks wires the two new detectors \
              in at subtask 4 — already exercised by this module's own tests"
)]
pub(crate) fn concave_chord_cut_issues(d: &Corridor) -> Vec<Issue> {
    let mut teeth: Vec<Point> = box_points(d)
        .filter(|&c| !d.contains(c) && is_concave_chord_cut(d, c))
        .collect();
    teeth.sort();
    teeth
        .into_iter()
        .map(|tooth| Issue::ConcaveChordCut { tooth })
        .collect()
}

/// The expanded coarse-hole mask `H = ⋃ block_points(c, k), c ∈ skel.hole`
/// (design doc §2 Ф6, `ArmsMerging`'s producing condition).
#[allow(
    dead_code,
    reason = "no production caller until phase4_static_checks wires the two new detectors \
              in at subtask 4 — already exercised by this module's own tests"
)]
fn expanded_hole_mask(skel: &CoarseSkeleton, k: i32) -> HashSet<Point> {
    skel.hole.iter().flat_map(|&c| block_points(c, k)).collect()
}

/// Every `ArmsMerging` issue over `d` — one issue per 4-connected component
/// of the drivable intrusion `H ∩ D`, anchored at that component's min
/// `Point`, emitted in ascending anchor order. Bounded by `|skel.hole| · k²`
/// (never a whole-corridor flood).
#[allow(
    dead_code,
    reason = "no production caller until phase4_static_checks wires the two new detectors \
              in at subtask 4 — already exercised by this module's own tests"
)]
pub(crate) fn arms_merging_issues(d: &Corridor, skel: &CoarseSkeleton, k: i32) -> Vec<Issue> {
    let h = expanded_hole_mask(skel, k);
    let mut remaining: BTreeSet<Point> = h.into_iter().filter(|&p| d.contains(p)).collect();
    let mut issues = Vec::new();
    while let Some(&anchor) = remaining.iter().next() {
        // BFS confined to `remaining` (the un-consumed intrusion set): the
        // smallest unconsumed point is always its own component's minimum
        // (any smaller in-component point would already be consumed by an
        // earlier iteration), so `anchor` needs no separate `min()` pass.
        let mut stack = vec![anchor];
        let mut component = HashSet::from([anchor]);
        while let Some(p) = stack.pop() {
            for n in p.neighbors4() {
                if remaining.contains(&n) && component.insert(n) {
                    stack.push(n);
                }
            }
        }
        issues.push(Issue::ArmsMerging { bridge: anchor });
        for p in &component {
            remaining.remove(p);
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::{bounded_complement_components, component_count};

    /// Build a corridor over `[origin, origin + (w, h))` with the given `(x, y)`
    /// cells marked drivable — mirrors `phase4.rs`'s test-only `corridor` helper.
    fn corridor(origin: (i32, i32), w: usize, h: usize, drivable: &[(i32, i32)]) -> Corridor {
        let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
        for &(x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    /// All `(x, y)` in the inclusive rectangle `[x0..=x1] × [y0..=y1]`.
    fn rect(x0: i32, x1: i32, y0: i32, y1: i32) -> Vec<(i32, i32)> {
        (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .collect()
    }

    #[test]
    fn axis_width_vertical_reads_the_vertical_run() {
        // A 3-row-tall corridor pinched to a single-row neck at x=3, mirroring
        // phase4.rs's narrow_sharp_single_cross_section_neck_fires_once
        // fixture: at the neck cell, the vertical run is 1.
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
        assert_eq!(axis_width(&d, Point::new(3, 2), Orient::Vertical), 1);
    }

    #[test]
    fn axis_width_horizontal_reads_the_horizontal_run() {
        let d = corridor((0, 0), 9, 9, &rect(0, 8, 0, 8));
        assert_eq!(axis_width(&d, Point::new(4, 4), Orient::Horizontal), 9);
    }

    #[test]
    fn axis_width_matches_narrow_at_derived_width_on_the_neck_fixture() {
        // Same fixture as phase4.rs's ac7_sharp_neck_yields_exactly_narrow:
        // pinch the left frame arm to a single drivable column at y=10 —
        // narrow_at derives width=1, axis=Horizontal at (2,10).
        let mut d = Corridor::filled(Point::new(0, 0), 21, 21);
        for y in 6..15 {
            for x in 6..15 {
                d.set(Point::new(x, y), false);
            }
        }
        for x in [0, 1, 3, 4, 5] {
            d.set(Point::new(x, 10), false);
        }
        assert_eq!(axis_width(&d, Point::new(2, 10), Orient::Horizontal), 1);
    }

    // ---- ConcaveChordCut --------------------------------------------------
    //
    // Shared geometry: a fully-drivable 9x9 box (0..9, 0..9) with a solid 3x3
    // non-drivable "hole" block at (4..=6, 4..=6) — the minimum size with no
    // degree-1 (leaf) hole cell of its own (every hole cell has >=2
    // non-drivable neighbours), so the plain hole alone yields zero teeth.

    fn hole_frame_d() -> Corridor {
        let mut d = Corridor::filled(Point::new(0, 0), 9, 9);
        for &(x, y) in &rect(4, 6, 4, 6) {
            d.set(Point::new(x, y), false);
        }
        d
    }

    #[test]
    fn plain_solid_hole_yields_no_concave_chord_cut() {
        assert!(concave_chord_cut_issues(&hole_frame_d()).is_empty());
    }

    #[test]
    fn concave_single_cell_poke_fires_exactly_one_tooth() {
        // Poke one frame cell (3,5) — left of the hole's left edge — as an
        // extra non-drivable cell: its sole non-drivable neighbour is the
        // hole cell (4,5), so it is a degree-1 tooth.
        let mut d = hole_frame_d();
        d.set(Point::new(3, 5), false);
        assert_eq!(
            concave_chord_cut_issues(&d),
            vec![Issue::ConcaveChordCut {
                tooth: Point::new(3, 5)
            }],
        );
    }

    #[test]
    fn concave_fill_makes_the_tooth_drivable_and_preserves_the_one_hole() {
        let mut d = hole_frame_d();
        d.set(Point::new(3, 5), false);
        d.set(Point::new(3, 5), true);
        assert!(d.contains(Point::new(3, 5)));
        assert_eq!(bounded_complement_components(&d), 1);
    }

    #[test]
    fn concave_degree_two_notch_does_not_fire() {
        // An L of two poked cells: each becomes the other's second
        // non-drivable neighbour (besides the hole), so neither is degree-1.
        let mut d = hole_frame_d();
        d.set(Point::new(3, 5), false);
        d.set(Point::new(3, 4), false);
        assert!(concave_chord_cut_issues(&d).is_empty());
    }

    #[test]
    fn concave_one_cell_hole_does_not_fire() {
        // AC6 near-miss: a lone non-drivable cell surrounded on all 4 sides
        // by drivable cells has zero non-drivable neighbours (degree 0), not
        // exactly one.
        let mut d = Corridor::filled(Point::new(0, 0), 5, 5);
        d.set(Point::new(2, 2), false);
        assert!(concave_chord_cut_issues(&d).is_empty());
    }

    #[test]
    fn concave_border_tooth_does_not_fire() {
        // AC6 near-miss: a non-drivable cell on the box edge whose sole
        // non-drivable neighbour is out-of-box is excluded by the in-box
        // clause (it belongs to the unbounded outfield, not a bounded hole).
        let mut d = Corridor::filled(Point::new(0, 0), 5, 5);
        d.set(Point::new(0, 2), false);
        assert!(concave_chord_cut_issues(&d).is_empty());
    }

    // ---- ArmsMerging -------------------------------------------------------
    //
    // Shared geometry: a fully-drivable 9x9 box with the coarse hole
    // `{(1,1)}` at `k=3`, so `H` is the fine 3x3 block (3..=5, 3..=5) —
    // comfortably inside the box, away from every border.

    fn arms_skel() -> CoarseSkeleton {
        CoarseSkeleton {
            ring: BTreeSet::new(),
            hole: BTreeSet::from([Point::new(1, 1)]),
            dir: gp_core::track::RaceDir::Cw,
        }
    }

    fn arms_frame_d() -> Corridor {
        Corridor::filled(Point::new(0, 0), 9, 9)
    }

    /// `arms_frame_d` with `H` (fine `3..=5, 3..=5`) entirely non-drivable —
    /// a clean, un-intruded infield.
    fn clean_infield_d() -> Corridor {
        let mut d = arms_frame_d();
        for &(x, y) in &rect(3, 5, 3, 5) {
            d.set(Point::new(x, y), false);
        }
        d
    }

    #[test]
    fn arms_clean_infield_yields_no_issue() {
        assert!(arms_merging_issues(&clean_infield_d(), &arms_skel(), 3).is_empty());
    }

    #[test]
    fn arms_one_drivable_intrusion_fires_exactly_one_bridge() {
        let mut d = clean_infield_d();
        d.set(Point::new(4, 4), true);
        assert_eq!(
            arms_merging_issues(&d, &arms_skel(), 3),
            vec![Issue::ArmsMerging {
                bridge: Point::new(4, 4)
            }],
        );
    }

    #[test]
    fn arms_remove_edit_clears_it_and_flood_fill_confirms_one_bounded_hole() {
        let mut d = clean_infield_d();
        d.set(Point::new(4, 4), true);
        d.set(Point::new(4, 4), false);
        assert!(arms_merging_issues(&d, &arms_skel(), 3).is_empty());
        assert_eq!(component_count(&d), 1);
        assert_eq!(bounded_complement_components(&d), 1);
    }
}
