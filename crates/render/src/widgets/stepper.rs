//! `Stepper` — port of `Stepper.jsx`/`Stepper.d.ts` (design § *Per-widget
//! prop surface* / *Style-mapping ground truth*, AC4).

use crate::tokens::{color, spacing, typography};
use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui,
};

/// `−`/`+` button side (`Stepper.jsx:16`).
const BTN_SIZE: f32 = 34.0;
/// Value cell minimum width (`Stepper.jsx:34`).
const VALUE_MIN_W: f32 = 40.0;
/// `−`/`+` glyph font size (`Stepper.jsx:18`).
const BTN_FS: f32 = 18.0;
/// Full box width: two buttons + the value cell.
const BOX_W: f32 = BTN_SIZE * 2.0 + VALUE_MIN_W;

/// A stepper direction, for [`stepped`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepDir {
    /// Increment (the `+` affordance).
    Up,
    /// Decrement (the `−` affordance).
    Down,
}

/// The pure style-resolution output of [`Stepper::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepperStyle {
    /// `−` glyph color (`TEXT_FAINT` when at `min`, else `TEXT_INK`, AC4).
    pub dec_fg: Color32,
    /// `+` glyph color (`TEXT_FAINT` when at `max`, else `TEXT_INK`, AC4).
    pub inc_fg: Color32,
    /// Container border color.
    pub border: Color32,
    /// Container fill.
    pub bg: Color32,
    /// Value cell foreground.
    pub value_fg: Color32,
    /// Inter-cell divider color.
    pub divider: Color32,
}

/// The response of [`Stepper::show`]: the whole-row `Response`, the
/// stepped+clamped current value, and whether it moved this frame.
#[derive(Debug)]
pub struct StepperResponse {
    /// The whole-row interaction response.
    pub response: Response,
    /// The stepped+clamped current value (updated on `−`/`+` click).
    pub value: i32,
    /// Whether the value changed this frame.
    pub changed: bool,
}

/// Stepper props (`Stepper.d.ts`): `value/min/max/step/label/disabled`.
#[derive(Clone, Copy, Debug)]
pub struct Stepper<'a> {
    /// The current value.
    pub value: i32,
    /// The minimum value.
    pub min: i32,
    /// The maximum value.
    pub max: i32,
    /// The step size.
    pub step: i32,
    /// Optional leading label.
    pub label: Option<&'a str>,
    /// `Stepper.d.ts` `disabled`, inverted.
    pub enabled: bool,
}

