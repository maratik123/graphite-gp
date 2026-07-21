//! `docs/design-system/tokens/effects.css` → Rust consts.
//!
//! Units: shadow offsets/blur/spread are `f32`-derived `i8`/`u8` geometry (the
//! CSS px numbers, unmodified); durations are `std::time::Duration`, carrying
//! `ms` in the type; ease curves are `[f32; 4]` cubic-bezier control points —
//! the four numbers *are* the token's value, there is no egui easing type to
//! port into.
//!
//! `--shadow-inset` ports under a distinct [`InsetShadow`] type (design
//! *`--shadow-inset` disposition*, AC1 branch (b) #9): `epaint::Shadow` has no
//! inner-shadow flag, so a `Shadow`-typed const would silently render an
//! *outer* drop shadow. `--ease-*` (AC1 branch (b) #4–6) and `--bg-grid` /
//! `--bg-dots` (AC1 branch (b) #7–8, decomposed below) round out this file's
//! share of the ten branch-(b) tokens.
//!
//! **Alpha is stored premultiplied** (AC6 carve-out, design § *Alpha
//! round-trip*): `Color32::from_rgba_unmultiplied_const` premultiplies at
//! construction, so the const's stored bytes are what epaint actually
//! renders — the faithful thing to assert — even though a *round-trip* back
//! through `to_srgba_unmultiplied()` is lossy for every shadow color here
//! except `--focus-shadow`, which happens to round-trip exactly.
//!
//! **Miri:** all 7 `tests` below assert constant/data parity against the
//! `include_str!`'d CSS (or a sibling const), so they carry
//! `#[cfg_attr(miri, ignore = "…")]` (design
//! `2026-07-21-miri-gate-token-tests`): interpreted wall-clock cost, no
//! production Miri UB signal — not an abort.

use crate::tokens::color::{GRID_DOT, GRID_LINE};
use egui::{Color32, Shadow};
use std::time::Duration;

// ---- Shadows (warm, low, subtle) ----

/// `--shadow-0`.
pub const SHADOW_0: Shadow = Shadow::NONE;
/// `--shadow-1`. CSS `rgba(32,30,26,0.08)`, alpha `round(0.08 * 255) = 20`.
pub const SHADOW_1: Shadow = Shadow {
    offset: [0, 1],
    blur: 2,
    spread: 0,
    color: Color32::from_rgba_unmultiplied_const(32, 30, 26, 20),
};
/// `--shadow-2`. CSS `rgba(32,30,26,0.10)`, alpha `round(0.10 * 255) = 26`.
pub const SHADOW_2: Shadow = Shadow {
    offset: [0, 2],
    blur: 6,
    spread: 0,
    color: Color32::from_rgba_unmultiplied_const(32, 30, 26, 26),
};
/// `--shadow-3`. CSS `rgba(32,30,26,0.14)`, alpha `round(0.14 * 255) = 36`.
pub const SHADOW_3: Shadow = Shadow {
    offset: [0, 8],
    blur: 24,
    spread: 0,
    color: Color32::from_rgba_unmultiplied_const(32, 30, 26, 36),
};
/// `--shadow-pop`. CSS `rgba(32,30,26,0.20)`, alpha `round(0.20 * 255) = 51`.
pub const SHADOW_POP: Shadow = Shadow {
    offset: [0, 12],
    blur: 40,
    spread: 0,
    color: Color32::from_rgba_unmultiplied_const(32, 30, 26, 51),
};

/// Port of `--shadow-inset` — pencil-press inner darkening for pressed
/// states.
///
/// A distinct type, not `egui::Shadow`: `epaint::Shadow` has no inner-shadow
/// semantics, so a `Shadow`-typed const here would compile but silently
/// render an *outer* drop shadow. Field names/types mirror `epaint::Shadow`
/// so a `Shadow`-taking API rejects an `InsetShadow` at compile time instead
/// of misrendering it (design *`--shadow-inset` disposition*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsetShadow {
    /// Same shape as `epaint::Shadow::offset`.
    pub offset: [i8; 2],
    /// Same shape as `epaint::Shadow::blur`.
    pub blur: u8,
    /// Same shape as `epaint::Shadow::spread`.
    pub spread: u8,
    /// Same shape as `epaint::Shadow::color`.
    pub color: Color32,
}

