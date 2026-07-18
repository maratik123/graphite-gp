//! `Slider` — port of `Slider.jsx`/`Slider.d.ts` (design § *Per-widget prop
//! surface* / *Style-mapping ground truth*, AC1).

use crate::tokens::{color, effects, spacing, typography};
use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense,
    Shadow, Stroke, Ui,
};

/// Track height (`Slider.jsx:41` `height: 4`).
const TRACK_H: f32 = 4.0;
/// Thumb diameter (`Slider.jsx:45` `18×18`).
const THUMB_D: f32 = 18.0;
/// Readout row height (label + value line).
const ROW_H: f32 = 20.0;
/// Gap between the readout row and the track row, equals `spacing::SPACE_2`
/// (`Slider.jsx:24` `marginBottom: 8`).
const READOUT_GAP: f32 = spacing::SPACE_2;
/// The track row's total vertical space (must fit the taller thumb).
const TRACK_ROW_H: f32 = THUMB_D;

/// The pure style-resolution output of [`Slider::resolve`] (AC7). Stateless
/// — the Slider's palette does not vary with state; `disabled` is a
/// paint-time opacity uniform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderStyle {
    /// Track (unfilled) background.
    pub track_bg: Color32,
    /// Track fill (up to the thumb).
    pub fill: Color32,
    /// Thumb fill.
    pub thumb_fill: Color32,
    /// Thumb ring color.
    pub thumb_ring: Color32,
    /// Thumb drop shadow (`Slider.jsx:45` `boxShadow: var(--shadow-1)`).
    pub thumb_shadow: Shadow,
    /// Track height.
    pub track_h: f32,
    /// Thumb diameter.
    pub thumb_d: f32,
    /// Track/fill corner radius.
    pub radius: f32,
}

/// The response of [`Slider::show`]: the whole-row `Response`, the
/// snapped+clamped current value, and whether it moved this frame.
#[derive(Debug)]
pub struct SliderResponse {
    /// The whole-row interaction response.
    pub response: Response,
    /// The snapped+clamped current value (updated while dragged).
    pub value: f32,
    /// Whether the value moved this frame.
    pub changed: bool,
}

/// Slider props (`Slider.d.ts`): `value/min/max/step/label/showValue/format/disabled`.
///
/// `format` is a [`Slider::show`] parameter, not a stored field (keeps this
/// builder `Copy` — a stored closure would not be).
#[derive(Clone, Copy, Debug)]
pub struct Slider<'a> {
    /// The current value.
    pub value: f32,
    /// The minimum value.
    pub min: f32,
    /// The maximum value.
    pub max: f32,
    /// The snap step.
    pub step: f32,
    /// Optional leading label.
    pub label: Option<&'a str>,
    /// Whether to draw the value readout.
    pub show_value: bool,
    /// `Slider.d.ts` `disabled`, inverted.
    pub enabled: bool,
}

