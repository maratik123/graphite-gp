//! The per-track baked geometry (design `2026-07-22-cache-track-geometry`):
//! builds, once per track, the chained/Chaikin-smoothed wall loops, their
//! outer/hole role split, and each loop's triangulation **topology** in
//! **lattice** space — rect-free. The triangle topology is a pure function
//! of a track's lattice-space silhouette and is invariant under the render
//! rect (`TrackTransform::map` is a plain affine map that only repositions
//! vertices, never re-tiles the polygon), so it is never rebuilt on resize:
//! the per-frame draw path only maps the baked verts through the current
//! rect's `TrackTransform` (`O(n)`), never re-runs the `O(n^3)` ear-clipping
//! pipeline.
//!
//! **Miri:** this module's tests build no `egui::Context` and drive no
//! painter — pure lattice-space geometry over a hand-built `TrackArtifact`
//! fixture — so none of them carry the Miri gate (design § Test Design,
//! subtask 3), mirroring the former `cache.rs`'s pure posture.

use super::regions::{self, LoopRoles};
use super::walls;
use gp_core::track::TrackArtifact;

/// The baked, rect-free lattice-space geometry for one track (design §
/// *The type*).
///
/// Holds the Chaikin-smoothed wall loops, their outer/hole role split, and
/// each loop's triangulated topology, all built once per track and reused
/// every frame regardless of the render rect. Every field is `pub(crate)` —
/// readable by in-crate draw code and tests, not part of the public API (the
/// opaque type name is the only public surface, per design § *The type*).
#[derive(Debug)]
pub struct BakedTrackGeometry {
    /// The chained, Chaikin-smoothed wall loops (lattice space) — the wall
    /// stroke's own input, and the per-frame map's source verts.
    pub(crate) smoothed_loops: Vec<Vec<(f32, f32)>>,
    /// The outer/hole role split over `smoothed_loops`.
    pub(crate) loop_roles: LoopRoles,
    /// Each `smoothed_loops` entry's triangulated **topology**, parallel to
    /// `smoothed_loops` by index. No screen-space vertices are stored — the
    /// per-frame draw path maps `smoothed_loops` through the current rect's
    /// `TrackTransform` and pairs the result with these borrowed indices.
    pub(crate) triangulated_indices: Vec<Vec<[u32; 3]>>,
}

impl BakedTrackGeometry {
    /// Builds the baked geometry for `track` — runs the same pipeline
    /// `draw_frame` used to run inline every frame: `walls::chain_walls` →
    /// `walls::chaikin_smooth` (per loop) → `regions::classify_loops` →
    /// `regions::triangulate_lattice` (per loop), rect-free. Deterministic
    /// over the identical `track` input — no rect input exists to vary, so
    /// every produced index list is bit-identical across calls (AC1).
    #[must_use]
    pub fn new(track: &TrackArtifact) -> Self {
        let wall_loops = walls::chain_walls(&track.walls);
        let smoothed_loops: Vec<Vec<(f32, f32)>> = wall_loops
            .iter()
            .map(|loop_corners| walls::chaikin_smooth(&track.corridor, loop_corners))
            .collect();

        let loop_roles = regions::classify_loops(&smoothed_loops);
        let triangulated_indices: Vec<Vec<[u32; 3]>> = smoothed_loops
            .iter()
            .map(|loop_points| regions::triangulate_lattice(loop_points))
            .collect();

        Self {
            smoothed_loops,
            loop_roles,
            triangulated_indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BakedTrackGeometry;
    use crate::track::test_support::ring_track;
    use gp_core::track::TrackArtifact;

    /// A minimal `TrackArtifact` over the shared `ring_3x3` corridor fixture
    /// — delegates to the single [`ring_track`] definition in `test_support`.
    fn fixture_track() -> TrackArtifact {
        ring_track()
    }

    /// AC1 — `new` on the ring fixture yields 2 smoothed loops, a
    /// `loop_roles` with exactly 1 outer + 1 hole, 2 non-empty triangulated
    /// index lists, each a well-formed triangulation (`len == loop.len() -
    /// 2`).
    #[test]
    fn new_produces_ring_loops_roles_and_indices() {
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);

        assert_eq!(geometry.smoothed_loops.len(), 2);
        assert_eq!(geometry.loop_roles.outer.len(), 1);
        assert_eq!(geometry.loop_roles.holes.len(), 1);
        assert_eq!(geometry.triangulated_indices.len(), 2);
        for (loop_points, indices) in geometry
            .smoothed_loops
            .iter()
            .zip(&geometry.triangulated_indices)
        {
            assert!(!indices.is_empty(), "expected a non-empty index list");
            assert_eq!(
                indices.len(),
                loop_points.len() - 2,
                "expected a well-formed ear-clipping triangulation"
            );
        }
    }

    /// AC1 (rect-independence) — `new(&track)` called twice yields equal
    /// `triangulated_indices`: the topology is a pure function of the track,
    /// with no rect input to vary (design `2026-07-22-cache-track-geometry`
    /// § *The pivot*).
    #[test]
    fn new_is_deterministic() {
        let track = fixture_track();
        let first = BakedTrackGeometry::new(&track);
        let second = BakedTrackGeometry::new(&track);
        assert_eq!(first.triangulated_indices, second.triangulated_indices);
    }
}
