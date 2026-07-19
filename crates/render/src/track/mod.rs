//! The track canvas (design doc §4): regions, walls, S/F, cars.
//!
//! Each layer is a submodule split into a pure lattice-space geometry fn
//! (Miri-clean, no `egui::Ui`, no allocation beyond a returned `Vec`) and a
//! thin `pub(crate) paint` fn that maps that geometry to screen space via
//! [`TrackTransform`] and strokes/fills it — the house pattern this crate's
//! sibling widgets already follow (design § *House pattern*).

#[allow(
    dead_code,
    reason = "layer wired into render_frame at subtask 8 (design decomposition); \
              its paint fn is covered by the subtask-9 golden, not a unit test"
)]
mod car;
#[allow(
    dead_code,
    reason = "layer wired into render_frame at subtask 8 (design decomposition); \
              its paint fn is covered by the subtask-9 golden, not a unit test"
)]
mod regions;
#[allow(
    dead_code,
    reason = "layer wired into render_frame at subtask 8 (design decomposition); \
              its paint fn is covered by the subtask-9 golden, not a unit test"
)]
mod sf;
mod transform;
#[allow(
    dead_code,
    reason = "layer wired into render_frame at subtask 8 (design decomposition); \
              its paint fn is covered by the subtask-9 golden, not a unit test"
)]
mod walls;

pub use car::CarRender;
pub use transform::TrackTransform;
