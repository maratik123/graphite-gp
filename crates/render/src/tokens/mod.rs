//! Design-token constants ported from `docs/design-system/tokens/*.css`
//! (design: `2026-07-17-render-design-tokens`).
//!
//! One submodule per source CSS file — [`color`] (56 tokens), [`spacing`]
//! (30), [`typography`] (26), [`effects`] (15) — 127 tokens total (AC1/AC8).
//! Each submodule's `#[cfg(test)] mod tests` value-checks its consts against
//! the CSS text via the shared parser in `css` (test-only, below).
//!
//! # Unit semantics — read this before using a bare `f32` token
//!
//! The numeric *type* is `f32` throughout, but the **unit** it represents is
//! **not** uniform — reading every token as "logical points" silently
//! misreads several groups:
//!
//! | Group | CSS example | Unit — what the number MEANS |
//! |---|---|---|
//! | `--space-*`, `--cell*`, `--radius-*`, `--bw-*`, `--control-h-*`, `--tap-min`, `--content-max`, `--panel-max`, `--fs-*` | `16px` | **logical points** (1 CSS px = 1 point) |
//! | `--lh-*` (3) | `1.05` *(unitless)* | **ratio of font size** — a line-height multiplier. `LH_TIGHT` at `FS_DISPLAY` means `56 * 1.05 = 58.8` points, not `1.05` points |
//! | `--ls-*` (4) | `-0.02em` | **ratio of font size** — an em is the font size, so `LS_DISPLAY` at `FS_DISPLAY` means `56 * -0.02 = -1.12` points, **not** `-0.02` points |
//! | `--fw-*` (4) | `400` *(unitless)* | **OpenType `wght` axis value**, 400–700 — not a length at all; fed straight to `VariationCoords` |
//! | `--dur-*` (3) | `120ms` | **milliseconds**, carried in the `Duration` type — no bare `f32` to misread |
//!
//! `FW_BOLD: f32 = 700.0` sitting in the same module as `FS_DISPLAY: f32 =
//! 56.0` would read as a 700-point length under a blanket "points" banner —
//! it is an OpenType axis value instead. The type system cannot catch this
//! (both genuinely are `f32`); the doc above is the only barrier.
//!
//! # AC1 disposition — the ten tokens that do not port as a plain 1:1 const
//!
//! Every token **not** listed here is a plain 1:1 const (branch (a)) — 117 of
//! the 127. These ten (branch (b)) needed a different disposition:
//!
//! | # | Token(s) | Disposition |
//! |---|---|---|
//! | 1–3 | `--font-display`, `--font-ui`, `--font-mono` | Primary family name only (`"Onest"` ×2, `"JetBrains Mono"`) — the CSS stack's fallbacks are a browser concept; egui supplies the fallback role structurally (`fonts.rs`). |
//! | 4–6 | `--ease-standard`, `--ease-out`, `--ease-in` | `[f32; 4]` control points — values exact, shape differs (no egui easing type). |
//! | 7 | `--bg-grid` | Decomposed: `effects::BG_GRID_RULING_WIDTH` + `effects::BG_GRID_COLOR` + pitch = `spacing::CELL`. A CSS gradient recipe, not a value. |
//! | 8 | `--bg-dots` | Decomposed: `effects::BG_DOTS_RADIUS`, `effects::BG_DOTS_TRANSPARENT_STOP`, `effects::BG_DOTS_COLOR`. |
//! | 9 | `--shadow-inset` | `effects::InsetShadow` — a distinct type; `epaint::Shadow` has no inner-shadow primitive. |
//! | 10 | `--text-eyebrow-transform` | **Excluded.** A text-transform behavior, not a value token; belongs to whichever component draws an eyebrow. |
//!
//! `--radius-pill` and `--shadow-0`/`--shadow-1`/`--shadow-2`/`--shadow-3`/
//! `--shadow-pop`/`--focus-shadow` are branch (a) — **not** among the ten —
//! despite carrying a saturating (`RADIUS_PILL`) or premultiplying
//! (`--shadow-*`'s alpha) conversion at their *use site*; the const's stored
//! value is still the CSS's own number, unaltered (design finding 2 and § *Alpha
//! round-trip*).

