//! # gp-game — Block 3b: game loop & orchestration (design doc §3b, §6)
//!
//! The runnable binary. Wires generation (block 1) → the physics core (block 3a)
//! → rendering (block 2), with player and AI controllers driving the same engine.
//! Kept separate from block 3a so training (thousands of headless envs) and live
//! play share one, non-diverging physics implementation.
//!
//! Owns the window + event loop (a deliberate override of issue #11's text in
//! favour of design doc §6 — see `ai-docs/key-decisions.md`); `gp-render`
//! stays draw-only.

use eframe::egui;

/// The app shell. Currently draws only the `gp-render` scaffold placeholder —
/// input, timing, and orchestration are block 3b's own future work.
struct GraphiteGpApp;

impl eframe::App for GraphiteGpApp {
    // Not `update` — `eframe` 0.35's `App` trait has no such method; `ui` is
    // the required call, `logic` is the optional pre-paint hook (unused here).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        gp_render::placeholder::draw_placeholder(ui.painter(), ui.max_rect());
    }
}

/// Open the window and run the app shell.
///
/// # Errors
/// Returns [`eframe::Error`] if the native window/graphics context fails to
/// initialise (e.g. no compatible Vulkan/GL adapter).
fn main() -> eframe::Result {
    eframe::run_native(
        "graphite-gp",
        eframe::NativeOptions::default(),
        Box::new(|creation_context| {
            creation_context
                .egui_ctx
                .set_fonts(gp_render::fonts::definitions());
            Ok(Box::new(GraphiteGpApp))
        }),
    )
}
