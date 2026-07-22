//! `Tag` — port of `Tag.jsx`/`Tag.d.ts` (design § *Per-widget prop
//! surface* / *Style-mapping ground truth*, AC4).

use crate::tokens::{color, spacing, typography};
use egui::{
    Align2, Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui,
};
use std::sync::Arc;

/// Tag height (`Tag.jsx`: `height: 26`) — not a `spacing` token (nearest
/// are 24/32), so a local module const per the magic-number rule.
const HEIGHT: f32 = 26.0;
/// Tag horizontal padding (`Tag.jsx`: `pad-x: 10`) — not a `spacing` token
/// (nearest are 8/12).
const PAD_X: f32 = 10.0;
/// Gap between the dot / label / remove affordance. `spacing::SPACE_1`.
const GAP: f32 = spacing::SPACE_1;
/// Color-dot diameter (`Tag.jsx`: `10×10 circle`).
const DOT_DIAMETER: f32 = 10.0;
/// Remove-affordance square side (`Tag.jsx`: `16×16`).
const REMOVE_SIZE: f32 = 16.0;
/// Remove-affordance corner radius (`Tag.jsx`: `radius-1`), `spacing::RADIUS_1`.
const REMOVE_RADIUS: f32 = spacing::RADIUS_1;

/// The pure style-resolution output of [`Tag::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TagStyle {
    /// Fill color.
    pub bg: Color32,
    /// Label + dot-ring color.
    pub fg: Color32,
    /// Border color.
    pub border: Color32,
    /// Border stroke width (`spacing::BW_HAIR` rest, `spacing::BW_1` selected).
    pub border_width: f32,
}

/// The response of [`Tag::show`]: the whole-chip `Response`, plus whether
/// the remove (×) affordance was clicked this frame (`Tag.d.ts`'s
/// `onRemove`).
#[derive(Debug)]
pub struct TagResponse {
    /// The whole-chip interaction response.
    pub response: Response,
    /// Whether the remove affordance was clicked.
    pub remove_clicked: bool,
}

/// Tag props (`Tag.d.ts`): `color` → `dot_color`, `onRemove` →
/// `show_remove`, `selected`, `children` → `label`.
#[derive(Clone, Copy, Debug)]
pub struct Tag<'a> {
    /// The label text (`Tag.d.ts` `children`).
    pub label: &'a str,
    /// The selected state (`bw-1` `border-strong` border).
    pub selected: bool,
    /// The optional leading color dot (`Tag.d.ts` `color`).
    pub dot_color: Option<Color32>,
    /// Whether to draw the remove (×) affordance.
    pub show_remove: bool,
}

/// Builds the tag's label galley once, at `fg` (`TagStyle::fg`, always
/// `TEXT_INK` — see `Tag::resolve`) — the single shaping pass shared by
/// [`Tag::show`] and its direct `paint`-layer gallery-harness callers
/// (`gallery.rs`, 4 sites), so `show`/the gallery/`paint` all agree by
/// construction (AC3).
pub(crate) fn tag_label_galley(painter: &Painter, label: &str, fg: Color32) -> Arc<Galley> {
    painter.layout_no_wrap(
        label.to_owned(),
        FontId::new(
            typography::FS_SM,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        ),
        fg,
    )
}

impl<'a> Tag<'a> {
    /// Builds a non-selected, dot-less, non-removable tag with `label`.
    #[must_use]
    pub const fn new(label: &'a str) -> Self {
        Self {
            label,
            selected: false,
            dot_color: None,
            show_remove: false,
        }
    }

    /// Sets `selected`.
    #[must_use]
    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets `dot_color`.
    #[must_use]
    pub const fn dot_color(mut self, color: Color32) -> Self {
        self.dot_color = Some(color);
        self
    }

    /// Sets `show_remove`.
    #[must_use]
    pub const fn show_remove(mut self, show_remove: bool) -> Self {
        self.show_remove = show_remove;
        self
    }

    /// The pure style-resolution layer (AC7): `selected` → colors. No
    /// `egui::Ui`, no allocation — Miri-clean.
    #[must_use]
    pub const fn resolve(selected: bool) -> TagStyle {
        if selected {
            TagStyle {
                bg: color::PAPER_2,
                fg: color::TEXT_INK,
                border: color::BORDER_STRONG,
                border_width: spacing::BW_1,
            }
        } else {
            TagStyle {
                bg: color::PAPER_0,
                fg: color::TEXT_INK,
                border: color::BORDER_HAIRLINE,
                border_width: spacing::BW_HAIR,
            }
        }
    }