impl<'a> Stepper<'a> {
    /// Builds a `[0, 99]`-ranged, step-`1`, label-less, enabled stepper at
    /// `value`.
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self {
            value,
            min: 0,
            max: 99,
            step: 1,
            label: None,
            enabled: true,
        }
    }

    /// Sets `min`.
    #[must_use]
    pub const fn min(mut self, min: i32) -> Self {
        self.min = min;
        self
    }

    /// Sets `max`.
    #[must_use]
    pub const fn max(mut self, max: i32) -> Self {
        self.max = max;
        self
    }

    /// Sets `step`.
    #[must_use]
    pub const fn step(mut self, step: i32) -> Self {
        self.step = step;
        self
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

    /// The pure style-resolution layer (AC7): `(dec_disabled, inc_disabled)`
    /// → colors. No `egui::Ui`, no allocation — Miri-clean. FORCED `const
    /// fn`: pure selection over `color`/`spacing` consts + a struct literal
    /// (design § Key decision 2).
    #[must_use]
    pub const fn resolve(dec_disabled: bool, inc_disabled: bool) -> StepperStyle {
        StepperStyle {
            dec_fg: if dec_disabled {
                color::TEXT_FAINT
            } else {
                color::TEXT_INK
            },
            inc_fg: if inc_disabled {
                color::TEXT_FAINT
            } else {
                color::TEXT_INK
            },
            border: color::GRAPHITE_900,
            bg: color::PAPER_0,
            value_fg: color::TEXT_INK,
            divider: color::BORDER_HAIRLINE,
        }
    }

    /// Draws the resolved `style` into `rect`: an optional label row, then
    /// the bordered `[−|value|+]` box with hairline cell dividers.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first.
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &StepperStyle,
        value_text: &str,
        label: Option<&str>,
        enabled: bool,
    ) {
        let tint = super::common::tint_fn(enabled);

        if let Some(label) = label {
            super::common::paint_form_label(
                painter,
                Pos2::new(rect.min.x, rect.min.y),
                label,
                &tint,
            );
        }

        let (box_rect, dec_rect, value_rect, inc_rect) = cells(rect, label.is_some());

        painter.rect_filled(box_rect, spacing::RADIUS_2, tint(style.bg));
        let divider = Stroke::new(spacing::BW_HAIR, tint(style.divider));
        painter.vline(dec_rect.max.x, box_rect.y_range(), divider);
        painter.vline(value_rect.max.x, box_rect.y_range(), divider);
        // Drawn LAST so the box border sits on top of the full-height
        // dividers and covers their ends (egui equivalent of the .jsx
        // container's `overflow: hidden`) — round 2 fix for PR #95 review
        // thread T#3609371837 (inconsistent divider heights).
        painter.rect_stroke(
            box_rect,
            spacing::RADIUS_2,
            Stroke::new(spacing::BW_1, tint(style.border)),
            StrokeKind::Inside,
        );

        painter.text(
            dec_rect.center(),
            Align2::CENTER_CENTER,
            "\u{2212}",
            FontId::new(
                BTN_FS,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
            ),
            tint(style.dec_fg),
        );
        painter.text(
            value_rect.center(),
            Align2::CENTER_CENTER,
            value_text,
            FontId::new(
                typography::FS_TITLE,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_MEDIUM.into()),
            ),
            tint(style.value_fg),
        );
        painter.text(
            inc_rect.center(),
            Align2::CENTER_CENTER,
            "+",
            FontId::new(
                BTN_FS,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
            ),
            tint(style.inc_fg),
        );
    }

    /// Allocates the label + box column, reads live pointer input for the
    /// `−`/`+` hit rects (gated by `enabled` and the bound-disabled state),
    /// resolves the style, draws it, and returns a [`StepperResponse`].
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(self, ui: &mut Ui) -> StepperResponse {
        let has_label = self.label.is_some();
        let label_h = if has_label {
            typography::FS_XS.mul_add(typography::LH_SNUG, spacing::SPACE_2)
        } else {
            0.0
        };
        let height = label_h + spacing::CONTROL_H_MD;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(BOX_W, height), Sense::hover());

        let (_box_rect, dec_rect, _value_rect, inc_rect) = cells(rect, has_label);

        let dec_disabled_now = dec_disabled(self.value, self.min);
        let inc_disabled_now = inc_disabled(self.value, self.max);

        let dec_sense = if self.enabled && !dec_disabled_now {
            Sense::click()
        } else {
            Sense::hover()
        };
        let dec_response = ui.interact(dec_rect, response.id.with("dec"), dec_sense);
        let inc_sense = if self.enabled && !inc_disabled_now {
            Sense::click()
        } else {
            Sense::hover()
        };
        let inc_response = ui.interact(inc_rect, response.id.with("inc"), inc_sense);

        let (value, changed) = if dec_response.clicked() {
            (
                stepped(self.value, self.step, self.min, self.max, StepDir::Down),
                true,
            )
        } else if inc_response.clicked() {
            (
                stepped(self.value, self.step, self.min, self.max, StepDir::Up),
                true,
            )
        } else {
            (self.value, false)
        };

        let style = Self::resolve(dec_disabled(value, self.min), inc_disabled(value, self.max));
        if ui.is_rect_visible(rect) {
            Self::paint(
                ui.painter(),
                rect,
                &style,
                &value.to_string(),
                self.label,
                self.enabled,
            );
        }

        StepperResponse {
            response,
            value,
            changed,
        }
    }
}

/// The `[label][−|value|+]` cell geometry within `rect`: `(box_rect,
/// dec_rect, value_rect, inc_rect)`. Shared by [`Stepper::paint`] and
/// [`Stepper::show`] so the drawn cells and the interactive hit rects never
/// drift apart.
fn cells(rect: Rect, has_label: bool) -> (Rect, Rect, Rect, Rect) {
    let box_top = if has_label {
        typography::FS_XS.mul_add(typography::LH_SNUG, rect.min.y + spacing::SPACE_2)
    } else {
        rect.min.y
    };
    let box_rect = Rect {
        min: Pos2::new(rect.min.x, box_top),
        max: Pos2::new(rect.min.x + BOX_W, box_top + spacing::CONTROL_H_MD),
    };
    let dec_rect = Rect {
        min: box_rect.min,
        max: Pos2::new(box_rect.min.x + BTN_SIZE, box_rect.max.y),
    };
    let value_rect = Rect {
        min: Pos2::new(dec_rect.max.x, box_rect.min.y),
        max: Pos2::new(dec_rect.max.x + VALUE_MIN_W, box_rect.max.y),
    };
    let inc_rect = Rect {
        min: Pos2::new(value_rect.max.x, box_rect.min.y),
        max: box_rect.max,
    };
    (box_rect, dec_rect, value_rect, inc_rect)
}

