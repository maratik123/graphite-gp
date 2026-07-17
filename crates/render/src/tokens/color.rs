//! `docs/design-system/tokens/colors.css` → Rust consts.
//!
//! 56 total: 38 base colors + 18 semantic aliases (each a const *reference*
//! to a base color, per the CSS's own `var(--x)` convention).

use egui::Color32;

// ---- Paper (the sheet everything sits on) ----

/// `--paper-0` — brightest highlight / card face.
pub const PAPER_0: Color32 = Color32::from_rgb(0xFB, 0xF8, 0xF0);
/// `--paper-1` — base graph-paper cream.
pub const PAPER_1: Color32 = Color32::from_rgb(0xF5, 0xF1, 0xE6);
/// `--paper-2` — recessed / infield tint.
pub const PAPER_2: Color32 = Color32::from_rgb(0xEC, 0xE6, 0xD6);
/// `--paper-3` — pressed / edge.
pub const PAPER_3: Color32 = Color32::from_rgb(0xE0, 0xD9, 0xC6);

// ---- Graphite (pencil ink; warm near-blacks → light strokes) ----

/// `--graphite-900` — darkest ink / primary text.
pub const GRAPHITE_900: Color32 = Color32::from_rgb(0x20, 0x1E, 0x1A);
/// `--graphite-800`.
pub const GRAPHITE_800: Color32 = Color32::from_rgb(0x32, 0x2F, 0x29);
/// `--graphite-700`.
pub const GRAPHITE_700: Color32 = Color32::from_rgb(0x47, 0x43, 0x3B);
/// `--graphite-600`.
pub const GRAPHITE_600: Color32 = Color32::from_rgb(0x62, 0x5C, 0x51);
/// `--graphite-500` — mid pencil.
pub const GRAPHITE_500: Color32 = Color32::from_rgb(0x83, 0x7B, 0x6D);
/// `--graphite-400`.
pub const GRAPHITE_400: Color32 = Color32::from_rgb(0xA6, 0x9D, 0x8C);
/// `--graphite-300` — faint pencil / hairline.
pub const GRAPHITE_300: Color32 = Color32::from_rgb(0xC4, 0xBB, 0xAA);
/// `--graphite-200`.
pub const GRAPHITE_200: Color32 = Color32::from_rgb(0xDA, 0xD2, 0xC1);

// ---- Grid (the graph-paper ruling) ----

/// `--grid-line` — faint engineering-blue ruling.
pub const GRID_LINE: Color32 = Color32::from_rgb(0xC3, 0xCE, 0xDD);
/// `--grid-line-major` — every-5th heavier line.
pub const GRID_LINE_MAJOR: Color32 = Color32::from_rgb(0xA9, 0xB8, 0xCC);
/// `--grid-dot` — lattice points.
pub const GRID_DOT: Color32 = Color32::from_rgb(0x93, 0xA2, 0xB8);

// ---- Asphalt (drivable corridor, derived from D) ----

/// `--asphalt-1` — asphalt fill.
pub const ASPHALT_1: Color32 = Color32::from_rgb(0x5E, 0x59, 0x4F);
/// `--asphalt-2` — asphalt shade.
pub const ASPHALT_2: Color32 = Color32::from_rgb(0x4A, 0x46, 0x3D);
/// `--wall` — wall = fill boundary.
pub const WALL: Color32 = Color32::from_rgb(0x20, 0x1E, 0x1A);

// ---- Racing accent (Graphite GP vermilion — the "GP" red) ----

/// `--accent`.
pub const ACCENT: Color32 = Color32::from_rgb(0xE2, 0x4A, 0x2B);
/// `--accent-hover`.
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0xC9, 0x3C, 0x20);
/// `--accent-press`.
pub const ACCENT_PRESS: Color32 = Color32::from_rgb(0xB2, 0x33, 0x19);
/// `--accent-tint` — washed accent surface.
pub const ACCENT_TINT: Color32 = Color32::from_rgb(0xF7, 0xD9, 0xCF);

// ---- Car colors (chalk hues; car 1 = the accent) ----

/// `--car-1` — vermilion.
pub const CAR_1: Color32 = Color32::from_rgb(0xE2, 0x4A, 0x2B);
/// `--car-2` — chalk blue.
pub const CAR_2: Color32 = Color32::from_rgb(0x2E, 0x6F, 0xB5);
/// `--car-3` — chalk green.
pub const CAR_3: Color32 = Color32::from_rgb(0x2E, 0x9E, 0x5B);
/// `--car-4` — chalk amber.
pub const CAR_4: Color32 = Color32::from_rgb(0xE8, 0xB2, 0x3A);
/// `--car-5` — chalk plum.
pub const CAR_5: Color32 = Color32::from_rgb(0x7B, 0x4B, 0x9E);
/// `--car-6` — chalk teal.
pub const CAR_6: Color32 = Color32::from_rgb(0x17, 0x99, 0x9B);

