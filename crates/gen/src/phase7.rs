//! Ф7: render-only racing centerline producer (design doc §2 line 191:
//! `centerline = racing_line(medial_axis(D))`).
//!
//! [`medial_axis`] deliberately leaves a *thin but imperfect* ridge cell set:
//! even-width 2-cell bands unthinned, a diagonal gap at each rectilinear
//! corner, and spur branches on infield-finger / hairpin tracks (its own
//! rustdoc names these as `racing_line`'s job). [`racing_line`] turns that set
//! into one closed, arc-length-parameterised, `race_dir`-oriented loop:
//! bridge cross-component gaps (4-connectivity + [`supercover`]) → prune
//! degree-1 spurs → walk a straightest-continuation cycle anchored at
//! `gate`'s forward face → orient by integer shoelace winding vs `race_dir`
//! → resample by arc length → wraparound unit tangents. Every failure path
//! (empty medial axis, an unbridgeable gap, an empty post-prune core, or a
//! walk that cannot close) returns [`Centerline::default()`] — render-only,
//! `Centerline::at` already degrades gracefully on an empty centerline; this
//! producer never panics.

use gp_core::geom::{Corridor, DistanceTransform, medial_axis};
use gp_core::track::{Centerline, RaceDir, TimingGate};

/// Produces the render-only racing centerline for corridor `d` (design doc §2
/// line 191).
///
/// Computes the distance transform + medial axis internally, trims and
/// orders the result into a single closed loop anchored at `gate`'s forward
/// face and oriented along `race_dir`, then resamples it by arc length.
/// Never panics: every failure path (empty medial axis and — once wired —
/// every later-stage fallback) returns [`Centerline::default()`], which
/// degrades gracefully under [`Centerline::at`].
pub fn racing_line(d: &Corridor, _gate: &TimingGate, _race_dir: RaceDir) -> Centerline {
    let dt = DistanceTransform::compute(d);
    let medial = medial_axis(&dt);
    if medial.is_empty() {
        return Centerline::default();
    }
    // Subtasks 2-5 wire the rest of the pipeline; until then every non-empty
    // medial axis also falls back (no producer overclaims yet).
    Centerline::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::Point;
    use gp_core::track::TimingGate;

    /// Subtask 1: an empty corridor (no drivable cells) has an empty medial
    /// axis, so `racing_line` falls back to `Centerline::default()` — empty
    /// samples, zero length, and `at` returning `None`.
    #[test]
    fn empty_corridor_yields_default_centerline() {
        let d = Corridor::new(Point::new(0, 0), 4, 4);
        let gate = TimingGate {
            behind: vec![],
            forward: gp_core::geom::Side::East,
        };
        let cl = racing_line(&d, &gate, RaceDir::Ccw);
        assert!(cl.samples.is_empty());
        assert!(cl.length.abs() < f32::EPSILON);
        assert!(cl.at(0.0).is_none());
    }
}
