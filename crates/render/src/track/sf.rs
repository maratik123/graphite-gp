//! S/F layer (design doc §4, layer 3): the checkered start/finish chord.
//!
//! Alternating `GRAPHITE_900`/`PAPER_0` unit cells across `StartFinish.chord`,
//! each `GRAPHITE_900`-hairline-stroked (per `Track.jsx`'s `i % 2 == 0 →
//! graphite-900` checker).

use super::TrackTransform;
use egui::{Color32, Painter, Rect, Stroke, StrokeKind};
use gp_core::geom::{Orient, Point};

/// Amendment — S/F thin bar (PR #100): the S/F bar's thickness in the racing
/// (perpendicular-to-chord) direction, as a fraction of one cell. Token-
/// derived, not a magic literal: exactly `Track.jsx`'s S/F rect proportions
/// (`width 16 × height CELL(24)` → `16/24`) `[measured: read
/// docs/design-system/ui_kits/game/Track.jsx:79-86,13]`.
const SF_BAR_THICKNESS_CELLS: f32 = crate::tokens::spacing::CELL_SM / crate::tokens::spacing::CELL;

/// One checkered S/F chord cell: the lattice point and its fill color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CheckerCell {
    /// The chord cell's lattice point.
    pub point: Point,
    /// Its checker fill — alternates `GRAPHITE_900`/`PAPER_0`, starting
    /// `GRAPHITE_900` at index `0` (AC4).
    pub fill: Color32,
}

/// Builds the checkered cell sequence for `chord`, alternating
/// `GRAPHITE_900`/`PAPER_0` starting `GRAPHITE_900` (AC4). Empty input yields
/// an empty output — total, no panic.
pub(crate) fn checker_cells(chord: &[Point]) -> Vec<CheckerCell> {
    chord
        .iter()
        .enumerate()
        .map(|(i, &point)| {
            let fill = if i % 2 == 0 {
                crate::tokens::color::GRAPHITE_900
            } else {
                crate::tokens::color::PAPER_0
            };
            CheckerCell { point, fill }
        })
        .collect()
}

/// Amendment — S/F thin bar (PR #100): the lattice-space `(min, max)` corners
/// of the S/F bar rect centered on `point` — full-cell (±0.5) along the chord
/// axis, `±SF_BAR_THICKNESS_CELLS/2` thin in the perpendicular (racing) axis
/// picked from `orient`: `Horizontal` (chord spans east–west, racing dir y) →
/// thin in y; `Vertical` (chord spans north–south, racing dir x) → thin in x
/// `[measured: read crates/core/src/geom/mod.rs:56-60]`.
#[allow(
    clippy::cast_precision_loss,
    reason = "cell coordinates are grid-realistic i32s, far below f32's \
              exact-integer range; precedent: gp-core track.rs::normalize"
)]
const fn bar_rect_lattice(point: Point, orient: Orient) -> ((f32, f32), (f32, f32)) {
    let (fx, fy) = (point.x as f32, point.y as f32);
    let half = SF_BAR_THICKNESS_CELLS / 2.0;
    match orient {
        Orient::Horizontal => ((fx - 0.5, fy - half), (fx + 0.5, fy + half)),
        Orient::Vertical => ((fx - half, fy - 0.5), (fx + half, fy + 0.5)),
    }
}

/// The screen-space S/F bar rect centered on lattice point `p`, per `orient`
/// (see [`bar_rect_lattice`]).
fn bar_rect(transform: &TrackTransform, p: Point, orient: Orient) -> Rect {
    let (min, max) = bar_rect_lattice(p, orient);
    let a = transform.map(min);
    let b = transform.map(max);
    Rect::from_two_pos(a, b)
}

/// Paints the checkered S/F chord: each cell filled per [`checker_cells`],
/// then `GRAPHITE_900`-hairline-stroked. Each cell renders as the thin bar
/// rect from [`bar_rect`] — full-cell along the chord, `2/3`-cell thin across
/// the racing direction (`orient`).
pub(crate) fn paint(
    painter: &Painter,
    transform: &TrackTransform,
    cells: &[CheckerCell],
    orient: Orient,
) {
    let stroke = Stroke::new(
        crate::tokens::spacing::BW_HAIR,
        crate::tokens::color::GRAPHITE_900,
    );
    for cell in cells {
        let rect = bar_rect(transform, cell.point, orient);
        painter.rect_filled(rect, 0, cell.fill);
        painter.rect_stroke(rect, 0, stroke, StrokeKind::Inside);
    }
}

#[cfg(test)]
mod tests {
    use super::{SF_BAR_THICKNESS_CELLS, bar_rect_lattice, checker_cells};
    use crate::tokens::{css::assert_f32, spacing};
    use gp_core::geom::{Orient, Point};

    /// AC4 — an N-cell chord yields N cells alternating
    /// `GRAPHITE_900`/`PAPER_0`, starting `GRAPHITE_900` at index 0.
    #[test]
    fn chord_checkers_alternate_starting_graphite() {
        let chord: Vec<Point> = (0..5).map(|x| Point::new(x, 0)).collect();
        let cells = checker_cells(&chord);
        assert_eq!(cells.len(), 5);
        for (i, cell) in cells.iter().enumerate() {
            assert_eq!(cell.point, chord[i]);
            let want = if i % 2 == 0 {
                crate::tokens::color::GRAPHITE_900
            } else {
                crate::tokens::color::PAPER_0
            };
            assert_eq!(cell.fill, want, "cell {i}");
        }
        assert_eq!(cells[0].fill, crate::tokens::color::GRAPHITE_900);
        assert_eq!(cells[1].fill, crate::tokens::color::PAPER_0);
    }

    /// Edge — an empty chord yields no cells, no panic.
    #[test]
    fn empty_chord_yields_no_cells() {
        assert!(checker_cells(&[]).is_empty());
    }

    /// Amendment (PR #100) — `Horizontal` (chord spans east–west, racing dir
    /// y) is thin in y, full-cell in x, centered on the point.
    #[test]
    fn bar_rect_lattice_horizontal_is_thin_in_y() {
        let half = SF_BAR_THICKNESS_CELLS / 2.0;
        let (min, max) = bar_rect_lattice(Point::new(2, 1), Orient::Horizontal);
        assert_f32("min.0", min.0, 1.5);
        assert_f32("min.1", min.1, 1.0 - half);
        assert_f32("max.0", max.0, 2.5);
        assert_f32("max.1", max.1, 1.0 + half);
        assert_f32("width", max.0 - min.0, 1.0);
    }

    /// Amendment (PR #100) — `Vertical` (chord spans north–south, racing dir
    /// x) is thin in x, full-cell in y, centered on the point.
    #[test]
    fn bar_rect_lattice_vertical_is_thin_in_x() {
        let half = SF_BAR_THICKNESS_CELLS / 2.0;
        let (min, max) = bar_rect_lattice(Point::new(1, 2), Orient::Vertical);
        assert_f32("min.0", min.0, 1.0 - half);
        assert_f32("min.1", min.1, 1.5);
        assert_f32("max.0", max.0, 1.0 + half);
        assert_f32("max.1", max.1, 2.5);
        assert_f32("height", max.1 - min.1, 1.0);
    }

    /// Amendment (PR #100) — the thin extent equals the token-derived ratio
    /// `spacing::CELL_SM / spacing::CELL` (= 2/3), not a hand-typed literal.
    #[test]
    fn sf_bar_thickness_matches_token_ratio() {
        assert_f32(
            "SF_BAR_THICKNESS_CELLS",
            SF_BAR_THICKNESS_CELLS,
            spacing::CELL_SM / spacing::CELL,
        );
    }
}
