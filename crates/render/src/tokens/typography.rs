//! `docs/design-system/tokens/typography.css` → Rust consts.
//!
//! Units are **not** uniform in this file (design § *Unit semantics*):
//! `--fs-*` are `f32` logical points (1 CSS px = 1 point), same as
//! `spacing.rs`. `--lh-*` are **unitless ratios** of the font size — a line
//! height multiplier, not a length. `--ls-*` are **em ratios** — an em is the
//! font size, so `LS_DISPLAY` at `FS_DISPLAY` means `56 * -0.02 = -1.12`
//! points, **not** `-0.02` points. `--fw-*` are OpenType `wght` axis values
//! (400–700), not lengths at all — fed straight to `VariationCoords`. Do not
//! read any of these four groups under a blanket "points" assumption.
//!
//! `--text-eyebrow-transform` is **excluded** (AC1 branch (b), disposition
//! 10): it is a text-transform behavior, not a value token, and belongs to
//! whichever component draws an eyebrow (`gp-render` #13–#16).

/// `--font-display`. Primary family name only.
///
/// The CSS stack's fallbacks (`ui-sans-serif`, `system-ui`, `sans-serif`) are
/// browser concepts; egui supplies the fallback role structurally via
/// `FontDefinitions`' per-family list (see `fonts.rs`).
pub const FONT_DISPLAY: &str = "Space Grotesk";
/// `--font-ui`.
pub const FONT_UI: &str = "Space Grotesk";
/// `--font-mono`.
pub const FONT_MONO: &str = "JetBrains Mono";

// ---- Type scale (px) — display heavy, tight tracking ----

/// `--fs-display` — hero wordmark / big numerals.
pub const FS_DISPLAY: f32 = 56.0;
/// `--fs-h1`.
pub const FS_H1: f32 = 40.0;
/// `--fs-h2`.
pub const FS_H2: f32 = 30.0;
/// `--fs-h3`.
pub const FS_H3: f32 = 22.0;
/// `--fs-title`.
pub const FS_TITLE: f32 = 18.0;
/// `--fs-body`.
pub const FS_BODY: f32 = 15.0;
/// `--fs-sm`.
pub const FS_SM: f32 = 13.0;
/// `--fs-xs`.
pub const FS_XS: f32 = 11.0;
/// `--fs-micro` — grid labels, fine print.
pub const FS_MICRO: f32 = 10.0;

// ---- Weights — OpenType `wght` axis values, NOT logical points ----

/// `--fw-regular`.
pub const FW_REGULAR: f32 = 400.0;
/// `--fw-medium`.
pub const FW_MEDIUM: f32 = 500.0;
/// `--fw-semibold`.
pub const FW_SEMIBOLD: f32 = 600.0;
/// `--fw-bold`.
pub const FW_BOLD: f32 = 700.0;

// ---- Line heights — unitless ratios of the font size, NOT points ----

/// `--lh-tight`.
pub const LH_TIGHT: f32 = 1.05;
/// `--lh-snug`.
pub const LH_SNUG: f32 = 1.2;
/// `--lh-normal`.
pub const LH_NORMAL: f32 = 1.5;

// ---- Letter spacing — em ratios of the font size, NOT points ----

/// `--ls-display`. Negative: tighter tracking at display size.
pub const LS_DISPLAY: f32 = -0.02;
/// `--ls-normal`. The one bare (unsuffixed) `0` in this file.
pub const LS_NORMAL: f32 = 0.0;
/// `--ls-label`.
pub const LS_LABEL: f32 = 0.06;
/// `--ls-mono`.
pub const LS_MONO: f32 = 0.02;

// ---- Roles as composites — const references, not restated literals ----

/// `--role-display-size: var(--fs-display)`.
pub const ROLE_DISPLAY_SIZE: f32 = FS_DISPLAY;
/// `--role-value-size: var(--fs-h2)` — telemetry big numbers.
pub const ROLE_VALUE_SIZE: f32 = FS_H2;

