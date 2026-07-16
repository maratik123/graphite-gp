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

use crate::tokens::{color, spacing};
use egui::{Color32, Painter, Pos2, Rangef, Rect, Stroke, StrokeKind, pos2};

/// The placeholder's fixed canvas: 192×128 logical points at `pixels_per_point
/// = 1.0`. The single source of truth for the golden's size, consumed by the
/// tessellation test and the golden test — `gp-game`'s window draws into
/// whatever size the OS gives it (`ui.max_rect()`), so this const has no
/// production (non-test) reader; see the `cfg_attr` below.
///
/// Built as a struct literal, not `Rect::from_min_size` — that constructor is
/// not `const fn` on 0.35, and `Rect`'s fields are public.
#[cfg_attr(not(test), allow(dead_code, reason = "test-only canvas fixture"))]
const CANVAS_RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(192.0, 128.0),
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
    PlaceholderGeometry {
        card_rect,
        hairline_y,
        hairline_x,
        paper_probe,
        hairline_probe,
    }
}

/// Draws the scaffold placeholder frame into `rect`.
///
/// Paints the paper background, a graph-paper ruling + dot motif, one
/// crisp-radius card, and one hairline stroke. Draws no text (design
/// *Font-proof amendment*).
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
    #[test]
    fn tessellation_smoke() {
        let ctx = egui::Context::default();
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
    /// (`f32`). `CANVAS_RECT` is 192×128 at `pixels_per_point = 1.0`, and
    /// every probe position returned by `geometry()` lies inside it, so the
    /// truncation below is in-domain and total.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "probe positions come from `geometry()` and always land \
                  inside the 192×128 CANVAS_RECT — truncation is in-domain"
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
    #[test]
    // Miri cannot execute foreign functions, and this test drives wgpu, which
    // `dlopen`s the Vulkan ICD via `libloading` (`error: unsupported operation:
    // can't call foreign function `dlopen` on OS `linux``). Without this gate
    // the advisory workspace Miri job aborts here, losing this crate's whole
    // binary — `tessellation_smoke` included, though it is pure CPU and passes
    // under Miri — and, via cargo's fail-fast, every phase queued behind it
    // (the doc-test phase never started). Other crates' unittest binaries happen
    // to be scheduled earlier, so they survive; that is binary ordering, not a
    // guarantee.
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

        let mut harness = egui_kittest::Harness::builder()
            .with_size(CANVAS_RECT.size())
            .with_pixels_per_point(1.0)
            .with_theme(egui::Theme::Light)
            .renderer(renderer)
            .build_ui(|ui| {
                // `ui.painter()` is inset 8px by the harness's central panel
                // margin, and `Painter::with_clip_rect` can only intersect,
                // never widen (design finding 5) — so a full-canvas painter
                // comes from the background layer instead (design finding 6).
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                draw_placeholder(&painter, CANVAS_RECT);
            });

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
