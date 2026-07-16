//! # gp-render — Block 2: rendering + UX (design doc §4)
//!
//! Reuses the duality: asphalt is *derived* from the corridor `D` (union of unit
//! cells), and walls are the fill boundary on the half-grid — never drawn
//! separately, never crossing a point.

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
/// TODO(2): choose a backend and draw. Cosmetic wall smoothing (Chaikin) is
/// allowed only within the half-cell gap — it must not cross any point or change
/// the set of drivable cells.
pub fn render_frame(_track: &TrackArtifact, _cars: &[CarState], _overlays: Overlays) {
    todo!("frame rendering (design doc §4)")
}
