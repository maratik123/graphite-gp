//! Shared widget infrastructure: the [`Size`] enum, the two non-token ghost
//! overlay colors, opacity consts, and the [`paint_surface`] draw helper.
//!
//! Style still comes only from `crate::tokens` — the two colors here
//! (`GHOST_HOVER_OVERLAY`/`GHOST_PRESS_OVERLAY`) are the design's flagged
//! 2-site non-token exception (design § *Non-token source colors*), lifted
//! here because Button and `IconButton` (2 files) both need them.

use egui::{Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind};

/// The three control sizes shared by every core widget that has one
/// (`Button`, `IconButton`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// Small.
    Sm,
    /// Medium — the default.
    Md,
    /// Large.
    Lg,
}

/// Ghost-variant hover overlay. `rgba(32,30,26,0.06)`, alpha
/// `round(0.06 * 255) = 15`; RGB channels equal `crate::tokens::color::GRAPHITE_900`.
/// Not a token — appears at 2 sites (Button ghost, `IconButton` ghost).
pub const GHOST_HOVER_OVERLAY: Color32 =
    Color32::from_rgba_unmultiplied_const(0x20, 0x1E, 0x1A, 15);
/// Ghost-variant press overlay. `rgba(32,30,26,0.12)`, alpha
/// `round(0.12 * 255) = 31`.
pub const GHOST_PRESS_OVERLAY: Color32 =
    Color32::from_rgba_unmultiplied_const(0x20, 0x1E, 0x1A, 31);

/// Disabled-state opacity multiplier applied at paint time (not inside any
/// `resolve`, since `Color32::gamma_multiply` is not const-stable).
pub const DISABLED_OPACITY: f32 = 0.45;

/// Grid-watermark opacity multiplier (Card's faint background grid).
pub const GRID_WATERMARK_OPACITY: f32 = 0.5;

/// Forms-widget disabled-state opacity multiplier, applied at paint time
/// (not inside any `resolve`, for the same reason as [`DISABLED_OPACITY`]).
/// `Slider.jsx:23`/`Switch.jsx:22`/`Stepper.jsx:30` all read `opacity:
/// disabled ? 0.5 : 1` — a 3-site shared const, distinct from the core
/// widgets' `DISABLED_OPACITY = 0.45` (design § Key decision 7;
/// `SegmentedControl` has no `disabled` prop, so it is not a 4th site).
#[allow(
    dead_code,
    reason = "consumed by switch.rs/slider.rs/stepper.rs in subtasks 2-4 (same PR); the test module below already reads it, but that is a separate --cfg test compilation unit"
)]
pub const FORMS_DISABLED_OPACITY: f32 = 0.5;

/// Draws a rounded-rect fill + border.
///
/// Shared by Button and `IconButton`, whose `paint` layers both fill/stroke a
/// rounded rect. `radius` is a raw token `f32`; `CornerRadius::from(f32)` is
/// applied here (deferred from `resolve`, which cannot call a non-const
/// `From` impl). Carries no press-shadow parameter — the pressed-state
/// inset-shadow band was dropped entirely (design § Amendment 3); the sole
/// press cue is the darker press bg already baked into `fill`.
pub(crate) fn paint_surface(
    painter: &Painter,
    rect: Rect,
    radius: f32,
    fill: Color32,
    border_color: Color32,
    border_width: f32,
) {
    let corner_radius = CornerRadius::from(radius);
    painter.rect_filled(rect, corner_radius, fill);
    if border_width > 0.0 {
        painter.rect_stroke(
            rect,
            corner_radius,
            Stroke::new(border_width, border_color),
            StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{FORMS_DISABLED_OPACITY, GHOST_HOVER_OVERLAY, GHOST_PRESS_OVERLAY};

    /// Pins the two non-token overlay STORED alpha values (`round(0.06*255)
    /// = 15`, `round(0.12*255) = 31`) as a tested contract, not a comment
    /// (design § Test Design, `common.rs` unit test). Integer `u8` field —
    /// no `float_cmp` concern.
    #[test]
    fn ghost_overlay_alphas_are_15_and_31() {
        assert_eq!(GHOST_HOVER_OVERLAY.a(), 15);
        assert_eq!(GHOST_PRESS_OVERLAY.a(), 31);
    }

    /// Pins the 3-site forms-widget disabled-opacity value (design § Test
    /// Design, `common.rs` unit test) as a tested contract, not a comment.
    #[test]
    fn forms_disabled_opacity_is_half() {
        crate::tokens::css::assert_f32("FORMS_DISABLED_OPACITY", FORMS_DISABLED_OPACITY, 0.5);
    }
}
