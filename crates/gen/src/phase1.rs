//! Ф1 — coarse-block ring (infield-first), design doc §2.
//!
//! Produces the coarse skeleton `{ ring, hole, dir }` at **coarse-block**
//! granularity: `ring` is the annulus, `hole` is the enclosed infield polyomino
//! `P`, and `dir` is the fixed global traversal orientation. The `k×k` fine
//! expansion to the actual corridor `D` is Ф2 — out of scope here.

use std::collections::BTreeSet;

use gp_core::geom::Point;
use gp_core::track::RaceDir;
use rand_chacha::ChaCha8Rng;

/// The Ф1 output: a coarse annulus `ring` enclosing exactly one hole `hole`
/// (the infield polyomino `P`), plus the fixed traversal orientation.
#[derive(Clone, Debug)]
pub struct CoarseSkeleton {
    /// The coarse-block ring — `dilate_moore(P, 1) \ P`. Connected, exactly
    /// one hole (AC2).
    pub ring: BTreeSet<Point>,
    /// The enclosed infield polyomino `P` (the ring's one hole).
    pub hole: BTreeSet<Point>,
    /// The fixed global traversal orientation, stable across repeated
    /// same-seed calls (AC4).
    pub dir: RaceDir,
}

/// Builds a coarse-block ring skeleton (design doc §2 Ф1) for a fixed
/// `l_min` (minimum coarse-block straight length) and RNG stream.
///
/// Infallible: a bounded same-stream retry plus a guaranteed-terminating
/// rectangular fallback make this a total function — no `Result`, no panic.
pub fn phase1_coarse_ring(l_min: i32, rng: &mut ChaCha8Rng) -> CoarseSkeleton {
    let _ = (l_min, rng);
    todo!(
        "Ф1 pipeline: base strip + growth + hole-fill + dilate + widen + check/fallback + orientation (subtasks 6-7)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_skeleton_carries_ring_hole_and_dir() {
        // AC1: CoarseSkeleton is a plain data carrier for {ring, hole, dir}.
        let skeleton = CoarseSkeleton {
            ring: BTreeSet::from([Point::new(0, 0)]),
            hole: BTreeSet::from([Point::new(1, 1)]),
            dir: RaceDir::Cw,
        };
        assert!(skeleton.ring.contains(&Point::new(0, 0)));
        assert!(skeleton.hole.contains(&Point::new(1, 1)));
        assert_eq!(skeleton.dir, RaceDir::Cw);
    }
}
