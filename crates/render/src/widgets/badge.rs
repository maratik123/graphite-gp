//! `Badge` — port of `Badge.jsx`/`Badge.d.ts` (design § *Per-widget prop
//! surface* / *Style-mapping ground truth*, AC3).
//!
//! Non-interactive pill; `show` still returns a `Response` (`Sense::hover()`)
//! for layout uniformity with the other widgets.

use crate::tokens::{color, spacing, typography};
use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Rect, Response, Sense, Ui};

/// Badge tone (`Badge.d.ts` `tone`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Neutral / default.
    Neutral,
    /// Accent (brand vermilion).
    Accent,
    /// Ok / success.
    Ok,
    /// Warn / caution.
    Warn,
    /// Danger / error.
    Danger,
}

/// Badge's tinted `ok` foreground. `#1E6B3C` — not a token, 1 site (design §
/// *Non-token source colors*).
pub const BADGE_OK_FG: Color32 = Color32::from_rgb(0x1E, 0x6B, 0x3C);
/// Badge's tinted `warn` foreground. `#8A6410` — not a token, 1 site.
pub const BADGE_WARN_FG: Color32 = Color32::from_rgb(0x8A, 0x64, 0x10);

/// Badge height (`Badge.jsx`: `height: 20`), equals `spacing::SPACE_5`.
const HEIGHT: f32 = spacing::SPACE_5;
/// Badge horizontal padding (`Badge.jsx`: `pad-x: 8`), equals `spacing::SPACE_2`.
const PAD_X: f32 = spacing::SPACE_2;

/// The pure style-resolution output of [`Badge::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadgeStyle {
    /// Fill color.
    pub bg: Color32,
    /// Foreground (label) color.
    pub fg: Color32,
    /// Border color (`Color32::TRANSPARENT` for `solid`).
    pub border: Color32,
    /// Pill radius (`spacing::RADIUS_PILL`, deferred to `CornerRadius::from`
    /// at the paint site — see `common.rs`'s doc on this pattern).
    pub radius: f32,
}

/// Badge props (`Badge.d.ts`): `tone`, `solid`, `children` → `label`.
#[derive(Clone, Copy, Debug)]
pub struct Badge<'a> {
    /// The tone.
    pub tone: Tone,
    /// Solid (filled) vs tinted.
    pub solid: bool,
    /// The label text (`Badge.d.ts` `children`).
    pub label: &'a str,
}

impl<'a> Badge<'a> {
    /// Builds a tinted (non-`solid`) badge with `label`.
    #[must_use]
    pub const fn new(tone: Tone, label: &'a str) -> Self {
        Self {
            tone,
            solid: false,
            label,
        }
    }

    /// Sets `solid`.
    #[must_use]
    pub const fn solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    /// The pure style-resolution layer (AC7): `(tone, solid)` → colors +
    /// radius. No `egui::Ui`, no allocation — Miri-clean.
    #[must_use]
    pub const fn resolve(tone: Tone, solid: bool) -> BadgeStyle {
        let (background, label_color, edge, opaque_fill) = match tone {
            Tone::Neutral => (
                color::PAPER_2,
                color::TEXT_BODY,
                color::BORDER_HAIRLINE,
                color::GRAPHITE_900,
            ),
            Tone::Accent => (
                color::ACCENT_TINT,
                color::ACCENT_PRESS,
                color::ACCENT,
                color::ACCENT,
            ),
            Tone::Ok => (color::OK_TINT, BADGE_OK_FG, color::OK, color::OK),
            Tone::Warn => (color::WARN_TINT, BADGE_WARN_FG, color::WARN, color::WARN),
            Tone::Danger => (
                color::DANGER_TINT,
                color::DANGER,
                color::DANGER,
                color::DANGER,
            ),
        };
        let (bg, fg, border) = if solid {
            (opaque_fill, color::PAPER_0, Color32::TRANSPARENT)
        } else {
            (background, label_color, edge)
        };
        BadgeStyle {
            bg,
            fg,
            border,
            radius: spacing::RADIUS_PILL,
        }
    }

