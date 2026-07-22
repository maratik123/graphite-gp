//! The per-track, per-rect geometry cache (design
//! `2026-07-22-cache-track-geometry`): memoizes the screen-space
//! triangulation [`super::draw_frame`] used to re-cut every frame, so a
//! steady-state (unchanging canvas rect) frame reuses the same meshes
//! instead of re-running the `O(n^3)` ear-clipping pipeline.
//!
//! **Miri:** this module's tests build no `egui::Context` and drive no
//! painter — pure lattice/screen-space geometry over a hand-built
//! `TrackArtifact` fixture — so none of them carry the Miri gate (design §
//! Test Design, subtask 1).

use super::regions::{self, LoopRoles};
use super::{TrackTransform, walls};
use egui::{Pos2, Rect};
use gp_core::track::TrackArtifact;

/// The cached, rect-keyed screen-space geometry for one track (design §
/// *The cache type*).
///
/// Holds the Chaikin-smoothed wall loops, their outer/hole role split, and
/// each loop's triangulated `(verts, indices)` mesh, all built once per
/// distinct `(track, rect)` and reused every frame the rect stays unchanged.
/// Every field is `pub(crate)` — readable by in-crate draw code and tests,
/// not part of the public API (the opaque type name is the only public
/// surface, per design § *The cache type*).
#[derive(Debug)]
pub struct TrackGeometryCache {
    /// The rect this cache was built for — the staleness key
    /// [`TrackGeometryCache::get_or_build`] compares against.
    pub(crate) rect: Rect,
    /// The rect-dependent transform every layer maps through.
    pub(crate) transform: TrackTransform,
    /// The chained, Chaikin-smoothed wall loops (lattice space) — the wall
    /// stroke's own input.
    pub(crate) smoothed_loops: Vec<Vec<(f32, f32)>>,
    /// The outer/hole role split over `smoothed_loops`.
    pub(crate) loop_roles: LoopRoles,
    /// Each `smoothed_loops` entry's triangulated screen mesh, parallel to
    /// `smoothed_loops` by index.
    pub(crate) triangulated: Vec<(Vec<Pos2>, Vec<[u32; 3]>)>,
}

impl TrackGeometryCache {
    /// Builds the cached geometry for `track` at `rect` — runs the same
    /// pipeline `draw_frame` used to run inline every frame:
    /// `walls::chain_walls` → `walls::chaikin_smooth` (per loop) →
    /// `regions::classify_loops` → `regions::triangulated_loop` (per loop),
    /// plus `TrackTransform::new(&track.corridor, rect)`. Deterministic over
    /// the identical `(corridor, rect)` input, so every produced mesh is
    /// bit-identical to the former per-frame path (AC6).
    pub(crate) fn build(track: &TrackArtifact, rect: Rect) -> Self {
        let transform = TrackTransform::new(&track.corridor, rect);

        let wall_loops = walls::chain_walls(&track.walls);
        let smoothed_loops: Vec<Vec<(f32, f32)>> = wall_loops
            .iter()
            .map(|loop_corners| walls::chaikin_smooth(&track.corridor, loop_corners))
            .collect();

        let loop_roles = regions::classify_loops(&smoothed_loops);
        let triangulated: Vec<(Vec<Pos2>, Vec<[u32; 3]>)> = smoothed_loops
            .iter()
            .map(|loop_points| regions::triangulated_loop(&transform, loop_points))
            .collect();

        Self {
            rect,
            transform,
            smoothed_loops,
            loop_roles,
            triangulated,
        }
    }

    /// Returns the cached geometry for `(track, rect)`, rebuilding into
    /// `slot` only when it is empty or was built for a different `rect`
    /// (design § *The cache type* — memo key = `(caller-reset presence,
    /// rect)`; the track-identity half is delegated to the caller resetting
    /// `slot` to `None` on a track swap). Total — never panics.
    pub(crate) fn get_or_build<'c>(
        slot: &'c mut Option<Self>,
        track: &TrackArtifact,
        rect: Rect,
    ) -> &'c Self {
        if slot.as_ref().is_none_or(|cache| cache.rect != rect) {
            *slot = None;
        }
        slot.get_or_insert_with(|| Self::build(track, rect))
    }
}

