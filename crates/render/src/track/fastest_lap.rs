//! Fastest-lap layer (design doc §4, layer 5 — analytics overlay): draws
//! `metrics.fastest_lap` as a thin, smooth, dashed `ACCENT` line over the
//! asphalt, under the S/F chord and cars.
//!
//! Pure geometry core ([`catmull_rom`]) plus a thin [`paint`] that maps to
//! screen via [`TrackTransform`] — the crate's house pattern (design §
//! *House pattern*). Purely visual: never touches the corridor `D`, the
//! walls, or any metric (design § Key decisions 3, AC2).

use super::TrackTransform;
use egui::{Painter, Pos2, Shape, Stroke};
use gp_core::geom::Point;

/// The stroke width (design § Constants; `Track.jsx:76` `strokeWidth 2`).
const FASTEST_LAP_WIDTH: f32 = crate::tokens::spacing::BW_2;
/// Fill alpha (design § Constants; `Track.jsx:76` `opacity 0.9`).
const FASTEST_LAP_ALPHA: f32 = 0.9;
/// Dash length factor, scaled by `TrackTransform::cell_size` (mirrors
/// `car.rs`'s `YOU_RING_DASH_FACTOR` idiom; `Track.jsx:76`
/// `strokeDasharray="2 6"`, first value — `car.rs:31` reserves this exact
/// pair for "the deferred fastest-lap ideal-line overlay", now implemented
/// here).
const FASTEST_LAP_DASH_FACTOR: f32 = 2.0 / crate::tokens::spacing::CELL_SM;
/// Gap length factor, scaled the same way (`Track.jsx:76`, second value).
const FASTEST_LAP_GAP_FACTOR: f32 = 6.0 / crate::tokens::spacing::CELL_SM;
/// Number of sampled segments per control-point span — the spline's
/// visual/testable resolution, not a design-doc-cited number.
const SEGMENTS_PER_SPAN: usize = 16;

/// Clamped index into `points`: `idx` saturated to `points.len() - 1` —
/// the "duplicate the endpoint" convention that turns an open polyline's
/// uniform Catmull-Rom into a clamped-endpoint spline (no ghost point
/// beyond the ends).
fn point_at(points: &[(f32, f32)], idx: usize) -> (f32, f32) {
    points[idx.min(points.len().saturating_sub(1))]
}

/// One uniform Catmull-Rom sample at parameter `t ∈ [0, 1]` across the span
/// `(p1, p2)`, using `p0`/`p3` as the neighboring control points (or a
/// clamped duplicate of `p1`/`p2` at an open end). Fixed parameter spacing
/// (no `sqrt`, no divide-by-zero on coincident points) — design § Key
/// decisions 3 rejects centripetal CR for exactly this NaN-hazard reason.
fn catmull_rom_point(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    t: f32,
) -> (f32, f32) {
    let t2 = t * t;
    let t3 = t2 * t;
    let axis = |coord0: f32, coord1: f32, coord2: f32, coord3: f32| -> f32 {
        let linear = (coord2 - coord0) * t;
        let quad_inner = 5.0f32.mul_add(-coord1, 2.0 * coord0);
        let quad = (4.0f32.mul_add(coord2, quad_inner) - coord3) * t2;
        let cubic_inner = 3.0f32.mul_add(coord1, -coord0);
        let cubic = (3.0f32.mul_add(-coord2, cubic_inner) + coord3) * t3;
        0.5 * (2.0f32.mul_add(coord1, linear) + quad + cubic)
    };
    (axis(p0.0, p1.0, p2.0, p3.0), axis(p0.1, p1.1, p2.1, p3.1))
}

/// Samples a uniform, clamped-endpoint Catmull-Rom spline through `points`
/// (open polyline, lattice space), at [`SEGMENTS_PER_SPAN`] samples per
/// control-point span (design § Key decisions 3). The sampled polyline
/// contains every original control point exactly at its knot index (`q(0)
/// == p_i`, no rounding). `points.len() < 2` returns `points` unchanged — a
/// no-op (AC7: an empty/singleton `fastest_lap` draws nothing).
pub(crate) fn catmull_rom(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let spans = points.len().saturating_sub(1);
    let mut sampled = Vec::with_capacity(spans.saturating_mul(SEGMENTS_PER_SPAN).saturating_add(1));
    for i in 0..spans {
        let p0 = point_at(points, i.saturating_sub(1));
        let p1 = points[i];
        let p2 = points[i.saturating_add(1)];
        let p3 = point_at(points, i.saturating_add(2));
        for step in 0..SEGMENTS_PER_SPAN {
            #[allow(
                clippy::cast_precision_loss,
                reason = "step/SEGMENTS_PER_SPAN are small fixed constants, far below \
                          f32's exact-integer range; precedent: gp-core track.rs::normalize"
            )]
            let t = step as f32 / SEGMENTS_PER_SPAN as f32;
            sampled.push(catmull_rom_point(p0, p1, p2, p3, t));
        }
    }
    sampled.push(points[points.len().saturating_sub(1)]);
    sampled
}