    /// Draws the resolved `style` into `rect` with `label` centered.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] into the drawing [`egui::Context`]
    /// first — this draws through `FontFamily::Name(fonts::JETBRAINS_MONO_MEDIUM)`.
    pub(crate) fn paint(painter: &Painter, rect: Rect, style: &BadgeStyle, label: &str) {
        let corner_radius = CornerRadius::from(style.radius);
        painter.rect_filled(rect, corner_radius, style.bg);
        if style.border != Color32::TRANSPARENT {
            painter.rect_stroke(
                rect,
                corner_radius,
                egui::Stroke::new(spacing::BW_HAIR, style.border),
                egui::StrokeKind::Inside,
            );
        }
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(
                typography::FS_XS,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_MEDIUM.into()),
            ),
            style.fg,
        );
    }

    /// Draws the badge and allocates its pill-shaped rect.
    ///
    /// Non-interactive (`Sense::hover()`), consistent with `Badge.d.ts`
    /// carrying no `onClick`.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first (same precondition as the
    /// private `paint` layer this delegates to).
    pub fn show(self, ui: &mut Ui) -> Response {
        let style = Self::resolve(self.tone, self.solid);
        let text_width = ui
            .painter()
            .layout_no_wrap(
                self.label.to_owned(),
                FontId::new(
                    typography::FS_XS,
                    FontFamily::Name(crate::fonts::JETBRAINS_MONO_MEDIUM.into()),
                ),
                style.fg,
            )
            .rect
            .width();
        let desired = egui::vec2(PAD_X.mul_add(2.0, text_width), HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if ui.is_rect_visible(rect) {
            Self::paint(ui.painter(), rect, &style, self.label);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{BADGE_OK_FG, BADGE_WARN_FG, Badge, Tone};
    use crate::tokens::{color, spacing};
    use egui::Color32;

    /// AC7 — every tone, both `solid` and tinted, per the Badge mapping
    /// table (design § *Style-mapping ground truth*).
    #[test]
    fn resolve_matches_the_badge_mapping_table() {
        let cases: [(Tone, Color32, Color32, Color32, Color32); 5] = [
            (
                Tone::Neutral,
                color::PAPER_2,
                color::TEXT_BODY,
                color::BORDER_HAIRLINE,
                color::GRAPHITE_900,
            ),
            (
                Tone::Accent,
                color::ACCENT_TINT,
                color::ACCENT_PRESS,
                color::ACCENT,
                color::ACCENT,
            ),
            (Tone::Ok, color::OK_TINT, BADGE_OK_FG, color::OK, color::OK),
            (
                Tone::Warn,
                color::WARN_TINT,
                BADGE_WARN_FG,
                color::WARN,
                color::WARN,
            ),
            (
                Tone::Danger,
                color::DANGER_TINT,
                color::DANGER,
                color::DANGER,
                color::DANGER,
            ),
        ];

        for (tone, tint_bg, tint_fg, border, solid_bg) in cases {
            let tinted = Badge::resolve(tone, false);
            assert_eq!(tinted.bg, tint_bg, "{tone:?} tinted bg");
            assert_eq!(tinted.fg, tint_fg, "{tone:?} tinted fg");
            assert_eq!(tinted.border, border, "{tone:?} tinted border");

            let solid = Badge::resolve(tone, true);
            assert_eq!(solid.bg, solid_bg, "{tone:?} solid bg");
            assert_eq!(solid.fg, color::PAPER_0, "{tone:?} solid fg");
            assert_eq!(solid.border, Color32::TRANSPARENT, "{tone:?} solid border");
        }
    }

    /// AC7 — pill radius is `spacing::RADIUS_PILL` regardless of tone/solid.
    #[test]
    fn resolve_radius_is_always_pill() {
        crate::test_util::assert_f32(
            "Badge::resolve(Neutral, false).radius",
            Badge::resolve(Tone::Neutral, false).radius,
            spacing::RADIUS_PILL,
        );
        crate::test_util::assert_f32(
            "Badge::resolve(Danger, true).radius",
            Badge::resolve(Tone::Danger, true).radius,
            spacing::RADIUS_PILL,
        );
    }

    /// Builder defaults: `new` starts tinted (`solid == false`).
    #[test]
    fn new_defaults_to_tinted() {
        let badge = Badge::new(Tone::Accent, "NEW");
        assert!(!badge.solid);
        assert_eq!(badge.label, "NEW");
    }
}
