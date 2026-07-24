//! # gp-gen — Block 1: track generation (design doc §2)
//!
//! Approach **A** (coarse-block ring, infield-first) **+ D** (local repair): make
//! the track almost-valid by construction so the expensive passability oracle
//! acts as a certifier, not a regeneration engine. The pipeline runs in phases
//! Ф1–Ф7 and emits a [`TrackArtifact`](gp_core::track::TrackArtifact).
//!
//! Ф6 (local repair, `[C3]`) is the last phase built so far:
//! [`phase6_local_repair`] applies one dual edge per committed edit, with an
//! edit-type-scoped `[C3]` recheck (add → local, remove → global flood-fill,
//! run-out → sink-to-sink), and returns [`RepairOutcome::Failed`] iff zero
//! edits were committed across the pass (`ai-docs/plans/
//! 2026-07-24-gp-gen-phase6-local-repair.design.md`).

mod coarse;
mod generate;
mod phase1;
mod phase2;
mod phase3;
mod phase4;
mod phase4_defects;
mod phase5;
mod phase5_runout;
mod phase5b;
mod phase6;
mod phase6_arms;
mod phase6_remove;
mod phase6_repair;
mod phase7;
#[cfg(test)]
mod testfix;

use gp_core::rng::Seeds;
use rand_xoshiro::Xoshiro256PlusPlus;

pub use generate::*;
pub use phase1::*;
pub use phase2::*;
pub use phase3::*;
pub use phase4::*;
pub use phase5::*;
pub use phase5_runout::phase5_runout_checks;
pub use phase5b::*;
pub use phase6::*;
pub use phase6_repair::*;
pub use phase7::*;

/// Generation parameters (design doc §2).
#[derive(Clone, Copy, Debug)]
pub struct GenParams {
    /// `m` — number of cars. Drives the width floor and the start/finish width.
    pub cars: u32,
    /// `L_min` — minimum straight length before a corner (run-out seed).
    pub min_straight: i32,
    /// `V` — oracle speed ceiling (scaffolding for the finite BFS, design doc §3).
    pub v_ceiling: i32,
    /// `k` — coarse-block size = the nominal corridor width (`k ≥ n`).
    pub block_size: i32,
    /// The grouped seeded-RNG config (issue #49) — `seeds.generation` feeds
    /// [`generation_rng`](Self::generation_rng), the pipeline's sole RNG path.
    pub seeds: Seeds,
    /// Outer-loop budget — max number of seeds (draws from the single
    /// `generation_rng` stream) to try before giving up on generation.
    pub seed_budget: u32,
    /// Inner-loop budget — max number of local-repair (Ф6) iterations per
    /// seed before abandoning that seed and drawing the next one.
    pub repair_budget: u32,
}

impl GenParams {
    /// Global minimum width `n = ⌈m/2⌉` (design doc §1).
    pub const fn min_width(&self) -> u32 {
        self.cars.div_ceil(2)
    }

    /// Start/finish width floor `≥ m` — cars start abreast across the corridor.
    pub const fn start_finish_width(&self) -> u32 {
        self.cars
    }

    /// A replay-deterministic RNG seeded from `self.seeds.generation`, for the
    /// generation pipeline's stochastic phases (design doc §2). No OS
    /// entropy — the same seed always yields the same draw stream (issue
    /// #49). The single generation RNG path (AC10) — no divergent duplicate.
    pub fn generation_rng(&self) -> Xoshiro256PlusPlus {
        self.seeds.generation_rng()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    /// A `GenParams` with `seeds.generation` set to `seed` and all other
    /// seeds zeroed.
    fn params(seed: u64) -> GenParams {
        GenParams {
            cars: 4,
            min_straight: 3,
            v_ceiling: 5,
            block_size: 6,
            seeds: Seeds {
                generation: seed,
                ..Default::default()
            },
            seed_budget: 1,
            repair_budget: 1,
        }
    }

    fn draws(mut rng: Xoshiro256PlusPlus) -> Vec<u64> {
        (0..8).map(|_| rng.next_u64()).collect()
    }

    #[test]
    fn rng_same_seed_yields_identical_stream() {
        // AC8: same generation seed -> identical stream.
        assert_eq!(
            draws(params(42).generation_rng()),
            draws(params(42).generation_rng())
        );
    }

    #[test]
    fn rng_different_seed_yields_different_stream() {
        assert_ne!(
            draws(params(1).generation_rng()),
            draws(params(2).generation_rng())
        );
    }

    #[test]
    fn generation_rng_is_the_sole_generation_path() {
        // AC10: a GenParams built with a given `seeds.generation` reproduces
        // the same stream regardless of the other three seeds.
        let a = GenParams {
            cars: 4,
            min_straight: 3,
            v_ceiling: 5,
            block_size: 6,
            seeds: Seeds {
                collision: 1,
                generation: 42,
                ai_learning: 2,
                ai_inference: 3,
            },
            seed_budget: 1,
            repair_budget: 1,
        };
        let b = GenParams {
            cars: 4,
            min_straight: 3,
            v_ceiling: 5,
            block_size: 6,
            seeds: Seeds {
                collision: 99,
                generation: 42,
                ai_learning: 98,
                ai_inference: 97,
            },
            seed_budget: 1,
            repair_budget: 1,
        };
        assert_eq!(draws(a.generation_rng()), draws(b.generation_rng()));
    }
}
