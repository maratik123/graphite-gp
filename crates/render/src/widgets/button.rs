//! `Button` — port of `Button.jsx`/`Button.d.ts` (design § *Per-widget prop
//! surface* / *Style-mapping ground truth*, AC1/AC7).

use super::Size;
use crate::icons::{self, ICON_LOGICAL_SIZE_PX};
use crate::tokens::{color, spacing, typography};
use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, TextureHandle, Ui,
};

/// Button variant (`Button.d.ts` `variant`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Primary (accent-filled) action.
    Primary,
    /// Secondary (paper-filled, bordered) action.
    Secondary,
    /// Ghost (transparent, tinted-on-hover) action.
    Ghost,
    /// Danger (destructive) action.
    Danger,
}

/// Button horizontal padding, small size (`Button.jsx`: pad-x 12), equals
/// `spacing::SPACE_3`.
const PAD_X_SM: f32 = spacing::SPACE_3;
/// Button horizontal padding, medium size (pad-x 16), equals `spacing::SPACE_4`.
const PAD_X_MD: f32 = spacing::SPACE_4;
/// Button horizontal padding, large size (pad-x 22) — not a `spacing` token
/// (nearest tokens are 20/24), so a local module const per the magic-number
/// rule.
const PAD_X_LG: f32 = 22.0;
/// Icon/label gap, small size — not a `spacing` token (nearest are 4/8).
const GAP_SM: f32 = 6.0;
/// Icon/label gap, medium size, equals `spacing::SPACE_2`.
const GAP_MD: f32 = spacing::SPACE_2;
/// Icon/label gap, large size — not a `spacing` token.
const GAP_LG: f32 = 10.0;

/// The pure style-resolution output of [`Button::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonStyle {
    /// Fill color.
    pub bg: Color32,
    /// Foreground (label/icon) color.
    pub fg: Color32,
    /// Border color (`Color32::TRANSPARENT` for primary/ghost).
    pub border: Color32,
    /// Control height (`spacing::CONTROL_H_{SM,MD,LG}`).
    pub height: f32,
    /// Horizontal padding.
    pub pad_x: f32,
    /// Icon/label gap.
    pub gap: f32,
    /// Label font size.
    pub font_size: f32,
    /// Corner radius (`spacing::RADIUS_2`, deferred to `CornerRadius::from`
    /// at the paint site).
    pub radius: f32,
}

/// Button props (`Button.d.ts`): `variant`, `size`, `iconLeft`/`iconRight`,
/// `fullWidth`, `disabled` → `enabled`, `children` → `label`.
///
/// Not `Debug`: `egui::TextureHandle` (the icon-slot type) has no `Debug`
/// impl.
#[derive(Clone, Copy)]
pub struct Button<'a> {
    /// The variant.
    pub variant: Variant,
    /// The size.
    pub size: Size,
    /// The label text (`Button.d.ts` `children`).
    pub label: &'a str,
    /// Optional leading icon (a pre-baked handle from a caller-owned
    /// `icons::IconSet`).
    pub icon_left: Option<&'a TextureHandle>,
    /// Optional trailing icon.
    pub icon_right: Option<&'a TextureHandle>,
    /// Stretches to `ui.available_width()` when set.
    pub full_width: bool,
    /// `Button.d.ts` `disabled`, inverted.
    pub enabled: bool,
}

impl<'a> Button<'a> {
    /// Builds a medium, enabled `Primary` button with `label`.
    #[must_use]
    pub const fn new(label: &'a str) -> Self {
        Self {
            variant: Variant::Primary,
            size: Size::Md,
            label,
            icon_left: None,
            icon_right: None,
            full_width: false,
            enabled: true,
        }
    }

    /// Sets `variant`.
    #[must_use]
    pub const fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets `size`.
    #[must_use]
    pub const fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Sets `icon_left`.
    #[must_use]
    pub const fn icon_left(mut self, icon: &'a TextureHandle) -> Self {
        self.icon_left = Some(icon);
        self
    }

    /// Sets `icon_right`.
    #[must_use]
    pub const fn icon_right(mut self, icon: &'a TextureHandle) -> Self {
        self.icon_right = Some(icon);
        self
    }

