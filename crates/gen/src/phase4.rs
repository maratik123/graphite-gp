//! Ф4 — static validation (design doc §2): connectivity, single-hole topology,
//! cross-section width, and finger liveness.
//!
//! Consumes the fine corridor `D`, the width floors, and the S/F chord; runs
//! four checks in a fixed order and returns a `Vec<Issue>` (empty ⟺ statically
//! valid). Two checks reuse the merged `gp_core::geom` helpers verbatim; the
//! other two are built on the [`DistanceTransform`](DistanceTransform)
//! / [`medial_axis`](gp_core::geom::medial_axis) primitives.

use std::collections::{BTreeMap, BTreeSet};

use gp_core::geom::{
    Coord, Corridor, DistanceTransform, Orient, Point, bounded_complement_components,
    component_count,
};
use gp_core::track::StartFinish;

use crate::CoarseSkeleton;
use crate::coarse::block_points;

/// One statically-detected defect of the fine corridor `D` (design doc §2, Ф4).
///
/// Payloads carry the minimum locality the future Ф6 repair phase needs to
/// re-derive the wall/edge it must move (design doc §2 Ф6: `NARROW →
/// push_outer_wall_out`, `LOST_HAIRPIN → trim_arm_wall / nudge_finger`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Issue {
    /// `D` has more than one 4-connected component (AC1).
    Disconnected,
    /// `D`'s complement does not have exactly one bounded hole (AC2).
    BadTopology,
    /// A perpendicular cross-section of `D` is narrower than the global width
    /// floor `n`.
    Narrow {
        /// The narrow chord's canonical cell — the shorter run's min-`Point`
        /// (its bottom/left cap cell).
        center: Point,
        /// The narrow chord's own orientation (across the corridor); Ф6 pushes
        /// the two capping outer walls apart along this axis.
        axis: Orient,
        /// The measured cross-section width.
        width: u32,
    },
    /// The start/finish chord is narrower than the S/F width floor `m`.
    NarrowSf {
        /// The S/F chord's canonical cell — its min-`Point` (bottom/left cap
        /// cell), the same convention as [`Narrow::center`](Issue::Narrow).
        center: Point,
        /// The S/F chord's own orientation (`sf.orient`).
        axis: Orient,
        /// The measured chord width (`sf.chord.len()`).
        width: u32,
    },
    /// A coarse infield finger has been absorbed — its separating strip is
    /// entirely filled, merging its two flanking arms (design doc §1).
    LostHairpin {
        /// The finger's coarse tip — the anchor Ф6's `nudge_finger` acts near.
        tip: Point,
    },
    /// A degree-1 non-drivable protrusion into the corridor cuts a concave
    /// corner the strict supercover predicate refuses to graze past (design
    /// doc §2 Ф6: `CONCAVE_CHORD_CUT → fill_inner_tooth`). Detected in
    /// `phase4_defects.rs`.
    ConcaveChordCut {
        /// The protruding cell Ф6's `fill_inner_tooth` makes drivable.
        tooth: Point,
    },
    /// A drivable fine cell has intruded into the expanded coarse-hole mask
    /// `H`, bridging the corridor across the infield and threatening to merge
    /// its two arms (design doc §2 Ф6: `ARMS_MERGING → trim_arm_wall`).
    /// Detected in `phase4_defects.rs`.
    ArmsMerging {
        /// The drivable intrusion cell Ф6's `trim_arm_wall` makes
        /// non-drivable — the min-`Point` anchor of its 4-connected
        /// intrusion component.
        bridge: Point,
    },
    /// A corner-entry path point has insufficient run-out room to brake from
    /// its attainable entry speed to a speed with a legal successor at the
    /// corner (design doc §2 Ф6: `NO_BRAKING → lengthen_straight /
    /// widen_corner`). A `v_target`-referenced run-out budget check, never a
    /// lap-existence check (#30's AC7 result). Detected in
    /// `phase5_runout.rs`.
    NoBraking {
        /// The maximal deficient run's first `fastest_lap` point.
        at: Point,
    },
}

/// Connectivity check (AC1): `Some(Issue::Disconnected)` iff `d` does not have
/// exactly one 4-connected component. Delegates verbatim to
/// [`component_count`].
fn check_connectivity(d: &Corridor) -> Option<Issue> {
    (component_count(d) != 1).then_some(Issue::Disconnected)
}

