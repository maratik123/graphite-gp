//! The track canvas (design doc §4): regions, walls, S/F, cars.
//!
//! Each layer is a submodule split into a pure lattice-space geometry fn
//! (Miri-clean, no `egui::Ui`, no allocation beyond a returned `Vec`) and a
//! thin `pub(crate) paint` fn that maps that geometry to screen space via
//! [`TrackTransform`] and strokes/fills it — the house pattern this crate's
//! sibling widgets already follow (design § *House pattern*).
//!
//! **Miri:** every `tests::render_shapes`-driven test below stands up an
//! `egui::Context` and runs a full-frame `run_ui` pass, so it carries
//! `#[cfg_attr(miri, ignore = "…")]` (design
//! `2026-07-21-miri-gate-render-tests`) — wall-clock cost under the
//! interpreter, not an abort (the helper only captures `output.shapes`,
//! never calls `tessellate`/`set_fonts`). `layer_order_matches_documented_names`
//! builds no `Context` and stays un-gated.

mod car;
mod fastest_lap;
mod geometry;
#[cfg(test)]
mod golden;
mod grid;
mod heatmap;
mod regions;
mod sf;
#[cfg(test)]
pub(crate) mod test_support;
mod transform;
mod walls;

pub use car::CarRender;
pub use geometry::BakedTrackGeometry;
pub use transform::TrackTransform;

use crate::Overlays;
use egui::{Painter, Rect};
use gp_core::track::TrackArtifact;
use strum::IntoEnumIterator;

/// The layers [`draw_frame`] draws, back-to-front (AC5/AC9, final documented
/// order): `regions` (which expands to [`regions::RegionLayer`]'s own
/// `outfield → asphalt → infield`) `→ heatmap → grid → walls → fastest-lap →
/// sf → cars`. [`draw_frame`] iterates
/// [`Layer::iter`](IntoEnumIterator::iter) and dispatches each variant
/// to its draw action, so this order **is** the draw order (no second,
/// separately-maintained sequence to drift from it) —
/// `layer_order_matches_documented_names` pins the flattened, 9-name list as
/// a tested contract.
#[derive(Clone, Copy, Debug, strum::EnumIter, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum Layer {
    /// The three regions (`outfield`, `asphalt`, `infield`) — see
    /// [`regions::RegionLayer`] for their own sub-order.
    Regions,
    /// The `speed_heatmap` analytics overlay (layer 1b, over the asphalt,
    /// design § Key decisions 1) — gated on `overlays.speed_heatmap`.
    Heatmap,
    /// The notebook-sheet grid overlay (layer 4, over the regions, design §
    /// Key decisions 4) — gated on `overlays.grid`.
    Grid,
    /// The Chaikin-smoothed, M6-guarded walls.
    Walls,
    /// The `fastest_lap` analytics overlay (layer 5, over the walls, design
    /// § Key decisions 3) — gated on `overlays.fastest_lap`. Documented name
    /// `fastest-lap` (kebab-case).
    FastestLap,
    /// The checkered S/F chord.
    Sf,
    /// Every car (trail, dot, velocity arrow, optional "you" ring).
    Cars,
}

