//! Scaffold placeholder drawing path (design: `2026-07-16-render-backend-decision`).
//!
//! `draw_placeholder` is the one drawing path shared by the tessellation smoke
//! test, the golden-image test, and `gp-game`'s window (design *Key decision —
//! the placeholder drawing path*). It is intentionally **not** `render_frame`:
//! `render_frame` stays `todo!()` (AC5) and takes no target rect, whereas this
//! function's `rect` parameter makes its output a pure function of its input,
//! which is what lets the golden guard derive its pixel probes instead of
//! hardcoding them.
//!
//! Colors and lengths below are repointed to `crate::tokens` (design:
//! `2026-07-17-render-design-tokens`, subtask 6) — this file's own scaffold
//! names are kept (repointed, not deleted) since they carry information the
//! raw token names do not, and the migration is pixel-neutral: the golden
//! stays byte-identical (AC12). `CARD_FILL` binds to `tokens::color::PAPER_2`
//! — **not** `tokens::color::SURFACE_CARD` (`= PAPER_0`, a different pixel);
//! see the design's binding table. `CARD_CORNER_RADIUS` and
//! `GRID_DOT_RADIUS` stay local: neither numerically matches any token in
//! the design system (the design system's radius ramp is 0/3/6/10, and its
//! only dot radius is `--bg-dots`'s 1.2, not this scaffold's 1.0).

use crate::fonts::{JETBRAINS_MONO_MEDIUM, ONEST_BOLD, ONEST_MEDIUM};
use crate::tokens::{color, spacing, typography};
use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rangef, Rect, Stroke, StrokeKind, pos2,
};

/// The placeholder's fixed canvas: 320×192 logical points at `pixels_per_point
/// = 1.0`. The single source of truth for the golden's size, consumed by the
/// tessellation test and the golden test — `gp-game`'s window draws into
/// whatever size the OS gives it (`ui.max_rect()`), so this const has no
/// production (non-test) reader; see the `cfg_attr` below.
///
/// Grown from 192×128 (design `2026-07-17-render-onest-font-swap`, D2) to fit
/// the three-row text sample the AC16 by-eye check needs — a too-small canvas
/// would clip silently (egui clips at the painter's clip rect), which is
/// exactly the failure class this task exists to remove.
///
/// Built as a struct literal, not `Rect::from_min_size` — that constructor is
/// not `const fn` on 0.35, and `Rect`'s fields are public.
#[cfg_attr(not(test), allow(dead_code, reason = "test-only canvas fixture"))]
const CANVAS_RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(320.0, 192.0),
};

/// Spacing between graph-paper ruling lines and dots. `tokens::spacing::CELL_SM`
/// — also equals `--space-4` (16px); it is a graph-paper pitch.
const GRID_SPACING: f32 = spacing::CELL_SM;
/// Radius of each graph-paper dot. Stays local: `1.0` matches no radius
/// token (the design system's only dot radius is `--bg-dots`'s `1.2`).
const GRID_DOT_RADIUS: f32 = 1.0;
/// Corner radius of the placeholder card — crisp, not rounded (AC4(a)).
/// Stays local: `4` matches no radius token (the ramp is 0/3/6/10).
const CARD_CORNER_RADIUS: u8 = 4;
/// Stroke width of the card border and the hairline. `tokens::spacing::BW_HAIR`.
const HAIRLINE_STROKE_WIDTH: f32 = spacing::BW_HAIR;

/// `tokens::color::PAPER_1` — the paper background fill.
const PAPER: Color32 = color::PAPER_1;
/// `tokens::color::PAPER_2` — the card fill. **Not** `color::SURFACE_CARD`
/// (`= PAPER_0`, a different pixel) — see this file's module doc.
const CARD_FILL: Color32 = color::PAPER_2;
/// `tokens::color::GRAPHITE_900` — the card stroke (crisp ink).
const CARD_STROKE: Color32 = color::GRAPHITE_900;
/// `tokens::color::GRAPHITE_300` — the hairline stroke, the token literally
/// named "hairline".
const HAIRLINE: Color32 = color::GRAPHITE_300;
/// `tokens::color::GRID_LINE` — the graph-paper ruling.
const GRID_LINE: Color32 = color::GRID_LINE;
/// `tokens::color::GRID_DOT` — the graph-paper dot at each ruling intersection.
const GRID_DOT: Color32 = color::GRID_DOT;