/// `--shadow-inset`. CSS `rgba(32,30,26,0.14)`, alpha `round(0.14 * 255) = 36`
/// — the same alpha as `--shadow-3`, a different offset/blur/spread.
pub const SHADOW_INSET: InsetShadow = InsetShadow {
    offset: [0, 1],
    blur: 2,
    spread: 0,
    color: Color32::from_rgba_unmultiplied_const(32, 30, 26, 36),
};

// ---- Motion ----

/// `--ease-standard`. `cubic-bezier(0.2, 0, 0.1, 1)` control points.
pub const EASE_STANDARD: [f32; 4] = [0.2, 0.0, 0.1, 1.0];
/// `--ease-out`. `cubic-bezier(0.16, 1, 0.3, 1)` control points.
pub const EASE_OUT: [f32; 4] = [0.16, 1.0, 0.3, 1.0];
/// `--ease-in`. `cubic-bezier(0.4, 0, 1, 1)` control points.
pub const EASE_IN: [f32; 4] = [0.4, 0.0, 1.0, 1.0];

/// `--dur-fast`. Milliseconds carried in the type — no bare `f32` to misread.
pub const DUR_FAST: Duration = Duration::from_millis(120);
/// `--dur-med`.
pub const DUR_MED: Duration = Duration::from_millis(200);
/// `--dur-slow`.
pub const DUR_SLOW: Duration = Duration::from_millis(320);

// ---- Graph-paper background (quad-ruled) ----

/// `--bg-grid`, decomposed (AC1 branch (b) #7): a CSS gradient recipe, not a
/// value. The ruling-line width, in points.
pub const BG_GRID_RULING_WIDTH: f32 = 1.0;
/// `--bg-grid`'s ruling color — the CSS's own `var(--grid-line)`.
pub const BG_GRID_COLOR: Color32 = GRID_LINE;
// `--bg-grid`'s pitch is `spacing::CELL` — reuse that const directly rather
// than restating it here.

/// `--bg-dots`, decomposed (AC1 branch (b) #8). Dot radius, in points.
pub const BG_DOTS_RADIUS: f32 = 1.2;
/// `--bg-dots`'s fully-transparent falloff stop, in points.
pub const BG_DOTS_TRANSPARENT_STOP: f32 = 1.4;
/// `--bg-dots`'s dot color — the CSS's own `var(--grid-dot)`.
pub const BG_DOTS_COLOR: Color32 = GRID_DOT;

// ---- Focus ----

/// `--focus-shadow`. CSS `rgba(226,74,43,0.35)`, alpha
/// `round(0.35 * 255) = 89` — the one shadow color here that round-trips
/// exactly through `to_srgba_unmultiplied()` (AC6 carve-out).
pub const FOCUS_SHADOW: Shadow = Shadow {
    offset: [0, 0],
    blur: 0,
    spread: 3,
    color: Color32::from_rgba_unmultiplied_const(226, 74, 43, 89),
};

#[cfg(test)]
mod tests {
    use super::{
        BG_DOTS_COLOR, BG_DOTS_RADIUS, BG_DOTS_TRANSPARENT_STOP, BG_GRID_COLOR,
        BG_GRID_RULING_WIDTH, DUR_FAST, DUR_MED, DUR_SLOW, EASE_IN, EASE_OUT, EASE_STANDARD,
        FOCUS_SHADOW, SHADOW_0, SHADOW_1, SHADOW_2, SHADOW_3, SHADOW_INSET, SHADOW_POP,
    };
    use crate::test_util::assert_f32;
    use crate::tokens::color::{GRID_DOT, GRID_LINE};
    use crate::tokens::css::{assert_cubic_bezier, value_of};
    use egui::Shadow;
    use std::time::Duration;

    const CSS: &str = include_str!("../../../../docs/design-system/tokens/effects.css");

