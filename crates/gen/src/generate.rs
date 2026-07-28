//! `generate()` — the Block-1 capstone that wires the already-landed phases
//! Ф1→Ф7 into the outer generation loop of `docs/design.md` §2
//! (`generate_track` pseudocode). No phase behaviour or signature changes —
//! orchestration + [`TrackArtifact`] assembly only
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
    /// The caller's [`GenObserver::is_cancelled`] reported `true` at one of
    /// the two loop boundaries (spec Scope 10) — no artifact was produced.
    #[error("track generation was cancelled")]
    Cancelled,
}

/// One of the seven fixed generation-pipeline phases (`docs/design.md` §2,
/// Ф1–Ф7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Ф1 — [`phase1_coarse_ring`].
    F1,
    /// Ф2 — [`phase2_rasterize`].
    F2,
    /// Ф3 — [`phase3_start_finish`].
    F3,
    /// Ф4 — [`phase4_static_checks`].
    F4,
    /// Ф5 — liveness / full oracle / run-out checks.
    F5,
    /// Ф6 — [`phase6_local_repair`].
    F6,
    /// Ф7 — `build_artifact` (crate-private).
    F7,
}

/// A single phase's outcome for one seed attempt / repair iteration.
///
/// Spec § Phase-status ordering. No `Pending` variant — `generate` never
/// *reports* pending; that is `gp-render`'s badge's own initial value
/// before any event arrives (design § Approach — *Per-phase outcome
/// table*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseOutcome {
    /// The run (or attempt/iteration) finished without ever executing this
    /// phase — e.g. Ф5's oracle gated off by `should_run_oracle`, or Ф6/Ф7
    /// on the accepting iteration.
    Skipped,
    /// The phase completed cleanly.
    Ok,
    /// The phase needed local repair.
    Repair,
    /// The phase produced a blocking issue on this attempt/iteration.
    Failed,
}

/// One phase-observation event: which phase, and its outcome for the
/// current seed attempt / repair iteration (spec Scope 10, AC9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseEvent {
    /// The phase this event reports on.
    pub phase: Phase,
    /// That phase's outcome this attempt/iteration.
    pub outcome: PhaseOutcome,
}

/// Cancellation + phase-observation hook for [`generate`] (spec Scope 10).
///
/// `impl GenObserver for ()` is the no-op `&mut ()` every pre-existing
/// caller passes — both methods default to inert (never cancelled, events
/// dropped).
pub trait GenObserver {
    /// Whether the caller wants generation to stop cooperatively. Checked
    /// at the top of each seed iteration and each repair iteration (the
    /// pipeline's two existing loop boundaries).
    fn is_cancelled(&self) -> bool {
        false
    }
    /// Reports one phase's outcome for the current seed attempt / repair
    /// iteration. Called for **every** attempt and iteration, not only the
    /// accepting one (aggregate-worst semantics, spec § Phase-status
    /// ordering).
    fn on_phase(&mut self, event: PhaseEvent) {
        let _ = event;
    }
}

impl GenObserver for () {}

/// Cheap-then-expensive gate (AC1): the expensive Vmax oracle
/// (`phase5_full_oracle`) is worth running only once both cheap checks are
/// clean — no outstanding Ф4 static issue, and the V=1 liveness probe already
/// finds a lap.
const fn should_run_oracle(static_issues: &[Issue], liveness: bool) -> bool {
    static_issues.is_empty() && liveness
}

/// Accept gate (AC6): a `Lappable` oracle verdict is only half the bar — the
/// run-out budget check (`phase5_runout_checks`) must come back clean too.
/// A non-empty result carries `NoBraking` issues, which are routed to Ф6 as a
/// repair iteration rather than silently accepted.
///
/// Extracted as a named predicate (mirroring [`should_run_oracle`]) so the
/// accept condition is exercised directly by a unit test instead of only
/// through a multi-minute end-to-end generation run.
const fn should_accept(runout_issues: &[Issue]) -> bool {
    runout_issues.is_empty()
}

