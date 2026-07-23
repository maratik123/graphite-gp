//! Ф6 — reachability-deficit → dual-edge mapping (design doc §2 Ф6
//! `DYNAMICALLY_DISCONNECTED`, `[N3]`; spec
//! `ai-docs/plans/2026-07-24-gp-gen-frontier-gap-mapping.spec.md`, design
//! `ai-docs/plans/2026-07-24-gp-gen-frontier-gap-mapping.design.md`).
//!
//! This module currently holds only the **AC7 monotonicity proof gate**
//! (subtask 2): an executable test that decides AC8's outcome-shape arity
//! before `RepairCandidate` and `map_frontier_gap_to_edge` are written
//! (subtask 5 onward).

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use gp_core::sim::CarState;
    use gp_core::track::RaceDir;

    use crate::testfix::*;
    use crate::{OracleResult, oracle_liveness_v1, phase5_full_oracle};

    /// `trap_ring`'s two defining properties (design § Fixture designs): (a)
    /// it **is** V=1 lappable, and (b) it carries a genuine higher-speed
    /// hazard — a non-empty `R \ B` at `V_ceil = 2` witnessed by the named
    /// dead state `CarState { x: 6, y: 5, vx: 0, vy: 2 }` (design's spur
    /// analysis). Non-vacuity: the witness is asserted **present** in `R`
    /// before asserting it **absent** from `B`, so the test cannot pass on
    /// an empty `R \ B`.
    #[test]
    fn trap_ring_is_v1_lappable_and_has_an_unbrakeable_hazard() {
        let (d, sf, grid) = trap_ring();

        assert!(oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw));

        let witness = car(6, 5, 0, 2);
        let seeds: Vec<CarState> = grid
            .positions
            .iter()
            .map(|&p| car(p.x, p.y, 0, 0))
            .collect();
        let v_ceil = 2;
        let r = crate::forward_reachable(&d, &seeds, v_ceil);
        assert!(r.contains(&witness), "witness must be forward-reachable");

        let goals = crate::phase5b::lap_close_goals(&d, &sf, &r, v_ceil);
        let b = crate::backward_reachable(&d, &goals, v_ceil);
        assert!(
            !b.contains(&witness),
            "the spur dead state must not reach any forward crossing"
        );

        let r_minus_b: HashSet<CarState> = r.difference(&b).copied().collect();
        assert!(
            !r_minus_b.is_empty(),
            "trap_ring must carry a non-empty R \\ B at V_ceil = 2"
        );
    }

    /// The AC7 monotonicity biconditional: `oracle_liveness_v1(d, grid, sf,
    /// dir) == matches!(phase5_full_oracle(d, grid, sf, dir),
    /// OracleResult::Lappable(_))`, evaluated over the whole fixture
    /// battery (design § The monotonicity proof gate; spec AC7) — the five
    /// pre-existing fixtures plus `trap_ring`, the purpose-built candidate
    /// counterexample. Each case is named in the assertion message so a
    /// failure identifies the falsifying fixture (which AC8 Branch B would
    /// then adopt as its regression test — design § AC8 Branch B
    /// contingency).
    #[test]
    fn ac7_v1_liveness_is_equivalent_to_full_oracle_lappability() {
        let cases: Vec<(
            &str,
            gp_core::geom::Corridor,
            gp_core::track::StartFinish,
            gp_core::track::StartGrid,
        )> = {
            let mut broken_ring = ring_corridor();
            broken_ring.set(gp_core::geom::Point::new(4, 2), false);

            let (dead_end_d, dead_end_sf, dead_end_grid) = dead_end_corridor();
            let (crash_d, crash_sf, crash_grid) = crash_pocket_fixture();
            let (trap_d, trap_sf, trap_grid) = trap_ring();

            vec![
                ("ring", ring_corridor(), ring_sf(), ring_grid()),
                ("broken_ring", broken_ring, ring_sf(), ring_grid()),
                ("dead_end", dead_end_d, dead_end_sf, dead_end_grid),
                (
                    "long_straight",
                    long_straight_corridor(),
                    long_straight_sf(),
                    long_straight_grid(),
                ),
                ("crash_pocket", crash_d, crash_sf, crash_grid),
                ("trap_ring", trap_d, trap_sf, trap_grid),
            ]
        };

        for (name, d, sf, grid) in &cases {
            let liveness = oracle_liveness_v1(d, grid, sf, RaceDir::Ccw);
            let full = matches!(
                phase5_full_oracle(d, grid, sf, RaceDir::Ccw),
                OracleResult::Lappable(_)
            );
            assert_eq!(
                liveness, full,
                "fixture {name}: oracle_liveness_v1 ({liveness}) must equal \
                 full-oracle lappability ({full})"
            );
        }
    }
}
