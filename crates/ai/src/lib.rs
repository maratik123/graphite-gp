//! # gp-ai — Block 4: AI training (design doc §5)
//!
//! A small feedforward policy over **honest, locally-perceivable features** (no
//! oracle, no heatmap input): features → 5 logits → legal-move mask (`−inf`) →
//! softmax → sampled action. Bots move through the exact same `legal_move` /
//! crash / collision layers as the player.

use gp_core::sim::{Action, CarState};
use gp_core::track::TrackArtifact;

/// Honest input features for one car (design doc §5): kinematics in the
/// centerline frame, signed lateral distances, speed-scaled look-ahead
/// curvature and free distance, and rivals' *relative* velocity. Everything is a
/// function of the locally perceivable state — nothing global.
#[derive(Clone, Debug, Default)]
pub struct Features {
    /// Flat feature vector (~25–40 values, normalized per design doc §5).
    pub values: Vec<f32>,
}

/// Extract the honest feature vector for `me` given its rivals and the track.
///
/// The key element is **speed-scaled look-ahead** (`~v²/2`): without it the net
/// physically cannot learn to brake for a corner it cannot yet see.
///
/// TODO(4): centerline-frame projection, ray/curvature look-ahead, rival deltas.
pub fn extract_features(_track: &TrackArtifact, _me: CarState, _rivals: &[CarState]) -> Features {
    todo!("feature extraction (design doc §5)")
}

/// Sample an action from the policy: `features → MLP (2×64 … 3×128) → 5 logits`,
/// mask illegal actions to `−inf`, softmax at `temperature`, then sample. Falls
/// back to [`Action::Coast`] if all five are illegal.
///
/// Temperature is the skill dial: low = a strong, smooth pilot; high = a noisy,
/// error-prone one — a spread of difficulty from a single network.
///
/// TODO(4): the MLP forward pass + masked sampling.
pub fn policy_action(_features: &Features, _mask: [bool; 5], _temperature: f32) -> Action {
    todo!("policy forward + masked sampling (design doc §5)")
}
