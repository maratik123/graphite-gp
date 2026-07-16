//! # gp-render — Block 2: rendering + UX (design doc §4)
//!
//! Reuses the duality: asphalt is *derived* from the corridor `D` (union of unit
//! cells), and walls are the fill boundary on the half-grid — never drawn
//! separately, never crossing a point.
//!
//! Draw backend is `egui` (rendering-only — the window/event loop lives in
//! `gp-game`; see the ownership override in `ai-docs/key-decisions.md`). The
//! backend pick, the rejected alternatives, and the golden-image refresh
//! workflow are documented there rather than here — see
//! `ai-docs/key-decisions.md`.

use egui::Painter;
use gp_core::sim::CarState;
use gp_core::track::TrackArtifact;

pub mod placeholder;

/// Optional analytics overlays (design doc §4).
#[derive(Clone, Copy, Debug, Default)]
pub struct Overlays {
    /// Color the asphalt by `speed_heatmap`.
    pub speed_heatmap: bool,
    /// Draw `fastest_lap` as the ideal line.
    pub fastest_lap: bool,
    /// Draw the "graph-paper" grid + dots.
    pub grid: bool,
}

/// Render one frame (design doc §4), back to front: the three regions
/// (outfield / infield / asphalt), walls, the S/F line, optional analytics
/// overlays, then the cars.
///
/// Takes a **borrowed** `egui::Painter` draw context — this function does
/// not own, construct, or store one; the window/event loop that produces it
/// lives in `gp-game` (see the ownership override in
/// `ai-docs/key-decisions.md`). Still `todo!()`: the block-1 generator
/// (`gp-gen`) that produces a `TrackArtifact` at runtime is itself
/// `todo!()`, so nothing can drive this yet. `crates/render/src/placeholder.rs`
/// ships a separate, non-`TrackArtifact` scaffold (`draw_placeholder`) that
/// exercises the same `Painter` shape in the meantime.
///
/// Cosmetic wall smoothing (Chaikin) is allowed only within the half-cell
/// gap — it must not cross any point or change the set of drivable cells.
pub fn render_frame(
    _painter: &Painter,
    _track: &TrackArtifact,
    _cars: &[CarState],
    _overlays: Overlays,
) {
    todo!("frame rendering (design doc §4)")
}
