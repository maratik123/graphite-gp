//! `IconButton` — port of `IconButton.jsx`/`IconButton.d.ts` (design §
//! *Per-widget prop surface* / *Style-mapping ground truth*, AC2).

use super::Size;
use crate::icons;
use crate::tokens::{color, spacing};
use egui::{Color32, Painter, Pos2, Rect, Response, Sense, TextureHandle, Ui};

/// `IconButton` variant (`IconButton.d.ts` `variant`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Secondary (paper-filled, bordered).
    Secondary,
    /// Ghost (transparent, tinted-on-hover).
    Ghost,
}

/// The pure style-resolution output of [`IconButton::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconButtonStyle {
    /// Fill color.
    pub bg: Color32,
    /// Glyph tint color.
    pub fg: Color32,
    /// Border color (`Color32::TRANSPARENT` for ghost).
    pub border: Color32,
    /// Square side length (`spacing::CONTROL_H_{SM,MD,LG}`, per the
    /// `IconButton.jsx` `dim` table — numerically identical to the Button
    /// control heights).
    pub dim: f32,
    /// Corner radius (`spacing::RADIUS_2`).
    pub radius: f32,
}

/// `IconButton` props (`IconButton.d.ts`): glyph `children` → `icon`,
/// `label` → hover text, `variant`, `size`, `active`, `disabled` → `enabled`.
///
/// Not `Debug`: `egui::TextureHandle` (the icon type) has no `Debug` impl.
#[derive(Clone, Copy)]
pub struct IconButton<'a> {
    /// The glyph (required — `IconButton.d.ts`'s `children`).
    pub icon: &'a TextureHandle,
    /// Accessible/hover text (`IconButton.d.ts` `label`).
    pub label: &'a str,
    /// The variant.
    pub variant: Variant,
    /// The size.
    pub size: Size,
    /// The toggled-on state (graphite fill).
    pub active: bool,
    /// `IconButton.d.ts` `disabled`, inverted.
    pub enabled: bool,
}

impl<'a> IconButton<'a> {
    /// Builds a medium, enabled, non-`active` `Secondary` icon button.
    #[must_use]
    pub const fn new(icon: &'a TextureHandle, label: &'a str) -> Self {
        Self {
            icon,
            label,
            variant: Variant::Secondary,
            size: Size::Md,
            active: false,
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

    /// Sets `active`.
    #[must_use]
    pub const fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Sets `enabled`.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The square side length for `size` alone (state-independent — used by
    /// both [`Self::resolve`] and [`Self::show`]'s measuring pass).
    const fn dim_for(size: Size) -> f32 {
        match size {
            Size::Sm => spacing::CONTROL_H_SM,
            Size::Md => spacing::CONTROL_H_MD,
            Size::Lg => spacing::CONTROL_H_LG,
        }
    }

    /// The pure style-resolution layer (AC7): `(variant, size, active,
    /// hovered, pressed)` → colors + metrics. No `egui::Ui`, no allocation
    /// — Miri-clean.
    #[must_use]
    pub const fn resolve(
        variant: Variant,
        size: Size,
        active: bool,
        hovered: bool,
        pressed: bool,
    ) -> IconButtonStyle {
        let (bg, fg, border) = if active {
            (color::GRAPHITE_900, color::PAPER_0, color::GRAPHITE_900)
        } else {
            match variant {
                Variant::Ghost => {
                    let bg = if pressed {
                        super::common::GHOST_PRESS_OVERLAY
                    } else if hovered {
                        super::common::GHOST_HOVER_OVERLAY
                    } else {
                        Color32::TRANSPARENT
                    };
                    (bg, color::TEXT_INK, Color32::TRANSPARENT)
                }
                Variant::Secondary => {
                    let bg = if pressed {
                        color::PAPER_3
                    } else if hovered {
                        color::PAPER_2
                    } else {
                        color::PAPER_0
                    };
                    (bg, color::TEXT_INK, color::BORDER_STRONG)
                }
            }
        };
        IconButtonStyle {
            bg,
            fg,
            border,
            dim: Self::dim_for(size),
            radius: spacing::RADIUS_2,
        }
    }

    /// Draws the resolved `style` into `rect`: bg/border/radius and the
    /// centered `icons::ICON_LOGICAL_SIZE_PX` glyph, honoring `enabled`'s
    /// opacity. The pressed-state inset-shadow band was dropped entirely
    /// (design § Amendment 3) — press = the darker press bg baked into
    /// `style.bg` only, no band, no content nudge (`IconButton`'s mapping
    /// never carried a nudge).
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &IconButtonStyle,
        icon: &TextureHandle,
        enabled: bool,
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

        let half_gap = (rect.width() - icons::ICON_LOGICAL_SIZE_PX) / 2.0;
        let icon_rect = Rect {
            min: Pos2::new(rect.min.x + half_gap, rect.min.y + half_gap),
            max: Pos2::new(
                rect.min.x + half_gap + icons::ICON_LOGICAL_SIZE_PX,
                rect.min.y + half_gap + icons::ICON_LOGICAL_SIZE_PX,
            ),
        };
        icons::draw_icon(painter, icon, icon_rect, tint(style.fg));
    }

    /// Allocates the square rect, reads live pointer input for hover/press,
    /// resolves the style, draws it, and returns the `Response` with
    /// `label` bound as `on_hover_text` (tooltip + a11y).
    pub fn show(self, ui: &mut Ui) -> Response {
        let dim = Self::dim_for(self.size);
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(dim, dim), sense);

        let hovered = self.enabled && response.hovered();
        let pressed = self.enabled && response.is_pointer_button_down_on();
        let style = Self::resolve(self.variant, self.size, self.active, hovered, pressed);

        if ui.is_rect_visible(rect) {
            Self::paint(ui.painter(), rect, &style, self.icon, self.enabled);
        }
        response.on_hover_text(self.label)
    }
}

#[cfg(test)]
mod tests {
    use super::{IconButton, Size, Variant};
    use crate::tokens::{color, spacing};