/// Topology check (AC2): `Some(Issue::BadTopology)` iff `d`'s complement does
/// not have exactly one bounded hole. Delegates verbatim to
/// [`bounded_complement_components`], which already counts only bounded
/// (non-border-touching), non-empty complement components.
fn check_topology(d: &Corridor) -> Option<Issue> {
    (bounded_complement_components(d) != 1).then_some(Issue::BadTopology)
}

/// Every cell point of `d`'s own bounding box, in row-major order — mirrors
/// Ф1/Ф2/Ф3's private `box_points` (`Corridor`'s own box-point iterator is
/// private; this is the same accepted re-derivation from the public
/// `origin`/`width`/`height` accessors, e.g. `crates/gen/src/phase3.rs`).
pub(crate) fn box_points(d: &Corridor) -> impl Iterator<Item = Point> + '_ {
    let origin = d.origin();
    let (w, h) = (d.width(), d.height());
    (0..h).flat_map(move |dy| {
        (0..w).map(move |dx| {
            Point::new(
                origin.x.saturating_add(i32::try_from(dx).unwrap_or(0)),
                origin.y.saturating_add(i32::try_from(dy).unwrap_or(0)),
            )
        })
    })
}

/// The count of consecutive `D` cells starting at (and including) `p`,
/// extending one step at a time along `(dx, dy)` until the first `¬D` /
/// out-of-box cell. Always `≥ 1` when `p ∈ D` (the loop's first iteration).
pub(crate) fn wall_run(d: &Corridor, p: Point, delta: (Coord, Coord)) -> usize {
    let (dx, dy) = delta;
    let mut count = 0usize;
    let mut cur = p;
    while d.contains(cur) {
        count = count.saturating_add(1);
        cur = Point::new(cur.x.saturating_add(dx), cur.y.saturating_add(dy));
    }
    count
}

/// The four in-`D` wall-distance walks from `p`: `(left, right, up, down)`,
/// each the step count (`p` included) to the first `¬D` / box-edge cell.
pub(crate) fn wall_runs(d: &Corridor, p: Point) -> (usize, usize, usize, usize) {
    (
        wall_run(d, p, (-1, 0)),
        wall_run(d, p, (1, 0)),
        wall_run(d, p, (0, 1)),
        wall_run(d, p, (0, -1)),
    )
}

/// The `Narrow` issue at `p`, or `None` — the DT pre-filter + exact
/// perpendicular cross-section confirmation (design doc §2 Ф4 Width).
///
/// DT pre-filter: skips `p` when `2·dt(p) − 1 ≥ n` — provably wide
/// (`w(p) ≥ 2·dt(p) − 1 ≥ n`, the DT pre-filter soundness argument). Otherwise
/// walks the four wall-distances to get `hrun`/`vrun`, and emits iff
/// `w(p) = min(hrun, vrun) < n` **and** `w(p) ∈ {2·dt(p) − 1, 2·dt(p)}` — the
/// DT-consistency test that rejects a staircase/taper-edge false positive
/// (design doc Risks) while still catching a doorway-neck DT valley.
fn narrow_at(d: &Corridor, dt: &DistanceTransform, p: Point, n: u32) -> Option<Issue> {
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    let dt_p = usize::try_from(dt.at(p)).unwrap_or(usize::MAX);
    let two_dt_minus_1 = dt_p.saturating_mul(2).saturating_sub(1);
    if two_dt_minus_1 >= n {
        return None;
    }

    let (left, right, up, down) = wall_runs(d, p);
    let hrun = left.saturating_add(right).saturating_sub(1);
    let vrun = up.saturating_add(down).saturating_sub(1);
    let w = hrun.min(vrun);
    let two_dt = dt_p.saturating_mul(2);
    if w >= n || (w != two_dt_minus_1 && w != two_dt) {
        return None;
    }

    let width = u32::try_from(w).unwrap_or(u32::MAX);
    if vrun <= hrun {
        let down_i32 = i32::try_from(down).unwrap_or(i32::MAX);
        let center = Point::new(p.x, p.y.saturating_sub(down_i32).saturating_add(1));
        Some(Issue::Narrow {
            center,
            axis: Orient::Vertical,
            width,
        })
    } else {
        let left_i32 = i32::try_from(left).unwrap_or(i32::MAX);
        let center = Point::new(p.x.saturating_sub(left_i32).saturating_add(1), p.y);
        Some(Issue::Narrow {
            center,
            axis: Orient::Horizontal,
            width,
        })
    }
}