impl<'a> Slider<'a> {
    /// Builds a `[0, 100]`-ranged, step-`1`, label-less, value-shown,
    /// enabled slider at `value`.
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self {
            value,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            label: None,
            show_value: true,
            enabled: true,
        }
    }

    /// Sets `min`.
    #[must_use]
    pub const fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    /// Sets `max`.
    #[must_use]
    pub const fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Sets `step`.
    #[must_use]
    pub const fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Sets `label`.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Sets `show_value`.
    #[must_use]
    pub const fn show_value(mut self, show_value: bool) -> Self {
        self.show_value = show_value;
        self
    }

    /// Sets `enabled`.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The pure style-resolution layer (AC7): stateless — no `egui::Ui`, no
    /// allocation. Miri-clean.
    #[must_use]
    pub const fn resolve() -> SliderStyle {
        SliderStyle {
            track_bg: color::PAPER_3,
            fill: color::ACCENT,
            thumb_fill: color::PAPER_0,
            thumb_ring: color::GRAPHITE_900,
            thumb_shadow: effects::SHADOW_1,
            track_h: TRACK_H,
            thumb_d: THUMB_D,
            radius: spacing::RADIUS_PILL,
        }
    }

    /// Draws the resolved `style` into `rect`: an optional readout row
    /// (`label` uppercase mono left, `value_text` mono right, iff either is
    /// `Some`), then the track row (track bg + fill up to `fraction`, thumb
    /// with its `SHADOW_1` drop shadow drawn first per `Slider.jsx:45`).
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first, when `label`/`value_text` is
    /// `Some`.
    #[allow(
        clippy::too_many_arguments,
        reason = "paint layer takes every resolved input explicitly, per the design's 3-layer split"
    )]
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &SliderStyle,
        fraction: f32,
        label: Option<&str>,
        value_text: Option<&str>,
        enabled: bool,
    ) {
        let opacity = if enabled {
            1.0
        } else {
            super::common::FORMS_DISABLED_OPACITY
        };
        let tint = |c: Color32| c.gamma_multiply(opacity);

        let has_readout_row = label.is_some() || value_text.is_some();
        let track_y0 = if has_readout_row {
            if let Some(label) = label {
                painter.text(
                    Pos2::new(rect.min.x, rect.min.y),
                    Align2::LEFT_TOP,
                    label.to_uppercase(),
                    FontId::new(
                        typography::FS_XS,
                        FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
                    ),
                    tint(color::TEXT_MUTED),
                );
            }
            if let Some(value_text) = value_text {
                painter.text(
                    Pos2::new(rect.max.x, rect.min.y),
                    Align2::RIGHT_TOP,
                    value_text,
                    FontId::new(
                        typography::FS_SM,
                        FontFamily::Name(crate::fonts::JETBRAINS_MONO_MEDIUM.into()),
                    ),
                    tint(color::TEXT_INK),
                );
            }
            rect.min.y + ROW_H + READOUT_GAP
        } else {
            rect.min.y
        };

        let track_w = rect.width();
        let track_top = track_y0 + (TRACK_ROW_H - style.track_h) / 2.0;
        let track_rect = Rect {
            min: Pos2::new(rect.min.x, track_top),
            max: Pos2::new(rect.min.x + track_w, track_top + style.track_h),
        };
        painter.rect_filled(track_rect, style.radius, tint(style.track_bg));

        let fill_w = fraction * track_w;
        let fill_rect = Rect {
            min: track_rect.min,
            max: Pos2::new(track_rect.min.x + fill_w, track_rect.max.y),
        };
        painter.rect_filled(fill_rect, style.radius, tint(style.fill));

        let thumb_cy = track_y0 + TRACK_ROW_H / 2.0;
        let thumb_left = fraction.mul_add(track_w, rect.min.x) - style.thumb_d / 2.0;
        let thumb_rect = Rect {
            min: Pos2::new(thumb_left, thumb_cy - style.thumb_d / 2.0),
            max: Pos2::new(thumb_left + style.thumb_d, thumb_cy + style.thumb_d / 2.0),
        };
        let thumb_radius = CornerRadius::from(style.thumb_d / 2.0);
        let shadow = if enabled {
            style.thumb_shadow
        } else {
            Shadow {
                color: style.thumb_shadow.color.gamma_multiply(opacity),
                ..style.thumb_shadow
            }
        };
        painter.add(shadow.as_shape(thumb_rect, thumb_radius));
        painter.circle_filled(
            thumb_rect.center(),
            style.thumb_d / 2.0,
            tint(style.thumb_fill),
        );
        painter.circle_stroke(
            thumb_rect.center(),
            style.thumb_d / 2.0,
            Stroke::new(spacing::BW_2, tint(style.thumb_ring)),
        );
    }

    /// Allocates the full-width row, reads live pointer input for the
    /// drag-x, resolves the style, draws it, and returns a
    /// [`SliderResponse`]. `format` renders `value` for the readout
    /// (invoked only when `show_value` is set).
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(self, ui: &mut Ui, format: impl Fn(f32) -> String) -> SliderResponse {
        let width = ui.available_width();
        let has_readout_row = self.label.is_some() || self.show_value;
        let height = if has_readout_row {
            ROW_H + READOUT_GAP + TRACK_ROW_H
        } else {
            TRACK_ROW_H
        };
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), Sense::click_and_drag());

        let value = if self.enabled
            && (response.dragged() || response.clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let px = (pos.x - rect.min.x).clamp(0.0, width);
            let raw = if width > 0.0 {
                (px / width).mul_add(self.max - self.min, self.min)
            } else {
                self.min
            };
            snap_clamp(raw, self.min, self.max, self.step)
        } else {
            self.value
        };
        let changed = value.to_bits() != self.value.to_bits();

        let style = Self::resolve();
        let value_text = self.show_value.then(|| format(value));
        if ui.is_rect_visible(rect) {
            Self::paint(
                ui.painter(),
                rect,
                &style,
                fraction(value, self.min, self.max),
                self.label,
                value_text.as_deref(),
                self.enabled,
            );
        }

        SliderResponse {
            response,
            value,
            changed,
        }
    }
}

