//! `Card` — port of `Card.jsx`/`Card.d.ts` (design § *Per-widget prop
//! surface* / *Style-mapping ground truth*, AC5).

use crate::tokens::{color, effects, spacing, typography};
use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, LayerId, Painter, Pos2, Rangef, Rect,
    Response, Sense, Shadow, Stroke, StrokeKind, Ui,
};

/// Card elevation (`Card.d.ts` `elevation: 0|1|2|3`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Elevation {
    /// No shadow.
    Level0,
    /// `--shadow-1` — the default.
    #[default]
    Level1,
    /// `--shadow-2`.
    Level2,
    /// `--shadow-3`.
    Level3,
}

/// The pure style-resolution output of [`Card::resolve`] (AC7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardStyle {
    /// Fill color (`color::SURFACE_CARD`).
    pub fill: Color32,
    /// Border color (`bw-hair` `border-hairline` rest, `bw-2`
    /// `border-strong` selected).
    pub border: Color32,
    /// Border stroke width.
    pub border_width: f32,
    /// Corner radius (`spacing::RADIUS_2`).
    pub radius: f32,
    /// The elevation shadow.
    pub shadow: Shadow,
}

/// Card props (`Card.d.ts`): `title`/`eyebrow`, `grid`, `selected`,
/// `elevation`, `padding`.
///
/// `right` (an optional header-right closure) and the body `children` are
/// taken directly by [`Card::show`], not stored here, to keep this builder
/// `Copy`.
#[derive(Clone, Copy, Debug)]
pub struct Card<'a> {
    /// Optional eyebrow (small caps label above the title).
    pub eyebrow: Option<&'a str>,
    /// Optional title.
    pub title: Option<&'a str>,
    /// Whether to paint the faint graph-paper grid watermark.
    pub grid: bool,
    /// The selected state (`bw-2` `border-strong` border).
    pub selected: bool,
    /// The elevation.
    pub elevation: Elevation,
    /// Inner padding (`Card.d.ts` `padding?: string`, default `spacing::SPACE_5`).
    pub padding: f32,
}

