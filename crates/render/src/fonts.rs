//! Vendored font faces (Onest, `JetBrains` Mono) + the [`FontDefinitions`]
//! builder that registers them.
//!
//! Both faces are variable fonts (a single `wght` axis) vendored from
//! `google/fonts` at the commit pinned in design `2026-07-17-render-onest-font-swap`
//! § Decomposition subtask 2, each beside its own `OFL.txt` (AC1, #12 AC15).
//! Onest's `wght` axis defaults to 400 (Regular), but every registered
//! instance still carries an explicit [`egui::epaint::text::VariationCoords`]
//! override — see the private `weighted_instance` helper's doc for why the
//! default alone is not enough.
//!
//! `gp-render` only *produces* the [`FontDefinitions`] value returned by
//! [`definitions`] — it never constructs an [`egui::Context`] (the crate stays
//! draw-only, #12 AC13). `gp-game` applies it via
//! `cc.egui_ctx.set_fonts(gp_render::fonts::definitions())`.

use egui::epaint::text::VariationCoords;
use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use std::sync::Arc;

/// Onest variable font bytes (`wght` axis, OFL-1.1). Vendored from
/// `ofl/onest/Onest[wght].ttf` at the pinned upstream commit.
pub const ONEST: &[u8] = include_bytes!("../fonts/onest/Onest[wght].ttf");

/// `JetBrains` Mono variable font bytes (`wght` axis, OFL-1.1). Vendored from
/// `ofl/jetbrainsmono/JetBrainsMono[wght].ttf` at the same pinned commit.
pub const JETBRAINS_MONO: &[u8] = include_bytes!("../fonts/jetbrains-mono/JetBrainsMono[wght].ttf");

/// Registration key for Onest at `wght` 400 (Regular).
pub const ONEST_REGULAR: &str = "Onest-Regular";
/// Registration key for Onest at `wght` 500 (Medium).
pub const ONEST_MEDIUM: &str = "Onest-Medium";
/// Registration key for Onest at `wght` 600 (`SemiBold`).
pub const ONEST_SEMIBOLD: &str = "Onest-SemiBold";
/// Registration key for Onest at `wght` 700 (Bold).
pub const ONEST_BOLD: &str = "Onest-Bold";
/// Registration key for `JetBrains` Mono at `wght` 400 (Regular).
pub const JETBRAINS_MONO_REGULAR: &str = "JetBrainsMono-Regular";
/// Registration key for `JetBrains` Mono at `wght` 500 (Medium).
pub const JETBRAINS_MONO_MEDIUM: &str = "JetBrainsMono-Medium";
/// Registration key for `JetBrains` Mono at `wght` 700 (Bold).
pub const JETBRAINS_MONO_BOLD: &str = "JetBrainsMono-Bold";

/// Build one weight instance of a variable face, with its `wght` axis pinned
/// explicitly — never left at the font's own default.
///
/// The builder registers **four** distinct Onest weights from **one** byte
/// array (plus three `JetBrains` Mono weights from another); without this
/// explicit per-instance override, all four Onest instances would render at
/// the face's default weight and Medium/`SemiBold`/Bold would be silently
/// identical to Regular.
fn weighted_instance(bytes: &'static [u8], wght: f32) -> FontData {
    FontData::from_static(bytes).tweak(FontTweak {
        coords: VariationCoords::new([(b"wght", wght)]),
        ..Default::default()
    })
}

