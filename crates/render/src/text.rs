//! Shared galley-paint helpers (issue #96, design
//! `2026-07-22-reuse-galley-text-shaping`).
//!
//! `egui-0.35`'s `Painter::text` is a three-line function
//! (`layout_no_wrap` → `Align2::anchor_size` → `Painter::galley`, returning
//! the anchored rect). The two fns below are that body split at the shaping
//! step, so a run built once (via `layout_no_wrap`) can be measured **and**
//! drawn without a second shaping pass — a pure factoring, byte-identical
//! to `Painter::text` (design § *The exact `Painter::text` equivalence*).
//!
//! Crate-root (not `widgets::common`, which is a private module unreachable
//! from `screens`/`app`) so every draw site — widgets, screens, and the app
//! shell — can reach it as `crate::text::paint_galley[_override]`.

use egui::{Align2, Color32, Galley, Painter, Pos2, Rect};
use std::sync::Arc;

/// Anchors `galley` at `pos` (per `anchor`) and draws it via
/// [`Painter::galley`], mirroring `Painter::text`'s fallback-color draw
/// path exactly. `color` must be the same color `galley` was shaped with
/// (`Painter::galley`'s `fallback_color` only fills
/// `Color32::PLACEHOLDER` runs, of which a `layout_no_wrap`-built plain run
/// has none) — static-color call sites.
///
/// Returns the anchored rect, identical to `Painter::text`'s return.
#[allow(
    dead_code,
    reason = "no production call site yet — subtask 1 of 10 (design 2026-07-22-reuse-galley-text-shaping); subtask 2 (telemetry) is the first caller, landing in the same PR"
)]
pub(crate) fn paint_galley(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    galley: Arc<Galley>,
    color: Color32,
) -> Rect {
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

/// Anchors `galley` at `pos` (per `anchor`) and draws it via
/// [`Painter::galley_with_override_text_color`], recoloring every glyph to
/// `color` regardless of the color `galley` was shaped with —
/// dynamic-color call sites (button/switch/segmented-control/nav, whose
/// final draw color is only known after the run must already be shaped for
/// allocation width).
///
/// Returns the anchored rect, identical to `Painter::text`'s return.
#[allow(
    dead_code,
    reason = "no production call site yet — subtask 1 of 10 (design 2026-07-22-reuse-galley-text-shaping); subtask 7 (button) is the first caller, landing in the same PR"
)]
pub(crate) fn paint_galley_override(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    galley: Arc<Galley>,
    color: Color32,
) -> Rect {
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley_with_override_text_color(rect.min, galley, color);
    rect
}

#[cfg(test)]
mod tests {
    use super::{paint_galley, paint_galley_override};
    use egui::{Align2, Color32, FontFamily, FontId, Pos2};

    /// AC1/AC4 regression guard — `paint_galley`'s anchored-rect math must
    /// equal `Painter::text`'s for every anchor egui uses in this crate, on
    /// the same shaped run. This is an exact `Rect` compare, not an image
    /// snapshot (design § Test Design).
    ///
    /// Constructs a real `egui::Context` + `crate::fonts::definitions` to
    /// lay out a real galley — interpreted wall-clock cost under Miri, no
    /// production UB signal (AGENTS.md § Rust Test Conventions, gp-render
    /// Context/painter gate).
    #[cfg_attr(
        miri,
        ignore = "constructs egui::Context to lay out real galleys — interpreted wall-clock cost, no UB signal"
    )]
    #[test]
    fn paint_galley_rect_matches_painter_text_for_every_anchor() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::definitions());
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(200.0, 200.0),
            )),
            ..Default::default()
        };

        let _ = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());

            let text = "SPEED";
            let font = FontId::new(
                16.0,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
            );
            let color = Color32::from_rgb(0x12, 0x34, 0x56);
            let pos = Pos2::new(37.0, 51.0);

            let anchors = [
                Align2::LEFT_TOP,
                Align2::RIGHT_TOP,
                Align2::LEFT_CENTER,
                Align2::CENTER_CENTER,
                Align2::LEFT_BOTTOM,
                Align2::RIGHT_BOTTOM,
            ];

            for anchor in anchors {
                let want = painter.text(pos, anchor, text, font.clone(), color);
                let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
                let got = paint_galley(&painter, pos, anchor, galley, color);
                assert_eq!(
                    got, want,
                    "{anchor:?}: paint_galley rect != Painter::text rect"
                );
            }
        });
    }

    /// Same regression guard for `paint_galley_override` — the override
    /// draw path shares the identical anchor math (only the paint call
    /// differs), so the returned rect must also match `Painter::text`'s.
    #[cfg_attr(
        miri,
        ignore = "constructs egui::Context to lay out real galleys — interpreted wall-clock cost, no UB signal"
    )]
    #[test]
    fn paint_galley_override_rect_matches_painter_text_for_every_anchor() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::definitions());
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(200.0, 200.0),
            )),
            ..Default::default()
        };

        let _ = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());

            let text = "TEMPO";
            let font = FontId::new(
                14.0,
                FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
            );
            let color = Color32::from_rgb(0x98, 0x76, 0x54);
            let pos = Pos2::new(12.0, 8.0);

            let anchors = [
                Align2::LEFT_TOP,
                Align2::RIGHT_TOP,
                Align2::LEFT_CENTER,
                Align2::CENTER_CENTER,
                Align2::LEFT_BOTTOM,
                Align2::RIGHT_BOTTOM,
            ];

            for anchor in anchors {
                let want = painter.text(pos, anchor, text, font.clone(), color);
                let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
                let got = paint_galley_override(&painter, pos, anchor, galley, color);
                assert_eq!(
                    got, want,
                    "{anchor:?}: paint_galley_override rect != Painter::text rect"
                );
            }
        });
    }
}
