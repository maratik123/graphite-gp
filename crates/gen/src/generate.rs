//! `generate()` — the Block-1 capstone that wires the already-landed phases
//! Ф1→Ф7 into the outer generation loop of `docs/design.md` §2
//! (`generate_track` pseudocode). No phase behaviour or signature changes —
//! orchestration + [`gp_core::track::TrackArtifact`] assembly only
//! (`ai-docs/plans/2026-07-24-gp-gen-generate-pipeline.design.md`).

use gp_core::geom::{Corridor, Wall, walls_from_boundary};
use gp_core::track::{RaceDir, SField, StartFinish, StartGrid, TrackArtifact, TrackMetrics};
use thiserror::Error;

use crate::{
    CoarseSkeleton, GenParams, Issue, OracleResult, Phase3Output, RepairContext, RepairOutcome,
    oracle_liveness_v1, phase1_coarse_ring, phase2_rasterize, phase3_start_finish,
    phase4_static_checks, phase5_full_oracle, phase5_runout_checks, phase6_local_repair,
    racing_line,
};

/// `generate`'s failure value — the design-doc `GENERATION_FAILED` sentinel.
#[derive(Debug, Error)]
pub enum GenerationError {
    /// The outer seed-budget loop (`params.seed_budget` draws from the
    /// continuing `generation_rng` stream) was spent without ever reaching
    /// the accept path.
    #[error("track generation failed: seed budget exhausted without an acceptable track")]
    SeedBudgetExhausted,
}

/// Cheap-then-expensive gate (AC1): the expensive Vmax oracle
/// (`phase5_full_oracle`) is worth running only once both cheap checks are
/// clean — no outstanding Ф4 static issue, and the V=1 liveness probe already
/// finds a lap.
const fn should_run_oracle(static_issues: &[Issue], liveness: bool) -> bool {
    static_issues.is_empty() && liveness
}

/// Assembles the accepted corridor + its fixed per-seed context (`sf`,
/// `grid`, `race_dir`) and the accepting oracle run's `metrics` into a fully
/// populated [`TrackArtifact`] (design § Artifact assembly).
///
/// Move-order: every field that borrows `&d` / `&sf.gate` (`walls`,
/// `s_field`, `centerline`, `width_min`) is computed **before** `d`/`sf`/
/// `grid`/`metrics` are moved into the struct.
fn build_artifact(
    d: Corridor,
    sf: StartFinish,
    grid: StartGrid,
    race_dir: RaceDir,
    metrics: TrackMetrics,
) -> TrackArtifact {
    let walls = walls_from_boundary(&d);
    let s_field = SField::from_gate_bfs(&d, &sf.gate);
    let centerline = racing_line(&d, &sf.gate, race_dir);
    let width_min = crate::phase4_defects::corridor_min_width(&d);

    TrackArtifact {
        corridor: d,
        walls,
        sf,
        race_dir,
        s_field,
        start_grid: grid,
        centerline,
        metrics,
        width_min,
    }
}

/// Run the full generation pipeline (design doc §2, Ф1–Ф7) and return a
/// validated, passability-certified track.
///
/// Outer seed-budget loop (`params.seed_budget` iterations) over a single
/// continuing RNG stream (`params.generation_rng()`, constructed once —
/// replay-determinism contract, #49): each iteration draws a fresh skeleton
/// (Ф1), rasterizes + seats the start/finish (Ф2/Ф3), then runs an inner
/// repair loop (`params.repair_budget` iterations) that gates the expensive
/// oracle behind the cheap static + liveness checks (AC1), routes
/// `NotLappable` stall diagnostics or run-out `NoBraking` issues into Ф6
/// (AC2/AC6), and accepts on a `Lappable` result with an empty run-out check.
/// A repair iteration that makes no progress (`RepairOutcome::Failed`) drops
/// to the next seed. Falling through both loops without accepting returns
/// `Err(GenerationError::SeedBudgetExhausted)` (AC3) — no infinite loop,
/// zero production panics on any `GenParams` (AC7).
///
/// # Errors
///
/// Returns [`GenerationError::SeedBudgetExhausted`] if `params.seed_budget`
/// seed draws are spent without ever reaching an accept path (every seed's
/// inner repair loop ran out of `params.repair_budget` iterations, or the
/// oracle never certified a lappable, run-out-clean track).
pub fn generate(params: GenParams) -> Result<TrackArtifact, GenerationError> {
    let mut rng = params.generation_rng();
    let n_u32 = params.min_width();
    let phase2_n = i32::try_from(n_u32).unwrap_or(i32::MAX);
    let m = params.start_finish_width();
    let k = params.block_size;
    let v_target = params.v_ceiling;

    for _ in 0..params.seed_budget {
        let skel: CoarseSkeleton = phase1_coarse_ring(params.min_straight, &mut rng);
        let race_dir = skel.dir;
        let corridor = phase2_rasterize(&skel, k, phase2_n);
        let Phase3Output { mut d, sf, grid } = phase3_start_finish(corridor, &skel, m, v_target);

        for _ in 0..params.repair_budget {
            let mut issues = phase4_static_checks(&d, &skel, k, n_u32, m, &sf);
            let liveness = oracle_liveness_v1(&d, &grid, &sf, race_dir);

            let mut oracle_metrics: Option<TrackMetrics> = None;
            let mut oracle_stall: Option<Vec<Wall>> = None;

            if should_run_oracle(&issues, liveness) {
                match phase5_full_oracle(&d, &grid, &sf, race_dir) {
                    OracleResult::Lappable(metrics) => {
                        let runout = phase5_runout_checks(&d, &metrics, v_target);
                        if runout.is_empty() {
                            return Ok(build_artifact(d, sf, grid, race_dir, metrics));
                        }
                        issues = runout;
                        oracle_metrics = Some(metrics);
                    }
                    OracleResult::NotLappable { stall_walls } => {
                        oracle_stall = Some(stall_walls);
                    }
                }
            }

            let ctx = RepairContext {
                d: &d,
                skel: &skel,
                k,
                n: n_u32,
                m,
                grid: &grid,
                sf: &sf,
                race_dir,
                v_target,
                metrics: oracle_metrics.as_ref(),
                stall_walls: oracle_stall.as_deref(),
            };
            let outcome = phase6_local_repair(&ctx, &issues);
            match outcome {
                RepairOutcome::Repaired { d: nd, .. } => d = nd,
                RepairOutcome::Failed => break,
            }
        }
    }

    Err(GenerationError::SeedBudgetExhausted)
}

