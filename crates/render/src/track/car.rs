//! Cars layer (design doc §4, layer 6): [`CarRender`] + move-animation.
//!
//! `gp-render` is draw-only (`ai-docs/key-decisions.md`, 2026-07-16) — it
//! buffers no history and owns no clock. Every per-car render input (color,
//! trail, `you`, animation clock) is caller-supplied via [`CarRender`]; the
//! caller (`gp-game`) owns history and time.

use super::TrackTransform;
use egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2};
use gp_core::geom::Point;
use gp_core::sim::CarState;

/// The car dot's radius, as a fraction of [`TrackTransform::cell_size`].
const DOT_RADIUS_FACTOR: f32 = 0.32;
/// Trail dot radius factor (smaller than the car's own dot).
const TRAIL_DOT_RADIUS_FACTOR: f32 = 0.18;
/// The dot/arrow/ring outline stroke width.
const STROKE_WIDTH: f32 = 1.5;
/// Faintest trail dot alpha (oldest entry); fades up to fully opaque at the
/// most recent entry.
const TRAIL_MIN_ALPHA: f32 = 0.15;
/// The "you" ring's radius, as a fraction of [`TrackTransform::cell_size`]
/// beyond the car dot's own radius.
const YOU_RING_GAP_FACTOR: f32 = 0.18;
/// Number of segments approximating the "you" ring's circle as a polyline
/// for [`Shape::dashed_line`].
const YOU_RING_SEGMENTS: usize = 32;
/// "You" ring dash length, scaled by cell size (`Track.jsx:101`
/// `strokeDasharray="2 3"`, first value, dimensioned against
/// `spacing::CELL_SM`'s 16px reference cell). Not `Track.jsx:76`'s `"2 6"` —
/// that is the deferred fastest-lap ideal-line overlay (design § Risks).
const YOU_RING_DASH_FACTOR: f32 = 2.0 / crate::tokens::spacing::CELL_SM;
/// "You" ring gap length, scaled the same way (`Track.jsx:101`, second
/// value).
const YOU_RING_GAP_LEN_FACTOR: f32 = 3.0 / crate::tokens::spacing::CELL_SM;

/// One car's render input for one frame (design § *`CarRender<'a>`*).
///
/// `color_index` resolves through [`crate::tokens::color::car_color`], which
/// is `None` out of range; [`CarRender::color`] falls back to
/// `CAR_COLORS[0]` rather than panicking.
#[derive(Clone, Copy, Debug)]
pub struct CarRender<'a> {
    /// The car's current discrete state (design § *Signature*).
    pub state: CarState,
    /// 0-based index into `crate::tokens::color::CAR_COLORS`.
    pub color_index: usize,
    /// Prior lattice cells, older → fainter, oldest first.
    pub trail: &'a [Point],
    /// Whether this is the player's own car (draws the dashed "you" ring).
    pub you: bool,
    /// The per-car move-animation clock, `∈ [0, 1]` (`0` = at `state`'s
    /// cell, `1` = fully moved to `state + (vx, vy)`). Values outside `[0,
    /// 1]` are clamped by `lerp_pos`, never panic.
    pub progress: f32,
}

impl<'a> CarRender<'a> {
    /// Builds a `CarRender` from its fields (design § *`CarRender<'a>`*).
    #[must_use]
    pub const fn new(
        state: CarState,
        color_index: usize,
        trail: &'a [Point],
        you: bool,
        progress: f32,
    ) -> Self {
        Self {
            state,
            color_index,
            trail,
            you,
            progress,
        }
    }

    /// This car's resolved color — [`crate::tokens::color::car_color`],
    /// falling back to `CAR_COLORS[0]` (== `ACCENT`) when `color_index` is
    /// out of range, never a panic.
    #[must_use]
    pub fn color(&self) -> Color32 {
        crate::tokens::color::car_color(self.color_index)
            .unwrap_or(crate::tokens::color::CAR_COLORS[0])
    }
}

/// The move-animation interpolated `(x, y)` lattice position of `state` at
/// clock `t` (design § *`CarRender<'a>`*, AC6): linear from `state`'s cell to
/// `state + (vx, vy)`. `reduced_motion` snaps straight to the final position
/// (no slide) regardless of `t`. `t` is clamped to `[0, 1]` — out-of-range
/// input never overshoots or panics.
#[allow(
    clippy::cast_precision_loss,
    reason = "cell/velocity coordinates are grid-realistic i32s, far below f32's \
              exact-integer range; precedent: gp-core track.rs::normalize"
)]
pub(crate) fn lerp_pos(state: CarState, t: f32, reduced_motion: bool) -> (f32, f32) {
    let final_x = state.x.saturating_add(state.vx);
    let final_y = state.y.saturating_add(state.vy);
    let (fx, fy) = (final_x as f32, final_y as f32);
    if reduced_motion {
        return (fx, fy);
    }
    let (sx, sy) = (state.x as f32, state.y as f32);
    let tt = t.clamp(0.0, 1.0);
    (tt.mul_add(fx - sx, sx), tt.mul_add(fy - sy, sy))
}

