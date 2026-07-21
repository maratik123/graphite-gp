//! `LapMeter` — port of `LapMeter.jsx`/`LapMeter.d.ts` (design § *Style-mapping
//! ground truth*, AC3/AC4/AC7).
//!
//! Non-interactive lap-progress readout; `show` returns a bare `Response`
//! (`Sense::hover()`), matching `Badge`/`Telemetry`.

use crate::tokens::{color, spacing, typography};
use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui,
};

/// Gap between the header row and the cells row (`LapMeter.jsx`: `gap: 6`) —
/// not a token (nearest is `spacing::SPACE_1 = 4`).
const ROW_GAP: f32 = 6.0;
/// Gap between the label and the readout in the header row
/// (`LapMeter.jsx`: `gap: 12`), equals `spacing::SPACE_3`.
const HEADER_GAP: f32 = spacing::SPACE_3;
/// Gap between adjacent cells (`LapMeter.jsx`: `gap: 3`) — not a token.
const CELL_GAP: f32 = 3.0;
/// Cell height (`LapMeter.jsx`: `height: 8`), equals `spacing::SPACE_2`.
const CELL_HEIGHT: f32 = spacing::SPACE_2;
/// Default label (`LapMeter.d.ts` `label` default).
const DEFAULT_LABEL: &str = "LAP";

/// The pure style-resolution output of [`LapMeter::resolve`] (AC7): the
/// clamped `done`/`total` cell counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LapMeterStyle {
    /// The number of filled (`ACCENT`) cells, clamped to `[0, total]`.
    pub done: i32,
    /// The total cell count, clamped to `>= 0`.
    pub total: i32,
}

/// The pure on-ink color-trio resolution output of [`LapMeter::ink_colors`]
/// (design § *The `LapMeter`-on-dark-band port gap*, mirrors
/// [`super::telemetry::TelemetryStyle`]'s color split).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LapMeterColors {
    /// The header label color.
    pub label: Color32,
    /// The laps-done readout color — the load-bearing legibility fix
    /// (`PAPER_0` on-ink, `TEXT_INK` off-ink).
    pub done: Color32,
    /// The `/total` readout color (`TEXT_FAINT` in both modes).
    pub total: Color32,
}

/// `LapMeter` props (`LapMeter.d.ts`): `lap`, `total`, `label`.
#[derive(Clone, Copy, Debug)]
pub struct LapMeter<'a> {
    /// The current (possibly out-of-range) lap count.
    pub lap: i32,
    /// The total lap count.
    pub total: i32,
    /// The header label (`LapMeter.d.ts` `label`, default `"LAP"`).
    pub label: &'a str,
    /// On-ink render mode (Rust-only field beyond the `.d.ts`, mirrors
    /// `Telemetry::on_ink`): swaps the label/done/total colors for their
    /// on-ink equivalents so the meter stays legible on a dark band.
    pub on_ink: bool,
}

impl<'a> LapMeter<'a> {
    /// Builds a `"LAP"`-labelled, off-ink lap meter for `lap` of `total`.
    #[must_use]
    pub const fn new(lap: i32, total: i32) -> Self {
        Self {
            lap,
            total,
            label: DEFAULT_LABEL,
            on_ink: false,
        }
    }

    /// Sets `label`.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Sets `on_ink`.
    #[must_use]
    pub const fn on_ink(mut self, on_ink: bool) -> Self {
        self.on_ink = on_ink;
        self
    }

    /// The pure style-resolution layer (AC7): `(lap, total)` → clamped
    /// `done`/`total`, by integer comparisons only (no casts) — const-stable
    /// (design Key decision 5).
    #[must_use]
    pub const fn resolve(lap: i32, total: i32) -> LapMeterStyle {
        let total = if total < 0 { 0 } else { total };
        let done = if lap < 0 {
            0
        } else if lap > total {
            total
        } else {
            lap
        };
        LapMeterStyle { done, total }
    }

