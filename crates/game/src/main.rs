//! # gp-game — Block 3b: game loop & orchestration (design doc §3b, §6)
//!
//! The runnable binary. Wires generation (block 1) → the physics core (block 3a)
//! → rendering (block 2), with player and AI controllers driving the same engine.
//! Kept separate from block 3a so training (thousands of headless envs) and live
//! play share one, non-diverging physics implementation.
//!
//! Owns the window + event loop (a deliberate override of issue #11's text in
//! favor of design doc §6 — see `ai-docs/key-decisions.md`); `gp-render`
//! stays draw-only.
//!
//! A thin CLI-parse → dispatch → [`eframe::run_native`] shim (design
//! `2026-07-28-game-loop-orchestration` § *Module decomposition*, A2) —
//! every other concern (`config`, the `eframe::App` glue, the game loop, the
//! generation worker, replay) lives in the `gp_game` lib target and is
//! reached as `gp_game::…`, not through a bin-local module tree.

use eframe::egui;
use gp_game::config;
use std::io::Write;

/// Parses CLI arguments, opens the window and runs the app shell.
///
/// An invalid argument reports the error and exits non-zero **before** the
/// window opens (AC15) — `ConfigError::exit` never returns. On the success
/// path, the resolved configuration is echoed to stdout (AC18) before
/// [`eframe::run_native`] is called.
///
/// # Errors
/// Returns [`eframe::Error`] if the native window/graphics context fails to
/// initialize (e.g. no compatible Vulkan/GL adapter).
fn main() -> eframe::Result {
    let config = match config::parse_from(std::env::args_os()) {
        Ok(config) => config,
        Err(err) => err.exit(),
    };

    // `--replay <PATH> --replay-mode headless` (C4, AC21): never opens the
    // window, and the startup echo below is skipped on this path — there
    // is no fresh race being started to echo the config of.
    if let (Some(path), config::ReplayMode::Headless) = (&config.replay, config.replay_mode) {
        std::process::exit(gp_game::replay::playback::run_headless_replay_from_file(
            path,
        ));
    }

    // `let _ = writeln!`, not `println!` — `println!` panics on a broken
    // pipe, which would be a new production panic path (AC14 in spirit).
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        config::render_startup_echo(&config)
    );

    eframe::run_native(
        "graphite-gp",
        eframe::NativeOptions::default(),
        Box::new(move |creation_context| {
            creation_context
                .egui_ctx
                .set_fonts(gp_render::fonts::definitions());
            // Pin one fixed palette (PR #118 round 3 — the app is
            // custom-drawn entirely from design tokens and has no
            // light/dark-switching feature; without this, egui defaults to
            // its dark `Visuals`, which is what made the body render black).
            creation_context
                .egui_ctx
                .set_visuals(egui::Visuals::light());
            Ok(Box::new(gp_game::app::GraphiteGpApp::new(config)))
        }),
    )
}
