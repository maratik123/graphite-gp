//! AC8 — the track canvas golden: one wgpu snapshot of a hand-built scene,
//! driven through the public [`crate::render_frame`] entry point (mirrors
//! `widgets::game_gallery`'s FORCED-value gallery). In-crate `#[cfg(test)]`
//! module so it can build a `TrackArtifact` fixture and exercise every layer
//! (regions, walls, S/F, cars — trail, dot, arrow, "you" ring) in one frame.
//!
//! The canvas draws no text (design § Risks), so unlike `placeholder.rs`'s
//! golden this needs no `set_fonts` frame-1-install dance — only the
//! wgpu/`dlopen` Miri abort applies, hence the sole `#[cfg_attr(miri,
//! ignore)]` guard below.

use crate::{CarRender, Overlays};
use egui::{Painter, Pos2, Rect};
use gp_core::geom::{Corridor, Orient, Point, Side, walls_from_boundary};
use gp_core::sim::CarState;
use gp_core::track::{RaceDir, StartFinish, TimingGate, TrackArtifact};

/// The golden's fixed canvas: 320×320 logical points at
/// `pixels_per_point = 1.0` — square, matching the scene's square corridor
/// bbox exactly (no aspect-fit letterboxing to reason about).
const CANVAS_RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(320.0, 320.0),
};

/// Amendment — widened golden fixture (PR #100): a hand-built chunky
/// rounded-rect corridor `TrackArtifact` over a 16×16 bbox — the outer block
/// `x∈[2,13] × y∈[2,13]` minus a centered hole `x∈[6,9] × y∈[6,9]`, a thick
/// loop with 4-cell-wide arms. The S/F chord is a `Vertical` column across
/// the bottom straight (thin in x = racing direction), matching `Track.jsx`'s
/// cross-track checkered bar. Every field `draw_frame` does not read stays at
/// its cheapest valid default.
fn scene_track() -> TrackArtifact {
    let mut corridor = Corridor::new(Point::new(0, 0), 16, 16);
    for x in 2..=13 {
        for y in 2..=13 {
            let in_hole = (6..=9).contains(&x) && (6..=9).contains(&y);
            if !in_hole {
                corridor.set(Point::new(x, y), true);
            }
        }
    }
    let walls = walls_from_boundary(&corridor);
    TrackArtifact {
        walls,
        sf: StartFinish {
            chord: vec![
                Point::new(7, 2),
                Point::new(7, 3),
                Point::new(7, 4),
                Point::new(7, 5),
            ],
            orient: Orient::Vertical,
            gate: TimingGate {
                behind: vec![],
                forward: Side::East,
            },
        },
        corridor,
        race_dir: RaceDir::Cw,
        s_field: gp_core::track::SField::default(),
        start_grid: gp_core::track::StartGrid::default(),
        centerline: gp_core::track::Centerline::default(),
        metrics: gp_core::track::TrackMetrics::default(),
    }
}

/// Draws the full scene into `rect` via the public `render_frame` entry
/// point: the widened corridor plus two cars — a moving, trailed "you" car
/// near the S/F on the bottom straight (mid move-animation, drawing the
/// dashed ring + velocity arrow) and a stationary rival parked on another arm
/// (no arrow, matching `Track.jsx:95`'s guard).
fn draw_scene(painter: &Painter, rect: Rect) {
    let track = scene_track();
    let you_trail = [Point::new(2, 3), Point::new(3, 3)];
    let rival_trail: [Point; 0] = [];
    let cars = [
        CarRender::new(
            CarState {
                x: 4,
                y: 3,
                vx: 2,
                vy: 0,
            },
            0,
            &you_trail,
            true,
            0.5,
        ),
        CarRender::new(
            CarState {
                x: 11,
                y: 7,
                vx: 0,
                vy: 0,
            },
            1,
            &rival_trail,
            false,
            0.0,
        ),
    ];
    crate::render_frame(painter, rect, &track, &cars, false, Overlays::default());
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_RECT, draw_scene};
    use egui::Pos2;

    /// A corner probe pixel, unambiguously outside the corridor bbox's
    /// drivable/infield cells — pure outfield background (AC9's flat-region
    /// guard: runs *before* the golden compare so a degenerate frame fails
    /// on the drawing code, not the golden, mirroring `placeholder.rs`'s
    /// `paper_probe`).
    const OUTFIELD_PROBE: Pos2 = Pos2::new(4.5, 4.5);

    /// Reads the pixel at `pos` out of a rendered `RgbaImage`.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "OUTFIELD_PROBE is a fixed, in-domain point inside CANVAS_RECT — \
                  precedent: placeholder.rs::pixel_at"
    )]
    fn pixel_at(image: &image::RgbaImage, pos: egui::Pos2) -> [u8; 4] {
        image.get_pixel(pos.x as u32, pos.y as u32).0
    }

    /// AC8 — one wgpu frame of the hand-built scene (regions, walls, S/F,
    /// cars) matches the minted golden exactly in flat regions (AA-exempt
    /// edges per `placeholder.rs`/`game_gallery.rs` precedent). Asserts the
    /// adapter is CPU/software (lavapipe), like every sibling golden.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    fn track_canvas_matches_golden() {
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
            .build_ui(move |ui| {
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                draw_scene(&painter, CANVAS_RECT);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        let outfield_pixel = pixel_at(&image, OUTFIELD_PROBE);
        assert_eq!(
            outfield_pixel,
            [0xF5, 0xF1, 0xE6, 0xFF],
            "outfield probe pixel does not match --surface-page/--paper-1 — \
             the scene did not draw"
        );

        let options = egui_kittest::SnapshotOptions::new()
            .threshold(0.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) = egui_kittest::try_image_snapshot_options(&image, "track", &options) {
            panic!("{err}");
        }
    }
}
