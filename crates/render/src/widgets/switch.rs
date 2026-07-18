//! `Switch` — port of `Switch.jsx`/`Switch.d.ts` (design § *Per-widget prop
//! surface* / *Style-mapping ground truth*, AC2).

use crate::tokens::{color, spacing, typography};
use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, Ui};

/// Track width (`Switch.jsx:27` `width: 40`).
const TRACK_W: f32 = 40.0;
/// Track height (`Switch.jsx:28` `height: 22`).
const TRACK_H: f32 = 22.0;
/// Knob diameter (`Switch.jsx:31` `16×16`).
const KNOB_D: f32 = 16.0;
/// Knob inset from the track edge when unchecked. `Switch.jsx:33` hard-codes
/// `left: 2`, but the product owner overrode that to `TRACK_W - KNOB_ON_X -
/// KNOB_D = 4` (PR #95 review round 1): the `left: 2` off-state jammed the
/// knob against the track's left border, visually asymmetric with the 4px
/// gap the "on" state leaves on the right (`TRACK_W - KNOB_ON_X - KNOB_D`).
/// `KNOB_ON_X` is unchanged — the reviewer confirmed "on" reads correctly.
const KNOB_INSET: f32 = TRACK_W - KNOB_ON_X - KNOB_D;
/// Knob `x` offset (from the track's left edge) when checked
/// (`Switch.jsx:33` `left: checked ? 20 : 2`).
const KNOB_ON_X: f32 = 20.0;
/// Knob ring stroke width, equals `spacing::BW_1`.
const KNOB_RING_W: f32 = spacing::BW_1;
/// Gap between the track and the label (`Switch.jsx:36` `gap: 10`).
const LABEL_GAP: f32 = 10.0;

/// The pure style-resolution output of [`Switch::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwitchStyle {
    /// Track fill (`checked ? ACCENT : PAPER_3`, AC2).
    pub track: Color32,
    /// Track border color.
    pub track_border: Color32,
    /// Knob fill.
    pub knob_fill: Color32,
    /// Knob ring color.
    pub knob_ring: Color32,
}

/// The response of [`Switch::show`]: the whole-row `Response`, the
/// post-click `checked` state, and whether it changed this frame.
#[derive(Debug)]
pub struct SwitchResponse {
    /// The whole-row interaction response.
    pub response: Response,
    /// `checked` = post-click state (`!checked` on click).
    pub checked: bool,
    /// Whether the switch was toggled this frame.
    pub changed: bool,
}

/// Switch props (`Switch.d.ts`): `checked`, `label`, `disabled` → `enabled`.
#[derive(Clone, Copy, Debug)]
pub struct Switch<'a> {
    /// The checked state.
    pub checked: bool,
    /// Optional trailing label.
    pub label: Option<&'a str>,
    /// `Switch.d.ts` `disabled`, inverted.
    pub enabled: bool,
}

impl<'a> Switch<'a> {
    /// Builds an unchecked, label-less, enabled switch.
    #[must_use]
    pub const fn new(checked: bool) -> Self {
        Self {
            checked,
            label: None,
            enabled: true,
        }
    }

    /// Sets `label`.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Sets `enabled`.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The pure style-resolution layer (AC7): `checked` → colors. No
    /// `egui::Ui`, no allocation — Miri-clean.
    #[must_use]
    pub const fn resolve(checked: bool) -> SwitchStyle {
        SwitchStyle {
            track: if checked {
                color::ACCENT
            } else {
                color::PAPER_3
            },
            track_border: color::GRAPHITE_900,
            knob_fill: color::PAPER_0,
            knob_ring: color::GRAPHITE_900,
        }
    }

    /// Whether a click flips `checked`.
    #[must_use]
    pub const fn toggled(checked: bool) -> bool {
        !checked
    }