/// The fraction of `value` between `min` and `max`, clamped to `[0, 1]`.
///
/// Guards `max <= min` to `0.0` (no NaN from a `0/0` division in thumb/fill
/// positioning). Plain `fn` — `missing_const_for_fn` declines on `f32`
/// division (design § Key decision 2).
#[must_use]
pub fn fraction(value: f32, min: f32, max: f32) -> f32 {
    if max <= min {
        0.0
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    }
}

/// Clamps `value` to `[min, max]`, then snaps it to the nearest multiple of
/// `step` from `min` (a no-op snap when `step <= 0`). Plain `fn` — same
/// const-fn decline as [`fraction`].
#[must_use]
pub fn snap_clamp(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let clamped = value.clamp(min, max);
    if step <= 0.0 {
        return clamped;
    }
    let steps = ((clamped - min) / step).round();
    steps.mul_add(step, min).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::{Slider, fraction, snap_clamp};
    use crate::tokens::{color, effects, spacing};

    /// Tolerant `f32` compare — a computed snap result is not bit-identical
    /// to a decimal literal for every input (design § Test Design), so this
    /// is used instead of `crate::tokens::css::assert_f32`'s exact compare.
    fn assert_close(label: &str, got: f32, want: f32) {
        assert!(
            (got - want).abs() < 1e-5,
            "{label}: got {got}, want ~{want}"
        );
    }

    /// AC7 — `resolve()` colors + metrics.
    #[test]
    fn resolve_uses_expected_colors_and_metrics() {
        let style = Slider::resolve();
        assert_eq!(style.track_bg, color::PAPER_3);
        assert_eq!(style.fill, color::ACCENT);
        assert_eq!(style.thumb_fill, color::PAPER_0);
        assert_eq!(style.thumb_ring, color::GRAPHITE_900);
        assert_eq!(style.thumb_shadow, effects::SHADOW_1);
        crate::tokens::css::assert_f32("track_h", style.track_h, 4.0);
        crate::tokens::css::assert_f32("thumb_d", style.thumb_d, 18.0);
        crate::tokens::css::assert_f32("radius", style.radius, spacing::RADIUS_PILL);
    }

    /// AC1 — fractional-step snapping, incl. the ULP case.
    #[test]
    fn snap_clamp_fractional_steps() {
        assert_close("0.37", snap_clamp(0.37, 0.0, 1.0, 0.05), 0.35);
        assert_close("0.9 ulp", snap_clamp(0.9, 0.0, 1.0, 0.05), 0.90);
        assert_close("7.4 int-step", snap_clamp(7.4, 2.0, 12.0, 1.0), 7.0);
    }

    /// AC1 — out-of-range both ends clamp to the bounds.
    #[test]
    fn snap_clamp_out_of_range_clamps() {
        assert_close("below min", snap_clamp(-0.3, 0.0, 1.0, 0.05), 0.0);
        assert_close("above max", snap_clamp(1.5, 0.0, 1.0, 0.05), 1.0);
    }

    /// AC1 — step boundaries + `step <= 0` returns the clamp.
    #[test]
    fn snap_clamp_step_boundaries_and_non_positive_step() {
        let snapped = snap_clamp(0.025, 0.0, 1.0, 0.05);
        assert_close("boundary rounds to a step multiple", snapped % 0.05, 0.0);
        assert_close(
            "step <= 0 returns clamp",
            snap_clamp(0.6, 0.0, 1.0, 0.0),
            0.6,
        );
        assert_close(
            "negative step returns clamp",
            snap_clamp(0.6, 0.0, 1.0, -1.0),
            0.6,
        );
    }

    /// AC1 — `fraction`, incl. the `max <= min` NaN guard.
    #[test]
    fn fraction_computes_ratio_and_guards_degenerate_range() {
        assert_close("6 of [2,12]", fraction(6.0, 2.0, 12.0), 0.4);
        assert_close("degenerate range", fraction(3.0, 5.0, 5.0), 0.0);
    }

    /// Builder defaults: `new` starts `[0,100]`, step 1, label-less,
    /// value-shown, enabled.
    #[test]
    fn new_has_expected_defaults() {
        let slider = Slider::new(50.0);
        crate::tokens::css::assert_f32("min", slider.min, 0.0);
        crate::tokens::css::assert_f32("max", slider.max, 100.0);
        crate::tokens::css::assert_f32("step", slider.step, 1.0);
        assert!(slider.label.is_none());
        assert!(slider.show_value);
        assert!(slider.enabled);
    }
}
