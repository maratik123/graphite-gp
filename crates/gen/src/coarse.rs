//! Coarse→fine block mapping (design doc §2 Ф2 `k×k` expansion), shared by
//! Ф2's rasterizer, Ф4's finger-liveness check, and Ф6's local-repair task
//! (`phase4_defects.rs`, `phase6_arms.rs`).
//!
//! Lifted from private duplicates in `phase2.rs` and `phase4.rs`
//! (`crates/gen/src/phase2.rs:48,53`, `crates/gen/src/phase4.rs:241,247`) once
//! a third and fourth consumer arrived — the ≥3-site shared-crate/module rule
//! (`ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md` § Approach).

use gp_core::geom::Point;

/// The fine-point origin of coarse block `c`'s `k×k` patch — `(c.x·k, c.y·k)`.
pub(crate) const fn block_origin(c: Point, k: i32) -> Point {
    Point::new(c.x.saturating_mul(k), c.y.saturating_mul(k))
}

/// Every fine point of coarse block `c`'s `k×k` patch, row-major.
pub(crate) fn block_points(c: Point, k: i32) -> impl Iterator<Item = Point> {
    let origin = block_origin(c, k);
    (0..k).flat_map(move |dy| {
        (0..k).map(move |dx| Point::new(origin.x.saturating_add(dx), origin.y.saturating_add(dy)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_origin_scales_by_k() {
        assert_eq!(block_origin(Point::new(2, 3), 5), Point::new(10, 15));
    }

    #[test]
    fn block_points_matches_expected_kxk_patch() {
        let pts: Vec<Point> = block_points(Point::new(2, 0), 3).collect();
        let expected: Vec<Point> = (0..3)
            .flat_map(|dy| (0..3).map(move |dx| Point::new(6 + dx, dy)))
            .collect();
        assert_eq!(pts, expected);
    }
}