/// The `Narrow` issues over **all** `D` cells (AC3) — deliberately not
/// restricted to `medial_axis`'s ridge, since a neck is a DT valley a
/// local-maximum ridge would miss (design doc Risks, Issue #1).
fn narrow_issues(d: &Corridor, dt: &DistanceTransform, n: u32) -> Vec<Issue> {
    box_points(d)
        .filter(|&p| d.contains(p))
        .filter_map(|p| narrow_at(d, dt, p, n))
        .collect()
}

/// The `NarrowSf` issue on `sf`'s chord, or `None` — no DT sampling needed,
/// the chord's width is `sf.chord.len()` directly (design doc §2 Ф4 Width).
fn check_narrow_sf(sf: &StartFinish, m: u32) -> Option<Issue> {
    let len = sf.chord.len();
    if len >= usize::try_from(m).unwrap_or(usize::MAX) {
        return None;
    }
    let center = sf.chord.iter().copied().min()?;
    Some(Issue::NarrowSf {
        center,
        axis: sf.orient,
        width: u32::try_from(len).unwrap_or(u32::MAX),
    })
}

/// The 4-connected degree of coarse hole cell `c` within `hole` — its count
/// of 4-neighbors also in `hole`.
fn hole_degree(hole: &BTreeSet<Point>, c: Point) -> usize {
    c.neighbors4()
        .into_iter()
        .filter(|n| hole.contains(n))
        .count()
}

/// The finger chain starting at coarse tip `tip` — the tip cell plus every
/// subsequent degree-`≤2` hole cell along the walk, stopping *before* the
/// first degree-`≥3` branch cell (design doc §2 Ф4 Finger liveness).
pub(crate) fn walk_finger(hole: &BTreeSet<Point>, tip: Point) -> Vec<Point> {
    let mut chain = vec![tip];
    let mut prev = None;
    let mut current = tip;
    loop {
        let next_candidates: Vec<Point> = current
            .neighbors4()
            .into_iter()
            .filter(|&n| hole.contains(&n) && Some(n) != prev)
            .collect();
        let [next] = next_candidates.as_slice() else {
            break;
        };
        if hole_degree(hole, *next) >= 3 {
            break;
        }
        chain.push(*next);
        prev = Some(current);
        current = *next;
    }
    chain
}

/// The infield peninsulas of the coarse hole `P` (`skel.hole`), keyed by
/// their tip `Point` (design doc §2 Ф4 Finger liveness).
///
/// A hole cell with exactly one 4-connected neighbor in `P` is a finger tip;
/// its finger is the chain of degree-`≤2` hole cells from that tip up to (but
/// excluding) the first degree-`≥3` branch cell.
pub(crate) fn infield_fingers(skel: &CoarseSkeleton) -> BTreeMap<Point, Vec<Point>> {
    skel.hole
        .iter()
        .copied()
        .filter(|&c| hole_degree(&skel.hole, c) == 1)
        .map(|tip| (tip, walk_finger(&skel.hole, tip)))
        .collect()
}

/// Whether `finger`'s fine footprint (the `×k` block expansion of each coarse
/// cell) is **entirely** drivable in `d` — the separating infield strip is
/// fully filled, so the finger's two flanking arms have merged (design doc §1
/// line 24).
pub(crate) fn absorbed(finger: &[Point], d: &Corridor, k: i32) -> bool {
    finger
        .iter()
        .all(|&c| block_points(c, k).all(|p| d.contains(p)))
}