    /// Sets `full_width`.
    #[must_use]
    pub const fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    /// Sets `enabled`.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The pure style-resolution layer (AC7): `(variant, size, hovered,
    /// pressed)` → colors + metrics. No `egui::Ui`, no allocation, no
    /// `disabled`-opacity (deferred to paint — see `common.rs`'s doc on the
    /// same pattern). Miri-clean.
    #[must_use]
    pub const fn resolve(
        variant: Variant,
        size: Size,
        hovered: bool,
        pressed: bool,
    ) -> ButtonStyle {
        let (bg_rest, bg_hover, bg_press, label_color, edge_color) = match variant {
            Variant::Primary => (
                color::ACCENT,
                color::ACCENT_HOVER,
                color::ACCENT_PRESS,
                color::TEXT_ON_ACCENT,
                Color32::TRANSPARENT,
            ),
            Variant::Secondary => (
                color::PAPER_0,
                color::PAPER_2,
                color::PAPER_3,
                color::TEXT_INK,
                color::BORDER_STRONG,
            ),
            Variant::Ghost => (
                Color32::TRANSPARENT,
                super::common::GHOST_HOVER_OVERLAY,
                super::common::GHOST_PRESS_OVERLAY,
                color::TEXT_BODY,
                Color32::TRANSPARENT,
            ),
            Variant::Danger => (
                color::DANGER_TINT,
                color::DANGER,
                color::ACCENT_PRESS,
                color::DANGER,
                color::DANGER,
            ),
        };
        let bg = if pressed {
            bg_press
        } else if hovered {
            bg_hover
        } else {
            bg_rest
        };
        // Danger's fg flips to text-on-accent once its bg goes solid
        // (hover/press); rest keeps the danger-red label on the tinted bg.
        let fg = if matches!(variant, Variant::Danger) && (hovered || pressed) {
            color::TEXT_ON_ACCENT
        } else {
            label_color
        };
        let (height, pad_x, gap, font_size) = match size {
            Size::Sm => (spacing::CONTROL_H_SM, PAD_X_SM, GAP_SM, typography::FS_SM),
            Size::Md => (spacing::CONTROL_H_MD, PAD_X_MD, GAP_MD, typography::FS_BODY),
            Size::Lg => (
                spacing::CONTROL_H_LG,
                PAD_X_LG,
                GAP_LG,
                typography::FS_TITLE,
            ),
        };
        ButtonStyle {
            bg,
            fg,
            border: edge_color,
            height,
            pad_x,
            gap,
            font_size,
            radius: spacing::RADIUS_2,
        }
    }

