//! Coordinate transform: corridor bounding box (lattice space) → screen `Rect`.
//!
//! Aspect-preserving fit, `y`-flipped (lattice `y` increases northward; egui
//! screen `y` increases downward). Consumed by every paint layer plus the
//! move-animation lerp (subtask 6) and Chaikin smoothing (subtask 4), which
//! both need `(f32, f32)` lattice coordinates, not just integer cell centers.

use egui::{Pos2, Rect};
use gp_core::geom::Corridor;

/// Maps `(f32, f32)` lattice coordinates (cell centers, half-grid corners, or
/// any interpolated point) to a screen [`Pos2`] inside a target `rect`,
/// aspect-preserving and `y`-flipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackTransform {
    /// Screen-space size of one lattice unit (cell edge); both axes equal.
    cell_size: f32,
    /// Screen-space `x` at lattice `x == lattice_min_x`.
    offset_x: f32,
    /// Screen-space `y` at lattice `y == lattice_min_y` (the lattice bbox's
    /// *bottom*, which the `y`-flip places at the *larger* screen `y`).
    offset_bottom_y: f32,
    /// Lattice-space `x` of the bbox's minimum corner (`origin.x - 0.5`, the
    /// half-cell margin that keeps wall corners at `(±0.5, ±0.5)` inside the
    /// mapped area).
    lattice_min_x: f32,
    /// Lattice-space `y` of the bbox's minimum corner (`origin.y - 0.5`).
    lattice_min_y: f32,
}

impl TrackTransform {
    /// Builds a transform fitting `corridor`'s bounding box (its cell grid,
    /// expanded by the half-cell margin on every side so wall geometry at
    /// `(±0.5, ±0.5)` corners stays inside `rect`) into `rect`,
    /// aspect-preserving and centered.
    ///
    /// Degenerate inputs (a zero-area `rect`, or a `0`-width/height
    /// corridor) resolve to a `cell_size` of `0.0` rather than dividing by
    /// zero — every mapped point then collapses to one point, but [`map`]
    /// stays total and panic-free.
    ///
    /// [`map`]: Self::map
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "corridor width/height are cell counts, far below f32's exact-integer \
                  range for any grid-realistic track; precedent: gp-core track.rs::normalize"
    )]
    pub fn new(corridor: &Corridor, rect: Rect) -> Self {
        let bbox_w = corridor.width() as f32;
        let bbox_h = corridor.height() as f32;
        let cell_size =
            if bbox_w <= 0.0 || bbox_h <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
                0.0
            } else {
                (rect.width() / bbox_w).min(rect.height() / bbox_h)
            };
        let content_w = bbox_w * cell_size;
        let content_h = bbox_h * cell_size;
        let offset_x = rect.min.x + (rect.width() - content_w) / 2.0;
        let offset_top_y = rect.min.y + (rect.height() - content_h) / 2.0;
        let origin = corridor.origin();
        Self {
            cell_size,
            offset_x,
            offset_bottom_y: offset_top_y + content_h,
            lattice_min_x: origin.x as f32 - 0.5,
            lattice_min_y: origin.y as f32 - 0.5,
        }
    }

    /// The screen-space size of one lattice unit (cell edge).
    #[inline]
    #[must_use]
    pub const fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Maps a `(x, y)` lattice-space point (cell center, half-grid corner,
    /// or interpolated) to a screen [`Pos2`].
    #[must_use]
    pub fn map(&self, (x, y): (f32, f32)) -> Pos2 {
        let sx = (x - self.lattice_min_x).mul_add(self.cell_size, self.offset_x);
        let sy = (y - self.lattice_min_y).mul_add(-self.cell_size, self.offset_bottom_y);
        Pos2::new(sx, sy)
    }
}

#[cfg(test)]
mod tests {
    use super::TrackTransform;
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::{Corridor, Point};

    fn corridor(origin: (i32, i32), w: usize, h: usize) -> Corridor {
        Corridor::new(Point::new(origin.0, origin.1), w, h)
    }

    fn assert_pos_eq(got: Pos2, want: Pos2) {
        crate::test_util::assert_f32("TrackTransform x", got.x, want.x);
        crate::test_util::assert_f32("TrackTransform y", got.y, want.y);
    }

    /// Happy path — a known lattice cell-center maps to the expected screen
    /// `Pos2`, for a square 2×2 corridor exactly filling a square rect.
    #[test]
    fn known_cell_center_maps_as_expected() {
        let d = corridor((0, 0), 2, 2);
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(20.0, 20.0));
        let t = TrackTransform::new(&d, rect);

        crate::test_util::assert_f32("cell_size", t.cell_size(), 10.0);
        assert_pos_eq(t.map((0.0, 0.0)), pos2(5.0, 15.0));
        assert_pos_eq(t.map((1.0, 1.0)), pos2(15.0, 5.0));
    }

    /// `y`-flip — increasing lattice `y` yields strictly *decreasing* screen
    /// `y`, for a fixed `x`.
    #[test]
    fn increasing_lattice_y_decreases_screen_y() {
        let d = corridor((0, 0), 3, 3);
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(30.0, 30.0));
        let t = TrackTransform::new(&d, rect);

        let low = t.map((0.0, 0.0)).y;
        let mid = t.map((0.0, 1.0)).y;
        let high = t.map((0.0, 2.0)).y;
        assert!(low > mid, "low={low} mid={mid}");
        assert!(mid > high, "mid={mid} high={high}");
    }

    /// Aspect preservation — a non-square bbox fit into a square rect keeps
    /// one uniform `cell_size` on both axes (never independently stretched)
    /// and centers the content within the excess dimension.
    #[test]
    fn non_square_bbox_preserves_aspect_and_centers() {
        let d = corridor((0, 0), 4, 2);
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(20.0, 20.0));
        let t = TrackTransform::new(&d, rect);

        // Limited by the wider axis: min(20/4, 20/2) = min(5, 10) = 5.
        crate::test_util::assert_f32("cell_size", t.cell_size(), 5.0);
        // content_w = 4*5 = 20 (fills rect exactly, no x-centering offset);
        // content_h = 2*5 = 10, centered within the 20-tall rect → 5px pad
        // above and below.
        assert_pos_eq(t.map((-0.5, -0.5)), pos2(0.0, 15.0));
        assert_pos_eq(t.map((-0.5, 1.5)), pos2(0.0, 5.0));
    }

    /// Edge cases — a 1×1 bbox and a degenerate zero-area rect never panic;
    /// `map` stays total.
    #[test]
    fn degenerate_inputs_do_not_panic() {
        let d = corridor((0, 0), 1, 1);
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(10.0, 10.0));
        let t = TrackTransform::new(&d, rect);
        let _ = t.map((0.0, 0.0));

        let zero_rect = Rect::from_min_max(Pos2::ZERO, Pos2::ZERO);
        let t0 = TrackTransform::new(&d, zero_rect);
        crate::test_util::assert_f32("degenerate cell_size", t0.cell_size(), 0.0);
        let mapped = t0.map((0.0, 0.0));
        assert!(mapped.x.is_finite() && mapped.y.is_finite());

        let empty_corridor = corridor((0, 0), 0, 0);
        let t1 = TrackTransform::new(&empty_corridor, rect);
        crate::test_util::assert_f32("empty-corridor cell_size", t1.cell_size(), 0.0);
        let mapped1 = t1.map((0.0, 0.0));
        assert!(mapped1.x.is_finite() && mapped1.y.is_finite());
    }
}