pub mod color;
pub mod effects;
pub mod spacing;
pub mod typography;

/// Shared CSS-parsing test infrastructure, used by every token submodule's
/// `#[cfg(test)] mod tests` (`color`, `spacing`, `typography`, `effects`).
///
/// Hoisted here so there is exactly one copy of the CSS-token parser
/// (`value_of`/`assert_token`/`assert_cubic_bezier`/`var_target`) for the
/// whole crate — see design `2026-07-17-render-design-tokens` § *Remedy*.
/// The shared `assert_f32`/`assert_f32_slice` helpers (and their single
/// `#[allow(clippy::float_cmp)]` site) live in `crate::test_util`.
#[cfg(test)]
pub(crate) mod css {
    /// The value text between `:` and the terminating `;`, for the declaration of
    /// `name` that starts a line (finding 6 + the prefix-collision clause).
    ///
    /// Anchored by SEARCHING the occurrences, not by taking the first: `--bg-grid`
    /// occurs in comment prose at `effects.css:27` before its real declaration at
    /// line 29, so a `split_once` anchor binds to the comment and dies. Deliberately
    /// free of index arithmetic — `clippy::arithmetic_side_effects` is a workspace
    /// `deny` and fires on the `colon + 1` spelling of this same function.
    pub(crate) fn value_of<'a>(css: &'a str, name: &str) -> &'a str {
        let rest = css
            .match_indices(name)
            .find_map(|(idx, _)| {
                let (before, at) = css.split_at(idx);
                let after = at.strip_prefix(name)?;
                // Rule 1b: the next non-space char must be `:` — `--cell` vs `--cell-sm`.
                let value = after.trim_start().strip_prefix(':')?;
                // Rule 1a: the declaration must start a line.
                before
                    .lines()
                    .next_back()
                    .is_none_or(|l| l.trim().is_empty())
                    .then_some(value)
            })
            .unwrap_or_else(|| panic!("token {name}: no declaration starts a line"));
        rest.split_once(';')
            .unwrap_or_else(|| panic!("token {name}: value has no terminating ';'"))
            .0
            .trim()
    }

    /// Parses a `px`/`em`/bare-numeric token value out of `css` and compares it
    /// against `want` via `assert_f32`.
    pub(crate) fn assert_token(css: &str, name: &str, want: f32) {
        let raw = value_of(css, name);
        let numeric = raw
            .strip_suffix("px")
            .or_else(|| raw.strip_suffix("em"))
            .unwrap_or(raw);
        let got: f32 = numeric
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("token {name}: unhandled unit in {raw:?}"));
        crate::test_util::assert_f32(name, got, want);
    }

    /// Parses a `cubic-bezier(a, b, c, d)` token value and compares its four
    /// control points element-wise against `want`.
    pub(crate) fn assert_cubic_bezier(css: &str, name: &str, want: [f32; 4]) {
        let raw = value_of(css, name);
        let Some(inner) = raw
            .strip_prefix("cubic-bezier(")
            .and_then(|s| s.strip_suffix(')'))
        else {
            panic!("token {name}: not a cubic-bezier(): {raw:?}");
        };
        let got: Vec<f32> = inner
            .split(',')
            .map(|p| {
                p.trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("token {name}: bad control point {p:?}"))
            })
            .collect();
        crate::test_util::assert_f32_slice(name, &got, &want);
    }

    /// Extracts the `--x` target name out of a `var(--x)` token value.
    pub(crate) fn var_target<'a>(css: &'a str, name: &str) -> &'a str {
        let raw = value_of(css, name);
        let Some(inner) = raw.strip_prefix("var(").and_then(|s| s.strip_suffix(')')) else {
            panic!("token {name}: not a var(): {raw:?}");
        };
        inner
    }

    /// Direct coverage of the shared parser contract itself — independent of
    /// which token submodule ends up exercising each helper, so this module's
    /// helpers are never dead code between the subtask that defines them and
    /// the subtask that first consumes them.
    #[cfg(test)]
    mod tests {
        use super::{assert_cubic_bezier, assert_token, var_target};

        #[test]
        fn assert_token_parses_px_em_and_bare_numbers() {
            // Indented like the real CSS files: `value_of`'s "starts a line"
            // rule checks that only whitespace precedes the match on its own
            // line, which a column-0 fixture (no indentation at all) cannot
            // exercise the same way the real, indented CSS does.
            const CSS: &str = "  --a: 24px;\n  --b: -0.02em;\n  --c: 700;\n";
            assert_token(CSS, "--a", 24.0);
            assert_token(CSS, "--b", -0.02);
            assert_token(CSS, "--c", 700.0);
        }

        #[test]
        fn assert_cubic_bezier_parses_control_points() {
            const CSS: &str = "  --ease: cubic-bezier(0.2, 0, 0.1, 1);\n";
            assert_cubic_bezier(CSS, "--ease", [0.2, 0.0, 0.1, 1.0]);
        }

        #[test]
        fn var_target_extracts_the_referenced_name() {
            const CSS: &str = "  --alias: var(--base);\n";
            assert_eq!(var_target(CSS, "--alias"), "--base");
        }
    }
}

