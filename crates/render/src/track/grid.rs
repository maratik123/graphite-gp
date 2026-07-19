//! Notebook-sheet grid layer (design doc §4, layer 4 — analytics overlay):
//! faint engineering-blue ruling at the corridor's lattice cell pitch, a
//! heavier major line every 5th, and a dotted lattice at every cell corner.
//!
//! Pure geometry core ([`line_coords`]) plus a thin [`paint`] that maps to
//! screen via [`TrackTransform`] and strokes/fills it — the crate's house
//! pattern (design § *House pattern*). Whole-canvas (not corridor-clipped),
//! drawn over the filled regions, under the walls (design § Key decisions
//! 4).

use super::TrackTransform;
use egui::{Painter, Pos2, Rect, Stroke};

/// Defensive minimum screen-space pitch, below which the grid degrades to a
/// no-op rather than emitting a runaway shape count (design § Risks — "Grid
/// runaway shape count if `cell_size` is tiny").
const MIN_GRID_PITCH_PX: f32 = 1.0;

/// A cell-corner reference lattice coordinate: any half-integer (`k + 0.5`)
/// position is congruent to every other, modulo `1.0`, regardless of the
/// corridor's actual (always-integer) origin — so `-0.5` is as valid a
/// reference as `origin - 0.5` for pure spacing purposes (design § Key
/// decisions 4).
const GRID_ANCHOR_LATTICE: f32 = -0.5;

/// The major-line interval: every Nth ruling line (counted from the anchor)
/// is drawn heavier in `GRID_LINE_MAJOR` rather than `GRID_LINE` (design § Key
/// decisions 4 — "a heavier major line every 5th").
const GRID_MAJOR_EVERY: i64 = 5;

/// One ruling line's screen position (`.0`) and whether it is a "major"
/// line (`.1`, every 5th line counted from `anchor`, design § Key decisions
/// 4). Covers every line whose position falls within `range` (inclusive).
///
/// `pitch <= 0` (or non-finite) and an inverted/non-finite `range` both
/// yield an empty result — total, never a panic (AC3/grid no-op guard).
pub(crate) fn line_coords(anchor: f32, pitch: f32, range: (f32, f32)) -> Vec<(f32, bool)> {
    let (lo, hi) = range;
    if !pitch.is_finite() || pitch <= 0.0 || !lo.is_finite() || !hi.is_finite() || lo > hi {
        return Vec::new();
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "(lo - anchor) / pitch is bounded by a real, on-canvas screen range \
                  divided by a positive pitch — far below i64's range"
    )]
    let k_min = ((lo - anchor) / pitch).ceil() as i64;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "see k_min above; same bounded on-canvas range"
    )]
    let k_max = ((hi - anchor) / pitch).floor() as i64;
    (k_min..=k_max)
        .map(|k| {
            #[allow(
                clippy::cast_precision_loss,
                reason = "k is bounded by a real, on-canvas screen range, far below \
                          f32's exact-integer range; precedent: gp-core track.rs::normalize"
            )]
            let pos = pitch.mul_add(k as f32, anchor);
            (pos, k.rem_euclid(GRID_MAJOR_EVERY) == 0)
        })
        .collect()
}

/// Strokes one axis's ruling lines (`coords`, from [`line_coords`]) as
/// full-canvas segments spanning `rect`, minor in `GRID_LINE` / major in
/// `GRID_LINE_MAJOR`. `vertical` picks which rect edges the segment spans:
/// `true` draws a vertical line (`coord` is an `x`) from `rect`'s top to
/// bottom edge; `false` draws a horizontal line (`coord` is a `y`) from
/// `rect`'s left to right edge.
fn paint_ruling(painter: &Painter, rect: Rect, coords: &[(f32, bool)], vertical: bool) {
    for &(coord, is_major) in coords {
        let color = if is_major {
            crate::tokens::color::GRID_LINE_MAJOR
        } else {
            crate::tokens::color::GRID_LINE
        };
        let stroke = Stroke::new(crate::tokens::effects::BG_GRID_RULING_WIDTH, color);
        let segment = if vertical {
            [Pos2::new(coord, rect.min.y), Pos2::new(coord, rect.max.y)]
        } else {
            [Pos2::new(rect.min.x, coord), Pos2::new(rect.max.x, coord)]
        };
        painter.line_segment(segment, stroke);
    }
}