/// The velocity-vector arrow's `(origin, vec)` in screen space, for
/// [`Painter::arrow`] (AC5): `origin` is `state`'s own cell (not the
/// move-animated position — the arrow shows the *discrete* velocity, not an
/// interpolated one), `vec` points to `state + (vx, vy)`, so direction and
/// length are proportional to the transformed `(vx, vy)`. `None` when the
/// car is stationary (`vx == 0 && vy == 0`), matching `Track.jsx:95`'s
/// `vx!==0 || vy!==0` guard — no arrow to draw.
#[allow(
    clippy::cast_precision_loss,
    reason = "cell/velocity coordinates are grid-realistic i32s, far below f32's \
              exact-integer range; precedent: gp-core track.rs::normalize"
)]
pub(crate) fn arrow_vector(state: CarState, transform: &TrackTransform) -> Option<(Pos2, Vec2)> {
    if state.vx == 0 && state.vy == 0 {
        return None;
    }
    let origin = transform.map((state.x as f32, state.y as f32));
    let target_x = state.x.saturating_add(state.vx);
    let target_y = state.y.saturating_add(state.vy);
    let tip = transform.map((target_x as f32, target_y as f32));
    // Built field-wise, not via `Pos2 - Pos2`: that operator overload fires
    // `clippy::arithmetic_side_effects` (deny) even though the equivalent raw
    // `f32` subtraction below does not (design finding 8).
    Some((origin, Vec2::new(tip.x - origin.x, tip.y - origin.y)))
}

/// A point on the "you" ring's circle, built field-wise (not `Pos2 + Vec2`,
/// same reason as [`arrow_vector`]'s tip/origin subtraction).
fn ring_point(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    Pos2::new(
        angle.cos().mul_add(radius, center.x),
        angle.sin().mul_add(radius, center.y),
    )
}

