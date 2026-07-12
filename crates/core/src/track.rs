//! The exported track artifact (design doc §2, Ф7) — the contract produced by
//! block 1 (generation) and consumed by blocks 2 (render), 3a (physics) and
//! 4 (AI).

use crate::geom::{Corridor, Orient, Point, Wall};

/// Global traversal orientation of the ring, fixed during generation (design
/// doc §2, Ф1). Everything downstream — the lap counter, AI progress/reward,
/// the ideal line — is oriented by this.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RaceDir {
    /// Clockwise.
    Cw,
    /// Counter-clockwise.
    Ccw,
}

/// The start/finish line — a full chord cutting the annulus into a simply
/// connected strip (design doc §3, lap counter). Being a *full* chord is what
/// makes the signed-crossing lap counter provably sufficient.
#[derive(Clone, Debug)]
pub struct StartFinish {
    /// The drivable points forming the chord across the corridor.
    pub chord: Vec<Point>,
    /// Chord orientation across the corridor (H or V).
    pub orient: Orient,
    // TODO(1): the exact gate segment(s) used for signed-crossing detection.
}

/// One sample of the parameterized centerline.
#[derive(Clone, Copy, Debug)]
pub struct CenterlineSample {
    /// Arc length from the start of the loop.
    pub s: f32,
    /// Sub-cell position of the centerline at `s`.
    pub pos: (f32, f32),
    /// Unit tangent, pointing along `race_dir`.
    pub tangent: (f32, f32),
}

/// The parameterized centerline (design doc §2) — a *first-class product* of
/// generation, not an internal Ф1 detail, because AI progress/reward and the
/// renderer's ideal line both depend on it.
///
/// `s` = distance along the track, grows along `race_dir`, and closes on itself.
#[derive(Clone, Debug, Default)]
pub struct Centerline {
    /// Ordered samples along the closed loop.
    pub samples: Vec<CenterlineSample>,
    /// Total loop length (used to normalize progress → track-invariance).
    pub length: f32,
}

impl Centerline {
    /// Sample the centerline at arc length `s`, wrapping around the closed loop.
    ///
    /// TODO(1): interpolate between the nearest samples.
    pub fn at(&self, _s: f32) -> Option<CenterlineSample> {
        todo!("centerline sampling (design doc §2)")
    }
}

/// Speed metrics derived by the passability oracle (design doc §3). Not inputs
/// to generation — *outputs* of it, produced almost for free on top of the
/// forward∩backward reachable set.
#[derive(Clone, Debug, Default)]
pub struct TrackMetrics {
    /// Peak attainable speed `Vmax_attain` (a poor scalar for "fastness").
    pub vmax_attain: Option<i32>,
    /// Lap tempo = lap length / move-count of the fastest lap (the honest one).
    pub tempo: Option<f32>,
    /// The path of the fastest lap.
    pub fastest_lap: Vec<Point>,
    /// Per-point max speed across live states — the where's-fast/slow heatmap.
    pub speed_heatmap: Vec<(Point, i32)>,
}

/// The full exported track artifact (design doc §2, Ф7).
#[derive(Clone, Debug)]
pub struct TrackArtifact {
    /// The corridor `D` — the set of drivable points.
    pub corridor: Corridor,
    /// Walls = dual edges on the boundary of `D`.
    pub walls: Vec<Wall>,
    /// Start/finish line.
    pub sf: StartFinish,
    /// Global traversal orientation.
    pub race_dir: RaceDir,
    /// Parameterized centerline.
    pub centerline: Centerline,
    /// Oracle-derived speed metrics.
    pub metrics: TrackMetrics,
}