impl<'a> Card<'a> {
    /// Builds a non-selected, `Elevation::Level1`, gridless, header-less
    /// card at the default padding (`spacing::SPACE_5`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            eyebrow: None,
            title: None,
            grid: false,
            selected: false,
            elevation: Elevation::Level1,
            padding: spacing::SPACE_5,
        }
    }

    /// Sets `eyebrow`.
    #[must_use]
    pub const fn eyebrow(mut self, eyebrow: &'a str) -> Self {
        self.eyebrow = Some(eyebrow);
        self
    }

    /// Sets `title`.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Sets `grid`.
    #[must_use]
    pub const fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }

    /// Sets `selected`.
    #[must_use]
    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets `elevation`.
    #[must_use]
    pub const fn elevation(mut self, elevation: Elevation) -> Self {
        self.elevation = elevation;
        self
    }

    /// Sets `padding`.
    #[must_use]
    pub const fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// The pure style-resolution layer (AC7): `(selected, elevation)` →
    /// colors + metrics. No `egui::Ui`, no allocation — Miri-clean.
    #[must_use]
    pub const fn resolve(selected: bool, elevation: Elevation) -> CardStyle {
        let (border, border_width) = if selected {
            (color::BORDER_STRONG, spacing::BW_2)
        } else {
            (color::BORDER_HAIRLINE, spacing::BW_HAIR)
        };
        let shadow = match elevation {
            Elevation::Level0 => effects::SHADOW_0,
            Elevation::Level1 => effects::SHADOW_1,
            Elevation::Level2 => effects::SHADOW_2,
            Elevation::Level3 => effects::SHADOW_3,
        };
        CardStyle {
            fill: color::SURFACE_CARD,
            border,
            border_width,
            radius: spacing::RADIUS_2,
            shadow,
        }
    }

    /// Draws the resolved `style` into `rect`: the elevation shadow, fill +
    /// border, an optional clipped grid watermark, and an optional
    /// eyebrow/title header stacked at `padding` from the top-left.
    ///
    /// `right` and the body `children` are NOT drawn here — they are
    /// interactive `egui` content, handled by [`Self::show`] alone; `paint`
    /// only draws what a plain [`Painter`] can express, so the AC8
    /// gallery can force any state through this layer directly.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first, when `eyebrow`/`title` is
    /// `Some`.
    pub(crate) fn paint(
        painter: &Painter,
        rect: Rect,
        style: &CardStyle,
        grid: bool,
        eyebrow: Option<&str>,
        title: Option<&str>,
        padding: f32,
    ) {
        let corner_radius = CornerRadius::from(style.radius);

        if style.shadow != Shadow::NONE {
            painter.add(style.shadow.as_shape(rect, corner_radius));
        }
        painter.rect_filled(rect, corner_radius, style.fill);
        painter.rect_stroke(
            rect,
            corner_radius,
            Stroke::new(style.border_width, style.border),
            StrokeKind::Inside,
        );

        if grid {
            paint_grid_watermark(painter, rect);
        }

        let mut cursor_y = rect.min.y + padding;
        if let Some(eyebrow) = eyebrow {
            painter.text(
                Pos2::new(rect.min.x + padding, cursor_y),
                Align2::LEFT_TOP,
                eyebrow.to_uppercase(),
                FontId::new(
                    typography::FS_XS,
                    FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
                ),
                color::TEXT_MUTED,
            );
            cursor_y = typography::FS_XS.mul_add(typography::LH_SNUG, cursor_y);
        }
        if let Some(title) = title {
            painter.text(
                Pos2::new(rect.min.x + padding, cursor_y),
                Align2::LEFT_TOP,
                title,
                FontId::new(
                    typography::FS_TITLE,
                    FontFamily::Name(crate::fonts::ONEST_SEMIBOLD.into()),
                ),
                color::TEXT_INK,
            );
        }
    }

    /// Draws the card chrome onto the background layer (so it renders
    /// behind `right`/`add_contents`, drawn afterward via normal `egui`
    /// widget calls in the same `ui`), then allocates the whole card rect
    /// and returns its `Response` (`Card.d.ts`'s `onClick` ≡
    /// `Response::clicked()`).
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(
        self,
        ui: &mut Ui,
        right: Option<impl FnOnce(&mut Ui)>,
        add_contents: impl FnOnce(&mut Ui),
    ) -> Response {
        let style = Self::resolve(self.selected, self.elevation);
        let header_height = header_height(self.eyebrow, self.title, self.padding);

        let outer = ui.scope(|ui| {
            ui.add_space(self.padding);
            if let Some(right) = right {
                ui.horizontal(|ui| {
                    ui.add_space(self.padding);
                    ui.add_space(header_height);
                    right(ui);
                    ui.add_space(self.padding);
                });
            } else if header_height > 0.0 {
                ui.add_space(header_height);
            }
            ui.horizontal(|ui| {
                ui.add_space(self.padding);
                ui.vertical(add_contents);
                ui.add_space(self.padding);
            });
            ui.add_space(self.padding);
        });

        let card_rect = outer.response.rect;
        let bg_painter = ui
            .ctx()
            .layer_painter(LayerId::background())
            .with_clip_rect(card_rect);
        Self::paint(
            &bg_painter,
            card_rect,
            &style,
            self.grid,
            self.eyebrow,
            self.title,
            self.padding,
        );

        outer.response.interact(Sense::click())
    }
}

impl Default for Card<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// The header block's height, in points — `padding` for eyebrow-only,
/// `padding` for title-only, or the stacked eyebrow+title height when both
/// are present; `0.0` when neither is set.
fn header_height(eyebrow: Option<&str>, title: Option<&str>, padding: f32) -> f32 {
    let eyebrow_height = if eyebrow.is_some() {
        typography::FS_XS * typography::LH_SNUG
    } else {
        0.0
    };
    let title_height = if title.is_some() {
        typography::FS_TITLE * typography::LH_SNUG
    } else {
        0.0
    };
    if eyebrow.is_none() && title.is_none() {
        0.0
    } else {
        padding + eyebrow_height + title_height
    }
}

