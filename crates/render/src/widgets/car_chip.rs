//! `CarChip` — port of `CarChip.jsx`/`CarChip.d.ts` (design § *Style-mapping
//! ground truth*, AC5/AC6/AC7).
//!
//! Non-interactive car token for rosters/standings; `show` returns a bare
//! `Response` (`Sense::hover()`), matching `Badge`/`Telemetry`/`LapMeter`.

use crate::tokens::{color, spacing, typography};
use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui,
};

/// Chip height (`CarChip.jsx`: `height: 34`) — not a token (nearest are
/// `CONTROL_H_SM = 30`/`_MD = 38`).
const HEIGHT: f32 = 34.0;
/// Gap between rank/dot/name/tag (`CarChip.jsx`: `gap: 10`) — not a token.
const GAP: f32 = 10.0;
/// Color-dot diameter (`CarChip.jsx`: `16×16 circle`).
const DOT_DIAMETER: f32 = 16.0;
/// Rank column minimum width (`CarChip.jsx`: `min-width: 18`).
const RANK_MIN_W: f32 = 18.0;
/// Kind-pill horizontal padding (`CarChip.jsx`: `padding: 1px 6px`).
const TAG_PAD_X: f32 = 6.0;
/// Kind-pill vertical padding (`CarChip.jsx`: `padding: 1px 6px`).
const TAG_PAD_Y: f32 = 1.0;
/// Chip left padding (`CarChip.jsx`: `padding: 0 12px 0 8px`), equals
/// `spacing::SPACE_2`.
const PAD_LEFT: f32 = spacing::SPACE_2;
/// Chip right padding (`CarChip.jsx`: `padding: 0 12px 0 8px`), equals
/// `spacing::SPACE_3`.
const PAD_RIGHT: f32 = spacing::SPACE_3;

/// Car kind (`CarChip.d.ts` `kind`): the player's car vs an AI opponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarKind {
    /// The player's car.
    You,
    /// An AI opponent.
    Ai,
}

impl CarKind {
    /// The uppercase pill label (`CarChip.jsx`: `You → "YOU"`, `Ai → "AI"`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::You => "YOU",
            Self::Ai => "AI",
        }
    }
}

/// The resolved style of the optional `kind` pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindTagStyle {
    /// Pill text (+ border) color.
    pub fg: Color32,
    /// Pill border color.
    pub border: Color32,
}

/// The pure style-resolution output of [`CarChip::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarChipStyle {
    /// Chip fill.
    pub bg: Color32,
    /// Chip border color.
    pub border: Color32,
    /// Chip border stroke width.
    pub border_width: f32,
    /// The `kind` pill's style, or `None` when `kind` is `None`.
    pub tag: Option<KindTagStyle>,
}

/// `CarChip` props (`CarChip.d.ts`): `color`, `name`, `rank`, `kind`, `active`.
#[derive(Clone, Copy, Debug)]
pub struct CarChip<'a> {
    /// The car's ramp color (`CarChip.d.ts` `color`, default `CAR_1`).
    pub color: Color32,
    /// The driver name.
    pub name: &'a str,
    /// The optional standings rank.
    pub rank: Option<u32>,
    /// The optional `You`/`Ai` kind pill.
    pub kind: Option<CarKind>,
    /// The active (raised) state.
    pub active: bool,
}

impl<'a> CarChip<'a> {
    /// Builds a resting, rank-less, kind-less chip for `name`, colored
    /// `CAR_1` (== `ACCENT`).
    #[must_use]
    pub const fn new(name: &'a str) -> Self {
        Self {
            color: color::CAR_1,
            name,
            rank: None,
            kind: None,
            active: false,
        }
    }

    /// Sets `color`.
    #[must_use]
    pub const fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Sets `rank`.
    #[must_use]
    pub const fn rank(mut self, rank: u32) -> Self {
        self.rank = Some(rank);
        self
    }