/// Ф5's decision, factored out of [`generate`] to keep it under the
/// per-function line cap (`clippy::too_many_lines`, pedantic = deny) — the
/// emission-table `match` below is exactly the design's *Per-phase outcome
/// table* row for Ф5.
enum Phase5Decision {
    /// `!liveness`, or an `OracleResult::NotLappable` verdict.
    Failed(Option<Vec<Wall>>),
    /// `should_run_oracle` gated the oracle off this iteration.
    Skipped,
    /// A `Lappable` verdict with a clean run-out check — the accept path.
    Accept(TrackMetrics),
    /// A `Lappable` verdict with a dirty run-out check — routed to Ф6.
    Repair(Vec<Issue>, TrackMetrics),
}

/// Runs Ф5 (liveness already known) and classifies the result per the
/// design's emission table. Takes `issues` (Ф4's result) and `liveness`
/// (`oracle_liveness_v1`, already computed by the caller since it also
/// feeds [`should_run_oracle`]).
fn decide_phase5(
    d: &Corridor,
    grid: &StartGrid,
    sf: &StartFinish,
    race_dir: RaceDir,
    v_target: i32,
    issues: &[Issue],
    liveness: bool,
) -> Phase5Decision {
    if !liveness {
        return Phase5Decision::Failed(None);
    }
    if !should_run_oracle(issues, liveness) {
        return Phase5Decision::Skipped;
    }
    match phase5_full_oracle(d, grid, sf, race_dir) {
        OracleResult::Lappable(metrics) => {
            let runout = phase5_runout_checks(d, &metrics, v_target);
            if should_accept(&runout) {
                Phase5Decision::Accept(metrics)
            } else {
                Phase5Decision::Repair(runout, metrics)
            }
        }
        OracleResult::NotLappable { stall_walls } => Phase5Decision::Failed(Some(stall_walls)),
    }
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
/// `obs` is checked for cancellation at the top of each seed iteration and
/// each repair iteration (spec Scope 10), and receives one [`PhaseEvent`]
/// per phase per attempt/iteration — every attempt, not only the accepting
/// one (aggregate-worst semantics). Pass `&mut ()` for the pre-B2 behaviour.
///
/// # Errors
///
/// Returns [`GenerationError::SeedBudgetExhausted`] if `params.seed_budget`
/// seed draws are spent without ever reaching an accept path (every seed's
/// inner repair loop ran out of `params.repair_budget` iterations, or the
/// oracle never certified a lappable, run-out-clean track). Returns
/// [`GenerationError::Cancelled`] if `obs.is_cancelled()` reports `true` at
/// either loop boundary — no artifact is produced.
pub fn generate(
    params: GenParams,
    obs: &mut dyn GenObserver,
) -> Result<TrackArtifact, GenerationError> {
    let mut rng = params.generation_rng();

    for _ in 0..params.seed_budget {
        if obs.is_cancelled() {
            return Err(GenerationError::Cancelled);
        }
        if let Some(artifact) = attempt_seed(&mut rng, params, obs)? {
            return Ok(artifact);
        }
    }

    obs.on_phase(PhaseEvent {
        phase: Phase::F7,
        outcome: PhaseOutcome::Skipped,
    });
    Err(GenerationError::SeedBudgetExhausted)
}

/// A seed attempt's mutable corridor + its fixed `sf`/`grid` — threaded
/// through the repair loop and consumed whole on accept.
struct AttemptState {
    d: Corridor,
    sf: StartFinish,
    grid: StartGrid,
}

/// A seed attempt's per-repair-iteration-invariant context, bundled so
/// [`run_repair_iteration`] stays under `clippy::too_many_arguments`
/// (pedantic = deny, default threshold 7).
struct AttemptFixed<'a> {
    skel: &'a CoarseSkeleton,
    k: i32,
    n_u32: u32,
    m: u32,
    race_dir: RaceDir,
    v_target: i32,
}

