//! The exported track artifact (design doc §2, Ф7) — the contract produced by
//! block 1 (generation) and consumed by blocks 2 (render), 3a (physics) and
//! 4 (AI).

use crate::geom::{Corridor, Orient, Point, Side, Wall};

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

/// The exact timing-gate segment used for [`StartFinish`]'s signed-crossing
/// test (design doc §3, \[C2\]).
///
/// `behind` holds the *drivable* cross-section cells immediately behind the
/// gate; each implied dual edge is `{cell: behind[i], side: forward}` — one
/// edge ahead of the front row, spanning the cross-section. This is **not** a
/// [`Wall`] set: a gate edge sits between two *drivable* cells (`D`'s
/// interior), not on `D`'s `¬D` boundary — a future reader of
/// `LapCounter::register_move` must not conflate the two.
#[derive(Clone, Debug)]
pub struct TimingGate {
    /// The drivable cross-section cells immediately behind the gate.
    pub behind: Vec<Point>,
    /// The forward (`+race_dir`) side each `behind` cell's implied dual edge
    /// faces.
    pub forward: Side,
}

impl TimingGate {
    /// The unit `(f32, f32)` direction of `forward`.
    #[inline]
    pub const fn forward_unit(&self) -> (f32, f32) {
        side_unit_f32(self.forward)
    }

    /// Whether the dual edge between `a` and `b` is one of this gate's implied
    /// cut edges.
    ///
    /// Order-independent / symmetric: `separates(a, b) == separates(b, a)` for
    /// all `a`, `b` — a reversed-pair query (as a 4-neighbor gradient walk may
    /// issue) must report the same cut, or the barrier silently leaks the
    /// cross-cut jump into the gradient for one traversal direction.
    pub fn separates(&self, a: Point, b: Point) -> bool {
        let (dx, dy) = self.forward.delta();
        let ahead_of = |p: Point| -> Option<Point> {
            Some(Point::new(p.x.checked_add(dx)?, p.y.checked_add(dy)?))
        };
        self.behind.iter().any(|&behind| {
            ahead_of(behind)
                .is_some_and(|ahead| (behind == a && ahead == b) || (behind == b && ahead == a))
        })
    }
}

/// The unit `(f32, f32)` direction of `side`, via a literal `match` — no
/// numeric cast.
const fn side_unit_f32(side: Side) -> (f32, f32) {
    match side {
        Side::East => (1.0, 0.0),
        Side::West => (-1.0, 0.0),
        Side::North => (0.0, 1.0),
        Side::South => (0.0, -1.0),
    }
}

/// The start/finish line — a full chord cutting the annulus into a simply
/// connected strip (design doc §3, lap counter).
///
/// Being a *full* chord is what makes the signed-crossing lap counter provably
/// sufficient. `gate` carries the exact timing-gate segment(s) and forward
/// direction that `LapCounter::register_move`'s half-open signed-crossing test
/// needs.
#[derive(Clone, Debug)]
pub struct StartFinish {
    /// The drivable points forming the chord across the corridor.
    pub chord: Vec<Point>,
    /// Chord orientation across the corridor (H or V).
    pub orient: Orient,
    /// The exact timing-gate segment(s) used for the signed-crossing test.
    pub gate: TimingGate,
}

/// An ordered list of distinct start positions in the corridor `D`, each
/// implicitly at rest (`v = (0, 0)`).
///
/// Invariant (upheld by the generator, not enforced here): every `positions`
/// element is distinct, lies in `D`, and the list is ordered front-to-back
/// along `−race_dir` (design doc §2, Ф3, \[C2\]).
#[derive(Clone, Debug, Default)]
pub struct StartGrid {
    /// Distinct start positions, ordered front-to-back along `−race_dir`.
    pub positions: Vec<Point>,
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
    /// Samples the centerline at arc length `s`, wrapping around the closed loop.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TimingGate (subtask 1) --------------------------------------

    #[test]
    fn forward_unit_matches_each_side() {
        let gate = |forward| TimingGate {
            behind: vec![],
            forward,
        };
        assert_eq!(gate(Side::East).forward_unit(), (1.0, 0.0));
        assert_eq!(gate(Side::West).forward_unit(), (-1.0, 0.0));
        assert_eq!(gate(Side::North).forward_unit(), (0.0, 1.0));
        assert_eq!(gate(Side::South).forward_unit(), (0.0, -1.0));
    }

    #[test]
    fn separates_true_only_for_the_implied_cut_edge() {
        let gate = TimingGate {
            behind: vec![Point::new(1, 1)],
            forward: Side::East,
        };
        // The implied cut edge.
        assert!(gate.separates(Point::new(1, 1), Point::new(2, 1)));
        // Non-adjacent pair.
        assert!(!gate.separates(Point::new(1, 1), Point::new(3, 1)));
        // Lateral pair (not along `forward`).
        assert!(!gate.separates(Point::new(1, 1), Point::new(1, 2)));
        // A different, non-gate adjacent pair.
        assert!(!gate.separates(Point::new(0, 1), Point::new(1, 1)));
    }

    #[test]
    fn separates_is_symmetric() {
        let gate = TimingGate {
            behind: vec![Point::new(1, 1)],
            forward: Side::East,
        };
        let (a, b) = (Point::new(1, 1), Point::new(2, 1));
        assert!(gate.separates(a, b));
        assert!(gate.separates(b, a));
    }

    // ---- StartGrid (subtask 2) ----------------------------------------

    #[test]
    fn start_grid_preserves_front_to_back_order_and_distinctness() {
        // race_dir such that "front" is +x, so front-to-back is decreasing x.
        let grid = StartGrid {
            positions: vec![Point::new(5, 0), Point::new(4, 0), Point::new(3, 0)],
        };
        assert_eq!(
            grid.positions,
            vec![Point::new(5, 0), Point::new(4, 0), Point::new(3, 0)]
        );
        let distinct: std::collections::HashSet<_> = grid.positions.iter().collect();
        assert_eq!(distinct.len(), grid.positions.len());
    }

    #[test]
    fn start_grid_default_is_empty() {
        assert!(StartGrid::default().positions.is_empty());
    }
}
