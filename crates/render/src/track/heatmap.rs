//! Speed heatmap layer (design doc §4, layer 1b — analytics overlay):
//! colors each cell in `metrics.speed_heatmap` by its per-cell max speed on
//! the `HEAT_0` (slowest) → `HEAT_3` (fastest) ramp, drawn as a
//! semi-transparent per-cell square over the asphalt fill.
//!
//! Pure geometry/color core ([`speed_bounds`], [`normalize`],
//! [`ramp_color`]) plus a thin [`paint`] that maps to screen via
//! [`TrackTransform`] — the crate's house pattern (design § *House
//! pattern*).

use super::TrackTransform;
use egui::{Color32, Painter, Rect};
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

/// Paints the speed heatmap (design § Key decisions 1): for each `(Point,
/// i32)` in `heatmap`, fills the cell's full rect in the `ramp_color`
/// mapping its speed's [`normalize`]d position across the observed
/// `[min, max]`, at [`HEATMAP_ALPHA`]. An empty `heatmap` — or one whose
/// [`speed_bounds`] is `None` — draws nothing (AC7 no-op).
pub(crate) fn paint(painter: &Painter, transform: &TrackTransform, heatmap: &[(Point, i32)]) {
    let Some((min, max)) = speed_bounds(heatmap) else {
        return;
    };
    for &(point, speed) in heatmap {
        let t = normalize(speed, min, max);
        let color = ramp_color(t).gamma_multiply(HEATMAP_ALPHA);
        painter.rect_filled(cell_rect(transform, point), 0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::{TrackTransform, normalize, paint, ramp_color, speed_bounds};
    use crate::tokens::color::{HEAT_0, HEAT_1, HEAT_3};
    use crate::tokens::css::assert_f32;
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::{Corridor, Point};

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

    /// Renders `paint` alone into a fresh frame and returns the count of
    /// emitted `Rect` fill shapes — mirrors `regions.rs`'s
    /// `fill_emits_asphalt_mesh_then_infield_mesh` capture idiom.
    fn painted_rect_count(heatmap: &[(Point, i32)]) -> usize {
        let d = Corridor::new(Point::new(0, 0), 5, 5);
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(200.0, 200.0));
        let transform = TrackTransform::new(&d, rect);

        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            paint(&painter, &transform, heatmap);
        });
        output
            .shapes
            .iter()
            .filter(|clipped| matches!(clipped.shape, egui::Shape::Rect(_)))
            .count()
    }

    /// AC1 — a 3-cell hand-populated heatmap emits exactly 3 filled rects.
    #[test]
    fn paint_emits_one_rect_per_cell() {
        let heatmap = vec![
            (Point::new(1, 1), 2),
            (Point::new(2, 1), 5),
            (Point::new(1, 2), 8),
        ];
        assert_eq!(painted_rect_count(&heatmap), 3);
    }

    /// AC7 — an empty heatmap draws no shapes at all (no-op).
    #[test]
    fn paint_is_noop_on_empty_heatmap() {
        assert_eq!(painted_rect_count(&[]), 0);
    }
}
