//! Speed heatmap layer (design doc §4, layer 1b — analytics overlay):
//! colors each cell in `metrics.speed_heatmap` by its per-cell max speed on
//! the `HEAT_0` (slowest) → `HEAT_3` (fastest) ramp.
//!
//! **Amendment (2026-07-20, design § Key decisions 1):** the heatmap recolors
//! the *same* Chaikin-smoothed asphalt mesh [`regions::fill`] draws,
//! per cell, via `Painter::with_clip_rect` — not independent per-cell
//! squares — so its outer silhouette traces the smoothed boundary exactly
//! (no more blocky staircase poking past the walls at a corner). The outer
//! loop's triangulation topology is baked once per track (design
//! `2026-07-22-cache-track-geometry`, `track::BakedTrackGeometry`) and the
//! shared index buffer is reused for every cell's `Mesh`; the infield
//! hole(s) are re-cut on top of the whole per-cell pass
//! (`regions::paint_infield_holes`) so heatmap color never bleeds into the
//! infield.
//!
//! Pure geometry/color core ([`speed_bounds`], [`normalize`],
//! [`ramp_color`]) plus a thin [`paint`] that maps to screen via
//! [`TrackTransform`] — the crate's house pattern (design § *House
//! pattern*).
//!
//! **Miri:** the 2 `tests::painted_meshes`-driven tests below stand up an
//! `egui::Context` and run `paint` through a `run_ui` pass, so they carry
//! `#[cfg_attr(miri, ignore = "…")]` (design
//! `2026-07-21-miri-gate-render-tests`) — wall-clock cost, not an abort. The
//! `speed_bounds_*`/`normalize_*`/`ramp_color_*` pure-logic tests build no
//! `Context` and stay un-gated.

use super::TrackTransform;
use super::regions::{self, LoopRoles};
use egui::{Color32, Painter, Pos2, Rect};
use gp_core::geom::Point;

/// Heatmap fill alpha: the opaque `ASPHALT_1` mesh reads ~10% through
/// (design § Key decisions 1; `Track.jsx:58` tints the asphalt path at
/// opacity `0.9`).
const HEATMAP_ALPHA: f32 = 0.9;

/// The observed `(min, max)` speed across `heatmap`'s values; `None` for an
/// empty slice — the AC7 no-op signal.
pub(crate) fn speed_bounds(heatmap: &[(Point, i32)]) -> Option<(i32, i32)> {
    let mut speeds = heatmap.iter().map(|&(_, speed)| speed);
    let first = speeds.next()?;
    Some(speeds.fold((first, first), |(min, max), speed| {
        (min.min(speed), max.max(speed))
    }))
}

/// Normalizes `speed` into `[0, 1]` across the observed `[min, max]` range
/// (design § Key decisions 2). `range = max.saturating_sub(min)` (never a raw
/// subtraction, since `arithmetic_side_effects` is deny); `range == 0` (a
/// single distinct value, or `min > max`) maps to `0.0` (all `HEAT_0`).
/// `missing_const_for_fn` (nursery, deny) is the authority making this a
/// `const fn`: `saturating_sub` plus a cast, an f32 div, and a branch, with
/// no fused-multiply pattern — precedent `sf.rs::bar_rect_lattice`.
#[allow(
    clippy::cast_precision_loss,
    reason = "speed values are grid-realistic i32s, far below f32's exact-integer \
              range; precedent: gp-core track.rs::normalize"
)]
const fn normalize(speed: i32, min: i32, max: i32) -> f32 {
    let range = max.saturating_sub(min);
    if range == 0 {
        return 0.0;
    }
    speed.saturating_sub(min) as f32 / range as f32
}

/// Blends one `u8` color channel linearly between `a` (at `t=0`) and `b` (at
/// `t=1`); `t` is expected `∈ [0, 1]` (callers clamp).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "t is clamped to [0, 1] by ramp_color and a/b are u8 channel bytes, so \
              the blended result stays within [0, 255] — precedent: walls.rs::point_in_drivable"
)]
fn blend_channel(a: u8, b: u8, t: f32) -> u8 {
    (f32::from(b) - f32::from(a)).mul_add(t, f32::from(a)) as u8
}