    /// Draws the resolved `style` into `rect` (radius-0 — a square chip):
    /// fill/border, an optional leading color dot, the mono label, and an
    /// optional remove (×) affordance sub-rect (`remove_hovered` controls
    /// its hover fill).
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first — draws through
    /// `FontFamily::Name(fonts::JETBRAINS_MONO_REGULAR)`.
    #[allow(
        clippy::too_many_arguments,
        reason = "paint layer takes every resolved input explicitly, per the design's 3-layer split"
    )]
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &TagStyle,
        label_galley: &Arc<Galley>,
        dot_color: Option<Color32>,
        show_remove: bool,
        remove_hovered: bool,
    ) {
        painter.rect_filled(rect, 0, style.bg);
        painter.rect_stroke(
            rect,
            0,
            Stroke::new(style.border_width, style.border),
            StrokeKind::Inside,
        );

        let mut cursor_x = rect.min.x + PAD_X;
        if let Some(dot) = dot_color {
            let center = Pos2::new(cursor_x + DOT_DIAMETER / 2.0, rect.center().y);
            painter.circle_filled(center, DOT_DIAMETER / 2.0, dot);
            painter.circle_stroke(
                center,
                DOT_DIAMETER / 2.0,
                Stroke::new(spacing::BW_1, color::GRAPHITE_900),
            );
            cursor_x += DOT_DIAMETER + GAP;
        }

        let remove_left = rect.max.x - PAD_X - REMOVE_SIZE;

        crate::text::paint_galley(
            painter,
            Pos2::new(cursor_x, rect.center().y),
            Align2::LEFT_CENTER,
            label_galley.clone(),
            style.fg,
        );

        if show_remove {
            let remove_rect = Rect {
                min: Pos2::new(remove_left, rect.center().y - REMOVE_SIZE / 2.0),
                max: Pos2::new(
                    remove_left + REMOVE_SIZE,
                    rect.center().y + REMOVE_SIZE / 2.0,
                ),
            };
            if remove_hovered {
                painter.rect_filled(remove_rect, REMOVE_RADIUS, color::PAPER_3);
            }
            let inset = 4.0;
            let top_left = Pos2::new(remove_rect.min.x + inset, remove_rect.min.y + inset);
            let bottom_right = Pos2::new(remove_rect.max.x - inset, remove_rect.max.y - inset);
            let top_right = Pos2::new(remove_rect.max.x - inset, remove_rect.min.y + inset);
            let bottom_left = Pos2::new(remove_rect.min.x + inset, remove_rect.max.y - inset);
            let stroke = Stroke::new(spacing::BW_1, color::TEXT_MUTED);
            painter.line_segment([top_left, bottom_right], stroke);
            painter.line_segment([top_right, bottom_left], stroke);
        }
    }

    /// Allocates the chip rect from measured content, reads live pointer
    /// input for the remove affordance's hover, resolves the style, draws
    /// it, and returns a [`TagResponse`].
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(self, ui: &mut Ui) -> TagResponse {
        let style = Self::resolve(self.selected);
        let label_galley = tag_label_galley(ui.painter(), self.label, style.fg);

        let mut content_width = label_galley.size().x;
        if self.dot_color.is_some() {
            content_width += DOT_DIAMETER + GAP;
        }
        if self.show_remove {
            content_width += REMOVE_SIZE + GAP;
        }
        let desired = egui::vec2(PAD_X.mul_add(2.0, content_width), HEIGHT);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

        let remove_rect = Rect {
            min: Pos2::new(
                rect.max.x - PAD_X - REMOVE_SIZE,
                rect.center().y - REMOVE_SIZE / 2.0,
            ),
            max: Pos2::new(rect.max.x - PAD_X, rect.center().y + REMOVE_SIZE / 2.0),
        };
        let remove_response = self
            .show_remove
            .then(|| ui.interact(remove_rect, response.id.with("remove"), Sense::click()));
        let remove_hovered = remove_response.as_ref().is_some_and(Response::hovered);
        let remove_clicked = remove_response.is_some_and(|r| r.clicked());

        if ui.is_rect_visible(rect) {
            Self::paint(
                ui.painter(),
                rect,
                &style,
                &label_galley,
                self.dot_color,
                self.show_remove,
                remove_hovered,
            );
        }

        TagResponse {
            response,
            remove_clicked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Tag;
    use crate::tokens::{color, spacing};

    /// AC7 — rest: `paper-0` + `bw-hair` `border-hairline`.
    #[test]
    fn rest_uses_paper_0_and_hairline_border() {
        let style = Tag::resolve(false);
        assert_eq!(style.bg, color::PAPER_0);
        assert_eq!(style.border, color::BORDER_HAIRLINE);
        crate::test_util::assert_f32("rest border_width", style.border_width, spacing::BW_HAIR);
    }

    /// AC7 — selected: `paper-2` + `bw-1` `border-strong` (the design's
    /// per-component mapping — `BW_1 = 1.5px`, not the AC prose's "2-pt").
    #[test]
    fn selected_uses_paper_2_and_strong_border() {
        let style = Tag::resolve(true);
        assert_eq!(style.bg, color::PAPER_2);
        assert_eq!(style.border, color::BORDER_STRONG);
        crate::test_util::assert_f32("selected border_width", style.border_width, spacing::BW_1);
    }

    /// `fg` is always `text-ink`, regardless of `selected`.
    #[test]
    fn fg_is_always_text_ink() {
        assert_eq!(Tag::resolve(false).fg, color::TEXT_INK);
        assert_eq!(Tag::resolve(true).fg, color::TEXT_INK);
    }

    /// Builder defaults: `new` starts non-selected, dot-less, non-removable.
    #[test]
    fn new_has_expected_defaults() {
        let tag = Tag::new("L3");
        assert!(!tag.selected);
        assert!(tag.dot_color.is_none());
        assert!(!tag.show_remove);
    }
}
