//! SVG → egui-texture icon bake pipeline (port of marshrutka's `emoji.rs`
//! bake half, design `2026-07-18-render-svg-icon-pipeline`).
//!
//! Vendors a curated Lucide (ISC) icon set under `crates/render/icons/` and
//! bakes each into a cached `egui::TextureHandle` on demand (`bake_texture`)
//! or eagerly for the whole curated set (`IconSet`). Unlike marshrutka's
//! `.unwrap()`-chain original, every fallible step here returns
//! [`IconError`] — `gp-render` stays at zero production panics (see the
//! design's "Fallible bake" section).

/// Logical (DPI-independent) side length, in points, at which the curated icon set is baked.
///
/// Consumed by the eager pre-bake (`IconSet::new`). Not the same axis as
/// `tokens::typography::FS_TITLE` (a font-size token) — this is an icon
/// glyph size.
pub const ICON_LOGICAL_SIZE_PX: f32 = 18.0;

/// The curated Lucide icon set vendored under `crates/render/icons/`
/// (design `2026-07-18-render-svg-icon-pipeline` — the five icons #13's
/// `IconButton` row references).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// `play` — Lucide, ISC.
    Play,
    /// `pause` — Lucide, ISC.
    Pause,
    /// `grid-3x3` — Lucide, ISC.
    Grid3x3,
    /// `zoom-in` — Lucide, ISC.
    ZoomIn,
    /// `settings` — Lucide, ISC.
    Settings,
}

impl Icon {
    /// Every vendored icon, in declaration order. Drives `IconSet::new`'s
    /// eager pre-bake loop.
    pub const ALL: [Self; 5] = [
        Self::Play,
        Self::Pause,
        Self::Grid3x3,
        Self::ZoomIn,
        Self::Settings,
    ];

    /// The vendored SVG source bytes for this icon (`include_bytes!`,
    /// mirroring `fonts.rs`'s vendored-asset pattern).
    #[must_use]
    pub const fn svg_bytes(self) -> &'static [u8] {
        match self {
            Self::Play => include_bytes!("../icons/play.svg"),
            Self::Pause => include_bytes!("../icons/pause.svg"),
            Self::Grid3x3 => include_bytes!("../icons/grid-3x3.svg"),
            Self::ZoomIn => include_bytes!("../icons/zoom-in.svg"),
            Self::Settings => include_bytes!("../icons/settings.svg"),
        }
    }

    /// The registration key / cache label for this icon (used as the
    /// `ctx.load_texture` name and the `Debug`-friendly identifier).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Grid3x3 => "grid-3x3",
            Self::ZoomIn => "zoom-in",
            Self::Settings => "settings",
        }
    }
}

/// Errors raised while baking an SVG icon into a texture.
#[derive(thiserror::Error, Debug)]
pub enum IconError {
    /// `usvg` failed to parse the SVG source bytes.
    #[error("failed to parse SVG source: {0}")]
    Parse(#[from] resvg::usvg::Error),
    /// `tiny_skia::Pixmap::new` (or a size computation feeding it) returned
    /// `None` for `width`/`height` — either dimension was zero.
    #[error("failed to allocate a {width}x{height} pixmap")]
    PixmapAlloc {
        /// The requested pixmap width, in physical pixels.
        width: u32,
        /// The requested pixmap height, in physical pixels.
        height: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Pre-step P's recorded byte sizes for the vendored SVGs (Lucide tag
    /// `1.25.0`) — the authoritative SHA-256 pin lives in the vendoring
    /// commit message; this is a dep-free identity guard against accidental
    /// truncation/corruption of the `include_bytes!`'d asset, not a second
    /// hash check (mirrors the fonts design's rejection of a duplicate
    /// identity check).
    const RECORDED_BYTE_SIZES: [(Icon, usize); 5] = [
        (Icon::Play, 306),
        (Icon::Pause, 313),
        (Icon::Grid3x3, 355),
        (Icon::ZoomIn, 376),
        (Icon::Settings, 586),
    ];

    #[test]
    fn all_icons_have_nonempty_svg_bytes() {
        for icon in Icon::ALL {
            let bytes = icon.svg_bytes();
            assert!(!bytes.is_empty(), "{icon:?} svg_bytes() was empty");
            assert!(
                bytes.windows(4).any(|w| w == b"<svg"),
                "{icon:?} svg_bytes() did not contain a `<svg` marker"
            );
        }
    }

    #[test]
    fn vendored_svg_byte_sizes_match_recorded_pins() {
        for (icon, expected_len) in RECORDED_BYTE_SIZES {
            assert_eq!(
                icon.svg_bytes().len(),
                expected_len,
                "{icon:?} svg_bytes() length drifted from pre-step P's recorded pin"
            );
        }
    }

    #[test]
    fn icon_all_and_names_are_the_five_distinct_variants() {
        assert_eq!(Icon::ALL.len(), 5);
        let names: HashSet<&str> = Icon::ALL.iter().map(|icon| icon.name()).collect();
        assert_eq!(
            names.len(),
            5,
            "Icon::name() values were not pairwise distinct"
        );
    }
}
