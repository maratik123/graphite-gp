//! Vendored font faces (Space Grotesk, `JetBrains` Mono) + the
//! `FontDefinitions` builder that registers them.
//!
//! Both faces are variable fonts (a single `wght` axis) vendored from
//! `google/fonts` at the commit pinned in
//! `ai-docs/plans/2026-07-17-render-design-tokens.design.md` § Vendoring pin,
//! each beside its own `OFL.txt` (AC9, AC15). Space Grotesk's `wght` axis
//! **defaults to 300** (Light), not 400, so every registered instance —
//! including the nominal "Regular" one — carries an explicit
//! [`egui::epaint::text::VariationCoords`] override; a bare registration would
//! silently render Light.
//!
//! `gp-render` only *produces* the [`FontDefinitions`] value returned by
//! [`definitions`] — it never constructs an [`egui::Context`] (the crate stays
//! draw-only, AC13). `gp-game` applies it via
//! `cc.egui_ctx.set_fonts(gp_render::fonts::definitions())`.

use egui::epaint::text::VariationCoords;
use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use std::sync::Arc;

/// Space Grotesk variable font bytes (`wght` axis, OFL-1.1). Vendored from
/// `ofl/spacegrotesk/SpaceGrotesk[wght].ttf` at the pinned upstream commit.
pub const SPACE_GROTESK: &[u8] = include_bytes!("../fonts/space-grotesk/SpaceGrotesk[wght].ttf");

/// `JetBrains` Mono variable font bytes (`wght` axis, OFL-1.1). Vendored from
/// `ofl/jetbrainsmono/JetBrainsMono[wght].ttf` at the same pinned commit.
pub const JETBRAINS_MONO: &[u8] = include_bytes!("../fonts/jetbrains-mono/JetBrainsMono[wght].ttf");

/// Registration key for Space Grotesk at `wght` 400 (Regular).
pub const SPACE_GROTESK_REGULAR: &str = "SpaceGrotesk-Regular";
/// Registration key for Space Grotesk at `wght` 500 (Medium).
pub const SPACE_GROTESK_MEDIUM: &str = "SpaceGrotesk-Medium";
/// Registration key for Space Grotesk at `wght` 600 (`SemiBold`).
pub const SPACE_GROTESK_SEMIBOLD: &str = "SpaceGrotesk-SemiBold";
/// Registration key for Space Grotesk at `wght` 700 (Bold).
pub const SPACE_GROTESK_BOLD: &str = "SpaceGrotesk-Bold";
/// Registration key for `JetBrains` Mono at `wght` 400 (Regular).
pub const JETBRAINS_MONO_REGULAR: &str = "JetBrainsMono-Regular";
/// Registration key for `JetBrains` Mono at `wght` 500 (Medium).
pub const JETBRAINS_MONO_MEDIUM: &str = "JetBrainsMono-Medium";
/// Registration key for `JetBrains` Mono at `wght` 700 (Bold).
pub const JETBRAINS_MONO_BOLD: &str = "JetBrainsMono-Bold";

/// Build one weight instance of a variable face, with its `wght` axis pinned
/// explicitly — never left at the font's own default (finding 3: Space
/// Grotesk defaults to 300, not 400).
fn weighted_instance(bytes: &'static [u8], wght: f32) -> FontData {
    FontData::from_static(bytes).tweak(FontTweak {
        coords: VariationCoords::new([(b"wght", wght)]),
        ..Default::default()
    })
}

/// Builds the app's [`FontDefinitions`].
///
/// Starts from egui's four bundled faces (via [`FontDefinitions::default`],
/// never [`FontDefinitions::empty`] — that would silently drop egui's
/// emoji/fallback coverage) and adds 7 registered weight instances across the
/// two vendored faces.
///
/// `Proportional` and `Monospace` get our regular weight **prepended** ahead
/// of egui's existing fallback chain; each of the 7 keys also gets its own
/// [`FontFamily::Name`] entry (`[key, ...fallbacks]`) so a caller can pick an
/// exact weight directly while emoji glyphs still resolve inside it.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        SPACE_GROTESK_REGULAR.to_owned(),
        Arc::new(weighted_instance(SPACE_GROTESK, 400.0)),
    );
    fonts.font_data.insert(
        SPACE_GROTESK_MEDIUM.to_owned(),
        Arc::new(weighted_instance(SPACE_GROTESK, 500.0)),
    );
    fonts.font_data.insert(
        SPACE_GROTESK_SEMIBOLD.to_owned(),
        Arc::new(weighted_instance(SPACE_GROTESK, 600.0)),
    );
    fonts.font_data.insert(
        SPACE_GROTESK_BOLD.to_owned(),
        Arc::new(weighted_instance(SPACE_GROTESK, 700.0)),
    );
    fonts.font_data.insert(
        JETBRAINS_MONO_REGULAR.to_owned(),
        Arc::new(weighted_instance(JETBRAINS_MONO, 400.0)),
    );
    fonts.font_data.insert(
        JETBRAINS_MONO_MEDIUM.to_owned(),
        Arc::new(weighted_instance(JETBRAINS_MONO, 500.0)),
    );
    fonts.font_data.insert(
        JETBRAINS_MONO_BOLD.to_owned(),
        Arc::new(weighted_instance(JETBRAINS_MONO, 700.0)),
    );

    let proportional_fallback = fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .clone();
    let monospace_fallback = fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .clone();

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, SPACE_GROTESK_REGULAR.to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, JETBRAINS_MONO_REGULAR.to_owned());

    for key in [
        SPACE_GROTESK_REGULAR,
        SPACE_GROTESK_MEDIUM,
        SPACE_GROTESK_SEMIBOLD,
        SPACE_GROTESK_BOLD,
    ] {
        let mut family = vec![key.to_owned()];
        family.extend(proportional_fallback.iter().cloned());
        fonts
            .families
            .insert(FontFamily::Name(Arc::from(key)), family);
    }
    for key in [
        JETBRAINS_MONO_REGULAR,
        JETBRAINS_MONO_MEDIUM,
        JETBRAINS_MONO_BOLD,
    ] {
        let mut family = vec![key.to_owned()];
        family.extend(monospace_fallback.iter().cloned());
        fonts
            .families
            .insert(FontFamily::Name(Arc::from(key)), family);
    }

    fonts
}

