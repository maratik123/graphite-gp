//! Ф6's local-repair task — two new static Ф4-family defect detectors
//! (`ConcaveChordCut`, `ArmsMerging`) plus their shared width helper.
//!
//! Sited as a sibling to `phase4.rs` rather than inlined there: `phase4.rs`
//! sits at 754 lines against the 800-line incl.-tests soft cap, and every new
//! detector body + fixture + test lands here instead
//! (`ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md` § Approach,
//! § Risks R1).

use gp_core::geom::{Corridor, Orient, Point};

use crate::phase4::wall_runs;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