    /// The pure on-ink color-resolution layer (mirrors
    /// [`super::telemetry::Telemetry::resolve`]): `on_ink` → the
    /// `(label, done, total)` color trio. No `egui::Ui`, no allocation —
    /// Miri-clean, const-stable.
    ///
    /// On-ink: `label = TEXT_FAINT`, `done = PAPER_0` (legible on the dark
    /// `GRAPHITE_900` HUD band), `total = TEXT_FAINT`. Off-ink (current):
    /// `label = TEXT_MUTED`, `done = TEXT_INK`, `total = TEXT_FAINT`.
    #[must_use]
    pub const fn ink_colors(on_ink: bool) -> LapMeterColors {
        if on_ink {
            LapMeterColors {
                label: color::TEXT_FAINT,
                done: color::PAPER_0,
                total: color::TEXT_FAINT,
            }
        } else {
            LapMeterColors {
                label: color::TEXT_MUTED,
                done: color::TEXT_INK,
                total: color::TEXT_FAINT,
            }
        }
    }

    /// Draws the resolved `style` into `rect`: a header row (uppercase
    /// `label` + `done/total` readout) above a row of `style.total`
    /// equal-width cells, the first `style.done` filled `ACCENT`.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first — draws through
    /// `FontFamily::Name(fonts::JETBRAINS_MONO_BOLD/_REGULAR)`.
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: LapMeterStyle,
        label: &str,
        colors: LapMeterColors,
    ) {
        let label_font = FontId::new(
            typography::FS_XS,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        );
        let label_width = painter
            .layout_no_wrap(label.to_uppercase(), label_font.clone(), colors.label)
            .rect
            .width();

        let done_text = style.done.to_string();
        let total_text = format!("/{}", style.total);
        let done_font = FontId::new(
            typography::FS_TITLE,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_BOLD.into()),
        );
        let done_width = painter
            .layout_no_wrap(done_text.clone(), done_font.clone(), colors.done)
            .rect
            .width();
        let total_width = painter
            .layout_no_wrap(total_text.clone(), done_font.clone(), colors.total)
            .rect
            .width();
        let readout_width = done_width + total_width;

        // Header row is `space-between` in `LapMeter.jsx`, with `HEADER_GAP`
        // as the minimum label↔readout gap when content would otherwise
        // overlap (design § Non-token dimensions).
        let readout_left = (rect.max.x - readout_width).max(rect.min.x + label_width + HEADER_GAP);

        painter.text(
            rect.min,
            Align2::LEFT_TOP,
            label.to_uppercase(),
            label_font,
            colors.label,
        );
        painter.text(
            Pos2::new(readout_left, rect.min.y),
            Align2::LEFT_TOP,
            done_text,
            done_font.clone(),
            colors.done,
        );
        painter.text(
            Pos2::new(readout_left + done_width, rect.min.y),
            Align2::LEFT_TOP,
            total_text,
            done_font,
            colors.total,
        );

        let header_height = typography::FS_TITLE;
        let cells_top = rect.min.y + header_height + ROW_GAP;
        let cells_rect = Rect::from_min_max(
            Pos2::new(rect.min.x, cells_top),
            Pos2::new(rect.max.x, cells_top + CELL_HEIGHT),
        );

        let total_cells = style.total.max(0);
        if total_cells == 0 {
            return;
        }
        let total_f = f32::from(u16::try_from(total_cells).unwrap_or(u16::MAX));
        let cell_width = CELL_GAP
            .mul_add(-(total_f - 1.0), cells_rect.width())
            .max(0.0)
            / total_f;

        for i in 0..total_cells {
            let index_f = f32::from(u16::try_from(i).unwrap_or(u16::MAX));
            let left = index_f.mul_add(cell_width + CELL_GAP, cells_rect.min.x);
            let fill_rect = Rect::from_min_max(
                Pos2::new(left, cells_rect.min.y),
                Pos2::new(left + cell_width, cells_rect.max.y),
            );
            let fill = if i < style.done {
                color::ACCENT
            } else {
                color::PAPER_3
            };
            painter.rect_filled(fill_rect, 0, fill);
            painter.rect_stroke(
                fill_rect,
                0,
                Stroke::new(spacing::BW_HAIR, color::GRAPHITE_900),
                StrokeKind::Inside,
            );
        }
    }

    /// Draws the lap meter and allocates its rect.
    ///
    /// Non-interactive (`Sense::hover()`), consistent with `LapMeter.d.ts`
    /// carrying no `onClick`.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first (same precondition as the private
    /// `paint` layer this delegates to).
    pub fn show(self, ui: &mut Ui) -> Response {
        let style = Self::resolve(self.lap, self.total);
        let colors = Self::ink_colors(self.on_ink);
        let width = ui.available_width();
        let height = typography::FS_TITLE + ROW_GAP + CELL_HEIGHT;
        let desired = egui::vec2(width, height);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if ui.is_rect_visible(rect) {
            Self::paint(ui.painter(), rect, style, self.label, colors);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_LABEL, LapMeter};
    use crate::tokens::color;

    /// AC4 — `lap <= 0` clamps `done` to `0`.
    #[test]
    fn resolve_clamps_negative_lap_to_zero() {
        let style = LapMeter::resolve(-3, 5);
        assert_eq!(style.done, 0);
        assert_eq!(style.total, 5);
    }

    /// AC4 — `lap >= total` clamps `done` to `total`.
    #[test]
    fn resolve_clamps_lap_above_total() {
        let style = LapMeter::resolve(9, 5);
        assert_eq!(style.done, 5);
        assert_eq!(style.total, 5);
    }

    /// AC4 — an intermediate `lap` passes through unclamped.
    #[test]
    fn resolve_intermediate_lap_passes_through() {
        let style = LapMeter::resolve(2, 5);
        assert_eq!(style.done, 2);
        assert_eq!(style.total, 5);
    }

    /// AC4 — a negative `total` clamps to `0`.
    #[test]
    fn resolve_clamps_negative_total_to_zero() {
        let style = LapMeter::resolve(2, -1);
        assert_eq!(style.total, 0);
        assert_eq!(style.done, 0);
    }

    /// AC4 — cell `i` is filled iff `i < done`.
    #[test]
    fn cell_fill_series_matches_done() {
        let style = LapMeter::resolve(2, 5);
        let filled: Vec<bool> = (0..style.total).map(|i| i < style.done).collect();
        assert_eq!(filled, vec![true, true, false, false, false]);
    }

    /// Builder defaults: `new` labels `"LAP"`; `.label` overrides.
    #[test]
    fn new_defaults_label_to_lap() {
        let meter = LapMeter::new(2, 5);
        assert_eq!(meter.label, DEFAULT_LABEL);
        assert_eq!(meter.label, "LAP");

        let renamed = meter.label("TOUR");
        assert_eq!(renamed.label, "TOUR");
        assert!(!meter.on_ink);
    }

    /// AC1 — `on_ink` builder sets the field.
    #[test]
    fn on_ink_builder_sets_field() {
        let meter = LapMeter::new(2, 5).on_ink(true);
        assert!(meter.on_ink);
    }

    /// AC1 — off-ink `ink_colors` returns the current trio unchanged:
    /// `label = TEXT_MUTED`, `done = TEXT_INK`, `total = TEXT_FAINT`.
    #[test]
    fn ink_colors_off_ink_is_current_trio() {
        let colors = LapMeter::ink_colors(false);
        assert_eq!(colors.label, color::TEXT_MUTED);
        assert_eq!(colors.done, color::TEXT_INK);
        assert_eq!(colors.total, color::TEXT_FAINT);
    }

    /// AC1 — on-ink `ink_colors` promotes `done` to `PAPER_0` (legible on
    /// the dark `GRAPHITE_900` HUD band), mirroring
    /// `Telemetry::resolve_on_ink_overrides_default_and_muted`.
    #[test]
    fn ink_colors_on_ink_promotes_done_to_paper_0() {
        let colors = LapMeter::ink_colors(true);
        assert_eq!(colors.label, color::TEXT_FAINT);
        assert_eq!(colors.done, color::PAPER_0);
        assert_eq!(colors.total, color::TEXT_FAINT);
    }
}
