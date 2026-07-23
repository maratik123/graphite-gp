//! Ф5b — full Vmax passability oracle (iterative deepening) + speed metrics +
//! `break_points` (design doc §2/§3, spec
//! `ai-docs/plans/2026-07-23-gp-gen-phase5b-full-oracle.spec.md`,
//! design `ai-docs/plans/2026-07-23-gp-gen-phase5b-full-oracle.design.md`).
//!
//! Composes the Ф5a substrate ([`crate::forward_reachable`] /
//! [`crate::backward_reachable`] / `within_v_ceil`, `phase5.rs`) into
//! [`phase5_full_oracle`] — an iterative-deepening driver that never
//! reimplements the flood edge (core's `legal_move`) or the crossing test
//! (core's `LapCounter::register_move`) (design § Approach; AC5).

use gp_core::geom::Point;
use gp_core::track::TrackMetrics;

/// The result of running [`phase5_full_oracle`] (design § Approach (3)).
///
/// `Lappable` populates the existing [`TrackMetrics`] fields (the exported
/// artifact contract, `gp-core::track`); `NotLappable` carries the raw
/// reachability-stall diagnostic `break_points` — a gen-internal input to
/// Ф6's `map_frontier_gap_to_edge` (out of scope here), not part of the
/// exported contract type.
#[derive(Clone, Debug)]
pub enum OracleResult {
    /// A closed lap exists; carries the populated speed metrics.
    Lappable(TrackMetrics),
    /// No closed lap exists in `live`; carries the frontier-gap diagnostic
    /// (design `[N3]`) — non-empty by the generator-guarantee dependency
    /// (design § Approach (3), AC3).
    NotLappable {
        /// The raw reachability-stall frontier between `R` and the
        /// lap-close goal.
        break_points: Vec<Point>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_result_variants_are_constructible_and_clonable() {
        // Subtask-1 compile-smoke test: both variants build, derive
        // Clone/Debug, and match as expected. `OracleResult` deliberately
        // does not derive `PartialEq` (its `TrackMetrics` payload doesn't
        // either, and adding it there is a gp-core change, out of scope) —
        // callers compare field-wise instead (design § Approach (3)).
        let lappable = OracleResult::Lappable(TrackMetrics::default());
        let lappable2 = lappable.clone();
        match (lappable, lappable2) {
            (OracleResult::Lappable(a), OracleResult::Lappable(b)) => {
                assert_eq!(a.vmax_attain, b.vmax_attain);
            }
            _ => panic!("expected both to be Lappable"),
        }

        let not_lappable = OracleResult::NotLappable {
            break_points: vec![Point::new(1, 2)],
        };
        let not_lappable2 = not_lappable.clone();
        match (not_lappable, not_lappable2) {
            (
                OracleResult::NotLappable { break_points: a },
                OracleResult::NotLappable { break_points: b },
            ) => assert_eq!(a, b),
            _ => panic!("expected both to be NotLappable"),
        }
    }
}