/// Maps `t` onto the 4-stop `HEAT_RAMP` at uniform stops `(0, 1/3, 2/3, 1)`
/// via piecewise-linear per-channel blend between the two bracketing stops
/// (design § Key decisions 2). `t` is clamped to `[0, 1]` first — never a
/// panic or out-of-range ramp index. Endpoints are exact: `ramp_color(0.0)
/// == HEAT_0`, `ramp_color(1.0) == HEAT_3` (the lerp reduces to the endpoint
/// channel byte with no rounding at the stop itself).
///
/// Not `const fn`: `f32::mul_add` (in [`blend_channel`]) is not `const`, so
/// `missing_const_for_fn` correctly does not fire here (design § Lint
/// posture).
fn ramp_color(t: f32) -> Color32 {
    let ramp = crate::tokens::color::HEAT_RAMP;
    let last = ramp.len().saturating_sub(1);
    let clamped = t.clamp(0.0, 1.0);
    #[allow(
        clippy::cast_precision_loss,
        reason = "ramp.len() is the fixed, tiny HEAT_RAMP stop count; precedent: \
                  gp-core track.rs::normalize"
    )]
    let scaled = clamped * last as f32;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "scaled is clamped into [0, last] above, so floor() lands within \
                  the ramp's own index range; precedent: walls.rs::point_in_drivable"
    )]
    let lo = (scaled.floor() as usize).min(last);
    let hi = lo.saturating_add(1).min(last);
    #[allow(
        clippy::cast_precision_loss,
        reason = "lo is a bounded ramp index far below f32's exact-integer range; \
                  precedent: gp-core track.rs::normalize"
    )]
    let frac = scaled - lo as f32;
    let (a, b) = (ramp[lo], ramp[hi]);
    Color32::from_rgb(
        blend_channel(a.r(), b.r(), frac),
        blend_channel(a.g(), b.g(), frac),
        blend_channel(a.b(), b.b(), frac),
    )
}

/// The lattice-space `(min, max)` corners of the full ±0.5 cell rect
/// centered on `point` — mirrors `sf::bar_rect`'s "full cell" extent, no
/// thinning.
#[allow(
    clippy::cast_precision_loss,
    reason = "cell coordinates are grid-realistic i32s, far below f32's exact-integer \
              range; precedent: gp-core track.rs::normalize"
)]
fn cell_rect_lattice(point: Point) -> ((f32, f32), (f32, f32)) {
    let (fx, fy) = (point.x as f32, point.y as f32);
    ((fx - 0.5, fy - 0.5), (fx + 0.5, fy + 0.5))
}

/// The screen-space cell rect centered on lattice point `p` (see
/// [`cell_rect_lattice`]).
fn cell_rect(transform: &TrackTransform, p: Point) -> Rect {
    let (min, max) = cell_rect_lattice(p);
    Rect::from_two_pos(transform.map(min), transform.map(max))
}

