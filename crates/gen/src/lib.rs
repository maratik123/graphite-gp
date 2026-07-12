//! # gp-gen — Block 1: track generation (design doc §2)
//!
//! Approach **A** (coarse-block ring, infield-first) **+ D** (local repair): make
//! the track almost-valid by construction so the expensive passability oracle
//! acts as a certifier, not a regeneration engine. The pipeline runs in phases
//! Ф1–Ф7 and emits a [`TrackArtifact`].

use gp_core::track::TrackArtifact;

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
    pub fn min_width(&self) -> u32 {
        self.cars.div_ceil(2)
    }

    /// Start/finish width floor `≥ m` — cars start abreast across the corridor.
    pub fn start_finish_width(&self) -> u32 {
        self.cars
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