/// Paints the fastest-lap overlay (design § Key decisions 3): splines
/// `fastest_lap` (lattice space) via [`catmull_rom`], maps every sampled
/// point to screen via `transform`, and strokes it as a dashed `ACCENT`
/// line at [`FASTEST_LAP_ALPHA`]. An empty or single-point path draws
/// nothing (AC7 no-op).
pub(crate) fn paint(painter: &Painter, transform: &TrackTransform, fastest_lap: &[Point]) {
    if fastest_lap.len() < 2 {
        return;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "cell coordinates are grid-realistic i32s, far below f32's \
                  exact-integer range; precedent: gp-core track.rs::normalize"
    )]
    let lattice_points: Vec<(f32, f32)> = fastest_lap
        .iter()
        .map(|p| (p.x as f32, p.y as f32))
        .collect();
    let sampled = catmull_rom(&lattice_points);
    let screen_points: Vec<Pos2> = sampled.iter().map(|&p| transform.map(p)).collect();

    let stroke = Stroke::new(
        FASTEST_LAP_WIDTH,
        crate::tokens::color::ACCENT.gamma_multiply(FASTEST_LAP_ALPHA),
    );
    let cell = transform.cell_size();
    for shape in Shape::dashed_line(
        &screen_points,
        stroke,
        FASTEST_LAP_DASH_FACTOR * cell,
        FASTEST_LAP_GAP_FACTOR * cell,
    ) {
        painter.add(shape);
    }
}

#[cfg(test)]
mod tests {
    use super::{SEGMENTS_PER_SPAN, catmull_rom, paint};
    use crate::test_util::assert_f32;
    use crate::track::test_support::transform_10x10 as transform;
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::Point;

    /// AC2 — uniform CR is interpolating: the sampled polyline contains
    /// every control point exactly at its knot index (`i * SEGMENTS_PER_SPAN`).
    #[test]
    fn catmull_rom_passes_through_every_control_point() {
        let points = [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (6.0, -1.0)];
        let sampled = catmull_rom(&points);
        for (i, &(px, py)) in points.iter().enumerate() {
            let knot = i * SEGMENTS_PER_SPAN;
            let (sx, sy) = sampled[knot];
            assert_f32(&format!("knot {i} x"), sx, px);
            assert_f32(&format!("knot {i} y"), sy, py);
        }
    }

    /// Edge — fewer than 2 points returns the input unchanged (no-op).
    #[test]
    fn catmull_rom_noop_below_two_points() {
        assert_eq!(catmull_rom(&[]), Vec::<(f32, f32)>::new());
        assert_eq!(catmull_rom(&[(1.0, 2.0)]), vec![(1.0, 2.0)]);
    }

    /// AC2 — a 2-point path samples a straight segment through both
    /// endpoints (every interior sample lies on the connecting line).
    #[test]
    fn catmull_rom_two_points_is_a_straight_segment() {
        let points = [(0.0, 0.0), (10.0, 0.0)];
        let sampled = catmull_rom(&points);
        assert_f32("first x", sampled.first().unwrap().0, 0.0);
        assert_f32("first y", sampled.first().unwrap().1, 0.0);
        assert_f32("last x", sampled.last().unwrap().0, 10.0);
        assert_f32("last y", sampled.last().unwrap().1, 0.0);
        for &(x, y) in &sampled {
            assert_f32("straight segment y", y, 0.0);
            assert!((0.0..=10.0).contains(&x), "x={x} out of segment range");
        }
    }

    /// Renders `paint` alone and returns the emitted shape count — mirrors
    /// `heatmap.rs`'s `painted_rect_count` capture idiom.
    fn painted_shape_count(fastest_lap: &[Point]) -> usize {
        let t = transform();
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(100.0, 100.0));
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            paint(&painter, &t, fastest_lap);
        });
        output.shapes.len()
    }

    /// AC7 — empty `fastest_lap` draws no shapes (no-op).
    #[test]
    fn paint_is_noop_on_empty_path() {
        assert_eq!(painted_shape_count(&[]), 0);
    }

    /// AC2 — a populated path draws at least one dashed shape.
    #[test]
    fn paint_draws_populated_path() {
        let path = vec![Point::new(1, 1), Point::new(3, 1), Point::new(3, 3)];
        assert!(painted_shape_count(&path) >= 1);
    }
}