/// Paints the speed heatmap (design § Key decisions 1, amended 2026-07-20;
/// design `2026-07-22-cache-track-geometry` — consumes the parallel baked
/// `indices`/per-frame `verts` slices, no per-frame re-triangulation, AC3).
///
/// For each `(Point, i32)` in `heatmap`, builds a per-cell `Mesh` from each
/// `roles.outer` loop's parallel `(verts, indices)` slice entry, colored
/// by the `ramp_color` mapping its speed's [`normalize`]d position across the
/// observed `[min, max]` at [`HEATMAP_ALPHA`], clipped to the cell's rect via
/// `Painter::with_clip_rect` — so the union over all cells is the smoothed
/// asphalt silhouette, colored per cell, never re-triangulated per cell (or
/// per frame — `indices` is baked once by [`super::geometry`] and reused,
/// borrowed; only `verts` is freshly mapped per frame). After the full
/// per-cell pass, `roles.holes` is re-cut as a `SURFACE_INFIELD` mesh via
/// [`regions::paint_infield_holes`] (mirrors `regions::fill`'s own
/// asphalt-then-infield structure) so heatmap color never bleeds into the
/// infield. An empty `heatmap` — or one whose [`speed_bounds`] is `None` —
/// returns **before** the infield re-cut, so the empty case stays a true
/// no-op (AC7): zero shapes, not even the re-cut.
///
/// `verts`, `indices`, and `roles` must come from the same per-frame map +
/// baked-geometry pair `draw_frame` also passes to `regions::fill`.
pub(crate) fn paint(
    painter: &Painter,
    transform: &TrackTransform,
    verts: &[Vec<Pos2>],
    indices: &[Vec<[u32; 3]>],
    roles: &LoopRoles,
    heatmap: &[(Point, i32)],
) {
    let Some((min, max)) = speed_bounds(heatmap) else {
        return;
    };

    for &(point, speed) in heatmap {
        let t = normalize(speed, min, max);
        let color = ramp_color(t).gamma_multiply(HEATMAP_ALPHA);
        let clip = painter.with_clip_rect(cell_rect(transform, point));
        for &idx in &roles.outer {
            if let (Some(v), Some(i)) = (verts.get(idx), indices.get(idx)) {
                regions::paint_mesh(&clip, v, i, color);
            }
        }
    }

    regions::paint_infield_holes(
        painter,
        verts,
        indices,
        roles,
        crate::tokens::color::SURFACE_INFIELD,
    );
}

#[cfg(test)]
mod tests {
    use super::super::regions::{self, LoopRoles};
    use super::super::walls;
    use super::{TrackTransform, normalize, paint, ramp_color, speed_bounds};
    use crate::test_util::assert_f32;
    use crate::tokens::color::{HEAT_0, HEAT_1, HEAT_3, SURFACE_INFIELD};
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::{Corridor, Point, walls_from_boundary};

    /// AC1 — `speed_bounds` of an empty slice is `None`.
    #[test]
    fn speed_bounds_of_empty_is_none() {
        assert_eq!(speed_bounds(&[]), None);
    }

    /// AC1 — `speed_bounds` finds the observed `(min, max)` regardless of
    /// input order.
    #[test]
    fn speed_bounds_finds_min_and_max() {
        let heatmap = vec![
            (Point::new(0, 0), 5),
            (Point::new(1, 0), 1),
            (Point::new(2, 0), 9),
            (Point::new(3, 0), 4),
        ];
        assert_eq!(speed_bounds(&heatmap), Some((1, 9)));
    }

    /// AC1 — `normalize` maps `min` to `0.0` (the `HEAT_0` end) and `max` to
    /// `1.0` (the `HEAT_3` end).
    #[test]
    fn normalize_maps_min_and_max_to_ramp_ends() {
        assert_f32("normalize(min)", normalize(2, 2, 8), 0.0);
        assert_f32("normalize(max)", normalize(8, 2, 8), 1.0);
    }

    /// AC1 — `normalize` is monotone: a larger speed never normalizes to a
    /// smaller `t`.
    #[test]
    fn normalize_is_monotone() {
        let (min, max) = (0, 10);
        let mut prev = normalize(min, min, max);
        for speed in (min + 1)..=max {
            let t = normalize(speed, min, max);
            assert!(t >= prev, "t={t} prev={prev} speed={speed}");
            prev = t;
        }
    }

    /// Degenerate — `max == min` (a single distinct value) normalizes every
    /// speed to `0.0` (all `HEAT_0`), never a divide-by-zero panic.
    #[test]
    fn normalize_degenerate_range_is_zero() {
        assert_f32("normalize(equal min/max)", normalize(5, 5, 5), 0.0);
        assert_f32("normalize(min > max)", normalize(0, 5, 5), 0.0);
    }

    /// AC1 — `ramp_color` at the exact stops returns the stop colors
    /// unchanged (no rounding at the endpoint).
    #[test]
    fn ramp_color_endpoints_are_exact() {
        assert_eq!(ramp_color(0.0), HEAT_0);
        assert_eq!(ramp_color(1.0), HEAT_3);
    }