// ---- Speed heatmap (slow → fast) ----

/// `--heat-0` — slowest (blue).
pub const HEAT_0: Color32 = Color32::from_rgb(0x2E, 0x6F, 0xB5);
/// `--heat-1` — teal.
pub const HEAT_1: Color32 = Color32::from_rgb(0x17, 0x99, 0x9B);
/// `--heat-2` — amber.
pub const HEAT_2: Color32 = Color32::from_rgb(0xE8, 0xB2, 0x3A);
/// `--heat-3` — fastest (red).
pub const HEAT_3: Color32 = Color32::from_rgb(0xE2, 0x4A, 0x2B);

// ---- Semantic status ----

/// `--ok` — valid track / green light.
pub const OK: Color32 = Color32::from_rgb(0x2E, 0x9E, 0x5B);
/// `--warn` — run-out / caution.
pub const WARN: Color32 = Color32::from_rgb(0xE8, 0xB2, 0x3A);
/// `--danger` — crash / illegal move.
pub const DANGER: Color32 = Color32::from_rgb(0xE2, 0x4A, 0x2B);
/// `--ok-tint`.
pub const OK_TINT: Color32 = Color32::from_rgb(0xDC, 0xEE, 0xE2);
/// `--warn-tint`.
pub const WARN_TINT: Color32 = Color32::from_rgb(0xF7, 0xEA, 0xCB);
/// `--danger-tint`.
pub const DANGER_TINT: Color32 = Color32::from_rgb(0xF7, 0xD9, 0xCF);

// ---- Semantic aliases — reach for these in components/kits ----

/// `--surface-page: var(--paper-1)`.
pub const SURFACE_PAGE: Color32 = PAPER_1;
/// `--surface-card: var(--paper-0)`. NOT `PAPER_2` — see design finding on
/// `placeholder.rs`'s `CARD_FILL`, which maps to `PAPER_2` instead of this.
pub const SURFACE_CARD: Color32 = PAPER_0;
/// `--surface-sunken: var(--paper-2)`.
pub const SURFACE_SUNKEN: Color32 = PAPER_2;
/// `--surface-infield: var(--paper-2)`.
pub const SURFACE_INFIELD: Color32 = PAPER_2;
/// `--surface-asphalt: var(--asphalt-1)`.
pub const SURFACE_ASPHALT: Color32 = ASPHALT_1;
/// `--surface-ink: var(--graphite-900)` — inverse / dark panels.
pub const SURFACE_INK: Color32 = GRAPHITE_900;

/// `--text-ink: var(--graphite-900)`.
pub const TEXT_INK: Color32 = GRAPHITE_900;
/// `--text-body: var(--graphite-800)`.
pub const TEXT_BODY: Color32 = GRAPHITE_800;
/// `--text-muted: var(--graphite-500)`.
pub const TEXT_MUTED: Color32 = GRAPHITE_500;
/// `--text-faint: var(--graphite-400)`.
pub const TEXT_FAINT: Color32 = GRAPHITE_400;
/// `--text-on-ink: var(--paper-1)`.
pub const TEXT_ON_INK: Color32 = PAPER_1;
/// `--text-on-accent: var(--paper-0)`.
pub const TEXT_ON_ACCENT: Color32 = PAPER_0;
/// `--text-link: var(--accent)`.
pub const TEXT_LINK: Color32 = ACCENT;
/// `--text-link-hover: var(--accent-hover)`.
pub const TEXT_LINK_HOVER: Color32 = ACCENT_HOVER;

/// `--border-hairline: var(--graphite-300)`.
pub const BORDER_HAIRLINE: Color32 = GRAPHITE_300;
/// `--border-strong: var(--graphite-900)`.
pub const BORDER_STRONG: Color32 = GRAPHITE_900;
/// `--border-soft: var(--graphite-200)`.
pub const BORDER_SOFT: Color32 = GRAPHITE_200;

/// `--focus-ring: var(--accent)`.
pub const FOCUS_RING: Color32 = ACCENT;

// ---- Ramps ----

