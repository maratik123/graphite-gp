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

pub mod app;
#[cfg(test)]
mod app_gallery;
pub mod fonts;
pub mod icons;
pub mod screens;
#[cfg(test)]
mod test_util;
mod text;
pub mod tokens;
pub mod track;
pub mod widgets;

pub use app::{AppShell, Screen, ShellResponse, ShellSession};
pub use screens::{
    Difficulty, LabInput, LabResponse, LabScreen, PhaseStatus, RaceConfig, RaceInput, RaceResponse,
    RaceScreen, RaceSummary, ResultsInput, ResultsResponse, ResultsScreen, StandingEntry,
};
pub use track::{BakedTrackGeometry, CarRender};

/// Optional analytics overlays (design doc §4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// The baked geometry for `track` (design
    /// `2026-07-22-cache-track-geometry`) — the chained/Chaikin-smoothed
    /// wall loops, their outer/hole role split, and each loop's baked
    /// triangulation topology. Must have been built from this same `track`
    /// (the same track↔geometry coupling the region-fill/heatmap
    /// `roles`/`indices` "same call" doc-contract already carries) — a
    /// mismatched pair is a caller bug, not a checked precondition.
    pub geometry: &'a BakedTrackGeometry,
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
/// scene)` (design § *Signature*).
///
/// `scene` bundles the caller-supplied, frame-immutable canvas inputs —
/// `track`, `geometry`, `cars`, `reduced_motion`, `overlays` — into one
/// [`Scene`] value (design `2026-07-22-consolidate-render-inputs`, amended
/// `2026-07-22-cache-track-geometry` to carry `geometry`). `geometry`'s
/// triangulation topology is baked once per track (never rebuilt by this
/// function, regardless of `rect` — AC2/AC5); only the cheap per-frame
/// `O(n)` lattice→screen vertex map runs here.
///
/// Cosmetic wall smoothing (Chaikin) is allowed only within the half-cell
/// gap — it must not cross any point or change the set of drivable cells
/// (the M6 guard, `track::walls::chaikin_smooth`).
pub fn render_frame(painter: &Painter, rect: Rect, scene: Scene<'_>) {
    let Scene {
        track,
        geometry,
        cars,
        reduced_motion,
        overlays,
    } = scene;
    track::draw_frame(
        painter,
        rect,
        track,
        geometry,
        cars,
        reduced_motion,
        overlays,
    );
}