    /// AC1 — an intermediate `t` blends (differs from both bracketing
    /// stops), not a hard snap.
    #[test]
    fn ramp_color_blends_between_stops() {
        let mid = ramp_color(1.0 / 6.0);
        assert_ne!(mid, HEAT_0);
        assert_ne!(mid, HEAT_1);
    }

    /// Edge — `t` outside `[0, 1]` is clamped, never a panic or OOB ramp
    /// index.
    #[test]
    fn ramp_color_clamps_out_of_range_t() {
        assert_eq!(ramp_color(-1.0), HEAT_0);
        assert_eq!(ramp_color(2.0), HEAT_3);
    }

    /// The ring fixture's chained, Chaikin-smoothed wall loops + outer/hole
    /// role split — exactly what `draw_frame` computes (`mod.rs`) before
    /// calling `heatmap::paint`.
    fn ring_3x3_loops_and_roles() -> (Corridor, Vec<Vec<(f32, f32)>>, LoopRoles) {
        let d = crate::track::test_support::ring_3x3();
        let boundary = walls_from_boundary(&d);
        let loops: Vec<Vec<(f32, f32)>> = walls::chain_walls(&boundary)
            .iter()
            .map(|corners| walls::chaikin_smooth(&d, corners))
            .collect();
        let roles = regions::classify_loops(&loops);
        (d, loops, roles)
    }

    /// Maps every `loops` entry via `transform` — exactly the per-frame map
    /// `draw_frame` now feeds `heatmap::paint` alongside the baked `indices`
    /// (design `2026-07-22-cache-track-geometry`).
    fn map_loops(loops: &[Vec<(f32, f32)>], transform: &TrackTransform) -> Vec<Vec<Pos2>> {
        loops
            .iter()
            .map(|loop_points| loop_points.iter().map(|&p| transform.map(p)).collect())
            .collect()
    }

    /// Triangulates every `loops` entry in lattice space — exactly the baked
    /// `indices` computation `draw_frame` now feeds `heatmap::paint`
    /// alongside the per-frame `verts` map (design
    /// `2026-07-22-cache-track-geometry`).
    fn triangulate_loops(loops: &[Vec<(f32, f32)>]) -> Vec<Vec<[u32; 3]>> {
        loops
            .iter()
            .map(|loop_points| regions::triangulate_lattice(loop_points))
            .collect()
    }