/// Draws one frame of the track canvas (design doc §4) into `rect`, back to
/// front per [`Layer`]: the three regions (`regions::fill` — see
/// [`regions::RegionLayer`] for `outfield → asphalt → infield`), the
/// `speed_heatmap` analytics overlay
/// (layer 1b, over the asphalt, design § Key decisions 1), the notebook-
/// sheet `grid` overlay (layer 4, over the regions, design § Key decisions
/// 4), the Chaikin-smoothed, M6-guarded walls, the `fastest_lap` analytics
/// overlay (layer 5, over the walls, design § Key decisions 3), the
/// checkered S/F chord, then every car (trail, dot, velocity arrow,
/// optional "you" ring).
///
/// `overlays` drives which analytics/grid layers are drawn (design § Key
/// decisions) — each flag adds or removes exactly its own layer's shapes;
/// all-off reproduces the #17 baseline byte-for-byte (design § Draw order).
///
/// `geometry` is the pre-baked, rect-free geometry for `track` (design
/// `2026-07-22-cache-track-geometry`) — its triangulation topology is never
/// recomputed here; only the cheap `O(n)` lattice→screen vertex map runs
/// per frame, via the rect-dependent [`TrackTransform`] this fn builds.
pub(crate) fn draw_frame(
    painter: &Painter,
    rect: Rect,
    track: &TrackArtifact,
    geometry: &BakedTrackGeometry,
    cars: &[CarRender<'_>],
    reduced_motion: bool,
    overlays: Overlays,
) {
    let transform = TrackTransform::new(&track.corridor, rect);

    // Amendment — Rounded track (PR #100): fill and stroke share the exact
    // same smoothed loops, so they cannot disagree at a corner by
    // construction (design § Decision, "Boundary reuse"). Map-on-the-fly
    // (design `2026-07-22-cache-track-geometry` § *Per-frame draw path*):
    // the baked `smoothed_loops` are mapped through this frame's `transform`
    // once and shared by `fill`/`heatmap::paint`; `triangulated_indices` is
    // borrowed straight from `geometry`, never recomputed or cloned.
    let mapped: Vec<Vec<egui::Pos2>> = geometry
        .smoothed_loops
        .iter()
        .map(|loop_points| loop_points.iter().map(|&p| transform.map(p)).collect())
        .collect();

    for layer in Layer::iter() {
        match layer {
            Layer::Regions => {
                regions::fill(
                    painter,
                    rect,
                    &mapped,
                    &geometry.triangulated_indices,
                    &geometry.loop_roles,
                );
            }
            Layer::Heatmap if overlays.speed_heatmap => {
                heatmap::paint(
                    painter,
                    &transform,
                    &mapped,
                    &geometry.triangulated_indices,
                    &geometry.loop_roles,
                    &track.metrics.speed_heatmap,
                );
            }
            Layer::Grid if overlays.grid => {
                grid::paint(painter, rect, &transform);
            }
            Layer::Walls => {
                walls::paint(painter, &transform, &geometry.smoothed_loops);
            }
            Layer::FastestLap if overlays.fastest_lap => {
                fastest_lap::paint(painter, &transform, &track.metrics.fastest_lap);
            }
            Layer::Sf => {
                let checker = sf::checker_cells(&track.sf.chord);
                sf::paint(painter, &transform, &checker, track.sf.orient);
            }
            Layer::Cars => {
                for render in cars {
                    car::paint(painter, &transform, render, render.progress, reduced_motion);
                }
            }
            // Each overlay layer draws nothing when its gate is off. Listing
            // the variants explicitly (not `_`) makes a new `Layer` variant a
            // compile error here rather than a silently-skipped layer.
            Layer::Heatmap | Layer::Grid | Layer::FastestLap => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BakedTrackGeometry, Layer};
    use crate::{CarRender, Overlays};
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::Point;
    use gp_core::sim::CarState;
    use gp_core::track::TrackArtifact;
    use strum::IntoEnumIterator;

    /// AC5/AC9 — the documented back-to-front layer order is exactly (final,
    /// flattened, 9-entry list) `outfield → asphalt → infield → heatmap →
    /// grid → walls → fastest-lap → sf → cars`: [`Layer::iter`] with
    /// `Layer::Regions` expanded to [`super::regions::RegionLayer::iter`]'s
    /// own three names.
    #[test]
    fn layer_order_matches_documented_names() {
        let flat: Vec<&'static str> = Layer::iter()
            .flat_map(|layer| match layer {
                Layer::Regions => super::regions::RegionLayer::iter()
                    .map(<&'static str>::from)
                    .collect::<Vec<_>>(),
                other => vec![<&'static str>::from(other)],
            })
            .collect();
        assert_eq!(
            flat,
            [
                "outfield",
                "asphalt",
                "infield",
                "heatmap",
                "grid",
                "walls",
                "fastest-lap",
                "sf",
                "cars",
            ]
        );
    }

    /// A minimal, hand-built `TrackArtifact` (a 3×3 ring) — delegates to the
    /// single [`ring_track`](super::test_support::ring_track) definition in
    /// `test_support`.
    fn fixture_track() -> TrackArtifact {
        super::test_support::ring_track()
    }

    /// Renders `track`/`cars` once with a bare (fontless) `egui::Context` —
    /// the track canvas draws no text, so no `set_fonts` install is needed —
    /// and returns the tessellation-independent `Shape` list for comparison.
    /// Builds a fresh [`BakedTrackGeometry`] from `track` (design
    /// `2026-07-22-cache-track-geometry`).
    fn render_shapes(
        track: &TrackArtifact,
        cars: &[CarRender<'_>],
        reduced_motion: bool,
        overlays: Overlays,
    ) -> String {
        let geometry = BakedTrackGeometry::new(track);
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            crate::render_frame(
                &painter,
                rect,
                crate::Scene {
                    track,
                    geometry: &geometry,
                    cars,
                    reduced_motion,
                    overlays,
                },
            );
        });
        format!("{:?}", output.shapes)
    }

    /// AC9 — `render_frame` no longer `todo!()`s: a full pass over a
    /// non-trivial track + a moving, trailed "you" car produces at least one
    /// shape and does not panic.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn render_frame_draws_without_panicking() {
        let track = fixture_track();
        let trail = [Point::new(1, 1), Point::new(2, 1)];
        let cars = [CarRender::new(
            CarState {
                x: 2,
                y: 1,
                vx: 1,
                vy: 0,
            },
            0,
            &trail,
            true,
            0.5,
        )];
        let shapes = render_shapes(&track, &cars, false, Overlays::default());
        assert_ne!(shapes, "[]", "render_frame produced no shapes");
    }

    /// AC2/AC5 — a resize does not rebuild the baked geometry: driving
    /// `render_frame` at rect A then rect B through the same
    /// `BakedTrackGeometry` runs the O(n³) ear-clip **zero** times in the draw
    /// path, at either rect (design `2026-07-22-cache-track-geometry`). The
    /// load-bearing assertion is the `regions::triangulate` call counter —
    /// reset after the bake, it must still read `0` after both frames; a
    /// regression that re-triangulated per frame (or per resize) would bump it
    /// even though the caller-owned `triangulated_indices` `&` pointer would
    /// stay put. The pointer check is kept as a cheap secondary guard.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "drives a fontless Context::run_ui pass over render_frame at \
                  two rects — interpreted-pass wall-clock cost, not an abort"
    )]
    fn resize_does_not_rebuild_geometry() {
        let track = fixture_track();
        let geometry = BakedTrackGeometry::new(&track);
        let indices_ptr_before = geometry.triangulated_indices.as_ptr();
        // The bake above triangulated once per loop; zero the counter so the
        // draw pass below is measured on its own.
        crate::track::regions::reset_triangulate_calls();
        let cars: [CarRender<'_>; 0] = [];

        for rect in [
            Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0)),
            Rect::from_min_max(Pos2::ZERO, pos2(240.0, 200.0)),
        ] {
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            };
            let _ = ctx.run_ui(input, |ui| {
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                crate::render_frame(
                    &painter,
                    rect,
                    crate::Scene {
                        track: &track,
                        geometry: &geometry,
                        cars: &cars,
                        reduced_motion: false,
                        overlays: Overlays::default(),
                    },
                );
            });
        }

        assert_eq!(
            crate::track::regions::triangulate_calls(),
            0,
            "the ear-clipping triangulation ran during a per-frame render \
            (it must run only at bake time)"
        );
        assert_eq!(
            geometry.triangulated_indices.as_ptr(),
            indices_ptr_before,
            "a resize rebuilt the baked triangulation topology"
        );
    }

    // `overlays_are_inert` (AC9, #17) is retired here (AC5): the `grid`
    // overlay wired in subtask 3 draws unconditionally on metrics, so
    // turning every flag on is no longer byte-identical to all-off. The
    // suite below (subtask 4) replaces it with the full per-overlay
    // difference/no-op/pure-visual coverage (design § Test Design, subtask
    // 4).

    /// [`fixture_track`], with `speed_heatmap` and `fastest_lap` hand-
    /// populated over the ring's own drivable cells — the AC1/AC2/AC4
    /// metric-populated fixture (block 1's generator is not yet built, so
    /// tests hand-populate `TrackMetrics` per design § Technical
    /// constraints).
    fn fixture_track_with_metrics() -> TrackArtifact {
        let mut track = fixture_track();
        track.metrics = gp_core::track::TrackMetrics {
            speed_heatmap: vec![
                (Point::new(1, 1), 2),
                (Point::new(2, 1), 5),
                (Point::new(3, 1), 8),
                (Point::new(3, 2), 6),
            ],
            fastest_lap: vec![
                Point::new(1, 1),
                Point::new(2, 1),
                Point::new(3, 1),
                Point::new(3, 2),
                Point::new(3, 3),
            ],
            ..gp_core::track::TrackMetrics::default()
        };
        track
    }

    /// AC4/AC5 — on the metric-populated fixture, each of the 3 overlay
    /// flags, turned on alone, changes the drawn output relative to
    /// all-off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn each_overlay_changes_output_when_on() {
        let track = fixture_track_with_metrics();
        let cars: [CarRender<'_>; 0] = [];
        let baseline = render_shapes(&track, &cars, false, Overlays::default());

        let heatmap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                speed_heatmap: true,
                ..Overlays::default()
            },
        );
        assert_ne!(heatmap_on, baseline, "speed_heatmap did not change output");

        let fastest_lap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                fastest_lap: true,
                ..Overlays::default()
            },
        );
        assert_ne!(
            fastest_lap_on, baseline,
            "fastest_lap did not change output"
        );

        let grid_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                grid: true,
                ..Overlays::default()
            },
        );
        assert_ne!(grid_on, baseline, "grid did not change output");
    }

    /// AC4 — every one of the 8 `Overlays` flag combinations renders at
    /// least one shape without panicking, on the metric-populated fixture.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call); \
                  this test additionally loops all 8 overlay combinations, \
                  multiplying the per-pass cost"
    )]
    fn all_overlay_combinations_render_without_panic() {
        let track = fixture_track_with_metrics();
        let cars: [CarRender<'_>; 0] = [];
        for speed_heatmap in [false, true] {
            for fastest_lap in [false, true] {
                for grid in [false, true] {
                    let overlays = Overlays {
                        speed_heatmap,
                        fastest_lap,
                        grid,
                    };
                    let shapes = render_shapes(&track, &cars, false, overlays);
                    assert_ne!(shapes, "[]", "no shapes for overlays={overlays:?}");
                }
            }
        }
    }

    /// AC4 — the all-off frame is exactly the #17, metrics-independent
    /// baseline: rendering the populated fixture with every flag off equals
    /// rendering the empty-metrics fixture with every flag off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn all_off_equals_metrics_independent_baseline() {
        let cars: [CarRender<'_>; 0] = [];
        let populated = render_shapes(
            &fixture_track_with_metrics(),
            &cars,
            false,
            Overlays::default(),
        );
        let empty = render_shapes(&fixture_track(), &cars, false, Overlays::default());
        assert_eq!(populated, empty);
    }

    /// AC7 — on the empty-metrics fixture, turning `speed_heatmap` on alone
    /// is a no-op: identical output to all-off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn heatmap_is_noop_on_empty_metrics() {
        let track = fixture_track();
        let cars: [CarRender<'_>; 0] = [];
        let baseline = render_shapes(&track, &cars, false, Overlays::default());
        let heatmap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                speed_heatmap: true,
                ..Overlays::default()
            },
        );
        assert_eq!(heatmap_on, baseline);
    }

    /// AC7 — on the empty-metrics fixture, turning `fastest_lap` on alone
    /// is a no-op: identical output to all-off.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn fastest_lap_is_noop_on_empty_metrics() {
        let track = fixture_track();
        let cars: [CarRender<'_>; 0] = [];
        let baseline = render_shapes(&track, &cars, false, Overlays::default());
        let fastest_lap_on = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                fastest_lap: true,
                ..Overlays::default()
            },
        );
        assert_eq!(fastest_lap_on, baseline);
    }

    /// AC2 — `fastest_lap` is pure-visual: a full render with it on leaves
    /// the corridor `D` and every metric unchanged (mirrors
    /// `walls.rs::corridor_is_unchanged_by_smoothing`). `draw_frame` only
    /// ever borrows `&TrackArtifact`, so this is enforced by the type
    /// system too — the test pins it as a documented, re-checkable AC2
    /// contract rather than relying on that alone.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "render_shapes drives a fontless Context::run_ui full-frame \
                  pass over the TrackArtifact fixture, capturing the \
                  tessellation-independent Shape list — interpreted-pass \
                  wall-clock cost, not an abort (no tessellate/set_fonts call)"
    )]
    fn fastest_lap_paint_does_not_mutate() {
        let track = fixture_track_with_metrics();
        let cells_before = super::test_support::corridor_cells(&track.corridor, 5);
        let fastest_lap_before = track.metrics.fastest_lap.clone();
        let speed_heatmap_before = track.metrics.speed_heatmap.clone();

        let cars: [CarRender<'_>; 0] = [];
        let _ = render_shapes(
            &track,
            &cars,
            false,
            Overlays {
                fastest_lap: true,
                ..Overlays::default()
            },
        );

        let cells_after = super::test_support::corridor_cells(&track.corridor, 5);
        assert_eq!(cells_before, cells_after);
        assert_eq!(fastest_lap_before, track.metrics.fastest_lap);
        assert_eq!(speed_heatmap_before, track.metrics.speed_heatmap);
    }
}