/// Paints one car (design doc §4, layer 6): the fading trail (older →
/// fainter), a `GRAPHITE_900`-outlined colored dot at the move-animated
/// position, the velocity-vector arrow (when moving), and — when
/// `render.you` — a dashed `ACCENT` ring (`Track.jsx:101`'s
/// `strokeDasharray="2 3"`).
pub(crate) fn paint(
    painter: &Painter,
    transform: &TrackTransform,
    render: &CarRender<'_>,
    t: f32,
    reduced_motion: bool,
) {
    #[allow(
        clippy::cast_precision_loss,
        reason = "cell coordinates are grid-realistic i32s, far below f32's \
                  exact-integer range; precedent: gp-core track.rs::normalize"
    )]
    let trail_len = render.trail.len() as f32;
    let color = render.color();
    for (i, &p) in render.trail.iter().enumerate() {
        #[allow(
            clippy::cast_precision_loss,
            reason = "trail index is bounded by a caller-supplied, realistically-sized \
                      trail slice; precedent: gp-core track.rs::normalize"
        )]
        let age_frac = (i as f32 + 1.0) / (trail_len + 1.0);
        let alpha = (1.0 - TRAIL_MIN_ALPHA).mul_add(age_frac, TRAIL_MIN_ALPHA);
        #[allow(
            clippy::cast_precision_loss,
            reason = "cell coordinates are grid-realistic i32s, far below f32's \
                      exact-integer range; precedent: gp-core track.rs::normalize"
        )]
        let pos = transform.map((p.x as f32, p.y as f32));
        painter.circle_filled(
            pos,
            TRAIL_DOT_RADIUS_FACTOR * transform.cell_size(),
            color.gamma_multiply(alpha),
        );
    }

    let center = transform.map(lerp_pos(render.state, t, reduced_motion));
    let radius = DOT_RADIUS_FACTOR * transform.cell_size();
    let outline = Stroke::new(STROKE_WIDTH, crate::tokens::color::GRAPHITE_900);
    painter.circle_filled(center, radius, color);
    painter.circle_stroke(center, radius, outline);

    if let Some((origin, vec)) = arrow_vector(render.state, transform) {
        painter.arrow(origin, vec, outline);
    }

    if render.you {
        let ring_radius = YOU_RING_GAP_FACTOR.mul_add(transform.cell_size(), radius);
        #[allow(
            clippy::cast_precision_loss,
            reason = "YOU_RING_SEGMENTS is a small fixed constant, far below f32's \
                      exact-integer range; precedent: gp-core track.rs::normalize"
        )]
        let points: Vec<Pos2> = (0..=YOU_RING_SEGMENTS)
            .map(|i| {
                let angle = std::f32::consts::TAU * (i as f32) / (YOU_RING_SEGMENTS as f32);
                ring_point(center, ring_radius, angle)
            })
            .collect();
        let cell = transform.cell_size();
        for shape in Shape::dashed_line(
            &points,
            Stroke::new(STROKE_WIDTH, crate::tokens::color::ACCENT),
            YOU_RING_DASH_FACTOR * cell,
            YOU_RING_GAP_LEN_FACTOR * cell,
        ) {
            painter.add(shape);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CarRender, arrow_vector, lerp_pos};
    use crate::track::test_support::transform_10x10 as transform;
    use gp_core::geom::Point;
    use gp_core::sim::CarState;

    fn car(x: i32, y: i32, vx: i32, vy: i32) -> CarState {
        CarState { x, y, vx, vy }
    }

    fn assert_pos(label: &str, got: (f32, f32), want: (f32, f32)) {
        crate::tokens::css::assert_f32(&format!("{label} x"), got.0, want.0);
        crate::tokens::css::assert_f32(&format!("{label} y"), got.1, want.1);
    }

    /// AC6 — `t=0` yields the start cell, `t=1` the fully-moved cell,
    /// `t=0.5` the exact midpoint.
    #[test]
    fn lerp_pos_interpolates_linearly() {
        let s = car(2, 3, 1, -1);
        assert_pos("lerp_pos t=0", lerp_pos(s, 0.0, false), (2.0, 3.0));
        assert_pos("lerp_pos t=1", lerp_pos(s, 1.0, false), (3.0, 2.0));
        assert_pos("lerp_pos t=0.5", lerp_pos(s, 0.5, false), (2.5, 2.5));
    }

    /// AC6 reduced-motion — snaps straight to the final position for any
    /// `t`, including `t=0`.
    #[test]
    fn reduced_motion_snaps_to_final() {
        let s = car(0, 0, 2, -3);
        for t in [0.0, 0.3, 0.5, 1.0] {
            assert_pos("reduced-motion", lerp_pos(s, t, true), (2.0, -3.0));
        }
    }

    /// `t` outside `[0, 1]` is clamped, never overshoots or panics.
    #[test]
    fn lerp_pos_clamps_out_of_range_t() {
        let s = car(0, 0, 1, 1);
        assert_eq!(lerp_pos(s, -5.0, false), lerp_pos(s, 0.0, false));
        assert_eq!(lerp_pos(s, 5.0, false), lerp_pos(s, 1.0, false));
    }

    /// AC9 — a constructed `CarRender` round-trips its fields.
    #[test]
    fn car_render_round_trips_fields() {
        let trail = [Point::new(0, 0), Point::new(1, 0)];
        let s = car(1, 1, 0, 1);
        let render = CarRender::new(s, 2, &trail, true, 0.5);
        assert_eq!(render.state, s);
        assert_eq!(render.color_index, 2);
        assert_eq!(render.trail, &trail);
        assert!(render.you);
        crate::tokens::css::assert_f32("progress", render.progress, 0.5);
    }

    /// AC9 — an out-of-range `color_index` falls back to `CAR_COLORS[0]`,
    /// never a panic.
    #[test]
    fn out_of_range_color_index_falls_back() {
        let s = car(0, 0, 0, 0);
        let render = CarRender::new(s, usize::MAX, &[], false, 0.0);
        assert_eq!(render.color(), crate::tokens::color::CAR_COLORS[0]);
    }

    /// AC5 — for a representative `(vx, vy)`, the arrow tip (`origin + vec`)
    /// equals `transform(x+vx, y+vy)`, and `vec`'s direction/length match the
    /// transformed `(vx, vy)` (length ∝ speed).
    #[test]
    fn arrow_vector_matches_velocity() {
        let t = transform();
        let s = car(2, 3, 1, -2);
        let (origin, vec) = arrow_vector(s, &t).expect("moving car has an arrow");

        crate::tokens::css::assert_f32("arrow origin x", origin.x, t.map((2.0, 3.0)).x);
        crate::tokens::css::assert_f32("arrow origin y", origin.y, t.map((2.0, 3.0)).y);

        let tip = t.map((3.0, 1.0)); // (x+vx, y+vy) = (3, 1)
        crate::tokens::css::assert_f32("arrow tip x", origin.x + vec.x, tip.x);
        crate::tokens::css::assert_f32("arrow tip y", origin.y + vec.y, tip.y);

        // Doubling the speed doubles the arrow's length (length ∝ speed).
        // `Vec2::length` goes through `sqrt`, whose last-bit f32 result differs
        // between native and Miri (~2 ULP at magnitude ~45), so this
        // proportionality is checked with a tolerance rather than the exact
        // `assert_f32` the bit-stable, transform-mapped origin/tip use above.
        let s2 = car(2, 3, 2, -4);
        let (_, vec2) = arrow_vector(s2, &t).expect("moving car has an arrow");
        let (doubled, want) = (vec2.length(), vec.length() * 2.0);
        assert!(
            (doubled - want).abs() <= 1e-3,
            "doubled speed doubles length: {doubled} vs {want}",
        );
    }

    /// Edge — a stationary car (`vx == vy == 0`) draws no arrow, matching
    /// `Track.jsx:95`'s `vx!==0 || vy!==0` guard.
    #[test]
    fn stationary_car_has_no_arrow() {
        let t = transform();
        assert!(arrow_vector(car(0, 0, 0, 0), &t).is_none());
    }
}
