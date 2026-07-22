//! AC8 — the forms-widgets gallery golden: a single wgpu snapshot of the
//! Slider/Switch/`SegmentedControl`/Stepper variant/state matrix, driven
//! directly through each widget's private `paint` layer with FORCED states
//! (no pointer-input simulation — mirrors `gallery.rs`'s `widget_gallery`).
//!
//! A separate golden from `gallery.rs`'s `widget_gallery` (design § Key
//! decision 6): avoids re-minting the merged #13 golden, and tracks
//! `forms.card.html` as its own design-system card.
//!
//! In-crate `#[cfg(test)]` (not `tests/`) so it can reach every widget's
//! crate-visible `paint` fn. `#[cfg_attr(miri, ignore)]` — drives wgpu,
//! which `dlopen`s the Vulkan ICD (no FFI under Miri).

use super::Size;
use super::segmented_control::{SegmentedControl, segment_galleys, segment_widths_from_galleys};
use super::slider::Slider;
use super::stepper::{Stepper, dec_disabled, inc_disabled};
use super::switch::Switch;
use egui::{Painter, Pos2, Rect};

/// The gallery's fixed canvas: 660×540 logical points at
/// `pixels_per_point = 1.0`.
const CANVAS_RECT: Rect = Rect {
    min: Pos2::ZERO,
    max: Pos2::new(660.0, 540.0),
};

/// Draws the Slider rows: a labeled enabled slider mid-track, and a
/// disabled slider (each rendering the thumb's `SHADOW_1` drop shadow).
fn draw_sliders(painter: &Painter, x0: f32, y0: f32) -> f32 {
    const SLIDER_W: f32 = 300.0;
    let style = Slider::resolve();

    let row1 = Rect {
        min: Pos2::new(x0, y0),
        max: Pos2::new(x0 + SLIDER_W, y0 + 70.0),
    };
    Slider::paint(
        painter,
        row1,
        &style,
        0.35,
        Some("TORQUE"),
        Some("0.35"),
        true,
    );

    let row2 = Rect {
        min: Pos2::new(x0, y0 + 80.0),
        max: Pos2::new(x0 + SLIDER_W, y0 + 150.0),
    };
    Slider::paint(
        painter,
        row2,
        &style,
        0.7,
        Some("DAMPING"),
        Some("0.70"),
        false,
    );

    y0 + 170.0
}

/// Draws the Switch row: on, off, and disabled — each with a label.
fn draw_switches(painter: &Painter, x0: f32, y0: f32) -> f32 {
    const CELL_W: f32 = 180.0;
    let states: [(&str, bool, bool); 3] = [
        ("On", true, true),
        ("Off", false, true),
        ("Disabled", true, false),
    ];
    for (col, (label, checked, enabled)) in states.into_iter().enumerate() {
        let x = col_x(x0, CELL_W, col);
        let rect = Rect {
            min: Pos2::new(x, y0),
            max: Pos2::new(x + 160.0, y0 + 22.0),
        };
        let style = Switch::resolve(checked);
        let label_galley = painter.layout_no_wrap(
            label.to_owned(),
            egui::FontId::new(
                crate::tokens::typography::FS_BODY,
                egui::FontFamily::Name(crate::fonts::ONEST_REGULAR.into()),
            ),
            crate::tokens::color::TEXT_BODY,
        );
        Switch::paint(painter, rect, &style, checked, Some(&label_galley), enabled);
    }
    y0 + 50.0
}

/// Draws the `SegmentedControl` rows: sm/md/lg sizes, each with a different
/// selected index. Re-shapes with `segment_galleys` (the same fn `paint`'s
/// caller uses internally) so the chrome rect matches the drawn segments
/// exactly.
fn draw_segmented_controls(painter: &Painter, x0: f32, y0: f32) -> f32 {
    let rows: [(&[&str], usize, Size); 3] = [
        (&["Rookie", "Pro", "Ace"], 0, Size::Sm),
        (&["Easy", "Normal", "Hard"], 1, Size::Md),
        (&["Solo", "Duo", "Team"], 2, Size::Lg),
    ];
    let mut y = y0;
    for (options, selected, size) in rows {
        let galleys = segment_galleys(painter, options, size);
        let total_w: f32 = segment_widths_from_galleys(&galleys).iter().sum();
        let height = SegmentedControl::resolve(false, size).height;
        let rect = Rect {
            min: Pos2::new(x0, y),
            max: Pos2::new(x0 + total_w, y + height),
        };
        SegmentedControl::paint(painter, rect, &galleys, Some(selected), size);
        y += height + 10.0;
    }
    y
}

/// Draws the Stepper row: mid-range, at-min (`−` disabled), at-max (`+`
/// disabled).
fn draw_steppers(painter: &Painter, x0: f32, y0: f32) -> f32 {
    const CELL_W: f32 = 160.0;
    let specs: [(i32, i32, i32, &str); 3] =
        [(50, 0, 99, "VOLUME"), (0, 0, 99, "MIN"), (99, 0, 99, "MAX")];
    for (col, (value, min, max, label)) in specs.into_iter().enumerate() {
        let x = col_x(x0, CELL_W, col);
        let rect = Rect {
            min: Pos2::new(x, y0),
            max: Pos2::new(x + 140.0, y0 + 70.0),
        };
        let style = Stepper::resolve(dec_disabled(value, min), inc_disabled(value, max));
        Stepper::paint(painter, rect, &style, &value.to_string(), Some(label), true);
    }
    y0 + 90.0
}

/// A column x-position at `col`, `cell_w` apart from `x0`.
fn col_x(x0: f32, cell_w: f32, col: usize) -> f32 {
    f32::from(u16::try_from(col).unwrap_or(u16::MAX)).mul_add(cell_w, x0)
}

/// Draws the full forms-widgets gallery matrix into `rect` (currently only
/// ever called with [`CANVAS_RECT`]).
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first (every widget's text draw resolves a
/// `FontFamily::Name(..)`).
fn draw_forms_gallery(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0, crate::tokens::color::SURFACE_PAGE);

    let x0 = rect.min.x + 20.0;
    let mut y = rect.min.y + 20.0;
    y = draw_sliders(painter, x0, y);
    y = draw_switches(painter, x0, y);
    y = draw_segmented_controls(painter, x0, y);
    let _ = draw_steppers(painter, x0, y);
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_RECT, draw_forms_gallery};

    /// AC8 — one wgpu frame renders the full forms-widgets gallery matrix
    /// and matches the minted golden (AA-exempt edges, `threshold(1.0)` +
    /// `failed_pixel_count_threshold(0)`, per `gallery.rs`'s `widget_gallery`
    /// precedent).
    #[cfg_attr(
        miri,
        ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)"
    )]
    #[test]
    fn forms_gallery_matches_golden() {
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

        // Frame-1-install/frame-2-draw (gallery.rs's widget_gallery
        // precedent): fonts can only be installed from inside the harness
        // closure, and `set_fonts` only takes effect at the *next* pass.
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
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                draw_forms_gallery(&painter, CANVAS_RECT);
            });

        harness.run_steps(1);

        let image = harness.render().expect("offscreen wgpu render failed");

        let options = egui_kittest::SnapshotOptions::new()
            .threshold(1.0)
            .failed_pixel_count_threshold(0);
        if let Err(err) =
            egui_kittest::try_image_snapshot_options(&image, "forms_gallery", &options)
        {
            panic!("{err}");
        }
    }
}
