//! `SegmentedControl` — port of `SegmentedControl.jsx`/
//! `SegmentedControl.d.ts` (design § *Per-widget prop surface* /
//! *Style-mapping ground truth*, AC3).

use super::Size;
use crate::tokens::{color, spacing, typography};
use egui::{
    Align2, Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui,
};
use std::sync::Arc;

/// Per-segment horizontal padding (`SegmentedControl.jsx:31` `0 14px`).
const SEG_PAD_X: f32 = 14.0;

/// The pure style-resolution output of [`SegmentedControl::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentStyle {
    /// Segment fill (`GRAPHITE_900` selected, else transparent, AC3).
    pub bg: Color32,
    /// Label color (`PAPER_0` selected, else `TEXT_BODY`, AC3).
    pub fg: Color32,
    /// Segment height (`spacing::CONTROL_H_{SM,MD,LG}`).
    pub height: f32,
    /// Label font size (`typography::FS_SM` at `Size::Sm`, else `FS_BODY`).
    pub font_size: f32,
}

/// The response of [`SegmentedControl::show`]: the whole-row `Response`,
/// the selected index after this frame, and whether a click moved it.
#[derive(Debug)]
pub struct SegmentedControlResponse {
    /// The whole-row interaction response.
    pub response: Response,
    /// The selected index: the clicked segment, else the index whose
    /// option matches the input `value`, else `None`.
    pub selected: Option<usize>,
    /// Whether a click moved the selection this frame.
    pub changed: bool,
}

/// `SegmentedControl` props (`SegmentedControl.d.ts`): `options/value/size`.
///
/// Each `options` entry is both the value and the label (design § Key
/// decision 5) — the `.d.ts` union collapses to plain strings in every real
/// usage.
#[derive(Clone, Copy, Debug)]
pub struct SegmentedControl<'a> {
    /// The segment options — each entry is value **and** label.
    pub options: &'a [&'a str],
    /// The currently-selected option's value. A value matching no option
    /// selects none (all segments render unselected).
    pub value: &'a str,
    /// The size.
    pub size: Size,
}

