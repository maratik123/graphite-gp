//! Ф6 — reachability-deficit → dual-edge mapping (design doc §2 Ф6
//! `DYNAMICALLY_DISCONNECTED`, `[N3]`; spec
//! `ai-docs/plans/2026-07-24-gp-gen-frontier-gap-mapping.spec.md`, design
//! `ai-docs/plans/2026-07-24-gp-gen-frontier-gap-mapping.design.md`).
//!
//! Holds the AC7 monotonicity proof gate (subtask 2, `#[cfg(test)]` only)
//! and [`map_frontier_gap_to_edge`] (subtask 5 onward) — a total,
//! deterministic, non-panicking mapping from a Ф5b stall diagnostic to a
//! verified-progress repair edit.
//!
//! # `[N3]` convergence risk
//!
//! This is the design's single riskiest, explicitly-unproven step
//! (`docs/design.md` §2 `[N3]`): whether the "almost-valid by construction +
//! oracle certifies + local repair" scheme converges rather than falling
//! back to a full reseed. The prototype-first spike here validates one edge
//! on a hand-built almost-valid track (the broken-ring fixture: `|P0|` grows
//! `3 → 16` and `phase5_full_oracle` flips `NotLappable → Lappable` off a
//! *single* dual-edge shift), but the general question is open: **does one
//! edge ever suffice?** On a multi-cell sever the honest answer may be "one
//! edge per iteration, `N` iterations" — this module's contract (AC5, strict
//! progress) is satisfiable either way, but full closure in one call is not
//! guaranteed and is not asserted beyond the one fixture where it provably
//! holds.
//!
//! # Outcome meaning (AC8 Branch A — two variants)
//!
//! [`RepairCandidate::Edge`] is a **verified**, not merely plausible, repair:
//! the caller may apply it to `D` and re-run the Ф5b oracle expecting
//! `|P0|` to have strictly grown (AC5), though not necessarily a closed lap
//! in one step (see the convergence risk above). [`RepairCandidate::NoCandidate`]
//! means no boundary edge derived from the diagnostic grows `P0` — this is a
//! genuine, reachable outcome on a box-filling corridor (`dead_end_corridor`,
//! `crash_pocket_fixture`: every off-`D` neighbor of every boundary wall lies
//! outside the bounding box, where [`gp_core::geom::Corridor::set`] is a
//! documented no-op), **not** a swallowed error to "fix" by relaxing the
//! growth check.
//!
//! **Why only two outcomes, not three.** The design phase executed the AC7
//! monotonicity proof gate (`phase6::tests::ac7_v1_liveness_is_equivalent_to_full_oracle_lappability`,
//! subtask 2) *before* `RepairCandidate`'s shape was locked (spec KD4). The
//! dynamic-only stall class — V=1 lappable, but the full-`Vmax` oracle is
//! not — **is empty**, not merely unobserved: `live` is monotone in
//! `V_ceil` (`within_v_ceil` admits a strict superset of states as `V_ceil`
//! grows, so `R`, the lap-close goal set, `B`, and `live = R ∩ B` all grow
//! too), so `phase5_full_oracle`'s `let Some(fastest) = fastest else` arm
//! (`phase5b.rs`) can only fire on its **first** iteration, at `V_ceil = 1`
//! — exactly what `oracle_liveness_v1` already reports. The AC7 test pins
//! that structural property against a future edit to the `V_ceil` loop; it
//! is **not** "no counterexample was found" (an empirical survey claim) but
//! a regression guard on a proven theorem.
//!
//! # Reseed fallback (`[N4]`)
//!
//! A [`RepairCandidate::NoCandidate`] result burns one repair-budget step
//! (out of scope here — the Ф6 repair loop's job); budget exhaustion
//! returns to Ф1 with a new seed (design §2 `generate()`'s
//! `if D == FAILED: break`, `[N4]` seed budget).
//!
//! # Tie-break refinement left un-adopted
//!
//! The mapper's max-growth-then-`wall_sort_key` tie-break was considered
//! against a "prefer the edge nearest the medial axis" quality refinement
//! (spec § Open questions). Not adopted: it would need a
//! [`gp_core::geom::DistanceTransform`] computed per candidate, for a
//! benefit this spike has no evidence for — left as a note for a future
//! quality pass, not a correctness gap.

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

    use gp_core::geom::{Corridor, Point, Side, Wall};
    use gp_core::sim::CarState;
    use gp_core::track::{RaceDir, StartGrid};

    use super::{RepairCandidate, map_frontier_gap_to_edge, p0_at_v1};
    use crate::phase5b::wall_neighbor;
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

    // ---- map_frontier_gap_to_edge (subtasks 6-7: AC3, AC4, AC5, AC6, AC9, AC10, AC11) ----

    /// The broken ring's stall diagnostic, obtained from a real
    /// `phase5_full_oracle` call rather than hand-built, so these tests
    /// cover the producer/consumer contract end to end.
    fn broken_ring_diagnostic() -> (Corridor, gp_core::track::StartFinish, StartGrid, Vec<Wall>) {
        let mut d = ring_corridor();
        d.set(Point::new(4, 2), false);
        let sf = ring_sf();
        let grid = ring_grid();

        let OracleResult::NotLappable { stall_walls } =
            phase5_full_oracle(&d, &grid, &sf, RaceDir::Ccw)
        else {
            panic!("expected the broken ring to return NotLappable");
        };
        (d, sf, grid, stall_walls)
    }

    #[test]
    fn maps_a_v1_sever_to_a_boundary_edge() {
        let (d, sf, grid, stall_walls) = broken_ring_diagnostic();

        let result = map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &stall_walls);

        let expected = Wall {
            cell: Point::new(4, 1),
            side: Side::North,
        };
        assert_eq!(result, RepairCandidate::Edge(expected));

        // AC4, independently of the mapper's own re-validation: the
        // returned wall is a genuine boundary edge of D.
        assert!(d.contains(expected.cell));
        let neighbor = wall_neighbor(expected).expect("in-box neighbor");
        assert!(!d.contains(neighbor));
    }

    #[test]
    fn returned_edit_strictly_grows_p0() {
        let (d, sf, grid, stall_walls) = broken_ring_diagnostic();

        let RepairCandidate::Edge(w) =
            map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &stall_walls)
        else {
            panic!("expected an Edge candidate on the broken ring");
        };
        let neighbor = wall_neighbor(w).expect("in-box neighbor");

        // Recomputed independently of the mapper's internals: a fresh
        // p0_at_v1 call before/after, not a reuse of the mapper's own
        // measurement.
        let before = p0_at_v1(&d, &grid, &sf).len();
        let mut d2 = d;
        d2.set(neighbor, true);
        let after = p0_at_v1(&d2, &grid, &sf).len();

        assert!(
            after > before,
            "expected strict growth, got {before} -> {after}"
        );
        // Measured (design § Approach (2)): 3 -> 16.
    }

    #[test]
    fn max_growth_selects_the_severed_edge_over_a_lesser_candidate() {
        // (3, 0)'s North side is also an admissible candidate on the broken
        // ring: its off-D neighbor (3, 1) is in-box and non-drivable, so it
        // passes the mapper's admissibility filter -- this test pins that
        // `max grown` (not `wall_sort_key`, which would pick (3,0) < (4,1))
        // is the rule that selects (4,1)North.
        let (d, sf, grid, _) = broken_ring_diagnostic();

        let severed = Wall {
            cell: Point::new(4, 1),
            side: Side::North,
        };
        let lesser = Wall {
            cell: Point::new(3, 0),
            side: Side::North,
        };
        let severed_neighbor = wall_neighbor(severed).expect("in-box neighbor");
        let lesser_neighbor = wall_neighbor(lesser).expect("in-box neighbor");
        assert!(d.contains(severed.cell) && !d.contains(severed_neighbor));
        assert!(d.contains(lesser.cell) && !d.contains(lesser_neighbor));

        let base = p0_at_v1(&d, &grid, &sf).len();
        let mut d_severed = d.clone();
        d_severed.set(severed_neighbor, true);
        let severed_growth = p0_at_v1(&d_severed, &grid, &sf).len() - base;

        let mut d_lesser = d.clone();
        d_lesser.set(lesser_neighbor, true);
        let lesser_growth = p0_at_v1(&d_lesser, &grid, &sf).len().saturating_sub(base);

        assert!(
            lesser_growth < severed_growth,
            "expected (3,0)'s growth ({lesser_growth}) to be strictly less than \
             (4,1)'s ({severed_growth})"
        );

        // Both candidates admissible; the mapper must still pick the
        // higher-growth one, not (3,0) via wall_sort_key ascending order.
        let both = [lesser, severed];
        assert_eq!(
            map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &both),
            RepairCandidate::Edge(severed)
        );
    }

    #[test]
    fn returned_edit_closes_the_lap() {
        let (d, sf, grid, stall_walls) = broken_ring_diagnostic();
        assert!(matches!(
            phase5_full_oracle(&d, &grid, &sf, RaceDir::Ccw),
            OracleResult::NotLappable { .. }
        ));

        let RepairCandidate::Edge(w) =
            map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &stall_walls)
        else {
            panic!("expected an Edge candidate on the broken ring");
        };
        let neighbor = wall_neighbor(w).expect("in-box neighbor");

        let mut d2 = d;
        d2.set(neighbor, true);
        assert!(matches!(
            phase5_full_oracle(&d2, &grid, &sf, RaceDir::Ccw),
            OracleResult::Lappable(_)
        ));
    }

    #[test]
    fn no_candidate_when_no_boundary_edge_grows_p0() {
        // Both dead_end_corridor and crash_pocket_fixture are box-filling
        // corridors: every off-D neighbor of any boundary wall lies
        // outside the bounding box, so Corridor::set is a documented no-op
        // there and no edit can grow P0.
        for (d, sf, grid) in [dead_end_corridor(), crash_pocket_fixture()] {
            let OracleResult::NotLappable { stall_walls } =
                phase5_full_oracle(&d, &grid, &sf, RaceDir::Ccw)
            else {
                panic!("expected NotLappable");
            };
            assert!(
                !stall_walls.is_empty(),
                "diagnostic must be non-empty for this to be a real decision"
            );

            assert_eq!(
                map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &stall_walls),
                RepairCandidate::NoCandidate
            );
        }
    }

    #[test]
    fn is_total_on_adversarial_input() {
        let (d, sf, grid) = dead_end_corridor();

        // Empty diagnostic.
        assert_eq!(
            map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &[]),
            RepairCandidate::NoCandidate
        );

        // Walls naming cells far outside D's bounding box, including
        // coordinates that would overflow wall_neighbor's checked_add --
        // guarded upfront by the d.contains(w.cell) check, so
        // wall_neighbor is never even reached for these; either way the
        // function must return cleanly, never panic.
        let adversarial = [
            Wall {
                cell: Point::new(9999, 9999),
                side: Side::East,
            },
            Wall {
                cell: Point::new(i32::MAX, i32::MAX),
                side: Side::East,
            },
            Wall {
                cell: Point::new(i32::MIN, i32::MIN),
                side: Side::West,
            },
        ];
        assert_eq!(
            map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &adversarial),
            RepairCandidate::NoCandidate
        );

        // A degenerate zero-area corridor.
        let empty_d = Corridor::new(Point::new(0, 0), 0, 0);
        let empty_grid = StartGrid {
            positions: vec![Point::new(0, 0)],
        };
        assert_eq!(
            map_frontier_gap_to_edge(&empty_d, &empty_grid, &sf, RaceDir::Ccw, &adversarial),
            RepairCandidate::NoCandidate
        );
    }

    #[test]
    fn is_deterministic_and_input_order_independent() {
        let (d, sf, grid, stall_walls) = broken_ring_diagnostic();

        let r1 = map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &stall_walls);
        let r2 = map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &stall_walls);
        assert_eq!(r1, r2, "repeated calls must agree");

        let mut reversed = stall_walls;
        reversed.reverse();
        let r3 = map_frontier_gap_to_edge(&d, &grid, &sf, RaceDir::Ccw, &reversed);
        assert_eq!(r1, r3, "a reversed input slice must yield the same outcome");
    }
}
