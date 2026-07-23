//! Ф4 — static validation (design doc §2): connectivity, single-hole topology,
//! cross-section width, and finger liveness.
//!
//! Consumes the fine corridor `D`, the width floors, and the S/F chord; runs
//! four checks in a fixed order and returns a `Vec<Issue>` (empty ⟺ statically
//! valid). Two checks reuse the merged `gp_core::geom` helpers verbatim; the
//! other two are built on the [`DistanceTransform`](gp_core::geom::DistanceTransform)
//! / [`medial_axis`](gp_core::geom::medial_axis) primitives.

use gp_core::geom::{Orient, Point};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