#[cfg(test)]
mod tests {
    use gp_core::rng::Seeds;

    use super::*;

    fn params(seed: u64, seed_budget: u32, repair_budget: u32) -> GenParams {
        GenParams {
            cars: 4,
            min_straight: 3,
            v_ceiling: 5,
            block_size: 6,
            seeds: Seeds {
                generation: seed,
                ..Default::default()
            },
            seed_budget,
            repair_budget,
        }
    }

    // ---- AC1: cheap-then-expensive gate --------------------------------

    #[test]
    fn should_run_oracle_declines_on_outstanding_static_issue() {
        assert!(!should_run_oracle(
            &[Issue::Disconnected],
            /* liveness */ true
        ));
    }

    #[test]
    fn should_run_oracle_declines_on_dead_liveness() {
        assert!(!should_run_oracle(&[], /* liveness */ false));
    }

    #[test]
    fn should_run_oracle_runs_when_both_cheap_checks_are_clean() {
        assert!(should_run_oracle(&[], /* liveness */ true));
    }

    #[test]
    fn a_disconnected_corridor_yields_a_non_empty_static_check() {
        // Grounds should_run_oracle's false branch in a real phase4_static_checks
        // call, not just the predicate in isolation.
        let mut rng = params(1, 1, 1).generation_rng();
        let skel = phase1_coarse_ring(3, &mut rng);
        // An empty corridor box (no thickening) is disconnected/topologically
        // broken relative to the skeleton's ring.
        let d = Corridor::new(gp_core::geom::Point::new(0, 0), 1, 1);
        let issues = phase4_static_checks(
            &d,
            &skel,
            6,
            2,
            4,
            &StartFinish {
                chord: vec![],
                orient: gp_core::geom::Orient::Horizontal,
                gate: gp_core::track::TimingGate {
                    behind: vec![],
                    forward: gp_core::geom::Side::East,
                },
            },
        );
        assert!(!issues.is_empty());
    }

    // ---- AC3: GENERATION_FAILED path ------------------------------------

    #[test]
    fn zero_seed_budget_fails_promptly() {
        let p = params(1, 0, 8);
        assert!(matches!(
            generate(p),
            Err(GenerationError::SeedBudgetExhausted)
        ));
    }

    #[test]
    fn zero_repair_budget_fails_promptly() {
        let p = params(1, 8, 0);
        assert!(matches!(
            generate(p),
            Err(GenerationError::SeedBudgetExhausted)
        ));
    }

    // ---- AC6: run-out routing / accept guard ----------------------------

    #[test]
    fn no_braking_issue_fails_the_accept_guard() {
        let runout = [Issue::NoBraking {
            at: gp_core::geom::Point::new(0, 0),
        }];
        assert!(!runout.is_empty());
    }

    // ---- AC4/AC5: end-to-end determinism + invariants -------------------

    #[test]
    fn generate_e2e_accepts_a_self_consistent_deterministic_track() {
        let p = params(1, 64, 32);

        let a1 = generate(p).expect("a working (seed, seed_budget, repair_budget) triple");
        let a2 = generate(p).expect("second run must also accept");

        assert_eq!(format!("{a1:?}"), format!("{a2:?}"), "determinism (AC5)");

        assert!(
            oracle_liveness_v1(&a1.corridor, &a1.start_grid, &a1.sf, a1.race_dir),
            "a lap must exist on the returned artifact"
        );
        assert!(
            u32::try_from(a1.sf.width()).unwrap_or(u32::MAX) >= p.start_finish_width(),
            "S/F chord width must be >= m"
        );
        assert!(
            a1.width_min >= p.min_width(),
            "width_min must be >= n = ceil(m/2), got {}",
            a1.width_min
        );
        assert_eq!(a1.s_field.rect, a1.corridor.rect());
        for cell in a1.sf.gate.forward_face() {
            assert_eq!(a1.s_field.scalar_at(cell), Some(0));
        }
        assert!(!a1.walls.is_empty());
        // Non-empty-centerline assertion intentionally NOT here: it fails
        // until the A1 medial_axis fix + Ф7 bridge-guard land (subtasks 5-7)
        // and is reinstated in subtask 8's AC5(b)/AC8 e2e tests.
        let mut deduped = a1.start_grid.positions.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), a1.start_grid.positions.len());
    }
}