#[cfg(test)]
mod tests {
    use super::{
        JETBRAINS_MONO, JETBRAINS_MONO_BOLD, JETBRAINS_MONO_MEDIUM, JETBRAINS_MONO_REGULAR,
        SPACE_GROTESK, SPACE_GROTESK_BOLD, SPACE_GROTESK_MEDIUM, SPACE_GROTESK_REGULAR,
        SPACE_GROTESK_SEMIBOLD, definitions,
    };
    use egui::FontData;
    use egui::text::FontDefinitions;

    /// AC9 — each vendored face parses and its `wght` axis range covers every
    /// weight the builder registers.
    #[test]
    fn wght_axis_covers_registered_weights() {
        let sg_axes = FontData::from_static(SPACE_GROTESK).variation_axes();
        let sg_wght = sg_axes
            .iter()
            .find(|axis| axis.tag == "wght")
            .expect("Space Grotesk reports a wght axis");
        assert!(sg_wght.range.min <= 400.0 && sg_wght.range.max >= 700.0);

        let jbm_axes = FontData::from_static(JETBRAINS_MONO).variation_axes();
        let jbm_wght = jbm_axes
            .iter()
            .find(|axis| axis.tag == "wght")
            .expect("JetBrains Mono reports a wght axis");
        assert!(jbm_wght.range.min <= 400.0 && jbm_wght.range.max >= 700.0);
    }

    /// AC10 — `definitions()` builds on top of egui's bundled faces (never
    /// `::empty()`), registers exactly 7 weight instances, and both curated
    /// families resolve non-empty with our face first.
    #[test]
    fn definitions_preserve_builtin_fonts_and_add_seven_instances() {
        let fonts = definitions();

        // 4 egui builtins + 7 of ours.
        assert_eq!(fonts.font_data.len(), 11);

        for key in [
            SPACE_GROTESK_REGULAR,
            SPACE_GROTESK_MEDIUM,
            SPACE_GROTESK_SEMIBOLD,
            SPACE_GROTESK_BOLD,
            JETBRAINS_MONO_REGULAR,
            JETBRAINS_MONO_MEDIUM,
            JETBRAINS_MONO_BOLD,
        ] {
            assert!(
                fonts.font_data.contains_key(key),
                "missing registration key {key}"
            );
        }

        for builtin in FontDefinitions::builtin_font_names() {
            assert!(
                fonts.font_data.contains_key(*builtin),
                "egui builtin {builtin} dropped from font_data"
            );
        }

        let proportional = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .expect("Proportional family present");
        assert_eq!(
            proportional.first().map(String::as_str),
            Some(SPACE_GROTESK_REGULAR)
        );

        let monospace = fonts
            .families
            .get(&egui::FontFamily::Monospace)
            .expect("Monospace family present");
        assert_eq!(
            monospace.first().map(String::as_str),
            Some(JETBRAINS_MONO_REGULAR)
        );

        // Every registered instance carries a non-default `wght` coords tweak.
        for key in [
            SPACE_GROTESK_REGULAR,
            SPACE_GROTESK_MEDIUM,
            SPACE_GROTESK_SEMIBOLD,
            SPACE_GROTESK_BOLD,
            JETBRAINS_MONO_REGULAR,
            JETBRAINS_MONO_MEDIUM,
            JETBRAINS_MONO_BOLD,
        ] {
            let data = &fonts.font_data[key];
            assert_ne!(
                data.tweak.coords,
                egui::epaint::text::VariationCoords::default(),
                "{key} is missing an explicit wght coords override"
            );
        }
    }
}