impl<'a> SegmentedControl<'a> {
    /// Builds a medium-size control over `options`, selecting whichever
    /// entry equals `value`.
    #[must_use]
    pub const fn new(options: &'a [&'a str], value: &'a str) -> Self {
        Self {
            options,
            value,
            size: Size::Md,
        }
    }

    /// Sets `size`.
    #[must_use]
    pub const fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The pure style-resolution layer (AC7): `(selected, size)` → colors +
    /// metrics. No `egui::Ui`, no allocation — Miri-clean.
    #[must_use]
    pub const fn resolve(selected: bool, size: Size) -> SegmentStyle {
        let (bg, fg) = if selected {
            (color::GRAPHITE_900, color::PAPER_0)
        } else {
            (Color32::TRANSPARENT, color::TEXT_BODY)
        };
        let (height, font_size) = match size {
            Size::Sm => (spacing::CONTROL_H_SM, typography::FS_SM),
            Size::Md => (spacing::CONTROL_H_MD, typography::FS_BODY),
            Size::Lg => (spacing::CONTROL_H_LG, typography::FS_BODY),
        };
        SegmentStyle {
            bg,
            fg,
            height,
            font_size,
        }
    }

    /// Draws the outer chrome (border/radius/bg) plus every segment (bg iff
    /// selected, a left divider for every segment but the first, and the
    /// centered label) into `rect`, measuring each label to place it
    /// (deterministic, no pointer).
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first.
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        galleys: &[Arc<Galley>],
        selected: Option<usize>,
        size: Size,
    ) {
        painter.rect_filled(rect, spacing::RADIUS_2, color::PAPER_0);

        let widths = segment_widths_from_galleys(galleys);
        let clipped = painter.with_clip_rect(rect);
        let last = galleys.len().saturating_sub(1);
        for (i, galley) in galleys.iter().enumerate() {
            let seg_rect = seg_rect_at(rect, &widths, i);
            let style = Self::resolve(Some(i) == selected, size);
            if Some(i) == selected {
                // Per-corner radius, not `0`: a bare rect clip cannot round
                // corners, so a `0`-radius fill on the first/last selected
                // segment squared off the outer `RADIUS_2` chrome corners
                // (PR #95 review round 1). Round only the OUTER corners that
                // coincide with the container's rounded edge; inner
                // (neighbor-shared) corners and middle segments stay square.
                // `CornerRadius::from(f32)` (not a raw `as u8` cast, which
                // pedantic clippy denies for sign-loss/truncation) is the
                // same conversion `common::paint_surface` and friends use
                // for a uniform radius; taking `.nw` from that reuses it to
                // get the single `u8` this per-corner struct needs.
                let outer_radius = egui::CornerRadius::from(spacing::RADIUS_2).nw;
                let corner_radius = if galleys.len() == 1 {
                    egui::CornerRadius::same(outer_radius)
                } else if i == 0 {
                    egui::CornerRadius {
                        nw: outer_radius,
                        sw: outer_radius,
                        ne: 0,
                        se: 0,
                    }
                } else if i == last {
                    egui::CornerRadius {
                        ne: outer_radius,
                        se: outer_radius,
                        nw: 0,
                        sw: 0,
                    }
                } else {
                    egui::CornerRadius::ZERO
                };
                // No border-width inset needed here: the selected fill
                // (`GRAPHITE_900`) is the same color as the outer border
                // stroke, so the fill seamlessly covers the border at the
                // rounded corner rather than needing to leave it visible.
                clipped.rect_filled(seg_rect, corner_radius, style.bg);
            }
            if i > 0 {
                clipped.vline(
                    seg_rect.min.x,
                    seg_rect.y_range(),
                    Stroke::new(spacing::BW_HAIR, color::GRAPHITE_900),
                );
            }
            crate::text::paint_galley_override(
                &clipped,
                seg_rect.center(),
                Align2::CENTER_CENTER,
                galley.clone(),
                style.fg,
            );
        }
        // Drawn LAST so the container border sits on top of the full-height
        // dividers/segment fills and covers their ends (egui equivalent of
        // the .jsx container's `overflow: hidden`) — round 2 fix for
        // PR #95 review thread T#3609371837 (inconsistent divider heights).
        painter.rect_stroke(
            rect,
            spacing::RADIUS_2,
            Stroke::new(spacing::BW_1, color::GRAPHITE_900),
            StrokeKind::Inside,
        );
    }

    /// Allocates the measured total width, reads live pointer input for
    /// each segment's hit rect, resolves the style, draws it, and returns a
    /// [`SegmentedControlResponse`].
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(self, ui: &mut Ui) -> SegmentedControlResponse {
        let galleys = segment_galleys(ui.painter(), self.options, self.size);
        let widths = segment_widths_from_galleys(&galleys);
        let total_width: f32 = widths.iter().sum();
        let height = Self::resolve(false, self.size).height;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(total_width, height), Sense::hover());

        let mut clicked_index = None;
        for i in 0..self.options.len() {
            let seg_rect = seg_rect_at(rect, &widths, i);
            let seg_response = ui.interact(seg_rect, response.id.with(i), Sense::click());
            if seg_response.clicked() {
                clicked_index = Some(i);
            }
        }

        let selected = clicked_index.or_else(|| selected_index(self.options, self.value));
        if ui.is_rect_visible(rect) {
            Self::paint(ui.painter(), rect, &galleys, selected, self.size);
        }

        SegmentedControlResponse {
            response,
            selected,
            changed: clicked_index.is_some(),
        }
    }
}

/// Builds every segment's label galley once, at `size`'s font — the single
/// shaping pass shared by [`SegmentedControl::show`] and
/// [`forms_gallery::draw_segmented_controls`](super::forms_gallery), so
/// `show`/the gallery/`paint` all agree by construction (AC3).
///
/// `pub(crate)`, not private: `forms_gallery` (subtask 6) needs the same
/// galleys to size its own demo rects correctly — reusing them here keeps
/// the golden's chrome matching what `paint` will actually draw, rather
/// than an independently-guessed width.
pub(crate) fn segment_galleys(painter: &Painter, options: &[&str], size: Size) -> Vec<Arc<Galley>> {
    let font_size = match size {
        Size::Sm => typography::FS_SM,
        Size::Md | Size::Lg => typography::FS_BODY,
    };
    let font = FontId::new(
        font_size,
        FontFamily::Name(crate::fonts::ONEST_MEDIUM.into()),
    );
    options
        .iter()
        .map(|label| painter.layout_no_wrap((*label).to_owned(), font.clone(), color::TEXT_BODY))
        .collect()
}