/// Paints the notebook-sheet grid (design § Key decisions 4): a whole-canvas
/// ruling at the lattice cell pitch (minor `GRID_LINE`, major every 5th
/// `GRID_LINE_MAJOR`), plus a `GRID_DOT` dot at every cell-corner
/// intersection. Degenerate (`cell_size` below [`MIN_GRID_PITCH_PX`], which
/// subsumes `<= 0`) draws nothing — no panic, no runaway shape count (design
/// § Risks).
pub(crate) fn paint(painter: &Painter, rect: Rect, transform: &TrackTransform) {
    let pitch = transform.cell_size();
    if pitch < MIN_GRID_PITCH_PX {
        return;
    }
    let anchor = transform.map((GRID_ANCHOR_LATTICE, GRID_ANCHOR_LATTICE));
    let xs = line_coords(anchor.x, pitch, (rect.min.x, rect.max.x));
    let ys = line_coords(anchor.y, pitch, (rect.min.y, rect.max.y));

    paint_ruling(painter, rect, &xs, true);
    paint_ruling(painter, rect, &ys, false);

    for &(x, _) in &xs {
        for &(y, _) in &ys {
            painter.circle_filled(
                Pos2::new(x, y),
                crate::tokens::effects::BG_DOTS_RADIUS,
                crate::tokens::color::GRID_DOT,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TrackTransform, line_coords, paint};
    use crate::test_util::assert_f32;
    use crate::track::test_support::transform_10x10 as transform;
    use egui::{Pos2, Rect, pos2};
    use gp_core::geom::{Corridor, Point};

    /// AC3 — `pitch <= 0` yields an empty result, never a panic.
    #[test]
    fn line_coords_non_positive_pitch_is_empty() {
        assert!(line_coords(0.0, 0.0, (0.0, 100.0)).is_empty());
        assert!(line_coords(0.0, -5.0, (0.0, 100.0)).is_empty());
    }

    /// Edge — an inverted range yields an empty result, never a panic.
    #[test]
    fn line_coords_inverted_range_is_empty() {
        assert!(line_coords(0.0, 10.0, (100.0, 0.0)).is_empty());
    }

    /// AC3 — consecutive ruling-line positions differ by exactly `pitch`
    /// (the lattice cell pitch).
    #[test]
    fn line_coords_pitch_matches_cell_size() {
        let coords = line_coords(0.0, 10.0, (0.0, 100.0));
        assert!(coords.len() >= 2, "expected multiple lines, got {coords:?}");
        for window in coords.windows(2) {
            let [(a, _), (b, _)] = window else { continue };
            assert_f32("consecutive line spacing", b - a, 10.0);
        }
    }

    /// AC3/design § Key decisions 4 — a major line lands every 5th, counted
    /// from `anchor`.
    #[test]
    fn line_coords_major_every_fifth() {
        let coords = line_coords(0.0, 1.0, (-12.0, 12.0));
        for (i, &(pos, is_major)) in coords.iter().enumerate() {
            #[allow(
                clippy::cast_possible_wrap,
                reason = "coords.len() is a small fixed test fixture size, far below i64's range"
            )]
            let k = i as i64 - 12;
            assert_eq!(
                is_major,
                k.rem_euclid(5) == 0,
                "pos={pos} k={k} is_major={is_major}"
            );
        }
    }

    /// Coverage — every returned position lies within the requested range,
    /// and no in-range line is skipped (the next line out on either side
    /// would fall outside it).
    #[test]
    fn line_coords_covers_the_requested_range() {
        let (lo, hi) = (0.0, 25.0);
        let coords = line_coords(0.0, 10.0, (lo, hi));
        for &(pos, _) in &coords {
            assert!((lo..=hi).contains(&pos), "pos={pos} out of [{lo}, {hi}]");
        }
        let first = coords.first().unwrap().0;
        let last = coords.last().unwrap().0;
        assert!(first - 10.0 < lo, "a line before the first was skippable");
        assert!(last + 10.0 > hi, "a line after the last was skippable");
    }

    /// Renders `paint` alone and returns the emitted shape count — mirrors
    /// `heatmap.rs`'s/`fastest_lap.rs`'s capture idiom.
    fn painted_shape_count(rect: Rect, transform: &TrackTransform) -> usize {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            paint(&painter, rect, transform);
        });
        output.shapes.len()
    }

    /// AC3 — over a known (`cell_size == 10`) transform, `paint` emits at
    /// least one ruling line plus one dot, and does not panic.
    #[test]
    fn paint_emits_ruling_and_dots() {
        let rect = Rect::from_min_max(Pos2::ZERO, pos2(100.0, 100.0));
        assert!(painted_shape_count(rect, &transform()) >= 1);
    }

    /// Edge — a degenerate (`cell_size == 0`) transform draws nothing, no
    /// panic.
    #[test]
    fn paint_is_noop_on_degenerate_transform() {
        let rect = Rect::from_min_max(Pos2::ZERO, Pos2::ZERO);
        let d = Corridor::new(Point::new(0, 0), 10, 10);
        let degenerate = TrackTransform::new(&d, rect);
        let render_rect = Rect::from_min_max(Pos2::ZERO, pos2(100.0, 100.0));
        assert_eq!(painted_shape_count(render_rect, &degenerate), 0);
    }
}