/// Row 1 sample text — the wordmark, in the display face at its heaviest
/// registered weight (design `2026-07-17-render-onest-font-swap` § Test
/// Design, D1/D3).
const SAMPLE_ROW_1: &str = "GRAPHITE GP";
/// Row 2 sample text — Cyrillic + en-dash + digits, exercising the display
/// face's non-ASCII coverage (the glyph the swap exists for, `Ф`).
const SAMPLE_ROW_2: &str = "Ф1 – Ф7";
/// Row 3 sample text — mono telemetry with a middot, an arrow, and `✓` at the
/// Badge's own weight (`--fw-medium`), reproducing the motivating use case
/// exactly (D3).
const SAMPLE_ROW_3: &str = "L3 · v4→6 ✓";

/// Derived positions for the placeholder's content, private so the guard and
/// the drawing path never write a probe coordinate twice.
///
/// `paper_probe`/`hairline_probe` have no production (non-test) reader —
/// they exist for the AC9 golden guard alone — hence the crate-conditional
/// `allow` below rather than a blanket one.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "AC9 guard probes are test-only readers")
)]
struct PlaceholderGeometry {
    /// The one card rect (fill + crisp radius + stroke).
    card_rect: Rect,
    /// Vertical position of the hairline stroke.
    hairline_y: f32,
    /// Horizontal extent of the hairline stroke.
    hairline_x: Rangef,
    /// A paper-background probe pixel, far from the card, the hairline, and
    /// any grid line/dot (AC9(a)).
    paper_probe: Pos2,
    /// A hairline probe pixel, on the stroke but off any grid line (AC9(b)).
    hairline_probe: Pos2,
    /// Top-left anchor of the row-1 wordmark text (`Align2::LEFT_TOP`).
    text_row_1: Pos2,
    /// Top-left anchor of the row-2 Cyrillic sample text.
    text_row_2: Pos2,
    /// Top-left anchor of the row-3 mono telemetry sample text.
    text_row_3: Pos2,
}

/// Derive every placeholder position from `rect`. Kept private — the guard
/// lives in this file's own `#[cfg(test)] mod tests`, so there is no
/// `pub`-for-test surface.
///
/// Positions are built as struct/tuple literals from field-wise `f32` sums,
/// not via `Pos2 + Vec2` operator overloads — `clippy::arithmetic_side_effects`
/// (deny) fires on the latter even though it never fires on a raw `f32` `+`
/// (design finding 8; `Pos2`/`Vec2` addition is a user-type operator, not the
/// primitive-float case the finding covers).
fn geometry(rect: Rect) -> PlaceholderGeometry {
    let card_rect = Rect {
        min: Pos2::new(rect.min.x + 112.0, rect.min.y + 16.0),
        max: Pos2::new(rect.min.x + 176.0, rect.min.y + 56.0),
    };
    // A half-integer coordinate so the 1.0-wide hairline stroke covers
    // exactly one pixel row at `pixels_per_point = 1.0`; an integer
    // coordinate would straddle two rows at ~50% each and halve AC9(b)'s
    // darkening margin.
    let hairline_y = rect.min.y + 100.5;
    let hairline_x = Rangef::new(rect.min.x + 16.0, rect.min.x + 176.0);
    // Half-pixel offsets so the probe centres land unambiguously inside one
    // pixel rather than exactly on a pixel boundary.
    let paper_probe = pos2(rect.min.x + 8.5, rect.min.y + 8.5);
    let hairline_probe = pos2(rect.min.x + 88.5, hairline_y);
    // Text row anchors (design § Test Design's sample table) — all three sit
    // below the hairline (`y > 100.5`) and clear both probes above, so every
    // existing guard keeps its meaning.
    let text_row_1 = pos2(rect.min.x + 16.0, rect.min.y + 106.0);
    let text_row_2 = pos2(rect.min.x + 16.0, rect.min.y + 140.0);
    let text_row_3 = pos2(rect.min.x + 16.0, rect.min.y + 168.0);
    PlaceholderGeometry {
        card_rect,
        hairline_y,
        hairline_x,
        paper_probe,
        hairline_probe,
        text_row_1,
        text_row_2,
        text_row_3,
    }
}

