//! SVG → egui-texture icon bake pipeline (port of marshrutka's `emoji.rs`
//! bake half, design `2026-07-18-render-svg-icon-pipeline`).
//!
//! Vendors a curated Lucide (ISC) icon set under `crates/render/icons/` and
//! bakes each into a cached `egui::TextureHandle` on demand
//! ([`bake_texture`]) or eagerly for the whole curated set ([`IconSet`]).
//! Unlike marshrutka's `.unwrap()`-chain original, every fallible step here
//! returns [`IconError`] — `gp-render` stays at zero production panics (see
//! the design's "Fallible bake" section).

use egui::epaint::textures::TextureOptions;
use egui::{ColorImage, TextureHandle};
use resvg::usvg::{Options, Transform, Tree};
use std::collections::HashMap;
use tiny_skia::Pixmap;

/// Logical (DPI-independent) side length, in points, at which the curated icon set is baked.
///
/// Consumed by the eager pre-bake ([`IconSet::new`]). Not the same axis as
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
    /// Every vendored icon, in declaration order. Drives [`IconSet::new`]'s
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

/// Bakes SVG source bytes into an [`egui::ColorImage`], Context-free and
/// GPU-free (the CPU half of the pipeline — the AC5 unit tests drive this
/// directly).
///
/// `logical_px` is the desired *width* in logical (DPI-independent) points;
/// `ppp` is `egui::Context::pixels_per_point()`. The physical pixel width is
/// `logical_px * ppp`, and the height is scaled to match the source SVG's
/// aspect ratio.
///
/// # Errors
///
/// Returns [`IconError::Parse`] if `svg` is not a parseable SVG document, or
/// [`IconError::PixmapAlloc`] if the computed physical size has a
/// zero-length dimension.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "logical_px/ppp are positive, finite, and bounded by realistic \
              UI sizes (icons render at tens of physical pixels); the \
              f32->u32 physical-size cast and the u32->f32 transform-scale \
              cast are both comfortably in-domain for that range"
)]
pub fn svg_to_color_image(svg: &[u8], logical_px: f32, ppp: f32) -> Result<ColorImage, IconError> {
    let tree = Tree::from_data(svg, &Options::default())?;
    let svg_size = tree.size();

    let target_width = (logical_px * ppp).round() as u32;
    let size =
        svg_size
            .to_int_size()
            .scale_to_width(target_width)
            .ok_or(IconError::PixmapAlloc {
                width: target_width,
                height: target_width,
            })?;

    let mut pixmap =
        Pixmap::new(size.width(), size.height()).ok_or_else(|| IconError::PixmapAlloc {
            width: size.width(),
            height: size.height(),
        })?;

    let transform = Transform::from_scale(
        size.width() as f32 / svg_size.width(),
        size.height() as f32 / svg_size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(ColorImage::from_rgba_premultiplied(
        [pixmap.width() as usize, pixmap.height() as usize],
        pixmap.data(),
    ))
}

/// Bakes SVG source bytes into a cached [`egui::TextureHandle`] (AC1): the
/// public, `Context`-touching surface wrapping [`svg_to_color_image`].
///
/// `name` becomes the texture's registration key (and `ctx.load_texture`'s
/// cache label). DPI is read from `ctx.pixels_per_point()`.
///
/// # Errors
///
/// Propagates [`svg_to_color_image`]'s errors.
pub fn bake_texture(
    ctx: &egui::Context,
    name: impl Into<String>,
    svg: &[u8],
    logical_px: f32,
) -> Result<TextureHandle, IconError> {
    let image = svg_to_color_image(svg, logical_px, ctx.pixels_per_point())?;
    Ok(ctx.load_texture(name, image, TextureOptions::default()))
}

/// The curated Lucide icon set, eagerly pre-baked into cached textures at
/// construction (AC1, AC2).
///
/// Keyed by [`Icon`] alone (a single logical size, [`ICON_LOGICAL_SIZE_PX`]
/// — see the design's "Baked size variants" resolution). A future
/// `(Icon, size)` cache is additive, not a rewrite, if a second size is
/// ever needed.
pub struct IconSet(HashMap<Icon, TextureHandle>);

impl IconSet {
    /// Eagerly bakes every [`Icon::ALL`] variant at [`ICON_LOGICAL_SIZE_PX`].
    ///
    /// # Errors
    ///
    /// Propagates the first [`IconError`] hit while baking any vendored
    /// icon (unreachable for the vendored assets in practice, but `Result`
    /// is honest since the same bake path is `pub` via [`bake_texture`]).
    pub fn new(ctx: &egui::Context) -> Result<Self, IconError> {
        let mut map = HashMap::with_capacity(Icon::ALL.len());
        for icon in Icon::ALL {
            let handle = bake_texture(ctx, icon.name(), icon.svg_bytes(), ICON_LOGICAL_SIZE_PX)?;
            map.insert(icon, handle);
        }
        Ok(Self(map))
    }

    /// Looks up the cached texture for `icon`, if it was baked.
    #[must_use]
    pub fn get(&self, icon: Icon) -> Option<&TextureHandle> {
        self.0.get(&icon)
    }
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

    /// `play.svg`'s `viewBox` is a square `0 0 24 24` (verified against the
    /// vendored asset); at `logical_px = 18.0, ppp = 2.0` the physical
    /// target width is `18 * 2 = 36`, and scaling a square SVG to that
    /// width yields an exact `36x36` physical size.
    ///
    /// Not Miri-gated: verified via `MIRIFLAGS=-Zmiri-tree-borrows cargo
    /// miri test -p gp-render icons::tests::svg_to_color_image_produces_square_rgba`
    /// that this resvg 0.47 / tiny-skia 0.12 raster call site does **not**
    /// abort Miri — the design's risk premise (mirroring
    /// `tessellation_smoke`'s `vello_cpu` checked-cast abort) does not hold
    /// here, so no truthful gate reason exists.
    #[test]
    fn svg_to_color_image_produces_square_rgba() {
        let image = svg_to_color_image(Icon::Play.svg_bytes(), ICON_LOGICAL_SIZE_PX, 2.0)
            .expect("play.svg bakes cleanly");

        assert_eq!(image.size, [36, 36]);
        assert!(!image.pixels.is_empty(), "baked ColorImage had no pixels");

        let alpha_values: HashSet<u8> = image.pixels.iter().map(egui::Color32::a).collect();
        assert!(
            alpha_values.len() > 1,
            "expected varying alpha (opaque strokes over a transparent \
             field), got a single alpha value {alpha_values:?}"
        );
    }

    /// Not Miri-gated: verified via `MIRIFLAGS=-Zmiri-tree-borrows cargo
    /// miri test -p gp-render icons::tests::svg_to_color_image_rejects_garbage`
    /// that the `usvg::Tree::from_data` parse-error path does not abort
    /// Miri either.
    #[test]
    fn svg_to_color_image_rejects_garbage() {
        let result = svg_to_color_image(b"not an svg", 18.0, 1.0);
        assert!(
            matches!(result, Err(IconError::Parse(_))),
            "expected IconError::Parse, got {result:?}"
        );
    }

    /// `Context::default()` needs no fonts/window to bake textures (the
    /// marshrutka `init_emojis` test proves a bare Context suffices).
    #[cfg_attr(
        miri,
        ignore = "bakes settings.svg at ICON_LOGICAL_SIZE_PX (18, ppp 1.0) \
                  through IconSet::new, which panics under Miri at \
                  tiny-skia-0.12.0/src/pipeline/mod.rs:205 (\"range end \
                  index 330 out of range for slice of length 324\") — \
                  isolated by bisecting Icon::ALL one variant at a time; \
                  play/pause/grid-3x3/zoom-in all bake cleanly under Miri \
                  at width 18, only settings.svg's stroke geometry \
                  triggers this tiny-skia \"simd\" feature scanline \
                  over-read (unrelated to the tessellation_smoke/vello_cpu \
                  checked-cast abort class)"
    )]
    #[test]
    fn icon_set_bakes_all_five() {
        let ctx = egui::Context::default();
        let set = IconSet::new(&ctx).expect("vendored icons bake");

        let mut ids = HashSet::new();
        for icon in Icon::ALL {
            let handle = set.get(icon).expect("every vendored icon was baked");
            assert!(
                ids.insert(handle.id()),
                "{icon:?}'s TextureHandle::id() collided with another icon's"
            );
        }
    }
}
