//! `Telemetry` — port of `Telemetry.jsx`/`Telemetry.d.ts` (design § *Style-mapping
//! ground truth*, AC1/AC2/AC7/AC9).
//!
//! Non-interactive mono-face readout; `show` returns a bare `Response`
//! (`Sense::hover()`), matching `Badge`/`Tag`.

use crate::tokens::{color, spacing, typography};
use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Ui};

/// Gap between the label row and the value row (`Telemetry.jsx`: `gap: 3`) —
/// not a token (nearest is `spacing::SPACE_1 = 4`).
const LABEL_VALUE_GAP: f32 = 3.0;
/// Gap between the value and its trailing unit (`Telemetry.jsx`: `gap: 4`),
/// equals `spacing::SPACE_1`.
const VALUE_UNIT_GAP: f32 = spacing::SPACE_1;

/// Telemetry tone (`Telemetry.d.ts` `tone`). Distinct from `badge::Tone` —
/// Telemetry colors solid text, not a tint/fg pair (design Key decision 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Default (ink, or on-ink `PAPER_0`).
    Default,
    /// Accent (brand vermilion).
    Accent,
    /// Ok / success.
    Ok,
    /// Warn / caution.
    Warn,
    /// Danger / error.
    Danger,
    /// Muted.
    Muted,
}

/// Telemetry text alignment (`Telemetry.d.ts` `align`), a union → enum port
/// (AC9). Exported as `TelemetryAlign` to avoid colliding with `egui::Align`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// Left-aligned (the default).
    Left,
    /// Right-aligned.
    Right,
}

/// The pure style-resolution output of [`Telemetry::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelemetryStyle {
    /// The value's color, per `tone` (and `on_ink`).
    pub value_color: Color32,
    /// The label/unit/muted-value color (`TEXT_MUTED` off-ink,
    /// `TEXT_FAINT` on-ink).
    pub muted_color: Color32,
    /// The value's font size, per `size`.
    pub value_size: f32,
}

/// Telemetry props (`Telemetry.d.ts`): `label`, `value`, `unit`, `tone`,
/// `size`, `align`, plus the Rust-only `on_ink` mode (design Key decision 4 /
/// spec Q1).
#[derive(Clone, Copy, Debug)]
pub struct Telemetry<'a> {
    /// The uppercase label (`Telemetry.d.ts` `label`).
    pub label: &'a str,
    /// The mono value string (`Telemetry.d.ts` `value`).
    pub value: &'a str,
    /// The optional trailing unit.
    pub unit: Option<&'a str>,
    /// The value's tone.
    pub tone: Tone,
    /// The value's size.
    pub size: super::Size,
    /// Text alignment.
    pub align: Align,
    /// On-ink render mode (Rust-only field beyond the `.d.ts`, AC9): swaps
    /// `Default`/muted colors for their on-ink equivalents; semantic tones
    /// are unchanged.
    pub on_ink: bool,
}