/// Draws the scaffold placeholder frame into `rect`.
///
/// Paints the paper background, a graph-paper ruling + dot motif, one
/// crisp-radius card, one hairline stroke, and a three-row font-proof text
/// sample (design `2026-07-17-render-onest-font-swap` — wordmark, Cyrillic +
/// en-dash + digits, mono telemetry with `✓`). **The caller must have
/// installed [`crate::fonts::definitions`] into the drawing [`egui::Context`]
/// first** — every row resolves through a [`FontFamily::Name`], which panics
/// at layout time if unbound (the design's *Load-bearing premise*); this
/// function does not (and, being draw-only per AC13, cannot) install fonts
/// itself.
///
/// `painter` is a borrowed draw context (design *Ownership override*) — this
/// function does not own, construct, or store one. `rect` is explicit rather
/// than derived from `painter.clip_rect()` because the clip rect depends on
/// the painter's provenance (e.g. `egui_kittest`'s harness insets `Ui::painter()`
/// by 8px); an explicit rect makes the output a pure function of `(rect)`.
pub fn draw_placeholder(painter: &Painter, rect: Rect) {
    let geometry = geometry(rect);

    painter.rect_filled(rect, 0, PAPER);
    draw_grid(painter, rect);

    painter.rect_filled(geometry.card_rect, CARD_CORNER_RADIUS, CARD_FILL);
    painter.rect_stroke(
        geometry.card_rect,
        CARD_CORNER_RADIUS,
        Stroke::new(HAIRLINE_STROKE_WIDTH, CARD_STROKE),
        StrokeKind::Inside,
    );

    painter.hline(
        geometry.hairline_x,
        geometry.hairline_y,
        Stroke::new(HAIRLINE_STROKE_WIDTH, HAIRLINE),
    );

    painter.text(
        geometry.text_row_1,
        Align2::LEFT_TOP,
        SAMPLE_ROW_1,
        FontId::new(typography::FS_H2, FontFamily::Name(ONEST_BOLD.into())),
        color::TEXT_INK,
    );
    painter.text(
        geometry.text_row_2,
        Align2::LEFT_TOP,
        SAMPLE_ROW_2,
        FontId::new(typography::FS_H3, FontFamily::Name(ONEST_MEDIUM.into())),
        color::TEXT_BODY,
    );
    painter.text(
        geometry.text_row_3,
        Align2::LEFT_TOP,
        SAMPLE_ROW_3,
        FontId::new(
            typography::FS_SM,
            FontFamily::Name(JETBRAINS_MONO_MEDIUM.into()),
        ),
        color::TEXT_MUTED,
    );
}

/// Positions of grid lines spaced `GRID_SPACING` logical points apart, from
/// `origin` to `origin + extent` inclusive.
///
/// A `for` loop over an integer range, not a `while` over floats:
/// `clippy::while_float` (part of `nursery`, deny) forbids a float
/// loop-termination test, since repeated float `+=` can drift off an exact
/// bound. The single cast pair is bounded and total — any on-screen `rect`
/// extent is comfortably under `u16::MAX` grid steps — and is the only place
/// in this module that needs one.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "extent is a non-negative on-screen rect dimension; the step \
              count is comfortably under u16::MAX for any realistic canvas"
)]
fn grid_lines(origin: f32, extent: f32) -> impl Iterator<Item = f32> {
    let steps = (extent / GRID_SPACING) as u16;
    (0..=steps).map(move |i| f32::from(i).mul_add(GRID_SPACING, origin))
}