/// Draws the faint graph-paper grid watermark (ruling + dots), clipped to
/// `rect`, at pitch `spacing::CELL`, each color dimmed by
/// `common::GRID_WATERMARK_OPACITY` — the `.jsx`'s `opacity: 0.5`. Reuses
/// `placeholder.rs`'s `draw_grid` shape as a precedent, not shared code (its
/// pitch/opacity/exemption differ).
fn paint_grid_watermark(painter: &Painter, rect: Rect) {
    let clipped = painter.with_clip_rect(rect);
    let ruling_color = effects::BG_GRID_COLOR.gamma_multiply(super::common::GRID_WATERMARK_OPACITY);
    let dot_color = effects::BG_DOTS_COLOR.gamma_multiply(super::common::GRID_WATERMARK_OPACITY);

    let v_range = Rangef::new(rect.min.y, rect.max.y);
    let h_range = Rangef::new(rect.min.x, rect.max.x);
    let xs: Vec<f32> = grid_lines(rect.min.x, rect.width());
    let ys: Vec<f32> = grid_lines(rect.min.y, rect.height());

    for &x in &xs {
        clipped.vline(
            x,
            v_range,
            Stroke::new(effects::BG_GRID_RULING_WIDTH, ruling_color),
        );
    }
    for &y in &ys {
        clipped.hline(
            h_range,
            y,
            Stroke::new(effects::BG_GRID_RULING_WIDTH, ruling_color),
        );
    }
    for &x in &xs {
        for &y in &ys {
            clipped.circle_filled(Pos2::new(x, y), effects::BG_DOTS_RADIUS, dot_color);
        }
    }
}

/// Positions of grid lines spaced `spacing::CELL` points apart, from
/// `origin` to `origin + extent` inclusive. A bounded integer loop, not a
/// `while` over floats (`clippy::while_float`, nursery = deny) — mirrors
/// `placeholder.rs`'s `grid_lines`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "extent is a non-negative on-screen rect dimension; the step \
              count is comfortably under u16::MAX for any realistic card"
)]
fn grid_lines(origin: f32, extent: f32) -> Vec<f32> {
    let steps = (extent / spacing::CELL) as u16;
    (0..=steps)
        .map(move |i| f32::from(i).mul_add(spacing::CELL, origin))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Card, Elevation};
    use crate::tokens::{color, effects, spacing};

    /// AC7 — elevation 0-3 → `SHADOW_0`-`SHADOW_3`.
    #[test]
    fn elevation_maps_to_shadow() {
        assert_eq!(
            Card::resolve(false, Elevation::Level0).shadow,
            effects::SHADOW_0
        );
        assert_eq!(
            Card::resolve(false, Elevation::Level1).shadow,
            effects::SHADOW_1
        );
        assert_eq!(
            Card::resolve(false, Elevation::Level2).shadow,
            effects::SHADOW_2
        );
        assert_eq!(
            Card::resolve(false, Elevation::Level3).shadow,
            effects::SHADOW_3
        );
    }

    /// AC7 — selected → `bw-2` `border-strong`, else `bw-hair` `border-hairline`
    /// (the design's per-component mapping — Card is `BW_2 = 2.0px`, unlike
    /// Tag's `BW_1 = 1.5px`).
    #[test]
    fn selected_uses_strong_border() {
        let selected = Card::resolve(true, Elevation::Level1);
        assert_eq!(selected.border, color::BORDER_STRONG);
        crate::test_util::assert_f32(
            "selected border_width",
            selected.border_width,
            spacing::BW_2,
        );

        let rest = Card::resolve(false, Elevation::Level1);
        assert_eq!(rest.border, color::BORDER_HAIRLINE);
        crate::test_util::assert_f32("rest border_width", rest.border_width, spacing::BW_HAIR);
    }

    /// Fill is always `surface-card`; radius is always `RADIUS_2`.
    #[test]
    fn fill_and_radius_are_constant() {
        let a = Card::resolve(false, Elevation::Level0);
        let b = Card::resolve(true, Elevation::Level3);
        assert_eq!(a.fill, color::SURFACE_CARD);
        assert_eq!(b.fill, color::SURFACE_CARD);
        crate::test_util::assert_f32("radius a", a.radius, spacing::RADIUS_2);
        crate::test_util::assert_f32("radius b", b.radius, spacing::RADIUS_2);
    }

    /// `Elevation::default()` is `Level1` (design: "default 1").
    #[test]
    fn elevation_defaults_to_level1() {
        assert_eq!(Elevation::default(), Elevation::Level1);
    }

    /// Builder defaults: `new` starts header-less, gridless, non-selected,
    /// `Level1`, at the default padding.
    #[test]
    fn new_has_expected_defaults() {
        let card = Card::new();
        assert!(card.eyebrow.is_none());
        assert!(card.title.is_none());
        assert!(!card.grid);
        assert!(!card.selected);
        assert_eq!(card.elevation, Elevation::Level1);
        crate::test_util::assert_f32("default padding", card.padding, spacing::SPACE_5);
    }
}