/// One repair iteration's classification, returned by
/// [`run_repair_iteration`].
enum RepairIterResult {
    /// Ф5 accepted and Ф7 built the artifact.
    Accepted(TrackArtifact),
    /// Ф6 repaired the corridor — try again with the updated state.
    Continue(AttemptState),
    /// Ф6 made no progress (`RepairOutcome::Failed`) — drop to the next
    /// seed.
    GiveUp,
}

/// Runs one repair iteration (Ф4–Ф7's accept path) per the design's
/// *Per-phase outcome table*, reporting every phase it touches through
/// `obs`. Split out of [`attempt_seed`] to keep both functions under the
/// per-function line cap (`clippy::too_many_lines`, pedantic = deny).
fn run_repair_iteration(
    state: AttemptState,
    fixed: &AttemptFixed<'_>,
    obs: &mut dyn GenObserver,
) -> RepairIterResult {
    let AttemptState { d, sf, grid } = state;
    let &AttemptFixed {
        skel,
        k,
        n_u32,
        m,
        race_dir,
        v_target,
    } = fixed;

    let mut issues = phase4_static_checks(&d, skel, k, n_u32, m, &sf);
    obs.on_phase(PhaseEvent {
        phase: Phase::F4,
        outcome: if issues.is_empty() {
            PhaseOutcome::Ok
        } else {
            PhaseOutcome::Failed
        },
    });
    let liveness = oracle_liveness_v1(&d, &grid, &sf, race_dir);

    let mut oracle_metrics: Option<TrackMetrics> = None;
    let mut oracle_stall: Option<Vec<Wall>> = None;

    match decide_phase5(&d, &grid, &sf, race_dir, v_target, &issues, liveness) {
        Phase5Decision::Failed(stall) => {
            obs.on_phase(PhaseEvent {
                phase: Phase::F5,
                outcome: PhaseOutcome::Failed,
            });
            oracle_stall = stall;
        }
        Phase5Decision::Skipped => {
            obs.on_phase(PhaseEvent {
                phase: Phase::F5,
                outcome: PhaseOutcome::Skipped,
            });
        }
        Phase5Decision::Accept(metrics) => {
            obs.on_phase(PhaseEvent {
                phase: Phase::F5,
                outcome: PhaseOutcome::Ok,
            });
            obs.on_phase(PhaseEvent {
                phase: Phase::F6,
                outcome: PhaseOutcome::Skipped,
            });
            let artifact = build_artifact(d, sf, grid, race_dir, metrics);
            obs.on_phase(PhaseEvent {
                phase: Phase::F7,
                outcome: PhaseOutcome::Ok,
            });
            return RepairIterResult::Accepted(artifact);
        }
        Phase5Decision::Repair(runout, metrics) => {
            obs.on_phase(PhaseEvent {
                phase: Phase::F5,
                outcome: PhaseOutcome::Repair,
            });
            issues = runout;
            oracle_metrics = Some(metrics);
        }
    }

    let ctx = RepairContext {
        d: &d,
        skel,
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
    match phase6_local_repair(&ctx, &issues) {
        RepairOutcome::Repaired { d: nd, .. } => {
            obs.on_phase(PhaseEvent {
                phase: Phase::F6,
                outcome: PhaseOutcome::Repair,
            });
            RepairIterResult::Continue(AttemptState { d: nd, sf, grid })
        }
        RepairOutcome::Failed => {
            obs.on_phase(PhaseEvent {
                phase: Phase::F6,
                outcome: PhaseOutcome::Failed,
            });
            RepairIterResult::GiveUp
        }
    }
}

/// One seed attempt: Ф1–Ф3 once, then the repair loop (Ф4–Ф6) up to
/// `params.repair_budget` iterations. `Ok(Some(artifact))` on accept,
/// `Ok(None)` to fall through to the next seed, `Err(Cancelled)` if `obs`
/// trips the repair-loop boundary check.
///
/// Split out of [`generate`] to keep it under the per-function line cap
/// (`clippy::too_many_lines`, pedantic = deny); the seed-loop boundary
/// cancel check stays in `generate` itself since it gates *entry* into an
/// attempt, not anything inside one.
fn attempt_seed(
    rng: &mut rand_xoshiro::Xoshiro256PlusPlus,
    params: GenParams,
    obs: &mut dyn GenObserver,
) -> Result<Option<TrackArtifact>, GenerationError> {
    let n_u32 = params.min_width();
    let phase2_n = i32::try_from(n_u32).unwrap_or(i32::MAX);
    let m = params.start_finish_width();
    let k = params.block_size;
    let v_target = params.v_ceiling;

    let skel: CoarseSkeleton = phase1_coarse_ring(params.min_straight, rng);
    obs.on_phase(PhaseEvent {
        phase: Phase::F1,
        outcome: PhaseOutcome::Ok,
    });
    let race_dir = skel.dir;
    let corridor = phase2_rasterize(&skel, k, phase2_n);
    obs.on_phase(PhaseEvent {
        phase: Phase::F2,
        outcome: PhaseOutcome::Ok,
    });
    let Phase3Output { d, sf, grid } = phase3_start_finish(corridor, &skel, m, v_target);
    obs.on_phase(PhaseEvent {
        phase: Phase::F3,
        outcome: PhaseOutcome::Ok,
    });

    let fixed = AttemptFixed {
        skel: &skel,
        k,
        n_u32,
        m,
        race_dir,
        v_target,
    };
    let mut state = AttemptState { d, sf, grid };

    for _ in 0..params.repair_budget {
        if obs.is_cancelled() {
            return Err(GenerationError::Cancelled);
        }

        match run_repair_iteration(state, &fixed, obs) {
            RepairIterResult::Accepted(artifact) => return Ok(Some(artifact)),
            RepairIterResult::Continue(next) => state = next,
            RepairIterResult::GiveUp => break,
        }
    }

    Ok(None)
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
            generate(p, &mut ()),
            Err(GenerationError::SeedBudgetExhausted)
        ));
    }

    #[test]
    fn zero_repair_budget_fails_promptly() {
        let p = params(1, 8, 0);
        assert!(matches!(
            generate(p, &mut ()),
            Err(GenerationError::SeedBudgetExhausted)
        ));
    }

    // ---- AC6: run-out routing / accept guard ----------------------------

    #[test]
    fn should_accept_declines_on_a_no_braking_issue() {
        // Exercises the *production* accept gate (`generate`'s `should_accept`
        // call at the `Lappable` arm), not a local vec: a non-empty run-out
        // check must route to Ф6 instead of being accepted.
        assert!(!should_accept(&[Issue::NoBraking {
            at: gp_core::geom::Point::new(0, 0),
        }]));
    }

    #[test]
    fn should_accept_allows_a_clean_run_out_check() {
        assert!(should_accept(&[]));
    }

    // ---- AC4/AC5/AC8: end-to-end determinism + invariants ---------------

    /// AC4 well-formedness check shared by the AC5(a)/AC5(b)/AC8 tests below:
    /// `samples` non-empty, `samples[0].s == 0`, strictly increasing `s`, the
    /// loop wraps (`at(length) ~ at(0)`), and `length > 0.0` with at least the
    /// minimum 4-connected grid cycle (4 samples).
    fn assert_well_formed_centerline(cl: &gp_core::track::Centerline) {
        assert!(!cl.samples.is_empty(), "centerline must have samples");
        assert!(cl.samples.len() >= 4, "minimum 4-connected grid cycle");
        assert!(cl.length > 0.0, "centerline must have positive length");
        assert!(
            cl.samples[0].s.abs() < f32::EPSILON,
            "first sample must seed s == 0"
        );
        for w in cl.samples.windows(2) {
            assert!(w[1].s > w[0].s, "s must be strictly increasing");
        }
        let at0 = cl.at(0.0).expect("must sample at s=0");
        let at_len = cl.at(cl.length).expect("must wrap at s=length");
        assert!(
            (at0.pos.0 - at_len.pos.0).abs() < 1e-3 && (at0.pos.1 - at_len.pos.1).abs() < 1e-3,
            "the loop must wrap: at(0) ~ at(length)"
        );
    }

    /// AC5(a), heavy: a larger-budget config run twice for full-artifact
    /// determinism + invariants, including the now-reinstated non-empty,
    /// well-formed centerline (AC4/AC8). `#[ignore]`d — measured at ~467s in
    /// debug on this machine (§ progress decisions log), far above the
    /// default-suite budget; run manually/nightly.
    #[test]
    #[ignore = "heavy: ~467s debug wall time for a 64-seed/32-repair-budget \
                sweep — AC5(b) below covers the always-on default-suite case"]
    fn generate_e2e_accepts_a_self_consistent_deterministic_track() {
        let p = params(1, 64, 32);

        let a1 = generate(p, &mut ()).expect("a working (seed, seed_budget, repair_budget) triple");
        let a2 = generate(p, &mut ()).expect("second run must also accept");

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
        assert_well_formed_centerline(&a1.centerline);
        let mut deduped = a1.start_grid.positions.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), a1.start_grid.positions.len());
    }

    /// AC5(b), cheap: the always-on default-suite config — accepts on the
    /// first seed draw, deterministic across two runs, non-empty
    /// well-formed centerline (AC4).
    #[test]
    fn generate_e2e_cheap_default_suite_has_a_non_empty_centerline() {
        let p = params(6, 1, 8);

        let a1 =
            generate(p, &mut ()).expect("bs=6 seed=6 seed_budget=1 repair_budget=8 must accept");
        let a2 = generate(p, &mut ()).expect("second run must also accept");

        assert_eq!(format!("{a1:?}"), format!("{a2:?}"), "determinism (AC5)");
        assert_well_formed_centerline(&a1.centerline);
        // The design's Risks section rules that a `width_min < n` red is a
        // REAL signal (the `Narrow` gate only fires on a DT-consistent neck),
        // never to be silenced — so it is asserted in the always-on test too,
        // not only inside the `#[ignore]`d AC5(a).
        assert!(
            a1.width_min >= p.min_width(),
            "width_min {} must be >= n = ceil(m/2) = {}",
            a1.width_min,
            p.min_width()
        );
    }

    /// AC8 regression: running the accepted artifact's own corridor/gate/
    /// `race_dir` directly through `racing_line` (not the hand-built annulus)
    /// still yields a non-empty, well-formed centerline — the A1
    /// `medial_axis` fix + Ф7 bridge guard hold on a real generated
    /// corridor.
    ///
    /// **Seed 9, deliberately — not the AC5(b) seed.** Seed 6's corridor is
    /// insensitive to the Ф7 bridge guard (`racing_line` yields 310 samples
    /// with or without it), so pinning AC8 there would regression-guard only
    /// the `gp-core` A1 half of the fix. Reverting the
    /// `components(&medial).len() > 1` guard takes seed 9 from **364 samples
    /// to 0**, so this test fails if either half of the fix regresses.
    #[test]
    fn ac8_racing_line_regression_on_a_real_generated_corridor() {
        let p = params(9, 1, 8);
        let a =
            generate(p, &mut ()).expect("bs=6 seed=9 seed_budget=1 repair_budget=8 must accept");
        let cl = racing_line(&a.corridor, &a.sf.gate, a.race_dir);
        assert_well_formed_centerline(&cl);
    }

    // ---- spec AC8: cancellation at both loop boundaries -----------------

    /// A `GenObserver` whose `is_cancelled()` returns `true` starting from
    /// its `after`-th call (0-indexed) — call 0 is the seed-loop-top check,
    /// call 1 is the first repair-loop-top check, and so on. `CancelAfter(0)`
    /// therefore trips the outer boundary before any phase runs;
    /// `CancelAfter(1)` lets the outer check pass once and trips the inner
    /// boundary instead, exercising both documented check sites.
    struct CancelAfter {
        after: usize,
        calls: std::cell::Cell<usize>,
    }

    impl GenObserver for CancelAfter {
        fn is_cancelled(&self) -> bool {
            let seen = self.calls.get();
            self.calls.set(seen.saturating_add(1));
            seen >= self.after
        }
    }

    #[test]
    fn cancel_after_zero_trips_the_seed_loop_boundary_before_any_phase_runs() {
        let p = params(6, 64, 32);
        let mut obs = CancelAfter {
            after: 0,
            calls: std::cell::Cell::new(0),
        };
        assert!(matches!(
            generate(p, &mut obs),
            Err(GenerationError::Cancelled)
        ));
    }

    #[test]
    fn cancel_after_one_trips_the_repair_loop_boundary() {
        let p = params(6, 64, 32);
        let mut obs = CancelAfter {
            after: 1,
            calls: std::cell::Cell::new(0),
        };
        assert!(matches!(
            generate(p, &mut obs),
            Err(GenerationError::Cancelled)
        ));
        // Both boundary checks were reached: the seed-loop-top check (call
        // 0, passed) and the repair-loop-top check (call 1, tripped).
        assert!(obs.calls.get() >= 2);
    }

    #[test]
    fn an_uncancelled_run_at_the_same_params_still_accepts() {
        let p = params(6, 64, 32);
        assert!(generate(p, &mut ()).is_ok());
    }

    // ---- spec AC9: per-phase observation on every attempt/iteration -----

    /// Records every [`PhaseEvent`] `generate` reports, in emission order.
    #[derive(Default)]
    struct RecordingObserver {
        events: Vec<PhaseEvent>,
    }

    impl GenObserver for RecordingObserver {
        fn on_phase(&mut self, event: PhaseEvent) {
            self.events.push(event);
        }
    }

    #[test]
    fn accepting_run_reports_all_seven_phases() {
        let p = params(6, 1, 8);
        let mut obs = RecordingObserver::default();
        assert!(generate(p, &mut obs).is_ok());

        for phase in [
            Phase::F1,
            Phase::F2,
            Phase::F3,
            Phase::F4,
            Phase::F5,
            Phase::F6,
            Phase::F7,
        ] {
            assert!(
                obs.events.iter().any(|e| e.phase == phase),
                "accepting run must report {phase:?}"
            );
        }
        // The accepting iteration's Ф6/Ф7 are the design's aggregate-worst
        // inputs: Ф6 never actually ran that iteration (Skipped), Ф7 did
        // (Ok).
        assert_eq!(
            obs.events.last(),
            Some(&PhaseEvent {
                phase: Phase::F7,
                outcome: PhaseOutcome::Ok,
            })
        );
    }

    #[test]
    fn budget_exhausting_run_reports_all_seven_phases_and_a_skipped_f7() {
        // repair_budget = 1 so the repair-loop body runs exactly once and
        // Ф4/Ф5/Ф6 genuinely emit before falling through to Ф7's terminal
        // Skipped.
        let p = params(1, 1, 1);
        let mut obs = RecordingObserver::default();
        assert!(matches!(
            generate(p, &mut obs),
            Err(GenerationError::SeedBudgetExhausted)
        ));

        for phase in [
            Phase::F1,
            Phase::F2,
            Phase::F3,
            Phase::F4,
            Phase::F5,
            Phase::F6,
            Phase::F7,
        ] {
            assert!(
                obs.events.iter().any(|e| e.phase == phase),
                "budget-exhausting run must report {phase:?}"
            );
        }
        assert_eq!(
            obs.events.last(),
            Some(&PhaseEvent {
                phase: Phase::F7,
                outcome: PhaseOutcome::Skipped,
            }),
            "Ф7 never runs at all on a budget-exhausted run"
        );
    }
}
