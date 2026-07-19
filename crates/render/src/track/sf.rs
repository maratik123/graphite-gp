//! S/F layer (design doc §4, layer 3): the checkered start/finish chord.
//!
//! Alternating `GRAPHITE_900`/`PAPER_0` unit cells across `StartFinish.chord`,
//! each `GRAPHITE_900`-hairline-stroked (per `Track.jsx`'s `i % 2 == 0 →
//! graphite-900` checker).

use super::TrackTransform;
use egui::{Color32, Painter, Rect, Stroke, StrokeKind};
use gp_core::geom::Point;

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

/// The screen-space unit-cell rect centered on lattice point `p`.
#[allow(
    clippy::cast_precision_loss,
    reason = "cell coordinates are grid-realistic i32s, far below f32's \
              exact-integer range; precedent: gp-core track.rs::normalize"
)]
fn cell_rect(transform: &TrackTransform, p: Point) -> Rect {
    let (fx, fy) = (p.x as f32, p.y as f32);
    let a = transform.map((fx - 0.5, fy - 0.5));
    let b = transform.map((fx + 0.5, fy + 0.5));
    Rect::from_two_pos(a, b)
}

/// Paints the checkered S/F chord: each cell filled per [`checker_cells`],
/// then `GRAPHITE_900`-hairline-stroked.
pub(crate) fn paint(painter: &Painter, transform: &TrackTransform, cells: &[CheckerCell]) {
    let stroke = Stroke::new(
        crate::tokens::spacing::BW_HAIR,
        crate::tokens::color::GRAPHITE_900,
    );
    for cell in cells {
        let rect = cell_rect(transform, cell.point);
        painter.rect_filled(rect, 0, cell.fill);
        painter.rect_stroke(rect, 0, stroke, StrokeKind::Inside);
    }
}

#[cfg(test)]
mod tests {
    use super::checker_cells;
    use gp_core::geom::Point;

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
}
