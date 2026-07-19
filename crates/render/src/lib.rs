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

use egui::{Painter, Rect};
use gp_core::track::TrackArtifact;

pub mod fonts;
pub mod icons;
pub mod placeholder;
pub mod tokens;
pub mod track;
pub mod widgets;

pub use track::CarRender;

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

/// Renders one frame of the track canvas (design doc §4) into `rect`.
///
/// Draws back to front: the three regions (outfield / asphalt / infield),
/// the walls (Chaikin-smoothed, M6-guarded), the checkered S/F chord, then
/// every car (`track::LAYER_ORDER` pins the exact order — AC9).
///
/// Takes a **borrowed** `egui::Painter` draw context — this function does
/// not own, construct, or store one; the window/event loop that produces it
/// lives in `gp-game` (see the ownership override in
/// `ai-docs/key-decisions.md`). `rect` is explicit (not derived from
/// `painter.clip_rect()`) so the drawn output is a pure function of `(rect,
/// track, cars, reduced_motion)` — the same precedent `draw_placeholder`
/// sets (design § *Signature*).
///
/// `cars` is caller-supplied per-frame render input
/// ([`CarRender`]) — this crate is draw-only and buffers no car history or
/// clock of its own (`ai-docs/key-decisions.md`, 2026-07-16).
/// `reduced_motion` snaps every car's move animation straight to its final
/// position (no slide). `overlays` is threaded but inert: layers 4 (grid)
/// and 5 (analytics) are deferred (design § *Rejected alternatives* / Q2),
/// so no flag in `overlays` changes anything this function draws yet.
///
/// Cosmetic wall smoothing (Chaikin) is allowed only within the half-cell
/// gap — it must not cross any point or change the set of drivable cells
/// (the M6 guard, `track::walls::chaikin_smooth`).
pub fn render_frame(
    painter: &Painter,
    rect: Rect,
    track: &TrackArtifact,
    cars: &[track::CarRender<'_>],
    reduced_motion: bool,
    overlays: Overlays,
) {
    track::draw_frame(painter, rect, track, cars, reduced_motion, overlays);
}