/// Builds the app's [`FontDefinitions`].
///
/// Starts from [`FontDefinitions::empty`] **explicitly** — never
/// [`FontDefinitions::default`], whose behaviour now depends on whether
/// `epaint`'s `default_fonts` feature is enabled (this workspace turns it
/// off, so `default()` itself collapses to `empty()`; relying on that
/// collapse would make the font set an accident of a Cargo feature flag
/// rather than a stated choice) — and registers 7 weight instances across
/// the two vendored faces.
///
/// Every family vector below is written out **in full**, not built by
/// snapshotting `families[Proportional]`/`families[Monospace]` before
/// prepending: under `empty()` both start as `vec![]`, so a snapshot-based
/// builder would silently produce single-entry `Name` families with no
/// fallback tail at all. Writing the vectors literally keeps the ordering
/// pin (`JetBrains` Mono before Onest in every mono family) visible at the
/// call site instead of emergent from empty-vector arithmetic.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::empty();

    fonts.font_data.insert(
        ONEST_REGULAR.to_owned(),
        Arc::new(weighted_instance(ONEST, 400.0)),
    );
    fonts.font_data.insert(
        ONEST_MEDIUM.to_owned(),
        Arc::new(weighted_instance(ONEST, 500.0)),
    );
    fonts.font_data.insert(
        ONEST_SEMIBOLD.to_owned(),
        Arc::new(weighted_instance(ONEST, 600.0)),
    );
    fonts.font_data.insert(
        ONEST_BOLD.to_owned(),
        Arc::new(weighted_instance(ONEST, 700.0)),
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

    fonts
        .families
        .insert(FontFamily::Proportional, vec![ONEST_REGULAR.to_owned()]);
    fonts.families.insert(
        FontFamily::Monospace,
        vec![JETBRAINS_MONO_REGULAR.to_owned(), ONEST_REGULAR.to_owned()],
    );

    for key in [ONEST_REGULAR, ONEST_MEDIUM, ONEST_SEMIBOLD, ONEST_BOLD] {
        fonts
            .families
            .insert(FontFamily::Name(Arc::from(key)), vec![key.to_owned()]);
    }

    // Weight-matched 1:1 JBM → Onest fallback (design D1): a Badge at
    // `--fw-medium` resolves through `Name("JetBrainsMono-Medium")`, not
    // `Monospace`, so the fallback must reach the per-weight families too
    // (spec constraint 3) — and at the *matching* Onest weight, not always
    // Regular, or a wght-500 run would silently fall back to wght 400.
    for (jbm_key, onest_key) in [
        (JETBRAINS_MONO_REGULAR, ONEST_REGULAR),
        (JETBRAINS_MONO_MEDIUM, ONEST_MEDIUM),
        (JETBRAINS_MONO_BOLD, ONEST_BOLD),
    ] {
        fonts.families.insert(
            FontFamily::Name(Arc::from(jbm_key)),
            vec![jbm_key.to_owned(), onest_key.to_owned()],
        );
    }

    fonts
}

#[cfg(test)]
mod tests {
    use super::{
        JETBRAINS_MONO, JETBRAINS_MONO_BOLD, JETBRAINS_MONO_MEDIUM, JETBRAINS_MONO_REGULAR, ONEST,
        ONEST_BOLD, ONEST_MEDIUM, ONEST_REGULAR, ONEST_SEMIBOLD, definitions,
    };
    use egui::FontData;
    use skrifa::MetadataProvider as _;

    /// AC9 — each vendored face parses and its `wght` axis range covers every
    /// weight the builder registers. Asserts the *requirement*
    /// (`min <= 400 && max >= 700`), not the exact upstream range — that
    /// would duplicate AC1's SHA-256 pin as a second, more brittle identity
    /// check.
    #[test]
    fn wght_axis_covers_registered_weights() {
        let onest_axes = FontData::from_static(ONEST).variation_axes();
        let onest_wght = onest_axes
            .iter()
            .find(|axis| axis.tag == "wght")
            .expect("Onest reports a wght axis");
        assert!(onest_wght.range.min <= 400.0 && onest_wght.range.max >= 700.0);

        let jbm_axes = FontData::from_static(JETBRAINS_MONO).variation_axes();
        let jbm_wght = jbm_axes
            .iter()
            .find(|axis| axis.tag == "wght")
            .expect("JetBrains Mono reports a wght axis");
        assert!(jbm_wght.range.min <= 400.0 && jbm_wght.range.max >= 700.0);
    }