/// The padded width (`SEG_PAD_X` either side) of every pre-shaped segment
/// galley — arithmetic on already-shaped runs, not a re-shape.
pub(crate) fn segment_widths_from_galleys(galleys: &[Arc<Galley>]) -> Vec<f32> {
    galleys
        .iter()
        .map(|galley| SEG_PAD_X.mul_add(2.0, galley.size().x))
        .collect()
}

/// The rect of segment `index` within `rect`, given each segment's
/// pre-measured `widths`. Shared by [`SegmentedControl::paint`] and
/// [`SegmentedControl::show`] so the drawn segments and the interactive hit
/// rects never drift apart.
fn seg_rect_at(rect: Rect, widths: &[f32], index: usize) -> Rect {
    let x0 = rect.min.x + widths[..index].iter().sum::<f32>();
    Rect {
        min: Pos2::new(x0, rect.min.y),
        max: Pos2::new(x0 + widths[index], rect.max.y),
    }
}

/// The index of the `options` entry equal to `value`, or `None` if no entry
/// matches (a defined, non-panicking case — all segments render unselected).
///
/// Plain `fn` — `missing_const_for_fn` declines on `&str` comparison (design
/// § Key decision 2).
#[must_use]
pub fn selected_index(options: &[&str], value: &str) -> Option<usize> {
    options.iter().position(|&opt| opt == value)
}

#[cfg(test)]
mod tests {
    use super::{SegmentedControl, selected_index};
    use crate::tokens::{color, spacing, typography};
    use crate::widgets::Size;

    /// AC3 — selected → `GRAPHITE_900` bg + `PAPER_0` fg; unselected →
    /// transparent bg + `TEXT_BODY` fg.
    #[test]
    fn resolve_selected_vs_unselected_colors() {
        let selected = SegmentedControl::resolve(true, Size::Md);
        assert_eq!(selected.bg, color::GRAPHITE_900);
        assert_eq!(selected.fg, color::PAPER_0);

        let unselected = SegmentedControl::resolve(false, Size::Md);
        assert_eq!(unselected.bg, egui::Color32::TRANSPARENT);
        assert_eq!(unselected.fg, color::TEXT_BODY);
    }

    /// AC7 — size → height/font.
    #[test]
    fn resolve_size_maps_to_height_and_font() {
        let sm = SegmentedControl::resolve(false, Size::Sm);
        crate::test_util::assert_f32("sm height", sm.height, spacing::CONTROL_H_SM);
        crate::test_util::assert_f32("sm font", sm.font_size, typography::FS_SM);

        let md = SegmentedControl::resolve(false, Size::Md);
        crate::test_util::assert_f32("md height", md.height, spacing::CONTROL_H_MD);
        crate::test_util::assert_f32("md font", md.font_size, typography::FS_BODY);

        let lg = SegmentedControl::resolve(false, Size::Lg);
        crate::test_util::assert_f32("lg height", lg.height, spacing::CONTROL_H_LG);
        crate::test_util::assert_f32("lg font", lg.font_size, typography::FS_BODY);
    }

    /// AC3 — single-selection: exactly one index matches, and it's the
    /// right one.
    #[test]
    fn selected_index_finds_exactly_one_match() {
        let options = ["Rookie", "Pro", "Ace"];
        assert_eq!(selected_index(&options, "Pro"), Some(1));
        let match_count = options.iter().filter(|&&opt| opt == "Pro").count();
        assert_eq!(match_count, 1);
    }

    /// AC3 — a value matching no option is `None`.
    #[test]
    fn selected_index_none_when_no_match() {
        let options = ["Rookie", "Pro", "Ace"];
        assert_eq!(selected_index(&options, "Nope"), None);
    }

    /// Builder defaults: `new` starts `Size::Md`.
    #[test]
    fn new_has_expected_defaults() {
        let options = ["A", "B"];
        let control = SegmentedControl::new(&options, "A");
        assert_eq!(control.size, Size::Md);
        assert_eq!(control.value, "A");
    }
}
