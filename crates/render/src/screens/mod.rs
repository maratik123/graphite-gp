//! Screen-level composition (design `2026-07-20-render-setup-screen` §
//! *Module placement*).
//!
//! Holds the shared config types [`RaceConfig`]/[`Difficulty`] plus the
//! incoming per-screen modules (`Screens.jsx` defines
//! Setup/Race/Lab/Results — [`setup`] is the first to land).
//!
//! `gp-render` is draw-only and has no dependency on `gp-gen`/`gp-ai`
//! (`ai-docs/key-decisions.md`), so these config types live here rather than
//! in a shared crate — a single definition, single consumer today (design §
//! *Config type*).

pub mod lab;
#[cfg(test)]
mod lab_gallery;
pub mod race;
pub mod setup;
#[cfg(test)]
mod setup_gallery;

pub use lab::{LabResponse, LabScreen, PhaseStatus};
pub use race::{RaceResponse, RaceScreen};

/// The pilot difficulty a player picks on the [`setup::SetupScreen`]
/// (`docs/design.md` §5 — the softmax skill dial).
///
/// **Ace = lowest temperature** (strong, smooth pilot), **Rookie = highest**
/// (noisy) — the direction is fixed by the design doc, not a free choice
/// here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    /// Highest temperature (noisiest pilot).
    Rookie,
    /// Mid temperature.
    Pro,
    /// Lowest temperature (strongest, smoothest pilot).
    Ace,
}

/// Placeholder `Rookie` temperature (design § *`Difficulty → temperature`*,
/// tunable — real values are empirical, set once `gp-ai` exists).
const TEMPERATURE_ROOKIE: f32 = 1.5;
/// Placeholder `Pro` temperature.
const TEMPERATURE_PRO: f32 = 1.0;
/// Placeholder `Ace` temperature.
const TEMPERATURE_ACE: f32 = 0.6;

/// The `SegmentedControl` option labels, in [`Difficulty::to_index`] order.
///
/// Kept an explicit const (feeds `SegmentedControl::new` as a `&[&str]`
/// slice) rather than derived from [`Difficulty::label`] at const time; the
/// `DIFFICULTY_LABELS[v.to_index()] == v.label()` drift-guard test (design §
/// *drift guard*) pins the two together instead.
pub const DIFFICULTY_LABELS: [&str; 3] = ["Rookie", "Pro", "Ace"];

impl Difficulty {
    /// The pilot temperature this difficulty maps to (design §
    /// *`Difficulty → temperature`*). FORCED `const fn` —
    /// `clippy::missing_const_for_fn` (nursery = deny) on a pure `match` over
    /// `f32` literals.
    #[must_use]
    pub const fn temperature(self) -> f32 {
        match self {
            Self::Rookie => TEMPERATURE_ROOKIE,
            Self::Pro => TEMPERATURE_PRO,
            Self::Ace => TEMPERATURE_ACE,
        }
    }

    /// This difficulty's display label — always equal to
    /// `DIFFICULTY_LABELS[self.to_index()]` (pinned by the drift-guard test).
    /// FORCED `const fn`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rookie => "Rookie",
            Self::Pro => "Pro",
            Self::Ace => "Ace",
        }
    }

    /// This difficulty's 0-based index into [`DIFFICULTY_LABELS`]. FORCED
    /// `const fn`.
    #[must_use]
    pub const fn to_index(self) -> usize {
        match self {
            Self::Rookie => 0,
            Self::Pro => 1,
            Self::Ace => 2,
        }
    }

    /// The difficulty at 0-based `index`, or `None` for any other index (a
    /// total, non-panicking mapping). FORCED `const fn`.
    #[must_use]
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Rookie),
            1 => Some(Self::Pro),
            2 => Some(Self::Ace),
            _ => None,
        }
    }
}

/// The assembled race configuration emitted by [`setup::SetupScreen`].
///
/// `cars`/`laps` match `gp_gen::GenParams`' integer domain, `v_target` is the
/// design input `V_target` (`docs/design.md` §2 \[D3\], **not**
/// `GenParams.v_ceiling`), and `difficulty` carries the player's choice
/// losslessly (temperature is derived on demand via
/// [`RaceConfig::temperature`]) — design § *Config type*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaceConfig {
    /// Number of cars (matches `gp_gen::GenParams.cars: u32`), bounded
    /// `[2, 6]` by the `SetupScreen` stepper.
    pub cars: u32,
    /// Number of laps, bounded `[1, 9]` by the `SetupScreen` stepper.
    pub laps: u32,
    /// The design-speed input `V_target`, whole cells/turn, bounded `[3, 10]`
    /// by the `SetupScreen` slider.
    pub v_target: i32,
    /// The chosen pilot difficulty.
    pub difficulty: Difficulty,
}

impl RaceConfig {
    /// This config's pilot temperature — delegates to
    /// [`Difficulty::temperature`], the single source of truth for the
    /// mapping. FORCED `const fn`.
    #[must_use]
    pub const fn temperature(self) -> f32 {
        self.difficulty.temperature()
    }
}

#[cfg(test)]
mod tests {
    use super::{DIFFICULTY_LABELS, Difficulty};

    /// AC3 — temperature ordering: Ace (strongest/smoothest) has the lowest
    /// temperature, Rookie (noisiest) the highest.
    #[test]
    fn temperature_ordering_is_ace_lowest_rookie_highest() {
        assert!(Difficulty::Ace.temperature() < Difficulty::Pro.temperature());
        assert!(Difficulty::Pro.temperature() < Difficulty::Rookie.temperature());
    }

    /// AC3 — exact placeholder temperature values (design § tunable
    /// placeholders).
    #[test]
    fn temperature_placeholder_values() {
        crate::test_util::assert_f32("Rookie", Difficulty::Rookie.temperature(), 1.5);
        crate::test_util::assert_f32("Pro", Difficulty::Pro.temperature(), 1.0);
        crate::test_util::assert_f32("Ace", Difficulty::Ace.temperature(), 0.6);
    }

    /// `label`/`to_index`/`from_index` round-trip for every variant.
    #[test]
    fn label_and_index_round_trip() {
        for variant in [Difficulty::Rookie, Difficulty::Pro, Difficulty::Ace] {
            let index = variant.to_index();
            assert_eq!(Difficulty::from_index(index), Some(variant));
        }
    }

    /// `from_index` is total: out-of-range indices return `None`, never a
    /// panic.
    #[test]
    fn from_index_out_of_range_is_none() {
        assert_eq!(Difficulty::from_index(3), None);
        assert_eq!(Difficulty::from_index(usize::MAX), None);
    }

    /// `DIFFICULTY_LABELS` is exactly `["Rookie", "Pro", "Ace"]` — drives the
    /// `SegmentedControl` options and AC3's "exactly Rookie/Pro/Ace".
    #[test]
    fn difficulty_labels_are_exact() {
        assert_eq!(DIFFICULTY_LABELS, ["Rookie", "Pro", "Ace"]);
    }

    /// Drift guard (design-review recommendation): `DIFFICULTY_LABELS` stays
    /// pinned to `Difficulty::label()` for every variant, so the two cannot
    /// silently diverge.
    #[test]
    fn difficulty_labels_match_label_for_every_variant() {
        for variant in [Difficulty::Rookie, Difficulty::Pro, Difficulty::Ace] {
            assert_eq!(DIFFICULTY_LABELS[variant.to_index()], variant.label());
        }
    }
}