    /// `--shadow-0`.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn shadow_0_is_none() {
        assert_eq!(SHADOW_0, Shadow::NONE);
    }

    /// AC1/AC6 — the four elevation shadows' geometry and stored (premultiplied)
    /// color bytes, matching the CSS numbers exactly.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn elevation_shadows_match_css() {
        assert_eq!(SHADOW_1.offset, [0, 1]);
        assert_eq!(SHADOW_1.blur, 2);
        assert_eq!(SHADOW_1.spread, 0);
        assert_eq!(SHADOW_1.color.to_array(), [3, 2, 2, 20]);

        assert_eq!(SHADOW_2.offset, [0, 2]);
        assert_eq!(SHADOW_2.blur, 6);
        assert_eq!(SHADOW_2.spread, 0);
        assert_eq!(SHADOW_2.color.to_array(), [3, 3, 3, 26]);

        assert_eq!(SHADOW_3.offset, [0, 8]);
        assert_eq!(SHADOW_3.blur, 24);
        assert_eq!(SHADOW_3.spread, 0);
        assert_eq!(SHADOW_3.color.to_array(), [5, 4, 4, 36]);

        assert_eq!(SHADOW_POP.offset, [0, 12]);
        assert_eq!(SHADOW_POP.blur, 40);
        assert_eq!(SHADOW_POP.spread, 0);
        assert_eq!(SHADOW_POP.color.to_array(), [6, 6, 5, 51]);
    }

    /// `--shadow-inset` — same stored color as `--shadow-3` (identical CSS
    /// alpha), distinct offset/blur/spread, and a distinct Rust type.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn shadow_inset_matches_css() {
        assert_eq!(SHADOW_INSET.offset, [0, 1]);
        assert_eq!(SHADOW_INSET.blur, 2);
        assert_eq!(SHADOW_INSET.spread, 0);
        assert_eq!(SHADOW_INSET.color.to_array(), [5, 4, 4, 36]);
    }

    /// `--focus-shadow` — the one shadow color that round-trips exactly
    /// (AC6 carve-out).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn focus_shadow_matches_css_and_round_trips() {
        assert_eq!(FOCUS_SHADOW.spread, 3);
        assert_eq!(FOCUS_SHADOW.offset, [0, 0]);
        assert_eq!(FOCUS_SHADOW.blur, 0);
        assert_eq!(
            FOCUS_SHADOW.color.to_srgba_unmultiplied(),
            [226, 74, 43, 89]
        );
    }

    /// AC1/AC6/AC8 — durations, both against `Duration` (integer-typed, no
    /// `float_cmp`) and against the raw CSS text.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn durations_match_css() {
        assert_eq!(DUR_FAST, Duration::from_millis(120));
        assert_eq!(DUR_MED, Duration::from_millis(200));
        assert_eq!(DUR_SLOW, Duration::from_millis(320));
        assert_eq!(value_of(CSS, "--dur-fast"), "120ms");
        assert_eq!(value_of(CSS, "--dur-med"), "200ms");
        assert_eq!(value_of(CSS, "--dur-slow"), "320ms");
    }

    /// AC1/AC6/AC8 — the three ease curves, value-checked from the CSS
    /// (round 3's upgrade over a hand-written expectation).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn eases_match_css() {
        assert_cubic_bezier(CSS, "--ease-standard", EASE_STANDARD);
        assert_cubic_bezier(CSS, "--ease-out", EASE_OUT);
        assert_cubic_bezier(CSS, "--ease-in", EASE_IN);
    }

    /// `--bg-grid` / `--bg-dots` decomposition — hand-written expectations
    /// (the numbers live inside a multi-line gradient recipe, not a
    /// `name: value;` declaration), routed through the shared comparator and
    /// additionally pinned against the raw CSS text.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "interpreted wall-clock cost, no production Miri UB \
                  signal: asserts constant/data parity against the \
                  include_str!'d design-system CSS (or a sibling const), \
                  or a total safe accessor over a static const table — \
                  safe-Rust comparisons, not an abort"
    )]
    fn bg_decomposition_matches_css() {
        assert_f32("BG_GRID_RULING_WIDTH", BG_GRID_RULING_WIDTH, 1.0);
        assert_f32("BG_DOTS_RADIUS", BG_DOTS_RADIUS, 1.2);
        assert_f32("BG_DOTS_TRANSPARENT_STOP", BG_DOTS_TRANSPARENT_STOP, 1.4);
        assert!(value_of(CSS, "--bg-grid").contains("1px"));
        assert!(value_of(CSS, "--bg-dots").contains("1.2px"));
        assert!(value_of(CSS, "--bg-dots").contains("1.4px"));
        assert_eq!(BG_GRID_COLOR, GRID_LINE);
        assert_eq!(BG_DOTS_COLOR, GRID_DOT);
    }
}