#[cfg(test)]
mod tests {
    use super::TrackGeometryCache;
    use crate::track::test_support::ring_3x3;
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::walls_from_boundary;
    use gp_core::geom::{Orient, Point, Side};
    use gp_core::track::{RaceDir, StartFinish, TimingGate, TrackArtifact};

    /// A minimal `TrackArtifact` over the shared `ring_3x3` corridor fixture
    /// — every field `build` does not read stays at its cheapest valid
    /// default (mirrors `track/mod.rs::tests::fixture_track`).
    fn fixture_track() -> TrackArtifact {
        let corridor = ring_3x3();
        let walls = walls_from_boundary(&corridor);
        TrackArtifact {
            walls,
            sf: StartFinish {
                chord: vec![Point::new(1, 1), Point::new(2, 1), Point::new(3, 1)],
                orient: Orient::Horizontal,
                gate: TimingGate {
                    behind: vec![],
                    forward: Side::East,
                },
            },
            corridor,
            race_dir: RaceDir::Cw,
            s_field: gp_core::track::SField::default(),
            start_grid: gp_core::track::StartGrid::default(),
            centerline: gp_core::track::Centerline::default(),
            metrics: gp_core::track::TrackMetrics::default(),
            width_min: 1,
        }
    }

    /// AC1 — `build` on the ring fixture yields 2 smoothed loops, a
    /// `loop_roles` with exactly 1 outer + 1 hole, 2 non-empty triangulated
    /// meshes, and stores the exact build rect.
    #[test]
    fn build_produces_ring_loops_and_meshes() {
        let track = fixture_track();
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let cache = TrackGeometryCache::build(&track, rect);

        assert_eq!(cache.rect, rect);
        assert_eq!(cache.smoothed_loops.len(), 2);
        assert_eq!(cache.loop_roles.outer.len(), 1);
        assert_eq!(cache.loop_roles.holes.len(), 1);
        assert_eq!(cache.triangulated.len(), 2);
        for (verts, indices) in &cache.triangulated {
            assert!(!verts.is_empty(), "expected a non-empty vertex list");
            assert!(!indices.is_empty(), "expected a non-empty index list");
        }
    }

    /// AC2 — a `get_or_build` hit (same rect) reuses the existing
    /// allocation: the `triangulated` buffer's pointer is unchanged, so the
    /// static pipeline ran zero times on the second call.
    #[test]
    fn get_or_build_hit_reuses_allocation() {
        let track = fixture_track();
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));

        let mut slot: Option<TrackGeometryCache> = None;
        let first_ptr = {
            let cache = TrackGeometryCache::get_or_build(&mut slot, &track, rect);
            cache.triangulated.as_ptr() as usize
        };

        let second_ptr = {
            let cache = TrackGeometryCache::get_or_build(&mut slot, &track, rect);
            cache.triangulated.as_ptr() as usize
        };

        assert_eq!(
            first_ptr, second_ptr,
            "a same-rect get_or_build call rebuilt the cache"
        );
    }

    /// AC5 — a `get_or_build` miss (rect change) rebuilds: the stored rect
    /// matches the new rect, and the first mesh's screen verts differ from
    /// the rect-A verts (a fresh transform was applied).
    #[test]
    fn get_or_build_miss_rebuilds_on_rect_change() {
        let track = fixture_track();
        let rect_a = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let rect_b = Rect::from_min_max(Pos2::ZERO, pos2(240.0, 200.0));

        let mut slot: Option<TrackGeometryCache> = None;
        let verts_a = {
            let cache = TrackGeometryCache::get_or_build(&mut slot, &track, rect_a);
            cache.triangulated[0].0.clone()
        };

        let cache_b = TrackGeometryCache::get_or_build(&mut slot, &track, rect_b);
        assert_eq!(cache_b.rect, rect_b);
        assert_ne!(
            cache_b.triangulated[0].0, verts_a,
            "rect change did not rebuild the screen mesh"
        );
    }
}