    /// AC7 — `active` overrides variant/hover/press with the graphite fill.
    #[test]
    fn active_uses_graphite_fill() {
        let style = IconButton::resolve(Variant::Ghost, Size::Md, true, true, true);
        assert_eq!(style.bg, color::GRAPHITE_900);
        assert_eq!(style.fg, color::PAPER_0);
        assert_eq!(style.border, color::GRAPHITE_900);
    }

    /// AC7 — ghost rest/hover/press, non-active.
    #[test]
    fn ghost_rest_hover_press() {
        let rest = IconButton::resolve(Variant::Ghost, Size::Md, false, false, false);
        assert_eq!(rest.bg, egui::Color32::TRANSPARENT);
        assert_eq!(rest.border, egui::Color32::TRANSPARENT);

        let hover = IconButton::resolve(Variant::Ghost, Size::Md, false, true, false);
        assert_eq!(hover.bg, super::super::common::GHOST_HOVER_OVERLAY);

        let press = IconButton::resolve(Variant::Ghost, Size::Md, false, false, true);
        assert_eq!(press.bg, super::super::common::GHOST_PRESS_OVERLAY);
    }

    /// AC7 — secondary rest/hover/press, non-active.
    #[test]
    fn secondary_rest_hover_press() {
        let rest = IconButton::resolve(Variant::Secondary, Size::Md, false, false, false);
        assert_eq!(rest.bg, color::PAPER_0);
        assert_eq!(rest.border, color::BORDER_STRONG);

        let hover = IconButton::resolve(Variant::Secondary, Size::Md, false, true, false);
        assert_eq!(hover.bg, color::PAPER_2);

        let press = IconButton::resolve(Variant::Secondary, Size::Md, false, false, true);
        assert_eq!(press.bg, color::PAPER_3);
    }

    /// AC7 — size → square dim (30/38/46).
    #[test]
    fn size_maps_to_dim() {
        crate::tokens::css::assert_f32(
            "sm dim",
            IconButton::resolve(Variant::Secondary, Size::Sm, false, false, false).dim,
            spacing::CONTROL_H_SM,
        );
        crate::tokens::css::assert_f32(
            "md dim",
            IconButton::resolve(Variant::Secondary, Size::Md, false, false, false).dim,
            spacing::CONTROL_H_MD,
        );
        crate::tokens::css::assert_f32(
            "lg dim",
            IconButton::resolve(Variant::Secondary, Size::Lg, false, false, false).dim,
            spacing::CONTROL_H_LG,
        );
    }

    /// AC7 — pressed → the press bg token, for secondary + ghost (design §
    /// Amendment 3 — replaces the removed `press_shadow` assert, since
    /// `IconButtonStyle` no longer has that field).
    #[test]
    fn pressed_yields_press_bg_token() {
        assert_eq!(
            IconButton::resolve(Variant::Secondary, Size::Md, false, false, true).bg,
            color::PAPER_3
        );
        assert_eq!(
            IconButton::resolve(Variant::Ghost, Size::Md, false, false, true).bg,
            super::super::common::GHOST_PRESS_OVERLAY
        );
    }

    /// AC7 — darker-press-bg-than-hover-bg invariant (design § Amendment 3 /
    /// Test Design → Darkness metric), for secondary + ghost. Composited over
    /// `PAPER_0` (identity for secondary, mandatory for ghost — `Color32` is
    /// premultiplied). Strict `<`, no `clippy::float_cmp`.
    #[test]
    fn press_bg_is_darker_than_hover_bg_for_secondary_and_ghost() {
        for variant in [Variant::Secondary, Variant::Ghost] {
            let press_bg = IconButton::resolve(variant, Size::Md, false, false, true).bg;
            let hover_bg = IconButton::resolve(variant, Size::Md, false, true, false).bg;
            let press_intensity = color::PAPER_0.blend(press_bg).intensity();
            let hover_intensity = color::PAPER_0.blend(hover_bg).intensity();
            assert!(
                press_intensity < hover_intensity,
                "{variant:?}: press intensity {press_intensity} should be darker than hover intensity {hover_intensity}"
            );
        }
    }
}