    /// Renders `paint` alone into a fresh frame and returns the captured
    /// `Mesh` shapes — mirrors `regions.rs`'s
    /// `fill_emits_asphalt_mesh_then_infield_mesh` capture idiom.
    fn painted_meshes(
        verts: &[Vec<Pos2>],
        indices: &[Vec<[u32; 3]>],
        roles: &LoopRoles,
        transform: &TrackTransform,
        rect: Rect,
        heatmap: &[(Point, i32)],
    ) -> Vec<std::sync::Arc<egui::Mesh>> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            paint(&painter, transform, verts, indices, roles, heatmap);
        });
        crate::track::test_support::captured_meshes(&output.shapes)
    }

    /// AC1 — a `K`-cell hand-populated heatmap over the ring fixture emits
    /// `K * roles.outer.len() + H` meshes (`K` per-cell clipped outer-asphalt
    /// meshes + `H` infield re-cut meshes); the infield re-cut mesh is
    /// `SURFACE_INFIELD`-colored, and the first cell's mesh first-vertex
    /// color equals the expected ramp color (design § Key decisions 1,
    /// amended 2026-07-20).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "painted_meshes drives a Context::run_ui + layer_painter \
                  pass through heatmap::paint, capturing per-cell meshes — \
                  interpreted-pass wall-clock cost, not an abort"
    )]
    fn paint_emits_per_cell_meshes_plus_infield_recut() {
        let (d, loops, roles) = ring_3x3_loops_and_roles();
        assert_eq!(
            roles.outer.len(),
            1,
            "ring fixture must have one outer loop"
        );
        assert_eq!(roles.holes.len(), 1, "ring fixture must have one hole");

        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let transform = TrackTransform::new(&d, rect);
        let verts = map_loops(&loops, &transform);
        let indices = triangulate_loops(&loops);

        let heatmap = vec![
            (Point::new(1, 1), 2),
            (Point::new(2, 1), 5),
            (Point::new(1, 3), 8),
        ];
        let meshes = painted_meshes(&verts, &indices, &roles, &transform, rect, &heatmap);
        assert_eq!(
            meshes.len(),
            heatmap.len() * roles.outer.len() + roles.holes.len(),
            "expected K per-cell meshes + H infield re-cut meshes"
        );

        // The infield re-cut mesh(es) are drawn after the full per-cell pass.
        let infield_start = heatmap.len() * roles.outer.len();
        for mesh in &meshes[infield_start..] {
            assert_eq!(mesh.vertices[0].color, SURFACE_INFIELD);
        }

        // The first cell's mesh is colored by its own ramp position.
        let (min, max) = speed_bounds(&heatmap).expect("non-empty heatmap has bounds");
        let expected = ramp_color(normalize(2, min, max)).gamma_multiply(super::HEATMAP_ALPHA);
        assert_eq!(meshes[0].vertices[0].color, expected);
    }

    /// AC7 — an empty heatmap draws no shapes at all, not even the infield
    /// re-cut (the `speed_bounds`-`None` early return precedes it).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "painted_meshes drives a Context::run_ui + layer_painter \
                  pass through heatmap::paint, capturing per-cell meshes — \
                  interpreted-pass wall-clock cost, not an abort"
    )]
    fn paint_is_noop_on_empty_heatmap() {
        let (d, loops, roles) = ring_3x3_loops_and_roles();
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let transform = TrackTransform::new(&d, rect);
        let verts = map_loops(&loops, &transform);
        let indices = triangulate_loops(&loops);
        let meshes = painted_meshes(&verts, &indices, &roles, &transform, rect, &[]);
        assert!(meshes.is_empty());
    }

    /// AC3 — `paint` consumes the baked, borrowed `indices` outer mesh rather
    /// than re-triangulating it: across a full per-cell heatmap pass the O(n³)
    /// ear-clip runs **zero** times (design `2026-07-22-cache-track-geometry`
    /// — the topology is baked once, reused for every cell). The load-bearing
    /// assertion is the `regions::triangulate` call counter, reset after the
    /// loops are triangulated once above; a regression that re-cut the outer
    /// mesh per cell (or per call) would bump it even though the borrowed
    /// `indices` `&` pointer would stay put. The pointer check is kept as a
    /// cheap secondary guard.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "painted_meshes drives a Context::run_ui + layer_painter \
                  pass through heatmap::paint, capturing per-cell meshes — \
                  interpreted-pass wall-clock cost, not an abort"
    )]
    fn heatmap_reuses_cached_outer_mesh() {
        let (d, loops, roles) = ring_3x3_loops_and_roles();
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let transform = TrackTransform::new(&d, rect);
        let verts = map_loops(&loops, &transform);
        let indices = triangulate_loops(&loops);

        let outer_idx = roles.outer[0];
        let indices_ptr_before = indices[outer_idx].as_ptr();

        // The `triangulate_loops` bake above ear-clipped once per loop; zero
        // the counter so the heatmap pass below is measured on its own.
        regions::reset_triangulate_calls();
        let heatmap = vec![(Point::new(1, 1), 2), (Point::new(2, 1), 5)];
        let _ = painted_meshes(&verts, &indices, &roles, &transform, rect, &heatmap);

        assert_eq!(
            regions::triangulate_calls(),
            0,
            "heatmap::paint re-triangulated instead of reusing the baked outer indices"
        );
        assert_eq!(
            indices[outer_idx].as_ptr(),
            indices_ptr_before,
            "heatmap::paint re-triangulated the outer mesh instead of reusing it"
        );
    }
}