/// AC1/AC2/AC8 — the full-crate token inventory: every `--x` declaration
/// across the four CSS files is either a plain 1:1 const (branch (a),
/// [`PORTED`]) or one of the ten AC1 dispositions (branch (b),
/// [`DEVIATIONS`]), and the two sets are exactly disjoint and exactly cover
/// the parsed names. A token added to (or removed from) any CSS file later
/// fails this test rather than silently going unported or double-counted.
#[cfg(test)]
mod inventory {
    /// Every `--x` declaration name in `css`, one per source-line, in
    /// source order. A line counts only if it both starts with `--` (after
    /// trimming) and contains a `:` — which the continuation lines of the
    /// multi-line `--bg-grid`/`--bg-dots` values do not, so they cannot
    /// inflate the count.
    fn token_names(css: &str) -> Vec<&str> {
        css.lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if !trimmed.starts_with("--") || !trimmed.contains(':') {
                    return None;
                }
                trimmed.split_once(':').map(|(name, _)| name)
            })
            .collect()
    }

    const COLORS_CSS: &str = include_str!("../../../../docs/design-system/tokens/colors.css");
    const SPACING_CSS: &str = include_str!("../../../../docs/design-system/tokens/spacing.css");
    const TYPOGRAPHY_CSS: &str =
        include_str!("../../../../docs/design-system/tokens/typography.css");
    const EFFECTS_CSS: &str = include_str!("../../../../docs/design-system/tokens/effects.css");

    /// The ten AC1 branch-(b) dispositions (module doc § *AC1 disposition*).
    const DEVIATIONS: [&str; 10] = [
        "--font-display",
        "--font-ui",
        "--font-mono",
        "--ease-standard",
        "--ease-out",
        "--ease-in",
        "--bg-grid",
        "--bg-dots",
        "--shadow-inset",
        "--text-eyebrow-transform",
    ];

    /// The 117 branch-(a) plain 1:1 consts — every token NOT in
    /// [`DEVIATIONS`], across all four files, in source order.
    const PORTED: [&str; 117] = [
        // colors.css — 56
        "--paper-0",
        "--paper-1",
        "--paper-2",
        "--paper-3",
        "--graphite-900",
        "--graphite-800",
        "--graphite-700",
        "--graphite-600",
        "--graphite-500",
        "--graphite-400",
        "--graphite-300",
        "--graphite-200",
        "--grid-line",
        "--grid-line-major",
        "--grid-dot",
        "--asphalt-1",
        "--asphalt-2",
        "--wall",
        "--accent",
        "--accent-hover",
        "--accent-press",
        "--accent-tint",
        "--car-1",
        "--car-2",
        "--car-3",
        "--car-4",
        "--car-5",
        "--car-6",
        "--heat-0",
        "--heat-1",
        "--heat-2",
        "--heat-3",
        "--ok",
        "--warn",
        "--danger",
        "--ok-tint",
        "--warn-tint",
        "--danger-tint",
        "--surface-page",
        "--surface-card",
        "--surface-sunken",
        "--surface-infield",
        "--surface-asphalt",
        "--surface-ink",
        "--text-ink",
        "--text-body",
        "--text-muted",
        "--text-faint",
        "--text-on-ink",
        "--text-on-accent",
        "--text-link",
        "--text-link-hover",
        "--border-hairline",
        "--border-strong",
        "--border-soft",
        "--focus-ring",
        // spacing.css — 30
        "--space-0",
        "--space-1",
        "--space-2",
        "--space-3",
        "--space-4",
        "--space-5",
        "--space-6",
        "--space-8",
        "--space-10",
        "--space-12",
        "--space-16",
        "--space-20",
        "--cell",
        "--cell-sm",
        "--cell-lg",
        "--radius-0",
        "--radius-1",
        "--radius-2",
        "--radius-3",
        "--radius-pill",
        "--bw-hair",
        "--bw-1",
        "--bw-2",
        "--bw-heavy",
        "--control-h-sm",
        "--control-h-md",
        "--control-h-lg",
        "--tap-min",
        "--content-max",
        "--panel-max",
        // typography.css — 22 (of 26; 3 --font-* + --text-eyebrow-transform deviate)
        "--fs-display",
        "--fs-h1",
        "--fs-h2",
        "--fs-h3",
        "--fs-title",
        "--fs-body",
        "--fs-sm",
        "--fs-xs",
        "--fs-micro",
        "--fw-regular",
        "--fw-medium",
        "--fw-semibold",
        "--fw-bold",
        "--lh-tight",
        "--lh-snug",
        "--lh-normal",
        "--ls-display",
        "--ls-normal",
        "--ls-label",
        "--ls-mono",
        "--role-display-size",
        "--role-value-size",
        // effects.css — 9 (of 15; --shadow-inset + 3 --ease-* + 2 --bg-* deviate)
        "--shadow-0",
        "--shadow-1",
        "--shadow-2",
        "--shadow-3",
        "--shadow-pop",
        "--dur-fast",
        "--dur-med",
        "--dur-slow",
        "--focus-shadow",
    ];

    /// AC1/AC8 — per-file counts: 56/30/26/15, total 127. Re-derives the
    /// exact denominator the design's `grep -cE` counts settled on.
    #[test]
    fn per_file_counts_match_ac1() {
        assert_eq!(token_names(COLORS_CSS).len(), 56);
        assert_eq!(token_names(SPACING_CSS).len(), 30);
        assert_eq!(token_names(TYPOGRAPHY_CSS).len(), 26);
        assert_eq!(token_names(EFFECTS_CSS).len(), 15);
    }

    /// AC1/AC2/AC8 — `PORTED` and `DEVIATIONS` are disjoint, and their union
    /// is EXACTLY the 127 names parsed live from the CSS — in either
    /// direction: a token added to a CSS file with no matching Rust
    /// disposition fails here, and a stale entry in either list with no
    /// matching CSS token fails here too.
    #[test]
    fn ported_and_deviations_partition_the_parsed_names() {
        let mut parsed: Vec<&str> = Vec::with_capacity(127);
        parsed.extend(token_names(COLORS_CSS));
        parsed.extend(token_names(SPACING_CSS));
        parsed.extend(token_names(TYPOGRAPHY_CSS));
        parsed.extend(token_names(EFFECTS_CSS));
        assert_eq!(parsed.len(), 127, "AC1 denominator drifted");

        let mut parsed_sorted = parsed.clone();
        parsed_sorted.sort_unstable();
        parsed_sorted.dedup();
        assert_eq!(
            parsed_sorted.len(),
            127,
            "a token name repeats across files"
        );

        assert_eq!(PORTED.len(), 117);
        assert_eq!(DEVIATIONS.len(), 10);

        let mut combined: Vec<&str> = PORTED.into_iter().chain(DEVIATIONS).collect();
        combined.sort_unstable();
        assert_eq!(
            combined, parsed_sorted,
            "PORTED \u{222a} DEVIATIONS != the parsed token names"
        );

        for name in DEVIATIONS {
            assert!(
                !PORTED.contains(&name),
                "{name}: listed in both PORTED and DEVIATIONS"
            );
        }
    }
}
