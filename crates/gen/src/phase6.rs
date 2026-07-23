//! Ф6 — reachability-deficit → dual-edge mapping (design doc §2 Ф6
//! `DYNAMICALLY_DISCONNECTED`, `[N3]`; spec
//! `ai-docs/plans/2026-07-24-gp-gen-frontier-gap-mapping.spec.md`, design
//! `ai-docs/plans/2026-07-24-gp-gen-frontier-gap-mapping.design.md`).
//!
//! Holds the AC7 monotonicity proof gate (subtask 2, `#[cfg(test)]` only)
//! and [`map_frontier_gap_to_edge`] (subtask 5 onward) — a total,
//! deterministic, non-panicking mapping from a Ф5b stall diagnostic to a
//! verified-progress repair edit.

use std::collections::HashSet;

use gp_core::geom::{Corridor, Point, Wall};
use gp_core::sim::CarState;
use gp_core::track::{RaceDir, StartFinish, StartGrid};

use crate::phase5::ORACLE_V1_CEIL;
use crate::phase5b::{fastest_lap_through_live, live_at, wall_neighbor, wall_sort_key};

/// The progress metric (spec § Progress metric): the phase-0 reachable
/// **cell** set `P0` at the fixed `V_ceil = 1` ceiling, post-race-start,
/// pre-lap-close — the same `P0` `fastest_lap_through_live` emits, and the
/// same set `p0_boundary_walls`'s diagnostic is keyed on. Reuses `live_at`
/// together with `ORACLE_V1_CEIL` (widened from `phase5.rs`) rather than
/// re-declaring the `1` literal.
pub(crate) fn p0_at_v1(d: &Corridor, grid: &StartGrid, sf: &StartFinish) -> HashSet<Point> {
    let seeds: Vec<CarState> = grid
        .positions
        .iter()
        .map(|&p| CarState {
            x: p.x,
            y: p.y,
            vx: 0,
            vy: 0,
        })
        .collect();
    let live = live_at(d, &seeds, sf, ORACLE_V1_CEIL);
    let (_, p0) = fastest_lap_through_live(d, &grid.positions, sf, &live, ORACLE_V1_CEIL);
    p0
}

/// The outcome of mapping a Ф5b stall diagnostic to a repair edit
/// (design `[N3]`, `docs/design.md` §2 Ф6 `DYNAMICALLY_DISCONNECTED`).
///
/// Two variants, per the executed AC7 proof gate (design § AC7 proof
/// gate → Consequence — AC8 Branch A): the dynamic-only stall class (V=1
/// lappable, but the full-`Vmax` oracle is not) is empty, so no `Declined`
/// arm is written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepairCandidate {
    /// The dual edge to shift outward: making the cell across `side`
    /// drivable is verified to strictly grow `|P0|` at `V_ceil = 1`.
    Edge(Wall),
    /// No boundary edge derived from the diagnostic grows `P0` — the
    /// caller burns a repair-budget step (`[N4]` reseed fallback), never a
    /// sentinel `Wall`.
    NoCandidate,
}

/// Map a Ф5b stall diagnostic to a verified-progress repair edit (design §
/// Approach (2)).
///
/// A **total, deterministic, non-panicking** function that only ever
/// returns an edge it has *proved* grows the progress metric: for each
/// `w` in `stall_walls`, re-validates that `w` is a genuine boundary edge
/// of `d` (never trusting the diagnostic — AC10), scratch-applies the
/// corresponding add-edit, and keeps the candidate only if it **strictly**
/// grows `|P0|` at `V_ceil = 1`. Among surviving candidates, picks the one
/// with **max growth**, ties broken by **min `wall_sort_key`** — a
/// function of the candidate *set*, not the input slice's order, so the
/// result is order-independent (AC11) even for an unsorted or shuffled
/// `stall_walls`. No surviving candidate yields
/// [`RepairCandidate::NoCandidate`] (AC9), never a sentinel `Wall`.
///
/// `race_dir` is accepted for signature fidelity and discarded, the same
/// convention `oracle_liveness_v1` (`phase5.rs`) and `phase5_full_oracle`
/// (`phase5b.rs`) already use.
pub fn map_frontier_gap_to_edge(
    d: &Corridor,
    grid: &StartGrid,
    sf: &StartFinish,
    race_dir: RaceDir,
    stall_walls: &[Wall],
) -> RepairCandidate {
    let _ = race_dir;

    let base = p0_at_v1(d, grid, sf).len();

    let mut best: Option<(usize, Wall)> = None;
    for &w in stall_walls {
        if !d.contains(w.cell) {
            continue;
        }
        let Some(q) = wall_neighbor(w) else {
            continue;
        };
        if d.contains(q) {
            continue;
        }

        let mut d2 = d.clone();
        d2.set(q, true);
        let grown = p0_at_v1(&d2, grid, sf).len();
        if grown <= base {
            continue;
        }

        best = Some(match best {
            None => (grown, w),
            Some((best_grown, best_w)) => {
                if grown > best_grown
                    || (grown == best_grown && wall_sort_key(w) < wall_sort_key(best_w))
                {
                    (grown, w)
                } else {
                    (best_grown, best_w)
                }
            }
        });
    }

    match best {
        Some((_, w)) => RepairCandidate::Edge(w),
        None => RepairCandidate::NoCandidate,
    }
}

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