/// Draw the graph-paper ruling + dot motif (AC4(c)) across `rect`.
fn draw_grid(painter: &Painter, rect: Rect) {
    let v_range = Rangef::new(rect.min.y, rect.max.y);
    let h_range = Rangef::new(rect.min.x, rect.max.x);

    for x in grid_lines(rect.min.x, rect.width()) {
        painter.vline(x, v_range, Stroke::new(1.0, GRID_LINE));
    }
    for y in grid_lines(rect.min.y, rect.height()) {
        painter.hline(h_range, y, Stroke::new(1.0, GRID_LINE));
    }
    for gx in grid_lines(rect.min.x, rect.width()) {
        for gy in grid_lines(rect.min.y, rect.height()) {
            painter.circle_filled(pos2(gx, gy), GRID_DOT_RADIUS, GRID_DOT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_RECT, draw_placeholder, geometry};
    use egui::epaint::Primitive;

    /// Minimum per-channel darkening (0–255 scale) the hairline probe pixel
    /// must show relative to the paper probe pixel (AC9(b)). `#C4BBAA` vs
    /// `#F5F1E6` is ≈50/channel; 16 is a safe, meaningful margin under
    /// feathering + lavapipe rounding (design *Key decision — three guards,
    /// one render*).
    const HAIRLINE_MIN_DARKENING: u32 = 16;

    /// AC6 — a full pass over the placeholder path yields non-empty
    /// tessellated output, with vertex/index counts strengthened past
    /// "non-empty": a non-empty `Vec<ClippedPrimitive>` can still carry
    /// zero-geometry meshes, which is precisely quartzite's actual defect
    /// (widgets that existed but had zero-size geometry).
    ///
    /// This test owns its `egui::Context` and calls `set_fonts` **before**
    /// the first (and only) `run_ui` pass, so one pass suffices — `run_ui`'s
    /// internal `begin_pass` consumes the deferred `new_font_definitions` and
    /// the fonts are live for that same pass (design D11). `golden_guard`
    /// below has no such window and needs a different mechanism — the two
    /// must not be copied onto each other.
    #[test]
    // Drawing the sample rasterises real glyphs — `epaint`'s glyph cache
    // (`epaint::text::font::FontCell::allocate_glyph_uncached`,
    // `epaint-0.35.0/src/text/font.rs:280`) reaches `vello_cpu`'s
    // `U8Kernel::copy_solid`, whose body is a **checked**
    // `bytemuck::cast_slice_mut::<u8, u32>` over the pixmap buffer. Native
    // malloc over-aligns the buffer so the cast succeeds; Miri's allocator
    // grants only `u8`'s alignment of 1, so the checked cast correctly
    // refuses, panicking with `TargetAlignmentGreaterAndInputNotAligned`.
    // This is a distinct abort site from `golden_guard`'s FFI/`dlopen`
    // ignore below — do not copy that reason here, it would be a false
    // justification for a different failure.
    #[cfg_attr(
        miri,
        ignore = "drawing text rasterises glyphs via vello_cpu, whose checked \
                  u8->u32 pixmap cast panics under Miri's 1-byte allocator \
                  alignment (TargetAlignmentGreaterAndInputNotAligned)"
    )]
    fn tessellation_smoke() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::definitions());
        let input = egui::RawInput {
            screen_rect: Some(CANVAS_RECT),
            ..Default::default()
        };

        let output = ctx.run_ui(input, |ui| {
            draw_placeholder(ui.painter(), CANVAS_RECT);
        });
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);

        assert!(
            !primitives.is_empty(),
            "draw_placeholder produced no tessellated primitives"
        );

        let (vertex_count, index_count) = primitives.iter().fold((0usize, 0usize), |acc, p| {
            let Primitive::Mesh(mesh) = &p.primitive else {
                return acc;
            };
            (acc.0 + mesh.vertices.len(), acc.1 + mesh.indices.len())
        });

        assert!(vertex_count > 0, "tessellated meshes carried zero vertices");
        assert!(index_count > 0, "tessellated meshes carried zero indices");
    }

    /// Read the pixel at `pos` out of a rendered `RgbaImage`.
    ///
    /// `RgbaImage::get_pixel` takes `u32`; `geometry()` returns `Pos2`
    /// (`f32`). `CANVAS_RECT` is 320×192 at `pixels_per_point = 1.0`, and
    /// every probe position returned by `geometry()` lies inside it, so the
    /// truncation below is in-domain and total.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "probe positions come from `geometry()` and always land \
                  inside the 320×192 CANVAS_RECT — truncation is in-domain"
    )]
    fn pixel_at(image: &image::RgbaImage, pos: egui::Pos2) -> [u8; 4] {
        image.get_pixel(pos.x as u32, pos.y as u32).0
    }

    /// AC10(b) + AC9 + AC8/AC10(a) — adapter assertion, the non-triviality
    /// guard, and the exact golden compare, sharing exactly **one**
    /// rasterisation (`Harness::snapshot_options` would call `self.render()`
    /// internally and rasterise a *second*, different image object, so this
    /// calls `render()` once and feeds that same image to both the guard and
    /// `try_image_snapshot_options`).
    ///
    /// **AC14 — this is a structural guard, not a typographic one.**
    /// `egui_kittest` hardcodes dify's anti-aliasing exemption with no
    /// disabling knob, so AA-classified edge pixels (hairline / card stroke /
    /// grid lines / glyph edges — text is almost entirely AA) may differ
    /// silently with no diff artifact; the guarantee is bit-exact in **flat**
    /// regions only, which AC9's paper probe covers. **Caught:** wrong face,
    /// tofu, a mixed-typeface label, missing text, a weight silently
    /// rendering Light — all change glyph shape/mass outside the AA-exempt
    /// edges. **Not caught:** sub-pixel rasterisation drift from a
    /// `skrifa`/`harfrust` bump — a deliberate omission, not a gap, or the
    /// golden would redden on every dependency bump. Dies with the
    /// placeholder at #17.
    #[test]
    // Miri cannot execute foreign functions, and this test drives wgpu, which
    // `dlopen`s the Vulkan ICD via `libloading` (`error: unsupported operation:
    // can't call foreign function `dlopen` on OS `linux``). Without this gate
    // the advisory workspace Miri job aborts here, losing this crate's whole
    // binary and, via cargo's fail-fast, every phase queued behind it (the
    // doc-test phase never started). Other crates' unittest binaries happen to
    // be scheduled earlier, so they survive; that is binary ordering, not a
    // guarantee. **Not** the only Miri abort site in this crate any more:
    // `tessellation_smoke` above now draws the same text sample from a bare
    // `Context` and carries its own, differently-caused, ignore — that one is
    // a checked-cast panic in `vello_cpu`, reached without any FFI at all.
    // Under Miri, gp-render then exercises only the `fonts.rs` + `tokens`
    // tests, not the placeholder path — an accepted coverage loss (design
    // § Risks), not an oversight.
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    fn golden_guard() {
        // `HarnessBuilder::renderer(..)` (used below) bypasses the builder's
        // `render_options` field entirely — its default `PREDICTABLE` value
        // is inert on this path — so `PREDICTABLE` must be passed explicitly
        // here or dithering + hardware texture filtering silently return,
        // making bit-exactness unattainable (design finding 4).
        let render_state = egui_kittest::wgpu::create_render_state(
            egui_kittest::wgpu::default_wgpu_setup(),
            egui_wgpu::RendererOptions::PREDICTABLE,
        );
        assert_eq!(
            render_state.adapter.get_info().device_type,
            egui_wgpu::wgpu::DeviceType::Cpu,
            "resolved wgpu adapter is not a CPU/software device — install a \
             Vulkan software ICD (mesa-vulkan-drivers / lavapipe) to match CI"
        );

        let renderer = egui_kittest::wgpu::WgpuTestRenderer::from_render_state(render_state);

        // Fonts cannot be installed post-build: `Harness::from_builder` runs
        // this closure at *build* time against a `None` ctx
        // (`egui_kittest` `lib.rs:144-146`, `builder.rs:189,239`), so a
        // post-build `set_fonts` line would be unreachable dead code, and
        // `HarnessBuilder` has no font/context hook to pre-seed one from
        // outside. Installing from *inside* the closure and drawing
        // immediately would panic instead — every sample row is a
        // `FontFamily::Name(..)`, unbound in a default `Context`
        // (`FontsImpl::font`, `epaint fonts.rs:1027-1031`), and `set_fonts`
        // is deferred: it only takes effect at the *next* pass's
        // `begin_pass`. So frame 1 installs the fonts and returns
        // **without drawing** — the early `return` below is load-bearing,
        // not dead code, because the panic is a layout-time event and
        // drawing nothing is a complete defence — and frame 2+ draws with
        // every family bound (design § Test Design).
        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_RECT.size())
            .with_pixels_per_point(1.0)
            .with_theme(egui::Theme::Light)
            .renderer(renderer)
            .build_ui(move |ui| {
                if !fonts_installed {
                    ui.ctx().set_fonts(crate::fonts::definitions());
                    fonts_installed = true;
                    return;
                }
                // `ui.painter()` is inset 8px by the harness's central panel
                // margin, and `Painter::with_clip_rect` can only intersect,
                // never widen (design finding 5) — so a full-canvas painter
                // comes from the background layer instead (design finding 6).
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                draw_placeholder(&painter, CANVAS_RECT);
            });

        // Unconditional — `run`/`run_ok`/`try_run` loop on *repaint
        // requests*, which would make this depend on whether `set_fonts`
        // happens to request one; `run_steps` guarantees a text-drawing pass
        // regardless (`from_builder`'s own `run_ok()` may already have run
        // frame 2, in which case this is simply one more identical frame).
        harness.run_steps(1);

        debug_assert_eq!(CANVAS_RECT, harness.ctx.content_rect());

        let image = harness.render().expect("offscreen wgpu render failed");
        let probes = geometry(CANVAS_RECT);

        // AC9 guard — run BEFORE the golden compare so a degenerate frame
        // fails with "the frame is uniform" (points at the drawing code),
        // not "images differ" (points at the golden). Catches a bit-exact
        // golden of an all-black frame, which would otherwise pass forever
        // (quartzite's five CI-enforced goldens were exactly that).
        let paper_pixel = pixel_at(&image, probes.paper_probe);
        assert_eq!(
            paper_pixel,
            [0xF5, 0xF1, 0xE6, 0xFF],
            "paper background probe pixel does not match --paper-1"
        );

        let mut colour_counts: std::collections::HashMap<[u8; 4], usize> =
            std::collections::HashMap::new();
        for pixel in image.pixels() {
            *colour_counts.entry(pixel.0).or_insert(0) += 1;
        }
        let modal_colour = colour_counts
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(colour, _)| *colour)
            .expect("rendered image has no pixels");
        assert_eq!(
            modal_colour, paper_pixel,
            "the paper background is not the image's modal colour — the fill did not draw"
        );

        let hairline_pixel = pixel_at(&image, probes.hairline_probe);
        let paper_luma: u32 =
            u32::from(paper_pixel[0]) + u32::from(paper_pixel[1]) + u32::from(paper_pixel[2]);
        let hairline_luma: u32 = u32::from(hairline_pixel[0])
            + u32::from(hairline_pixel[1])
            + u32::from(hairline_pixel[2]);
        assert!(
            paper_luma.saturating_sub(hairline_luma) >= HAIRLINE_MIN_DARKENING.saturating_mul(3),
            "hairline probe pixel is not measurably darker than the paper probe"
        );

        assert!(
            colour_counts.len() > 1,
            "rendered image has a single distinct colour — quartzite's all-black failure"
        );

        // AC8 + AC10(a) — the golden compare, exact bit-to-bit in flat
        // regions. Both upstream fallbacks are overridden explicitly:
        // `threshold` defaults to 0.6 ("enough for most egui tests to pass
        // across different wgpu backends" — a live trap here), and
        // `failed_pixel_count_threshold` is spelled out rather than relied
        // on at its already-0 default. `0.0` is safe — dify's `Identical`
        // short-circuit runs before any threshold math (design finding 13),
        // so it can never make every pixel fail. Not literal bit-to-bit:
        // `egui_kittest` hardcodes an anti-aliasing exemption with no
        // disabling knob, so AA-classified edge pixels (the hairline / card
        // stroke / grid lines) may differ silently with no diff artifact —
        // the guarantee is bit-exact in flat regions, which AC9's paper
        // probe covers. Premise: llvmpipe output is expected stable on this
        // single Linux/lavapipe CI lane. If it bites, revisit; do not
        // pre-emptively loosen. Both this threshold and the AA exemption are
        // deferred together to the same trigger — a dx12 (Windows) or metal
        // (macOS) lane joining the CI matrix.
        let options = egui_kittest::SnapshotOptions::new()
            .threshold(0.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) = egui_kittest::try_image_snapshot_options(&image, "placeholder", &options)
        {
            panic!("{err}");
        }
    }
}