/// The six car colors, indexed by 0-based car index (AC3).
pub const CAR_COLORS: [Color32; 6] = [CAR_1, CAR_2, CAR_3, CAR_4, CAR_5, CAR_6];
/// The speed heatmap ramp, ordered slow → fast (AC4).
pub const HEAT_RAMP: [Color32; 4] = [HEAT_0, HEAT_1, HEAT_2, HEAT_3];

/// Looks up a car's color by its 0-based index; `None` if `index` is out of
/// range.
///
/// Deliberately not `const fn`: `<[T]>::get` is not yet const-stable
/// (attempting `const fn` here hits `E0658`) — see design finding 4. This is
/// the idiomatic combinator form; `car_color(6)` and `car_color(usize::MAX)`
/// are both `None`, never a panic.
pub fn car_color(index: usize) -> Option<Color32> {
    CAR_COLORS.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT, ACCENT_HOVER, ACCENT_PRESS, ACCENT_TINT, ASPHALT_1, ASPHALT_2, BORDER_HAIRLINE,
        BORDER_SOFT, BORDER_STRONG, CAR_1, CAR_2, CAR_3, CAR_4, CAR_5, CAR_6, CAR_COLORS, DANGER,
        DANGER_TINT, FOCUS_RING, GRAPHITE_200, GRAPHITE_300, GRAPHITE_400, GRAPHITE_500,
        GRAPHITE_600, GRAPHITE_700, GRAPHITE_800, GRAPHITE_900, GRID_DOT, GRID_LINE,
        GRID_LINE_MAJOR, HEAT_0, HEAT_1, HEAT_2, HEAT_3, HEAT_RAMP, OK, OK_TINT, PAPER_0, PAPER_1,
        PAPER_2, PAPER_3, SURFACE_ASPHALT, SURFACE_CARD, SURFACE_INFIELD, SURFACE_INK,
        SURFACE_PAGE, SURFACE_SUNKEN, TEXT_BODY, TEXT_FAINT, TEXT_INK, TEXT_LINK, TEXT_LINK_HOVER,
        TEXT_MUTED, TEXT_ON_ACCENT, TEXT_ON_INK, WALL, WARN, WARN_TINT, car_color,
    };
    use crate::tokens::css::{value_of, var_target};
    use egui::Color32;

    const CSS: &str = include_str!("../../../../docs/design-system/tokens/colors.css");

    /// Every base (non-alias) color token, in source order.
    const BASE: [(&str, Color32); 38] = [
        ("--paper-0", PAPER_0),
        ("--paper-1", PAPER_1),
        ("--paper-2", PAPER_2),
        ("--paper-3", PAPER_3),
        ("--graphite-900", GRAPHITE_900),
        ("--graphite-800", GRAPHITE_800),
        ("--graphite-700", GRAPHITE_700),
        ("--graphite-600", GRAPHITE_600),
        ("--graphite-500", GRAPHITE_500),
        ("--graphite-400", GRAPHITE_400),
        ("--graphite-300", GRAPHITE_300),
        ("--graphite-200", GRAPHITE_200),
        ("--grid-line", GRID_LINE),
        ("--grid-line-major", GRID_LINE_MAJOR),
        ("--grid-dot", GRID_DOT),
        ("--asphalt-1", ASPHALT_1),
        ("--asphalt-2", ASPHALT_2),
        ("--wall", WALL),
        ("--accent", ACCENT),
        ("--accent-hover", ACCENT_HOVER),
        ("--accent-press", ACCENT_PRESS),
        ("--accent-tint", ACCENT_TINT),
        ("--car-1", CAR_1),
        ("--car-2", CAR_2),
        ("--car-3", CAR_3),
        ("--car-4", CAR_4),
        ("--car-5", CAR_5),
        ("--car-6", CAR_6),
        ("--heat-0", HEAT_0),
        ("--heat-1", HEAT_1),
        ("--heat-2", HEAT_2),
        ("--heat-3", HEAT_3),
        ("--ok", OK),
        ("--warn", WARN),
        ("--danger", DANGER),
        ("--ok-tint", OK_TINT),
        ("--warn-tint", WARN_TINT),
        ("--danger-tint", DANGER_TINT),
    ];

    /// Every semantic alias: name, its const, the base const it must equal,
    /// and the base token name the CSS's own `var(--x)` must still target.
    const ALIASES: [(&str, Color32, Color32, &str); 18] = [
        ("--surface-page", SURFACE_PAGE, PAPER_1, "--paper-1"),
        ("--surface-card", SURFACE_CARD, PAPER_0, "--paper-0"),
        ("--surface-sunken", SURFACE_SUNKEN, PAPER_2, "--paper-2"),
        ("--surface-infield", SURFACE_INFIELD, PAPER_2, "--paper-2"),
        (
            "--surface-asphalt",
            SURFACE_ASPHALT,
            ASPHALT_1,
            "--asphalt-1",
        ),
        ("--surface-ink", SURFACE_INK, GRAPHITE_900, "--graphite-900"),
        ("--text-ink", TEXT_INK, GRAPHITE_900, "--graphite-900"),
        ("--text-body", TEXT_BODY, GRAPHITE_800, "--graphite-800"),
        ("--text-muted", TEXT_MUTED, GRAPHITE_500, "--graphite-500"),
        ("--text-faint", TEXT_FAINT, GRAPHITE_400, "--graphite-400"),
        ("--text-on-ink", TEXT_ON_INK, PAPER_1, "--paper-1"),
        ("--text-on-accent", TEXT_ON_ACCENT, PAPER_0, "--paper-0"),
        ("--text-link", TEXT_LINK, ACCENT, "--accent"),
        (
            "--text-link-hover",
            TEXT_LINK_HOVER,
            ACCENT_HOVER,
            "--accent-hover",
        ),
        (
            "--border-hairline",
            BORDER_HAIRLINE,
            GRAPHITE_300,
            "--graphite-300",
        ),
        (
            "--border-strong",
            BORDER_STRONG,
            GRAPHITE_900,
            "--graphite-900",
        ),
        ("--border-soft", BORDER_SOFT, GRAPHITE_200, "--graphite-200"),
        ("--focus-ring", FOCUS_RING, ACCENT, "--accent"),
    ];

    /// Parses a `#RRGGBB` CSS color literal. Test-only; the value handed in
    /// always comes from `value_of`, which has already cut at `;` and trimmed.
    fn parse_hex_color(value: &str) -> Color32 {
        let hex = value
            .strip_prefix('#')
            .unwrap_or_else(|| panic!("not a hex colour: {value:?}"));
        let r = u8::from_str_radix(&hex[0..2], 16).expect("valid hex byte");
        let g = u8::from_str_radix(&hex[2..4], 16).expect("valid hex byte");
        let b = u8::from_str_radix(&hex[4..6], 16).expect("valid hex byte");
        Color32::from_rgb(r, g, b)
    }

    /// AC1/AC6/AC8 — every base token's parsed hex matches its const. The
    /// parser cuts at `;`, never at end-of-line: 28 of these 56 color
    /// declarations carry a trailing `/* ... */` comment (design finding 6).
    #[test]
    fn base_colors_match_css() {
        for (name, want) in BASE {
            let parsed = parse_hex_color(value_of(CSS, name));
            assert_eq!(parsed, want, "{name}: CSS value != const");
        }
    }

    /// AC5 — every alias equals its base const, and the CSS's own `var(--x)`
    /// still points at that same base token.
    #[test]
    fn aliases_match_their_base() {
        for (name, alias, base, target) in ALIASES {
            assert_eq!(alias, base, "{name}: alias const != base const");
            assert_eq!(
                var_target(CSS, name),
                target,
                "{name}: var() target drifted"
            );
        }
    }

    /// AC3 — the car ramp: length, first entry, and `car_color`'s totality.
    #[test]
    fn car_colors_and_accessor() {
        assert_eq!(CAR_COLORS.len(), 6);
        assert_eq!(CAR_COLORS[0], ACCENT);
        assert_eq!(car_color(0), Some(ACCENT));
        assert_eq!(car_color(5), Some(CAR_6));
        assert_eq!(car_color(6), None);
        assert_eq!(car_color(usize::MAX), None);
    }

    /// AC4 — the heat ramp, ordered slow → fast.
    #[test]
    fn heat_ramp_is_ordered_slow_to_fast() {
        assert_eq!(HEAT_RAMP.len(), 4);
        assert_eq!(HEAT_RAMP, [HEAT_0, HEAT_1, HEAT_2, HEAT_3]);
        assert_eq!(HEAT_0, Color32::from_rgb(0x2E, 0x6F, 0xB5));
        assert_eq!(HEAT_3, Color32::from_rgb(0xE2, 0x4A, 0x2B));
    }

    /// AC6 — cross-file identity: car 1 is the accent, and the accent is the
    /// ramp's fastest color; a spot-check alias identity too.
    #[test]
    fn cross_identities_hold() {
        assert_eq!(CAR_COLORS[0], ACCENT);
        assert_eq!(ACCENT, HEAT_RAMP[3]);
        assert_eq!(SURFACE_PAGE, PAPER_1);
    }
}