    /// Sets `kind`.
    #[must_use]
    pub const fn kind(mut self, kind: CarKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Sets `active`.
    #[must_use]
    pub const fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The pure style-resolution layer (AC7): `(active, kind)` → chip
    /// chrome + optional pill style. No `egui::Ui`, no allocation —
    /// Miri-clean (design Key decision 6).
    #[must_use]
    pub const fn resolve(active: bool, kind: Option<CarKind>) -> CarChipStyle {
        let (bg, border, border_width) = if active {
            (color::PAPER_2, color::GRAPHITE_900, spacing::BW_2)
        } else {
            (color::PAPER_0, color::BORDER_HAIRLINE, spacing::BW_HAIR)
        };
        let tag = match kind {
            Some(CarKind::You) => Some(KindTagStyle {
                fg: color::ACCENT,
                border: color::ACCENT,
            }),
            Some(CarKind::Ai) => Some(KindTagStyle {
                fg: color::TEXT_MUTED,
                border: color::BORDER_HAIRLINE,
            }),
            None => None,
        };
        CarChipStyle {
            bg,
            border,
            border_width,
            tag,
        }
    }

    /// Draws the resolved `style` into `rect`: an optional mono `rank`, a
    /// colored dot, the `name` in the UI face, and an optional mono `kind`
    /// pill.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first — draws through
    /// `FontFamily::Name(fonts::JETBRAINS_MONO_BOLD/_REGULAR/ONEST_MEDIUM)`.
    #[allow(
        clippy::too_many_arguments,
        reason = "paint layer takes every resolved input explicitly, per the design's 3-layer split"
    )]
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: CarChipStyle,
        dot_color: Color32,
        name: &str,
        rank: Option<u32>,
        kind: Option<CarKind>,
    ) {
        let corner_radius = spacing::RADIUS_1;
        painter.rect_filled(rect, corner_radius, style.bg);
        painter.rect_stroke(
            rect,
            corner_radius,
            Stroke::new(style.border_width, style.border),
            StrokeKind::Inside,
        );

        let mut cursor_x = rect.min.x + PAD_LEFT;

        if let Some(rank) = rank {
            let rank_font = FontId::new(
                typography::FS_TITLE,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_BOLD.into()),
            );
            painter.text(
                Pos2::new(cursor_x + RANK_MIN_W / 2.0, rect.center().y),
                Align2::CENTER_CENTER,
                rank.to_string(),
                rank_font,
                color::TEXT_INK,
            );
            cursor_x += RANK_MIN_W + GAP;
        }

        let dot_center = Pos2::new(cursor_x + DOT_DIAMETER / 2.0, rect.center().y);
        painter.circle_filled(dot_center, DOT_DIAMETER / 2.0, dot_color);
        painter.circle_stroke(
            dot_center,
            DOT_DIAMETER / 2.0,
            Stroke::new(spacing::BW_2, color::GRAPHITE_900),
        );
        cursor_x += DOT_DIAMETER + GAP;

        let name_font = FontId::new(
            typography::FS_BODY,
            FontFamily::Name(crate::fonts::ONEST_MEDIUM.into()),
        );
        let name_pos = Pos2::new(cursor_x, rect.center().y);
        let name_galley = painter.layout_no_wrap(name.to_owned(), name_font, color::TEXT_INK);
        let name_width = name_galley.rect.width();
        painter.text(
            name_pos,
            Align2::LEFT_CENTER,
            name,
            FontId::new(
                typography::FS_BODY,
                FontFamily::Name(crate::fonts::ONEST_MEDIUM.into()),
            ),
            color::TEXT_INK,
        );
        cursor_x += name_width;

        if let (Some(kind), Some(tag)) = (kind, style.tag) {
            cursor_x += GAP;
            let pill_font = FontId::new(
                typography::FS_MICRO,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
            );
            let label = kind.label();
            let text_width = painter
                .layout_no_wrap(label.to_owned(), pill_font.clone(), tag.fg)
                .rect
                .width();
            let pill_height = TAG_PAD_Y.mul_add(2.0, typography::FS_MICRO);
            let pill_rect = Rect::from_min_max(
                Pos2::new(cursor_x, rect.center().y - pill_height / 2.0),
                Pos2::new(
                    TAG_PAD_X.mul_add(2.0, cursor_x + text_width),
                    rect.center().y + pill_height / 2.0,
                ),
            );
            painter.rect_stroke(
                pill_rect,
                spacing::RADIUS_PILL,
                Stroke::new(spacing::BW_HAIR, tag.border),
                StrokeKind::Inside,
            );
            painter.text(
                pill_rect.center(),
                Align2::CENTER_CENTER,
                label,
                pill_font,
                tag.fg,
            );
        }
    }

    /// Draws the car chip and allocates its rect.
    ///
    /// Non-interactive (`Sense::hover()`), consistent with `CarChip.d.ts`
    /// carrying no `onClick`.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first (same precondition as the private
    /// `paint` layer this delegates to).
    pub fn show(self, ui: &mut Ui) -> Response {
        let style = Self::resolve(self.active, self.kind);

        let name_font = FontId::new(
            typography::FS_BODY,
            FontFamily::Name(crate::fonts::ONEST_MEDIUM.into()),
        );
        let name_width = ui
            .painter()
            .layout_no_wrap(self.name.to_owned(), name_font, color::TEXT_INK)
            .rect
            .width();

        let mut width = PAD_LEFT + DOT_DIAMETER + GAP + name_width + PAD_RIGHT;
        if self.rank.is_some() {
            width += RANK_MIN_W + GAP;
        }
        if let Some(kind) = self.kind {
            let pill_font = FontId::new(
                typography::FS_MICRO,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
            );
            let pill_text_width = ui
                .painter()
                .layout_no_wrap(kind.label().to_owned(), pill_font, color::TEXT_INK)
                .rect
                .width();
            width += TAG_PAD_X.mul_add(2.0, GAP + pill_text_width);
        }

        let desired = egui::vec2(width, HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if ui.is_rect_visible(rect) {
            Self::paint(
                ui.painter(),
                rect,
                style,
                self.color,
                self.name,
                self.rank,
                self.kind,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{CarChip, CarKind, KindTagStyle};
    use crate::tokens::{color, spacing};

    /// AC6 — car-ramp identity: index 0 (`CAR_1`) equals `ACCENT`, and
    /// `new`'s default color is `CAR_1`.
    #[test]
    fn car_ramp_identity() {
        assert_eq!(color::CAR_COLORS[0], color::ACCENT);
        assert_eq!(color::CAR_1, color::ACCENT);
        assert_eq!(CarChip::new("You").color, color::CAR_1);
    }

    /// AC6 — active vs resting chrome.
    #[test]
    fn resolve_active_vs_resting() {
        let active = CarChip::resolve(true, None);
        assert_eq!(active.bg, color::PAPER_2);
        assert_eq!(active.border, color::GRAPHITE_900);
        crate::test_util::assert_f32("active border_width", active.border_width, spacing::BW_2);

        let resting = CarChip::resolve(false, None);
        assert_eq!(resting.bg, color::PAPER_0);
        assert_eq!(resting.border, color::BORDER_HAIRLINE);
        crate::test_util::assert_f32(
            "resting border_width",
            resting.border_width,
            spacing::BW_HAIR,
        );
    }

    /// AC6 — `kind` → tag color.
    #[test]
    fn resolve_kind_maps_tag_color() {
        assert_eq!(
            CarChip::resolve(false, Some(CarKind::You)).tag,
            Some(KindTagStyle {
                fg: color::ACCENT,
                border: color::ACCENT,
            })
        );
        assert_eq!(
            CarChip::resolve(false, Some(CarKind::Ai)).tag,
            Some(KindTagStyle {
                fg: color::TEXT_MUTED,
                border: color::BORDER_HAIRLINE,
            })
        );
        assert_eq!(CarChip::resolve(false, None).tag, None);
    }

    /// `CarKind::label` mapping.
    #[test]
    fn kind_label_mapping() {
        assert_eq!(CarKind::You.label(), "YOU");
        assert_eq!(CarKind::Ai.label(), "AI");
    }

    /// Builder defaults: `new(name)`.
    #[test]
    fn new_has_expected_defaults() {
        let chip = CarChip::new("Rival Blue");
        assert_eq!(chip.color, color::CAR_1);
        assert_eq!(chip.name, "Rival Blue");
        assert!(chip.rank.is_none());
        assert!(chip.kind.is_none());
        assert!(!chip.active);
    }
}