impl<'a> Telemetry<'a> {
    /// Builds a `Default`-tone, `Md`-size, left-aligned, off-ink telemetry
    /// readout with `label` and `value`, no unit.
    #[must_use]
    pub const fn new(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            unit: None,
            tone: Tone::Default,
            size: super::Size::Md,
            align: Align::Left,
            on_ink: false,
        }
    }

    /// Sets the trailing unit.
    #[must_use]
    pub const fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Sets `tone`.
    #[must_use]
    pub const fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Sets `size`.
    #[must_use]
    pub const fn size(mut self, size: super::Size) -> Self {
        self.size = size;
        self
    }

    /// Sets `align`.
    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets `on_ink`.
    #[must_use]
    pub const fn on_ink(mut self, on_ink: bool) -> Self {
        self.on_ink = on_ink;
        self
    }

    /// The value font size for `size` (`Telemetry.jsx`'s size→px table).
    const fn value_font_size(size: super::Size) -> f32 {
        match size {
            super::Size::Sm => typography::FS_TITLE,
            super::Size::Md => typography::FS_H3,
            super::Size::Lg => typography::FS_H2,
        }
    }

    /// The pure style-resolution layer (AC7): `(tone, size, on_ink)` → colors
    /// + value size. No `egui::Ui`, no allocation — Miri-clean.
    #[must_use]
    pub const fn resolve(tone: Tone, size: super::Size, on_ink: bool) -> TelemetryStyle {
        let muted_color = if on_ink {
            color::TEXT_FAINT
        } else {
            color::TEXT_MUTED
        };
        let value_color = match tone {
            Tone::Default => {
                if on_ink {
                    color::PAPER_0
                } else {
                    color::TEXT_INK
                }
            }
            Tone::Accent => color::ACCENT,
            Tone::Ok => color::OK,
            Tone::Warn => color::WARN,
            Tone::Danger => color::DANGER,
            Tone::Muted => muted_color,
        };
        TelemetryStyle {
            value_color,
            muted_color,
            value_size: Self::value_font_size(size),
        }
    }

    /// Draws the resolved `style` into `rect`: an uppercased label row above
    /// a baseline value(+unit) row, `align`ed within `rect`.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first — draws through
    /// `FontFamily::Name(fonts::JETBRAINS_MONO_BOLD/_REGULAR)`.
    #[allow(
        clippy::too_many_arguments,
        reason = "paint layer takes every resolved input explicitly, per the design's 3-layer split"
    )]
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &TelemetryStyle,
        label: &str,
        value: &str,
        unit: Option<&str>,
        align: Align,
    ) {
        let (anchor_x, align2) = match align {
            Align::Left => (rect.min.x, Align2::LEFT_TOP),
            Align::Right => (rect.max.x, Align2::RIGHT_TOP),
        };

        let label_pos = Pos2::new(anchor_x, rect.min.y);
        let label_rect = painter.text(
            label_pos,
            align2,
            label.to_uppercase(),
            FontId::new(
                typography::FS_XS,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
            ),
            style.muted_color,
        );

        let value_top = label_rect.max.y + LABEL_VALUE_GAP;
        let value_pos = Pos2::new(anchor_x, value_top);
        let value_rect = painter.text(
            value_pos,
            align2,
            value,
            FontId::new(
                style.value_size,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_BOLD.into()),
            ),
            style.value_color,
        );

        if let Some(unit) = unit {
            let unit_x = match align {
                Align::Left => value_rect.max.x + VALUE_UNIT_GAP,
                Align::Right => value_rect.min.x - VALUE_UNIT_GAP,
            };
            let unit_align2 = match align {
                Align::Left => Align2::LEFT_BOTTOM,
                Align::Right => Align2::RIGHT_BOTTOM,
            };
            painter.text(
                Pos2::new(unit_x, value_rect.max.y),
                unit_align2,
                unit,
                FontId::new(
                    typography::FS_SM,
                    FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
                ),
                style.muted_color,
            );
        }
    }

    /// Draws the telemetry readout and allocates its rect.
    ///
    /// Non-interactive (`Sense::hover()`), consistent with `Telemetry.d.ts`
    /// carrying no `onClick`.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first (same precondition as the private
    /// `paint` layer this delegates to).
    pub fn show(self, ui: &mut Ui) -> Response {
        let style = Self::resolve(self.tone, self.size, self.on_ink);

        let label_font = FontId::new(
            typography::FS_XS,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        );
        let value_font = FontId::new(
            style.value_size,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_BOLD.into()),
        );
        let unit_font = FontId::new(
            typography::FS_SM,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        );

        let painter = ui.painter();
        let label_width = painter
            .layout_no_wrap(
                self.label.to_uppercase(),
                label_font.clone(),
                style.muted_color,
            )
            .rect
            .width();
        let value_width = painter
            .layout_no_wrap(self.value.to_owned(), value_font.clone(), style.value_color)
            .rect
            .width();
        let unit_width = self.unit.map_or(0.0, |unit| {
            VALUE_UNIT_GAP
                + painter
                    .layout_no_wrap(unit.to_owned(), unit_font, style.muted_color)
                    .rect
                    .width()
        });

        let width = label_width.max(value_width + unit_width);
        let label_height = painter
            .layout_no_wrap(self.label.to_uppercase(), label_font, style.muted_color)
            .rect
            .height();
        let value_height = painter
            .layout_no_wrap(self.value.to_owned(), value_font, style.value_color)
            .rect
            .height();
        let height = label_height + LABEL_VALUE_GAP + value_height;

        let desired = egui::vec2(width, height);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if ui.is_rect_visible(rect) {
            Self::paint(
                ui.painter(),
                rect,
                &style,
                self.label,
                self.value,
                self.unit,
                self.align,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{Align, Telemetry, Tone};
    use crate::tokens::{color, typography};
    use crate::widgets::Size;

    /// AC2 — off-ink tone → value color, per the mapping table.
    #[test]
    fn resolve_off_ink_tone_maps_value_color() {
        let cases: [(Tone, egui::Color32); 6] = [
            (Tone::Default, color::TEXT_INK),
            (Tone::Accent, color::ACCENT),
            (Tone::Ok, color::OK),
            (Tone::Warn, color::WARN),
            (Tone::Danger, color::DANGER),
            (Tone::Muted, color::TEXT_MUTED),
        ];
        for (tone, want) in cases {
            let style = Telemetry::resolve(tone, Size::Md, false);
            assert_eq!(style.value_color, want, "{tone:?} off-ink value_color");
            assert_eq!(
                style.muted_color,
                color::TEXT_MUTED,
                "{tone:?} off-ink muted_color"
            );
        }
    }

    /// AC2 — `size` → value font size, per the size→px table.
    #[test]
    fn resolve_size_maps_value_font_size() {
        let cases: [(Size, f32); 3] = [
            (Size::Sm, typography::FS_TITLE),
            (Size::Md, typography::FS_H3),
            (Size::Lg, typography::FS_H2),
        ];
        for (size, want) in cases {
            let style = Telemetry::resolve(Tone::Default, size, false);
            crate::tokens::css::assert_f32(&format!("{size:?} value_size"), style.value_size, want);
        }
    }

    /// AC2 — on-ink overrides: `Default` → `PAPER_0`, `Muted` → `TEXT_FAINT`,
    /// `muted_color` → `TEXT_FAINT` for every tone.
    #[test]
    fn resolve_on_ink_overrides_default_and_muted() {
        let default_style = Telemetry::resolve(Tone::Default, Size::Md, true);
        assert_eq!(default_style.value_color, color::PAPER_0);
        assert_eq!(default_style.muted_color, color::TEXT_FAINT);

        let muted_style = Telemetry::resolve(Tone::Muted, Size::Md, true);
        assert_eq!(muted_style.value_color, color::TEXT_FAINT);
        assert_eq!(muted_style.muted_color, color::TEXT_FAINT);
    }

    /// AC2 — off-ink `muted_color` is `TEXT_MUTED` (not `TEXT_FAINT`).
    #[test]
    fn resolve_off_ink_muted_color_is_text_muted() {
        let style = Telemetry::resolve(Tone::Default, Size::Md, false);
        assert_eq!(style.muted_color, color::TEXT_MUTED);
    }

    /// AC2 — on-ink leaves semantic tones unchanged.
    #[test]
    fn resolve_on_ink_leaves_semantic_tones_unchanged() {
        let cases: [(Tone, egui::Color32); 4] = [
            (Tone::Accent, color::ACCENT),
            (Tone::Ok, color::OK),
            (Tone::Warn, color::WARN),
            (Tone::Danger, color::DANGER),
        ];
        for (tone, want) in cases {
            let style = Telemetry::resolve(tone, Size::Md, true);
            assert_eq!(
                style.value_color, want,
                "{tone:?} on-ink value_color unchanged"
            );
        }
    }

    /// Builder defaults: `new(label, value)`.
    #[test]
    fn new_has_expected_defaults() {
        let telemetry = Telemetry::new("SPEED", "128");
        assert_eq!(telemetry.tone, Tone::Default);
        assert_eq!(telemetry.size, Size::Md);
        assert_eq!(telemetry.align, Align::Left);
        assert!(telemetry.unit.is_none());
        assert!(!telemetry.on_ink);
    }
}