/// Runs Ф4's four static-validation checks over `d`, in fixed order.
///
/// Connectivity → topology → width (`Narrow`/`NarrowSf`) → finger liveness;
/// returns every issue found (empty ⟺ statically valid, design doc §2 Ф4).
///
/// - `skel` — the coarse skeleton (`skel.hole` drives finger extraction).
/// - `k` — the coarse-block size, mapping coarse fingers to fine blocks
///   (Ф2's `k`).
/// - `n` — the global width floor (`GenParams::min_width`).
/// - `m` — the S/F width floor (`GenParams::start_finish_width`).
/// - `sf` — the start/finish chord.
///
/// Total and deterministic: no `Result`, no production panic (design doc
/// Risks) — mirrors Ф1/Ф2/Ф3.
pub fn phase4_static_checks(
    d: &Corridor,
    skel: &CoarseSkeleton,
    k: i32,
    n: u32,
    m: u32,
    sf: &StartFinish,
) -> Vec<Issue> {
    let mut issues = Vec::new();
    issues.extend(check_connectivity(d));
    issues.extend(check_topology(d));

    let dt = DistanceTransform::compute(d);
    issues.extend(narrow_issues(d, &dt, n));
    issues.extend(check_narrow_sf(sf, m));

    for (&tip, finger) in &infield_fingers(skel) {
        if absorbed(finger, d, k) {
            issues.push(Issue::LostHairpin { tip });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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

    /// All `(x, y)` in the inclusive rectangle `[x0..=x1] × [y0..=y1]`.
    fn rect(x0: Coord, x1: Coord, y0: Coord, y1: Coord) -> Vec<(Coord, Coord)> {
        (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .collect()
    }

    #[test]
    fn connectivity_single_component_has_no_disconnected_issue() {
        // AC1: one 4-connected component → no Disconnected.
        let d = corridor((0, 0), 5, 5, &rect(1, 3, 1, 3));
        assert_eq!(check_connectivity(&d), None);
    }

    #[test]
    fn connectivity_two_components_trips_disconnected() {
        // AC1: two disjoint blocks → Disconnected.
        let d = corridor((0, 0), 5, 5, &[(0, 0), (1, 0), (3, 3), (4, 3)]);
        assert_eq!(check_connectivity(&d), Some(Issue::Disconnected));
    }

    #[test]
    fn topology_valid_annulus_has_no_bad_topology_issue() {
        // AC2: exactly one bounded hole (the annulus's enclosed center) → OK.
        let ring: Vec<_> = rect(1, 3, 1, 3)
            .into_iter()
            .filter(|&p| p != (2, 2))
            .collect();
        let d = corridor((0, 0), 5, 5, &ring);
        assert_eq!(check_topology(&d), None);
    }

    #[test]
    fn topology_disk_has_bad_topology_issue() {
        // AC2: annulus→disk merge (the center cell filled back in) → zero
        // bounded holes → BadTopology.
        let d = corridor((0, 0), 5, 5, &rect(1, 3, 1, 3));
        assert_eq!(check_topology(&d), Some(Issue::BadTopology));
    }

    #[test]
    fn topology_two_holes_trips_bad_topology() {
        // AC2: two disjoint enclosed holes inside one D → BadTopology.
        let mut drivable = rect(0, 6, 0, 6);
        // Punch two separate 1-cell holes.
        drivable.retain(|&p| p != (2, 2) && p != (4, 4));
        let d = corridor((0, 0), 7, 7, &drivable);
        assert_eq!(check_topology(&d), Some(Issue::BadTopology));
    }

    #[test]
    fn issue_variants_are_eq_hash_and_order_independent_in_a_set() {
        // Ф6-facing contract: Issue is Eq + Hash so tests (and the future
        // repair phase) can compare an order-independent HashSet<Issue>.
        let a = Issue::Narrow {
            center: Point::new(1, 1),
            axis: Orient::Horizontal,
            width: 2,
        };
        let b = Issue::LostHairpin {
            tip: Point::new(3, 3),
        };
        let set: HashSet<Issue> = [a, Issue::Disconnected, Issue::BadTopology, b]
            .into_iter()
            .collect();
        assert_eq!(set.len(), 4);
        assert!(set.contains(&a));
        assert!(set.contains(&Issue::Disconnected));
    }

    /// Collect `narrow_issues` into a `HashSet` (AC7 compares this way; a
    /// narrow chord's ≤2 centered cells collapse to one entry).
    fn narrow_set(d: &Corridor, n: u32) -> HashSet<Issue> {
        let dt = DistanceTransform::compute(d);
        narrow_issues(d, &dt, n).into_iter().collect()
    }

    /// A minimal `StartFinish` fixture: `chord` at the given `(x, y)`
    /// points, oriented `orient`, with a placeholder single-cell gate (the
    /// gate is not read by the width check).
    fn sf_fixture(chord: &[(Coord, Coord)], orient: Orient) -> StartFinish {
        let chord: Vec<Point> = chord.iter().map(|&(x, y)| Point::new(x, y)).collect();
        StartFinish {
            gate: gp_core::track::TimingGate {
                behind: chord.clone(),
                forward: gp_core::geom::Side::East,
            },
            chord,
            orient,
        }
    }

    #[test]
    fn narrow_clean_ring_has_no_issues() {
        // A thickness-2 frame: every cross-section is exactly 2 cells wide, so
        // n=2 (strict `<`) never fires.
        let mut d = Corridor::filled(Point::new(0, 0), 9, 9);
        for y in 2..7 {
            for x in 2..7 {
                d.set(Point::new(x, y), false);
            }
        }
        assert!(narrow_set(&d, 2).is_empty());
    }

    #[test]
    fn narrow_sharp_single_cross_section_neck_fires_once() {
        // A 3-row-tall corridor pinched to a single-row neck at x=3 (n=3):
        // exactly one Narrow, centered on the neck cell.
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
        let issues = narrow_set(&d, 3);
        assert_eq!(
            issues,
            HashSet::from([Issue::Narrow {
                center: Point::new(3, 2),
                axis: Orient::Vertical,
                width: 1,
            }]),
        );
    }

    #[test]
    fn narrow_doorway_neck_is_caught_by_completeness() {
        // Two 5x5 rooms joined by a sub-n width-3 doorway corridor (n=4): the
        // neck's DT is a valley (its along-flow neighbors in the wider rooms
        // have strictly greater DT), so a ridge-restricted sampler would miss
        // it — the all-cells scan must still catch it (design doc Risks,
        // Issue #1 completeness argument).
        let mut drivable = Vec::new();
        drivable.extend((0..5).flat_map(|x| (0..5).map(move |y| (x, y))));
        drivable.extend((10..15).flat_map(|x| (0..5).map(move |y| (x, y))));
        drivable.extend((5..10).flat_map(|x| (1..4).map(move |y| (x, y))));
        let d = corridor((0, 0), 15, 5, &drivable);
        let issues = narrow_set(&d, 4);
        assert!(
            issues.iter().any(|issue| matches!(
                issue,
                Issue::Narrow {
                    width: 3,
                    axis: Orient::Vertical,
                    ..
                }
            )),
            "expected a width-3 Narrow in the doorway, got {issues:?}"
        );
    }

    #[test]
    fn narrow_staircase_taper_edge_is_not_a_false_positive() {
        // A wide (height-6) corridor, n=4, with a diagonal staircase taper
        // carved from the top-left corner over 3 rows: row y=0 loses x<3,
        // y=1 loses x<2, y=2 loses x<1. This genuinely exercises the
        // DT-consistency clause — unlike a shallow rectangular notch, whose
        // per-column height never dips below n so the clause is never
        // reached (the original, vacuous version of this fixture).
        //
        // At the cells directly below the staircase's toe (x=0, y∈{3,4,5}):
        // hrun=14 (the full untouched row length) but vrun=3 (up is capped
        // by the staircase notch at y<3, x=0; down by the box edge at
        // y=5) — so w = min(hrun, vrun) = 3 < n = 4, and WITHOUT the
        // DT-consistency clause this cross-section would be flagged
        // `Narrow`. But at these cells dt = 1 (each is only 1 step from the
        // staircase's nearest carved-out cell / box edge — a genuinely
        // *centered* width-3 run would have dt = 2), so
        // `w = 3 ∉ {2·dt−1, 2·dt} = {1, 2}` — the canonical design-doc Risks
        // case (§ Risks, "w=3, dt=1") — and the clause correctly rejects
        // it: the corridor's true perpendicular width (height 6) stays ≥ n
        // throughout.
        let mut drivable: Vec<(Coord, Coord)> =
            (0..14).flat_map(|x| (0..6).map(move |y| (x, y))).collect();
        drivable.retain(|&(x, y)| !((y == 0 && x < 3) || (y == 1 && x < 2) || (y == 2 && x < 1)));
        let d = corridor((0, 0), 14, 6, &drivable);
        assert!(
            narrow_set(&d, 4).is_empty(),
            "the staircase taper edge must not report a false Narrow"
        );
    }

    #[test]
    fn narrow_sf_below_floor_emits_issue_keyed_on_min_point() {
        let sf = sf_fixture(&[(3, 5), (4, 5), (5, 5)], Orient::Horizontal);
        assert_eq!(
            check_narrow_sf(&sf, 4),
            Some(Issue::NarrowSf {
                center: Point::new(3, 5),
                axis: Orient::Horizontal,
                width: 3,
            }),
        );
    }

    #[test]
    fn narrow_sf_at_or_above_floor_emits_none() {
        let sf = sf_fixture(&[(3, 5), (4, 5), (5, 5)], Orient::Horizontal);
        assert_eq!(check_narrow_sf(&sf, 3), None);
        assert_eq!(check_narrow_sf(&sf, 2), None);
    }

    /// A coarse hole `P`: a 2x2 blob `{(0,0),(1,0),(0,1),(1,1)}` (all
    /// degree-2, no branch) with a one-cell spur `(2,0)` attached to
    /// `(1,0)` — `(1,0)` becomes the sole degree-3 branch cell and `(2,0)`
    /// the sole finger tip.
    fn skeleton_with_one_cell_peninsula() -> CoarseSkeleton {
        CoarseSkeleton {
            ring: BTreeSet::new(),
            hole: BTreeSet::from([
                Point::new(0, 0),
                Point::new(1, 0),
                Point::new(0, 1),
                Point::new(1, 1),
                Point::new(2, 0),
            ]),
            dir: gp_core::track::RaceDir::Cw,
        }
    }

    #[test]
    fn infield_fingers_finds_the_one_cell_peninsula_keyed_by_its_tip() {
        let skel = skeleton_with_one_cell_peninsula();
        let fingers = infield_fingers(&skel);
        assert_eq!(fingers.len(), 1);
        assert_eq!(
            fingers.get(&Point::new(2, 0)),
            Some(&vec![Point::new(2, 0)])
        );
    }

    #[test]
    fn finger_alive_when_footprint_not_drivable() {
        let skel = skeleton_with_one_cell_peninsula();
        let finger = &infield_fingers(&skel)[&Point::new(2, 0)];
        // The k=3 fine footprint of coarse cell (2,0) is x:6..9, y:0..3 — an
        // empty D leaves it entirely ¬D, so the finger survives.
        let d = Corridor::new(Point::new(0, 0), 12, 6);
        assert!(!absorbed(finger, &d, 3));
    }

    #[test]
    fn finger_absorbed_when_footprint_fully_drivable() {
        let skel = skeleton_with_one_cell_peninsula();
        let finger = &infield_fingers(&skel)[&Point::new(2, 0)];
        let mut d = Corridor::new(Point::new(0, 0), 12, 6);
        for p in block_points(Point::new(2, 0), 3) {
            d.set(p, true);
        }
        assert!(absorbed(finger, &d, 3));
    }

    // ---- Orchestrator (AC6/AC7/AC8) --------------------------------------
    //
    // Shared fixture geometry: k=3, n=3, m=4. A 21x21 outer box with a 9x9
    // centered fine hole (x:6..15, y:6..15) — a thickness-6 frame, well above
    // n=3. The coarse hole is the matching 3x3 coarse-block region
    // {2,3,4}x{2,3,4} (block size k=3), with an optional one-cell peninsula
    // block (5,3) attached to (4,3) — its fine footprint x:15..18, y:9..12
    // sits inside the frame's thickness (with a 3-cell buffer to the box
    // border), so carving/filling it never breaches the frame or merges the
    // hole with the outfield.

    /// The plain thickness-6 frame ring, no finger notch.
    fn base_ring_d() -> Corridor {
        let mut d = Corridor::filled(Point::new(0, 0), 21, 21);
        for y in 6..15 {
            for x in 6..15 {
                d.set(Point::new(x, y), false);
            }
        }
        d
    }

    /// `base_ring_d` with the peninsula block (5,3)'s fine footprint carved
    /// out (not drivable) — the finger's separating strip is intact.
    fn notch_ring_d() -> Corridor {
        let mut d = base_ring_d();
        for x in 15..18 {
            for y in 9..12 {
                d.set(Point::new(x, y), false);
            }
        }
        d
    }

    /// The matching coarse hole, no peninsula.
    fn plain_hole_skel() -> CoarseSkeleton {
        let hole: BTreeSet<Point> = (2..5)
            .flat_map(|x| (2..5).map(move |y| Point::new(x, y)))
            .collect();
        CoarseSkeleton {
            ring: BTreeSet::new(),
            hole,
            dir: gp_core::track::RaceDir::Cw,
        }
    }

    /// `plain_hole_skel` plus the one-cell peninsula block `(5,3)`.
    fn hole_with_finger_skel() -> CoarseSkeleton {
        let mut skel = plain_hole_skel();
        skel.hole.insert(Point::new(5, 3));
        skel
    }

    /// A clean S/F chord, width 4 (`≥ m`).
    fn clean_sf() -> StartFinish {
        sf_fixture(&[(0, 0), (0, 1), (0, 2), (0, 3)], Orient::Vertical)
    }

    /// An S/F chord of width 3 — `∈ [n, m)`, so only `NarrowSf` fires.
    fn narrow_sf_chord() -> StartFinish {
        sf_fixture(&[(0, 0), (0, 1), (0, 2)], Orient::Vertical)
    }

    #[test]
    fn ac6_clean_ring_with_intact_finger_is_empty() {
        let d = notch_ring_d();
        let skel = hole_with_finger_skel();
        let sf = clean_sf();
        let issues = phase4_static_checks(&d, &skel, 3, 3, 4, &sf);
        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    #[test]
    fn ac7_sharp_neck_yields_exactly_narrow() {
        // Pinch the left frame arm to a single drivable column at y=10.
        let mut d = base_ring_d();
        for x in [0, 1, 3, 4, 5] {
            d.set(Point::new(x, 10), false);
        }
        let skel = plain_hole_skel();
        let sf = clean_sf();
        let issues: HashSet<Issue> = phase4_static_checks(&d, &skel, 3, 3, 4, &sf)
            .into_iter()
            .collect();
        assert_eq!(
            issues,
            HashSet::from([Issue::Narrow {
                center: Point::new(2, 10),
                axis: Orient::Horizontal,
                width: 1,
            }]),
        );
    }

    #[test]
    fn ac7_disk_merge_yields_exactly_bad_topology() {
        let d = Corridor::filled(Point::new(0, 0), 21, 21);
        let skel = plain_hole_skel();
        let sf = clean_sf();
        let issues: HashSet<Issue> = phase4_static_checks(&d, &skel, 3, 3, 4, &sf)
            .into_iter()
            .collect();
        assert_eq!(issues, HashSet::from([Issue::BadTopology]));
    }

    #[test]
    fn ac7_narrow_sf_yields_exactly_narrow_sf() {
        let d = base_ring_d();
        let skel = plain_hole_skel();
        let sf = narrow_sf_chord();
        let issues: HashSet<Issue> = phase4_static_checks(&d, &skel, 3, 3, 4, &sf)
            .into_iter()
            .collect();
        assert_eq!(
            issues,
            HashSet::from([Issue::NarrowSf {
                center: Point::new(0, 0),
                axis: Orient::Vertical,
                width: 3,
            }]),
        );
    }

    #[test]
    fn ac7_filled_finger_yields_exactly_lost_hairpin() {
        let d = base_ring_d();
        let skel = hole_with_finger_skel();
        let sf = clean_sf();
        let issues: HashSet<Issue> = phase4_static_checks(&d, &skel, 3, 3, 4, &sf)
            .into_iter()
            .collect();
        assert_eq!(
            issues,
            HashSet::from([Issue::LostHairpin {
                tip: Point::new(5, 3),
            }]),
        );
    }

    #[test]
    fn ac8_repeated_calls_are_set_identical() {
        let d = notch_ring_d();
        let skel = hole_with_finger_skel();
        let sf = clean_sf();
        let a: HashSet<Issue> = phase4_static_checks(&d, &skel, 3, 3, 4, &sf)
            .into_iter()
            .collect();
        let b: HashSet<Issue> = phase4_static_checks(&d, &skel, 3, 3, 4, &sf)
            .into_iter()
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn ac8_degenerate_inputs_are_total_no_panic() {
        let skel = plain_hole_skel();
        let sf = clean_sf();

        // An empty D: no drivable cells at all.
        let empty = Corridor::new(Point::new(0, 0), 4, 4);
        let issues: HashSet<Issue> = phase4_static_checks(&empty, &skel, 3, 3, 4, &sf)
            .into_iter()
            .collect();
        assert!(
            issues.contains(&Issue::Disconnected),
            "an empty D has zero components, not one"
        );

        // A degenerate 1x1 corridor.
        let mut tiny = Corridor::new(Point::new(0, 0), 1, 1);
        tiny.set(Point::new(0, 0), true);
        let _ = phase4_static_checks(&tiny, &skel, 3, 3, 4, &sf);
    }
}
