//! `docs/design-system/tokens/spacing.css` → Rust consts.
//!
//! Unit: **`f32` logical points** (1 CSS px = 1 point), the same convention as
//! `typography.rs`'s `--fs-*`. `--space-0` is the one bare (unsuffixed) `0` in
//! this file — every other declaration carries an explicit `px`. Radii are
//! `f32` too (not `u8`): `--radius-pill: 999px` ports exactly as
//! `RADIUS_PILL: f32 = 999.0`, and `From<f32> for CornerRadius` saturates to
//! `255` at the use site — see the design's finding 2 (ratified).
//!
//! **Miri:** both `tests` below assert constant/data parity against the
//! `include_str!`'d CSS (or a hand-computed saturation), so they carry
//! `#[cfg_attr(miri, ignore = "…")]` (design
//! `2026-07-21-miri-gate-token-tests`): interpreted wall-clock cost, no
//! production Miri UB signal — not an abort.

// ---- Spacing scale (4px lattice) ----

/// `--space-0`. The one bare (unsuffixed) declaration in this file.
pub const SPACE_0: f32 = 0.0;
/// `--space-1`.
pub const SPACE_1: f32 = 4.0;
/// `--space-2`.
pub const SPACE_2: f32 = 8.0;
/// `--space-3`.
pub const SPACE_3: f32 = 12.0;
/// `--space-4`.
pub const SPACE_4: f32 = 16.0;
/// `--space-5`.
pub const SPACE_5: f32 = 20.0;
/// `--space-6`.
pub const SPACE_6: f32 = 24.0;
/// `--space-8`.
pub const SPACE_8: f32 = 32.0;
/// `--space-10`.
pub const SPACE_10: f32 = 40.0;
/// `--space-12`.
pub const SPACE_12: f32 = 48.0;
/// `--space-16`.
pub const SPACE_16: f32 = 64.0;
/// `--space-20`.
pub const SPACE_20: f32 = 80.0;

// ---- Graph-paper cell (the design grid pitch) ----

/// `--cell` — one graph-paper square.
pub const CELL: f32 = 24.0;
/// `--cell-sm`.
pub const CELL_SM: f32 = 16.0;
/// `--cell-lg`.
pub const CELL_LG: f32 = 32.0;

// ---- Radii — crisp / blueprint. Small by default. ----

/// `--radius-0` — grid elements, chips-on-grid.
pub const RADIUS_0: f32 = 0.0;
/// `--radius-1`.
pub const RADIUS_1: f32 = 3.0;
/// `--radius-2` — default cards / inputs.
pub const RADIUS_2: f32 = 6.0;
/// `--radius-3` — panels.
pub const RADIUS_3: f32 = 10.0;
/// `--radius-pill`. A normal token (design finding 2, ratified).
///
/// Ported exactly as `f32` — **not** an exclusion, and not to be re-typed as
/// `u8`. `From<f32> for CornerRadius` saturates `999.0` to `255` at the use
/// site, which is epaint's documented behavior, not a lossy re-typing here.
pub const RADIUS_PILL: f32 = 999.0;

// ---- Border widths (pencil strokes) ----

/// `--bw-hair`.
pub const BW_HAIR: f32 = 1.0;
/// `--bw-1`.
pub const BW_1: f32 = 1.5;
/// `--bw-2`.
pub const BW_2: f32 = 2.0;
/// `--bw-heavy` — wall emphasis.
pub const BW_HEAVY: f32 = 3.0;

// ---- Control sizing ----

/// `--control-h-sm`.
pub const CONTROL_H_SM: f32 = 30.0;
/// `--control-h-md`.
pub const CONTROL_H_MD: f32 = 38.0;
/// `--control-h-lg`.
pub const CONTROL_H_LG: f32 = 46.0;
/// `--tap-min` — min hit target.
pub const TAP_MIN: f32 = 44.0;

// ---- Containers ----

/// `--content-max`.
pub const CONTENT_MAX: f32 = 1200.0;
/// `--panel-max`.
pub const PANEL_MAX: f32 = 420.0;

#[cfg(test)]
mod tests {
    use super::{
        BW_1, BW_2, BW_HAIR, BW_HEAVY, CELL, CELL_LG, CELL_SM, CONTENT_MAX, CONTROL_H_LG,
        CONTROL_H_MD, CONTROL_H_SM, PANEL_MAX, RADIUS_0, RADIUS_1, RADIUS_2, RADIUS_3, RADIUS_PILL,
        SPACE_0, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_5, SPACE_6, SPACE_8, SPACE_10, SPACE_12,
        SPACE_16, SPACE_20, TAP_MIN,
    };
    use crate::tokens::css::assert_token;
    use egui::CornerRadius;

    const CSS: &str = include_str!("../../../../docs/design-system/tokens/spacing.css");

    /// Every token in this file, name-to-const, in source order. Value-checked
    /// against the CSS (AC1/AC6/AC8) via the shared `assert_token` helper.
    const TOKENS: [(&str, f32); 30] = [
        ("--space-0", SPACE_0),
        ("--space-1", SPACE_1),
        ("--space-2", SPACE_2),
        ("--space-3", SPACE_3),
        ("--space-4", SPACE_4),
        ("--space-5", SPACE_5),
        ("--space-6", SPACE_6),
        ("--space-8", SPACE_8),
        ("--space-10", SPACE_10),
        ("--space-12", SPACE_12),
        ("--space-16", SPACE_16),
        ("--space-20", SPACE_20),
        ("--cell", CELL),
        ("--cell-sm", CELL_SM),
        ("--cell-lg", CELL_LG),
        ("--radius-0", RADIUS_0),
        ("--radius-1", RADIUS_1),
        ("--radius-2", RADIUS_2),
        ("--radius-3", RADIUS_3),
        ("--radius-pill", RADIUS_PILL),
        ("--bw-hair", BW_HAIR),
        ("--bw-1", BW_1),
        ("--bw-2", BW_2),
        ("--bw-heavy", BW_HEAVY),
        ("--control-h-sm", CONTROL_H_SM),
        ("--control-h-md", CONTROL_H_MD),
        ("--control-h-lg", CONTROL_H_LG),
        ("--tap-min", TAP_MIN),
        ("--content-max", CONTENT_MAX),
        ("--panel-max", PANEL_MAX),
    ];

    /// AC1/AC6/AC8 — every token's parsed CSS value matches its const. Also
    /// covers `--space-0`'s bare `0` (the parser's hardest case: declaration
    /// #1 in the file, the only unsuffixed numeric among 29 otherwise-`px`
    /// tokens) and the two AC6 exemplars against their longer-named,
    /// short-first-in-source siblings (`--cell` vs `--cell-sm`).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn tokens_match_css() {
        for (name, want) in TOKENS {
            assert_token(CSS, name, want);
        }
    }

    /// Pins design finding 2 (ratified): `RADIUS_PILL` round-trips through
    /// `CornerRadius::from(f32)` as a saturating clamp to `255`, so the
    /// exact-port claim is a tested contract, not a comment. Integer
    /// comparison (`CornerRadius`'s fields are `u8`) — needs no `float_cmp`
    /// allow.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn radius_pill_saturates_to_255() {
        assert_eq!(CornerRadius::from(RADIUS_PILL), CornerRadius::same(255));
    }
}
