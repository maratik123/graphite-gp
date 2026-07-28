//! The `gp-game` config error type + its process-exit path (issue #41).
//!
//! Split out of `config/mod.rs` (AGENTS.md's 800-line file-size soft cap) —
//! the natural seam the design names: `clap`'s own diagnostics wrapped
//! transparently, plus the one cross-field invariant this crate validates by
//! hand.

use clap::CommandFactory;
use thiserror::Error;

use super::Cli;

/// A `gp-game` config error — `clap`'s own diagnostics for tokenizing and
/// per-flag ranges, plus the cross-field invariant `block_size ≥
/// ⌈cars/2⌉`.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A `clap` parsing/validation error (unknown flag, missing value,
    /// unparseable value, out-of-range value, unrecognised difficulty).
    #[error(transparent)]
    Cli(#[from] clap::Error),
    /// `--block-size` is below the corridor-width floor `⌈cars/2⌉` implied
    /// by `--cars` (AC5).
    #[error(
        "--block-size {block_size} is below the corridor-width floor \
         ceil(cars/2) = {floor} implied by --cars {cars}"
    )]
    BlockSizeBelowWidthFloor {
        /// The `--cars` value the floor was derived from.
        cars: u32,
        /// The rejected `--block-size` value.
        block_size: i32,
        /// The derived floor, `GenParams::min_width()`.
        floor: u32,
    },
    /// `--replay-mode` was given without `--replay` (spec § Replay CLI,
    /// `AC21d`).
    #[error("--replay-mode requires --replay")]
    ReplayModeWithoutReplay,
    /// `--record` and `--replay` were given together (spec § Replay CLI,
    /// `AC21d`).
    #[error("--record cannot be combined with --replay")]
    RecordWithReplay,
}

impl ConfigError {
    /// Reports the error and exits the process non-zero — never returns.
    /// `clap`-formatted for every variant, including the cross-field one
    /// (via `Cli::command().error(..)`, so its rendering matches `clap`'s
    /// own diagnostics).
    pub fn exit(self) -> ! {
        match self {
            Self::Cli(err) => err.exit(),
            other @ (Self::BlockSizeBelowWidthFloor { .. }
            | Self::ReplayModeWithoutReplay
            | Self::RecordWithReplay) => Cli::command()
                .error(clap::error::ErrorKind::ValueValidation, other.to_string())
                .exit(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{parse, parse_err, rendered};
    use super::*;

    // ---- AC6: clap-sourced error messages name flag, value and domain ----

    #[test]
    fn ac6_cars_error_names_flag_value_and_domain() {
        let text = rendered(&["--cars", "7"]);
        assert!(text.contains("--cars"), "{text}");
        assert!(text.contains('7'), "{text}");
        assert!(text.contains("2..=6"), "{text}");
    }

    #[test]
    fn ac6_difficulty_error_names_accepted_domain() {
        let text = rendered(&["--difficulty", "wizard"]);
        assert!(text.contains("--difficulty"), "{text}");
        assert!(text.contains("Rookie, Pro, Ace"), "{text}");
    }

    // ---- AC5: cross-field invariant ----

    #[test]
    fn ac5_block_size_below_width_floor_is_rejected() {
        match parse_err(&["--cars", "6", "--block-size", "2"]) {
            ConfigError::BlockSizeBelowWidthFloor {
                cars,
                block_size,
                floor,
            } => {
                assert_eq!(cars, 6);
                assert_eq!(block_size, 2);
                assert_eq!(floor, 3);
            }
            other @ (ConfigError::Cli(_)
            | ConfigError::ReplayModeWithoutReplay
            | ConfigError::RecordWithReplay) => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn ac5_block_size_at_width_floor_is_accepted() {
        assert_eq!(parse(&["--cars", "6", "--block-size", "3"]).block_size, 3);
    }

    // ---- AC6: cross-field error message names flags and values ----
    //
    // Fix 1 (self-review round 1, major): the original needles
    // (`contains("--block-size")`, `contains('2')`, `contains('3')`,
    // `contains("--cars")`, `contains('6')`) are all satisfiable by the
    // `#[error(...)]` template's OWN literal text (`"--block-size {..} is
    // below the corridor-width floor ceil(cars/2) = {..} implied by --cars
    // {..}"`) even with every substitution blanked out — the template
    // contains the literal digit `2` (in `cars/2`) and the literal
    // substrings `--block-size` / `--cars`. Replaced with contiguous
    // flag-plus-VALUE needles the template text cannot satisfy on its own:
    // `"--block-size 2"` (the received value, not just the flag),
    // `"= 3"` (the derived floor immediately after its only `=` in the
    // template), and `"--cars 6"` (the received value, not just the flag).
    //
    // Verified this bites: temporarily deleted `{block_size}` from the
    // `#[error(...)]` template above and re-ran `cargo test -p gp-game
    // --lib config::error::tests::ac6_cross_field_error_names_flags_and_values`
    // — it FAILED on the `"--block-size 2"` assertion as expected, then the
    // template was restored and the same test re-confirmed green.
    #[test]
    fn ac6_cross_field_error_names_flags_and_values() {
        let text = rendered(&["--cars", "6", "--block-size", "2"]);
        assert!(text.contains("--block-size 2"), "{text}");
        assert!(text.contains("= 3"), "{text}");
        assert!(text.contains("--cars 6"), "{text}");
    }

    // ---- `AC21d`: the two replay cross-field errors name both flags ----

    #[test]
    fn ac21d_replay_mode_without_replay_names_both_flags() {
        let text = rendered(&["--replay-mode", "headless"]);
        assert!(text.contains("--replay-mode"), "{text}");
        assert!(text.contains("--replay"), "{text}");
    }

    #[test]
    fn ac21d_record_with_replay_names_both_flags() {
        let text = rendered(&["--record", "a", "--replay", "b"]);
        assert!(text.contains("--record"), "{text}");
        assert!(text.contains("--replay"), "{text}");
    }
}