#[cfg(test)]
mod tests {
    use super::{
        FONT_DISPLAY, FONT_MONO, FONT_UI, FS_BODY, FS_DISPLAY, FS_H1, FS_H2, FS_H3, FS_MICRO,
        FS_SM, FS_TITLE, FS_XS, FW_BOLD, FW_MEDIUM, FW_REGULAR, FW_SEMIBOLD, LH_NORMAL, LH_SNUG,
        LH_TIGHT, LS_DISPLAY, LS_LABEL, LS_MONO, LS_NORMAL, ROLE_DISPLAY_SIZE, ROLE_VALUE_SIZE,
    };
    use crate::tokens::css::{assert_f32, assert_token, value_of, var_target};

    const CSS: &str = include_str!("../../../../docs/design-system/tokens/typography.css");

    /// Every non-family, non-role numeric token in this file, value-checked
    /// against the CSS (AC1/AC6/AC8). Exercises every unit branch of the
    /// shared parser: `px` (`--fs-*`), bare weight/ratio (`--fw-*`/`--lh-*`),
    /// `em` incl. the bare `0` (`--ls-normal`) and the negative
    /// (`--ls-display: -0.02em`).
    const TOKENS: [(&str, f32); 20] = [
        ("--fs-display", FS_DISPLAY),
        ("--fs-h1", FS_H1),
        ("--fs-h2", FS_H2),
        ("--fs-h3", FS_H3),
        ("--fs-title", FS_TITLE),
        ("--fs-body", FS_BODY),
        ("--fs-sm", FS_SM),
        ("--fs-xs", FS_XS),
        ("--fs-micro", FS_MICRO),
        ("--fw-regular", FW_REGULAR),
        ("--fw-medium", FW_MEDIUM),
        ("--fw-semibold", FW_SEMIBOLD),
        ("--fw-bold", FW_BOLD),
        ("--lh-tight", LH_TIGHT),
        ("--lh-snug", LH_SNUG),
        ("--lh-normal", LH_NORMAL),
        ("--ls-display", LS_DISPLAY),
        ("--ls-normal", LS_NORMAL),
        ("--ls-label", LS_LABEL),
        ("--ls-mono", LS_MONO),
    ];

    /// AC1/AC6/AC8 — every numeric token's parsed CSS value matches its
    /// const.
    #[test]
    fn numeric_tokens_match_css() {
        for (name, want) in TOKENS {
            assert_token(CSS, name, want);
        }
    }

    /// The primary family name out of a `var()`-free, comma-separated CSS
    /// font stack, e.g. `'Space Grotesk', ui-sans-serif, system-ui,
    /// sans-serif` → `Space Grotesk`. The CSS quotes the primary name with
    /// `'...'`; the fallbacks after the first comma are dropped (egui
    /// supplies its own fallback role — see `fonts.rs`).
    fn primary_family<'a>(css: &'a str, name: &str) -> &'a str {
        let raw = value_of(css, name);
        let (_, after_quote) = raw
            .split_once('\'')
            .unwrap_or_else(|| panic!("token {name}: family value not quoted: {raw:?}"));
        after_quote
            .split_once('\'')
            .unwrap_or_else(|| panic!("token {name}: unterminated quote: {raw:?}"))
            .0
    }

    /// AC1/AC6/AC8 — the three family tokens' primary names.
    #[test]
    fn family_names_match_css() {
        assert_eq!(primary_family(CSS, "--font-display"), FONT_DISPLAY);
        assert_eq!(primary_family(CSS, "--font-ui"), FONT_UI);
        assert_eq!(primary_family(CSS, "--font-mono"), FONT_MONO);
    }

    /// AC5 — the two `--role-*` aliases, checked on BOTH sides (design round
    /// 4, note 3): the CSS-side `var()` target name, and the Rust-side
    /// identity via the shared comparator. The Rust-side check is not a
    /// tautology — a const pointed at the wrong base compiles fine; only
    /// this assertion catches it (proven: `ROLE_DISPLAY_SIZE` mis-pointed at
    /// `FS_H2` passes the CSS-side check and fails this one, 30 vs 56).
    #[test]
    fn role_aliases_match_their_target() {
        assert_eq!(var_target(CSS, "--role-display-size"), "--fs-display");
        assert_eq!(var_target(CSS, "--role-value-size"), "--fs-h2");
        assert_f32("ROLE_DISPLAY_SIZE", ROLE_DISPLAY_SIZE, FS_DISPLAY);
        assert_f32("ROLE_VALUE_SIZE", ROLE_VALUE_SIZE, FS_H2);
    }
}
