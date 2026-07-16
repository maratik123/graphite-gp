//! # gp-gen — Block 1: track generation (design doc §2)
//!
//! Approach **A** (coarse-block ring, infield-first) **+ D** (local repair): make
//! the track almost-valid by construction so the expensive passability oracle
//! acts as a certifier, not a regeneration engine. The pipeline runs in phases
//! Ф1–Ф7 and emits a [`TrackArtifact`].

use gp_core::track::TrackArtifact;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

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
    /// Seed for the (replay-deterministic) RNG.
    pub seed: u64,
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

    /// A replay-deterministic RNG seeded from `self.seed`, for the generation
    /// pipeline's stochastic phases (design doc §2). No OS entropy — the same
    /// `seed` always yields the same draw stream (issue #49).
    pub fn rng(&self) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(self.seed)
    }
}

/// Run the full generation pipeline (design doc §2, Ф1–Ф7) and return a
/// validated, passability-certified track.
///
/// TODO(1): implement the phased pipeline
///   Ф1 skeleton ring · Ф2 rasterize to `D` · Ф3 start/finish + grid ·
///   Ф4 static validation · Ф5 passability oracle · Ф6 local repair · Ф7 export.
pub fn generate(_params: GenParams) -> TrackArtifact {
    todo!("track generation pipeline (design doc §2)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    fn params(seed: u64) -> GenParams {
        GenParams {
            cars: 4,
            min_straight: 3,
            v_ceiling: 5,
            block_size: 6,
            seed,
        }
    }

    #[test]
    fn rng_same_seed_yields_identical_stream() {
        let mut a = params(42).rng();
        let mut b = params(42).rng();
        let draws_a: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let draws_b: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(draws_a, draws_b);
    }

    #[test]
    fn rng_different_seed_yields_different_stream() {
        let mut a = params(1).rng();
        let mut b = params(2).rng();
        let draws_a: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let draws_b: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_ne!(draws_a, draws_b);
    }
}