    /// Draws the resolved `style` into `rect` (a row starting at
    /// `rect.min`, `TRACK_H` tall): the pill track, the knob (positioned by
    /// `checked`), and an optional trailing label.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first, when `label` is `Some`.
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &SwitchStyle,
        checked: bool,
        label: Option<&str>,
        enabled: bool,
    ) {
        let opacity = if enabled {
            1.0
        } else {
            super::common::FORMS_DISABLED_OPACITY
        };
        let tint = |c: Color32| c.gamma_multiply(opacity);

        let track_rect = Rect {
            min: rect.min,
            max: Pos2::new(rect.min.x + TRACK_W, rect.min.y + TRACK_H),
        };
        painter.rect_filled(track_rect, spacing::RADIUS_PILL, tint(style.track));
        painter.rect_stroke(
            track_rect,
            spacing::RADIUS_PILL,
            Stroke::new(spacing::BW_1, tint(style.track_border)),
            egui::StrokeKind::Inside,
        );

        let knob_x = track_rect.min.x + if checked { KNOB_ON_X } else { KNOB_INSET };
        let knob_center = Pos2::new(knob_x + KNOB_D / 2.0, track_rect.center().y);
        painter.circle_filled(knob_center, KNOB_D / 2.0, tint(style.knob_fill));
        painter.circle_stroke(
            knob_center,
            KNOB_D / 2.0,
            Stroke::new(KNOB_RING_W, tint(style.knob_ring)),
        );

        if let Some(label) = label {
            painter.text(
                Pos2::new(track_rect.max.x + LABEL_GAP, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::new(
                    typography::FS_BODY,
                    FontFamily::Name(crate::fonts::ONEST_REGULAR.into()),
                ),
                tint(color::TEXT_BODY),
            );
        }
    }

    /// Allocates the row rect from measured content, reads live pointer
    /// input, resolves the style, draws it, and returns a [`SwitchResponse`].
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(self, ui: &mut Ui) -> SwitchResponse {
        let mut content_width = TRACK_W;
        if let Some(label) = self.label {
            let text_width = ui
                .painter()
                .layout_no_wrap(
                    label.to_owned(),
                    FontId::new(
                        typography::FS_BODY,
                        FontFamily::Name(crate::fonts::ONEST_REGULAR.into()),
                    ),
                    color::TEXT_BODY,
                )
                .rect
                .width();
            content_width += LABEL_GAP + text_width;
        }

        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(content_width, TRACK_H), sense);

        let clicked = self.enabled && response.clicked();
        let checked = if clicked {
            Self::toggled(self.checked)
        } else {
            self.checked
        };
        let style = Self::resolve(checked);

        if ui.is_rect_visible(rect) {
            Self::paint(
                ui.painter(),
                rect,
                &style,
                checked,
                self.label,
                self.enabled,
            );
        }

        SwitchResponse {
            response,
            checked,
            changed: clicked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Switch;
    use crate::tokens::color;

    /// AC2 — checked uses `ACCENT`, unchecked uses `PAPER_3`.
    #[test]
    fn checked_uses_accent_unchecked_uses_paper_3() {
        assert_eq!(Switch::resolve(true).track, color::ACCENT);
        assert_eq!(Switch::resolve(false).track, color::PAPER_3);
    }

    /// Knob fill/ring are constant regardless of `checked`.
    #[test]
    fn knob_colors_are_constant() {
        for checked in [false, true] {
            let style = Switch::resolve(checked);
            assert_eq!(style.knob_fill, color::PAPER_0);
            assert_eq!(style.knob_ring, color::GRAPHITE_900);
            assert_eq!(style.track_border, color::GRAPHITE_900);
        }
    }

    /// `toggled` flips the boolean.
    #[test]
    fn toggled_flips_checked() {
        assert!(!Switch::toggled(true));
        assert!(Switch::toggled(false));
    }

    /// Builder defaults: `new` starts label-less, enabled.
    #[test]
    fn new_has_expected_defaults() {
        let switch = Switch::new(false);
        assert!(!switch.checked);
        assert!(switch.label.is_none());
        assert!(switch.enabled);
    }
}
