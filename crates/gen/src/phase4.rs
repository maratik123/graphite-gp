//! Ф4 — static validation (design doc §2): connectivity, single-hole topology,
//! cross-section width, and finger liveness.
//!
//! Consumes the fine corridor `D`, the width floors, and the S/F chord; runs
//! four checks in a fixed order and returns a `Vec<Issue>` (empty ⟺ statically
//! valid). Two checks reuse the merged `gp_core::geom` helpers verbatim; the
//! other two are built on the [`DistanceTransform`](gp_core::geom::DistanceTransform)
//! / [`medial_axis`](gp_core::geom::medial_axis) primitives.

use gp_core::geom::{Corridor, Orient, Point, bounded_complement_components, component_count};

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
}

/// Connectivity check (AC1): `Some(Issue::Disconnected)` iff `d` does not have
/// exactly one 4-connected component. Delegates verbatim to
/// [`component_count`].
#[allow(
    dead_code,
    reason = "wired into phase4_static_checks by subtask 6 of this same file \
              (design doc decomposition Task 3 → Task 6); dead outside tests \
              until then"
)]
fn check_connectivity(d: &Corridor) -> Option<Issue> {
    (component_count(d) != 1).then_some(Issue::Disconnected)
}

/// Topology check (AC2): `Some(Issue::BadTopology)` iff `d`'s complement does
/// not have exactly one bounded hole. Delegates verbatim to
/// [`bounded_complement_components`], which already counts only bounded
/// (non-border-touching), non-empty complement components.
#[allow(
    dead_code,
    reason = "wired into phase4_static_checks by subtask 6 of this same file \
              (design doc decomposition Task 3 → Task 6); dead outside tests \
              until then"
)]
fn check_topology(d: &Corridor) -> Option<Issue> {
    (bounded_complement_components(d) != 1).then_some(Issue::BadTopology)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::Coord;
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
}
