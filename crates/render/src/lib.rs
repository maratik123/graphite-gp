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
pub mod screens;
#[cfg(test)]
mod test_util;
pub mod tokens;
pub mod track;
pub mod widgets;

pub use screens::{
    Difficulty, LabInput, LabResponse, LabScreen, PhaseStatus, RaceConfig, RaceResponse, RaceScreen,
};
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

/// The frame-immutable canvas inputs consumed by [`render_frame`].
///
/// Bundles track, cars, reduced-motion, and overlays into one cohesive value
/// (design `2026-07-22-consolidate-render-inputs` § *The central decision*).
/// Embedded in [`screens::RaceInput`], the only screen whose canvas
/// re-renders `Scene` on interactive toggles.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// The track fixture drawn this frame.
    pub track: &'a TrackArtifact,
    /// Caller-supplied per-frame render input ([`CarRender`]) — this crate
    /// is draw-only and buffers no car history or clock of its own
    /// (`ai-docs/key-decisions.md`, 2026-07-16).
    pub cars: &'a [CarRender<'a>],
    /// Snaps every car's move animation straight to its final position (no
    /// slide) when `true`.
    pub reduced_motion: bool,
    /// Drives the individually-toggleable analytics/grid layers (design doc
    /// §4 layers 4/5) — each flag adds or removes exactly its own layer's
    /// drawn shapes; the all-off frame is byte-identical to the pre-#18
    /// baseline (design § Draw order).
    pub overlays: Overlays,
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
/// scene)` — the same precedent `draw_placeholder` sets (design §
/// *Signature*).
///
/// `scene` bundles the caller-supplied, frame-immutable canvas inputs —
/// `track`, `cars`, `reduced_motion`, `overlays` — into one [`Scene`] value
/// (design `2026-07-22-consolidate-render-inputs`).
///
/// Cosmetic wall smoothing (Chaikin) is allowed only within the half-cell
/// gap — it must not cross any point or change the set of drivable cells
/// (the M6 guard, `track::walls::chaikin_smooth`).
pub fn render_frame(painter: &Painter, rect: Rect, scene: Scene<'_>) {
    let Scene {
        track,
        cars,
        reduced_motion,
        overlays,
    } = scene;
    track::draw_frame(painter, rect, track, cars, reduced_motion, overlays);
}
