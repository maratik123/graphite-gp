//! Cars layer (design doc §4, layer 6): [`CarRender`] + move-animation.
//!
//! `gp-render` is draw-only (`ai-docs/key-decisions.md`, 2026-07-16) — it
//! buffers no history and owns no clock. Every per-car render input (color,
//! trail, `you`, animation clock) is caller-supplied via [`CarRender`]; the
//! caller (`gp-game`) owns history and time.

use egui::Color32;
use gp_core::geom::Point;
use gp_core::sim::CarState;

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

#[cfg(test)]
mod tests {
    use super::{CarRender, lerp_pos};
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
}