    /// Draws the resolved `style` into `rect`: bg/border/radius plus the
    /// 1-pt downward content nudge when pressed, `icon_left`/label/
    /// `icon_right`, honoring `enabled`'s opacity. The pressed-state
    /// inset-shadow band was dropped entirely (design § Amendment 3) — the
    /// darker press bg baked into `style.bg` is the sole press cue; `pressed`
    /// here only drives the content nudge, since `ButtonStyle` no longer
    /// carries a press-shadow field.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first — draws through
    /// `FontFamily::Name(fonts::ONEST_SEMIBOLD)`.
    #[allow(
        clippy::too_many_arguments,
        reason = "paint layer takes every resolved input explicitly, per the design's 3-layer split; splitting further would fragment one cohesive draw call"
    )]
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &ButtonStyle,
        label: &str,
        icon_left: Option<&TextureHandle>,
        icon_right: Option<&TextureHandle>,
        enabled: bool,
        pressed: bool,
    ) {
        let opacity = if enabled {
            1.0
        } else {
            super::common::DISABLED_OPACITY
        };
        let tint = |c: Color32| c.gamma_multiply(opacity);

        super::common::paint_surface(
            painter,
            rect,
            style.radius,
            tint(style.bg),
            tint(style.border),
            spacing::BW_1,
        );

        // 1-pt downward content nudge when pressed (enabled only).
        let nudge = if enabled && pressed { 1.0 } else { 0.0 };
        let icon_y = rect.min.y + (rect.height() - ICON_LOGICAL_SIZE_PX) / 2.0 + nudge;
        let mut cursor_x = rect.min.x + style.pad_x;
        if let Some(icon) = icon_left {
            let icon_rect = Rect {
                min: Pos2::new(cursor_x, icon_y),
                max: Pos2::new(
                    cursor_x + ICON_LOGICAL_SIZE_PX,
                    icon_y + ICON_LOGICAL_SIZE_PX,
                ),
            };
            icons::draw_icon(painter, icon, icon_rect, tint(style.fg));
            cursor_x += ICON_LOGICAL_SIZE_PX + style.gap;
        }
        let text_pos = Pos2::new(cursor_x, rect.center().y + nudge);
        let galley = painter.text(
            text_pos,
            Align2::LEFT_CENTER,
            label,
            FontId::new(
                style.font_size,
                FontFamily::Name(crate::fonts::ONEST_SEMIBOLD.into()),
            ),
            tint(style.fg),
        );
        if let Some(icon) = icon_right {
            let after_text_x = cursor_x + galley.size().x + style.gap;
            let icon_rect = Rect {
                min: Pos2::new(after_text_x, icon_y),
                max: Pos2::new(
                    after_text_x + ICON_LOGICAL_SIZE_PX,
                    icon_y + ICON_LOGICAL_SIZE_PX,
                ),
            };
            icons::draw_icon(painter, icon, icon_rect, tint(style.fg));
        }
    }

    /// Allocates the button's rect from measured content, reads live
    /// pointer input for hover/press, resolves the style, draws it, and
    /// returns the `Response` (`Response::clicked()` ≡ `Button.d.ts`'s
    /// `onClick`).
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(self, ui: &mut Ui) -> Response {
        // Measured with rest-state style: height/pad_x/gap/font_size don't
        // depend on hover/press, only bg/fg/border do.
        let metrics = Self::resolve(self.variant, self.size, false, false);
        let text_width = ui
            .painter()
            .layout_no_wrap(
                self.label.to_owned(),
                FontId::new(
                    metrics.font_size,
                    FontFamily::Name(crate::fonts::ONEST_SEMIBOLD.into()),
                ),
                metrics.fg,
            )
            .rect
            .width();
        let mut content_width = text_width;
        if self.icon_left.is_some() {
            content_width += ICON_LOGICAL_SIZE_PX + metrics.gap;
        }
        if self.icon_right.is_some() {
            content_width += ICON_LOGICAL_SIZE_PX + metrics.gap;
        }
        let min_width = metrics.pad_x.mul_add(2.0, content_width);
        let width = if self.full_width {
            ui.available_width().max(min_width)
        } else {
            min_width
        };

        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, metrics.height), sense);

        let hovered = self.enabled && response.hovered();
        let pressed = self.enabled && response.is_pointer_button_down_on();
        let style = Self::resolve(self.variant, self.size, hovered, pressed);

        if ui.is_rect_visible(rect) {
            Self::paint(
                ui.painter(),
                rect,
                &style,
                self.label,
                self.icon_left,
                self.icon_right,
                self.enabled,
                pressed,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, Size, Variant};
    use crate::tokens::{color, spacing};

    /// AC7 — primary at rest.
    #[test]
    fn primary_rest_uses_accent() {
        let style = Button::resolve(Variant::Primary, Size::Md, false, false);
        assert_eq!(style.bg, color::ACCENT);
        assert_eq!(style.fg, color::TEXT_ON_ACCENT);
        assert_eq!(style.border, egui::Color32::TRANSPARENT);
    }

    /// AC7 — primary hover darkens to `accent-hover`.
    #[test]
    fn primary_hover_uses_accent_hover() {
        let style = Button::resolve(Variant::Primary, Size::Md, true, false);
        assert_eq!(style.bg, color::ACCENT_HOVER);
    }

    /// AC7 — primary press uses `accent-press`.
    #[test]
    fn primary_press_uses_accent_press() {
        let style = Button::resolve(Variant::Primary, Size::Md, false, true);
        assert_eq!(style.bg, color::ACCENT_PRESS);
    }

    /// AC7 — size → height for every size.
    #[test]
    fn size_maps_to_control_height() {
        crate::tokens::css::assert_f32(
            "sm height",
            Button::resolve(Variant::Secondary, Size::Sm, false, false).height,
            spacing::CONTROL_H_SM,
        );
        crate::tokens::css::assert_f32(
            "md height",
            Button::resolve(Variant::Secondary, Size::Md, false, false).height,
            spacing::CONTROL_H_MD,
        );
        crate::tokens::css::assert_f32(
            "lg height",
            Button::resolve(Variant::Secondary, Size::Lg, false, false).height,
            spacing::CONTROL_H_LG,
        );
    }

    /// AC7 — pressed → the press bg token, per variant (design § Amendment 3
    /// — replaces the removed `press_shadow == Some(SHADOW_INSET)` assert,
    /// since `ButtonStyle` no longer has that field).
    #[test]
    fn pressed_yields_press_bg_token() {
        assert_eq!(
            Button::resolve(Variant::Primary, Size::Md, false, true).bg,
            color::ACCENT_PRESS
        );
        assert_eq!(
            Button::resolve(Variant::Secondary, Size::Md, false, true).bg,
            color::PAPER_3
        );
        assert_eq!(
            Button::resolve(Variant::Ghost, Size::Md, false, true).bg,
            super::super::common::GHOST_PRESS_OVERLAY
        );
        assert_eq!(
            Button::resolve(Variant::Danger, Size::Md, false, true).bg,
            color::ACCENT_PRESS
        );
    }

    /// AC7 — darker-press-bg-than-hover-bg invariant (design § Amendment 3 /
    /// Test Design → Darkness metric), for every variant. Composited over
    /// `PAPER_0` (identity for the opaque variants, mandatory for ghost —
    /// `Color32` is premultiplied, so ghost's raw overlay channels compare in
    /// the WRONG direction without compositing). Strict `<`, no
    /// `clippy::float_cmp` (that lint fires only on `==`/`!=`).
    #[test]
    fn press_bg_is_darker_than_hover_bg_for_every_variant() {
        for variant in [
            Variant::Primary,
            Variant::Secondary,
            Variant::Ghost,
            Variant::Danger,
        ] {
            let press_bg = Button::resolve(variant, Size::Md, false, true).bg;
            let hover_bg = Button::resolve(variant, Size::Md, true, false).bg;
            let press_intensity = color::PAPER_0.blend(press_bg).intensity();
            let hover_intensity = color::PAPER_0.blend(hover_bg).intensity();
            assert!(
                press_intensity < hover_intensity,
                "{variant:?}: press intensity {press_intensity} should be darker than hover intensity {hover_intensity}"
            );
        }
    }

    /// AC7 — danger's fg flips `DANGER` -> `TEXT_ON_ACCENT` once hovered or
    /// pressed (design § Test Design).
    #[test]
    fn danger_fg_flips_on_hover_and_press() {
        assert_eq!(
            Button::resolve(Variant::Danger, Size::Md, false, false).fg,
            color::DANGER
        );
        assert_eq!(
            Button::resolve(Variant::Danger, Size::Md, true, false).fg,
            color::TEXT_ON_ACCENT
        );
        assert_eq!(
            Button::resolve(Variant::Danger, Size::Md, false, true).fg,
            color::TEXT_ON_ACCENT
        );
    }

    /// AC7 — ghost hover/press bg use the non-token overlay consts.
    #[test]
    fn ghost_hover_and_press_use_overlay_consts() {
        assert_eq!(
            Button::resolve(Variant::Ghost, Size::Md, true, false).bg,
            super::super::common::GHOST_HOVER_OVERLAY
        );
        assert_eq!(
            Button::resolve(Variant::Ghost, Size::Md, false, true).bg,
            super::super::common::GHOST_PRESS_OVERLAY
        );
        assert_eq!(
            Button::resolve(Variant::Ghost, Size::Md, false, false).bg,
            egui::Color32::TRANSPARENT
        );
    }

    /// Secondary's border is `border-strong`; primary/ghost are transparent.
    #[test]
    fn secondary_has_a_border_primary_and_ghost_do_not() {
        assert_eq!(
            Button::resolve(Variant::Secondary, Size::Md, false, false).border,
            color::BORDER_STRONG
        );
        assert_eq!(
            Button::resolve(Variant::Primary, Size::Md, false, false).border,
            egui::Color32::TRANSPARENT
        );
        assert_eq!(
            Button::resolve(Variant::Ghost, Size::Md, false, false).border,
            egui::Color32::TRANSPARENT
        );
    }

    /// Builder defaults: `new` starts `Primary`/`Md`/enabled/non-full-width.
    #[test]
    fn new_has_expected_defaults() {
        let button = Button::new("Go");
        assert_eq!(button.variant, Variant::Primary);
        assert_eq!(button.size, Size::Md);
        assert!(button.enabled);
        assert!(!button.full_width);
        assert!(button.icon_left.is_none());
        assert!(button.icon_right.is_none());
    }
}