    /// AC6, AC7 — `definitions()` builds on `FontDefinitions::empty()`
    /// (never `default()`), registers exactly 7 weight instances, each with
    /// an explicit `wght` coords tweak, and `Proportional`/`Monospace`
    /// resolve to their **exact** family lists by full-vector equality —
    /// not `first()` / non-empty / `contains`. Replaces #12's
    /// `definitions_preserve_builtin_fonts_and_add_seven_instances`, which
    /// asserted the *opposite* of the new behaviour (constraint 2): under
    /// `empty()`, `FontDefinitions::builtin_font_names()` returns `&[]`, so
    /// that test's loop went vacuous-but-green rather than red.
    #[test]
    fn definitions_registers_seven_instances_with_exact_families() {
        let fonts = definitions();

        assert_eq!(fonts.font_data.len(), 7);

        let keys = [
            ONEST_REGULAR,
            ONEST_MEDIUM,
            ONEST_SEMIBOLD,
            ONEST_BOLD,
            JETBRAINS_MONO_REGULAR,
            JETBRAINS_MONO_MEDIUM,
            JETBRAINS_MONO_BOLD,
        ];
        for key in keys {
            assert!(
                fonts.font_data.contains_key(key),
                "missing registration key {key}"
            );
        }
        for key in keys {
            let data = &fonts.font_data[key];
            assert_ne!(
                data.tweak.coords,
                egui::epaint::text::VariationCoords::default(),
                "{key} is missing an explicit wght coords override"
            );
        }

        assert_eq!(
            fonts.families[&egui::FontFamily::Proportional],
            vec![ONEST_REGULAR.to_owned()]
        );
        assert_eq!(
            fonts.families[&egui::FontFamily::Monospace],
            vec![JETBRAINS_MONO_REGULAR.to_owned(), ONEST_REGULAR.to_owned()]
        );
    }

    /// AC8, design D1 — every per-weight mono `Name` family carries its
    /// weight-matched Onest instance behind its `JetBrains` Mono instance
    /// (JBM **first** — ordering is load-bearing, § AC7), so `✓` renders at
    /// the Badge's own weight rather than always falling back to Onest
    /// Regular. Also: the four proportional `Name` families stay
    /// single-entry (no JBM tail — AC7's `Proportional` pin is one-entry).
    #[test]
    fn mono_name_families_fall_back_to_weight_matched_onest() {
        let fonts = definitions();

        for (jbm_key, onest_key) in [
            (JETBRAINS_MONO_REGULAR, ONEST_REGULAR),
            (JETBRAINS_MONO_MEDIUM, ONEST_MEDIUM),
            (JETBRAINS_MONO_BOLD, ONEST_BOLD),
        ] {
            let family = &fonts.families[&egui::FontFamily::Name(std::sync::Arc::from(jbm_key))];
            assert_eq!(family, &vec![jbm_key.to_owned(), onest_key.to_owned()]);
        }

        for key in [ONEST_REGULAR, ONEST_MEDIUM, ONEST_SEMIBOLD, ONEST_BOLD] {
            let family = &fonts.families[&egui::FontFamily::Name(std::sync::Arc::from(key))];
            assert_eq!(family, &vec![key.to_owned()]);
        }
    }

    /// AC9, AC12 — Onest's charmap has `Ф` U+0424 (the glyph the swap exists
    /// for) and `✓` U+2713 (the glyph design D1's fallback exists for), plus
    /// every codepoint of the three sample strings. Control: `JetBrains`
    /// Mono's charmap **lacks** `✓` (spec Key decision 9) while carrying
    /// `·`/`→`/`–`, which is what makes the Onest fallback load-bearing
    /// rather than decorative.
    #[test]
    fn vendored_faces_cover_the_glyphs_the_swap_exists_for() {
        let onest_font =
            skrifa::FontRef::from_index(ONEST, 0).expect("Onest parses as a valid font");
        let onest_charmap = onest_font.charmap();

        for ch in ['Ф', '✓'] {
            assert!(
                onest_charmap.map(ch).is_some(),
                "Onest charmap is missing {ch:?}"
            );
        }
        for ch in "GRAPHITE GP".chars() {
            assert!(
                onest_charmap.map(ch).is_some(),
                "Onest charmap is missing {ch:?} (row 1 sample)"
            );
        }
        for ch in "Ф1 – Ф7".chars() {
            assert!(
                onest_charmap.map(ch).is_some(),
                "Onest charmap is missing {ch:?} (row 2 sample)"
            );
        }
        for ch in "L3 · v4→6 ✓".chars() {
            assert!(
                onest_charmap.map(ch).is_some(),
                "Onest charmap is missing {ch:?} (row 3 sample)"
            );
        }

        let jbm_font = skrifa::FontRef::from_index(JETBRAINS_MONO, 0)
            .expect("JetBrains Mono parses as a valid font");
        let jbm_charmap = jbm_font.charmap();
        assert!(
            jbm_charmap.map('✓').is_none(),
            "JetBrains Mono unexpectedly carries U+2713 — the Onest fallback \
             is no longer load-bearing for the mono families"
        );
        for ch in ['·', '→', '–'] {
            assert!(
                jbm_charmap.map(ch).is_some(),
                "JetBrains Mono is missing {ch:?}"
            );
        }
    }
}