/// Whether the `−` affordance is disabled: `value` is already at `min`
/// (AC4). FORCED `const fn` — a plain integer compare.
#[must_use]
pub const fn dec_disabled(value: i32, min: i32) -> bool {
    value <= min
}

/// Whether the `+` affordance is disabled: `value` is already at `max`
/// (AC4). FORCED `const fn`.
#[must_use]
pub const fn inc_disabled(value: i32, max: i32) -> bool {
    value >= max
}

/// Steps `value` by `step` in `dir` (saturating — no overflow panic at
/// `i32::MIN`/`i32::MAX`).
///
/// Then clamps into `[min, max]` with **manual** `if` comparisons —
/// `<i32 as Ord>::max`/`min`/`clamp` are `E0658` inside a `const fn` on this
/// toolchain (design § Key decision 2, Risks).
#[must_use]
pub const fn stepped(value: i32, step: i32, min: i32, max: i32, dir: StepDir) -> i32 {
    let stepped = match dir {
        StepDir::Up => value.saturating_add(step),
        StepDir::Down => value.saturating_sub(step),
    };
    if stepped < min {
        min
    } else if stepped > max {
        max
    } else {
        stepped
    }
}

#[cfg(test)]
mod tests {
    use super::{StepDir, Stepper, dec_disabled, inc_disabled, stepped};
    use crate::tokens::color;

    /// AC7 — `dec_fg`/`inc_fg` flip to `TEXT_FAINT` when that affordance is
    /// disabled, else `TEXT_INK`.
    #[test]
    fn resolve_fg_flips_on_disabled() {
        assert_eq!(Stepper::resolve(true, false).dec_fg, color::TEXT_FAINT);
        assert_eq!(Stepper::resolve(false, false).dec_fg, color::TEXT_INK);
        assert_eq!(Stepper::resolve(false, true).inc_fg, color::TEXT_FAINT);
        assert_eq!(Stepper::resolve(false, false).inc_fg, color::TEXT_INK);
    }

    /// Container border/dividers are the design's fixed tokens, regardless
    /// of disabled state.
    #[test]
    fn container_chrome_is_constant() {
        let style = Stepper::resolve(false, false);
        assert_eq!(style.border, color::GRAPHITE_900);
        assert_eq!(style.divider, color::BORDER_HAIRLINE);
        assert_eq!(style.bg, color::PAPER_0);
        assert_eq!(style.value_fg, color::TEXT_INK);
    }

    /// AC4 — `stepped` increments/decrements and clamps at the bounds.
    #[test]
    fn stepped_moves_and_clamps() {
        assert_eq!(stepped(5, 1, 2, 6, StepDir::Up), 6);
        assert_eq!(stepped(6, 1, 2, 6, StepDir::Up), 6);
        assert_eq!(stepped(2, 1, 2, 6, StepDir::Down), 2);
    }

    /// AC4 — `dec_disabled`/`inc_disabled` at and off the bounds.
    #[test]
    fn dec_inc_disabled_at_bounds() {
        assert!(dec_disabled(2, 2));
        assert!(!dec_disabled(3, 2));
        assert!(inc_disabled(6, 6));
        assert!(!inc_disabled(5, 6));
    }

    /// `i32::MIN` down-step saturates instead of panicking.
    #[test]
    fn down_step_at_i32_min_saturates() {
        assert_eq!(
            stepped(i32::MIN, 1, i32::MIN, i32::MAX, StepDir::Down),
            i32::MIN
        );
    }

    /// Builder defaults: `new` starts `[0,99]`, step 1, label-less, enabled.
    #[test]
    fn new_has_expected_defaults() {
        let stepper = Stepper::new(10);
        assert_eq!(stepper.min, 0);
        assert_eq!(stepper.max, 99);
        assert_eq!(stepper.step, 1);
        assert!(stepper.label.is_none());
        assert!(stepper.enabled);
    }
}
