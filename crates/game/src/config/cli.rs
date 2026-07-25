//! The `gp-game` raw `clap`-derived CLI surface (issue #41).
//!
//! Split out of `config/mod.rs` (AGENTS.md's 800-line file-size soft cap) —
//! the whole `#[derive(Parser)]` struct plus its per-flag bound tests, a
//! natural seam distinct from `GameConfig` assembly, the cross-field error
//! (`error`), and the startup echo (`echo`). [`Cli`] stays `pub(super)` —
//! visible to `config` (for `GameConfig`'s `TryFrom<Cli>`) and every other
//! descendant of `config` (`error`'s `Cli::command()`), never outside it.

use clap::Parser;
use gp_render::screens::{DIFFICULTY_LABELS, Difficulty};

use super::{
    BLOCK_SIZE_MAX, BLOCK_SIZE_MIN, CARS_MAX, CARS_MIN, DEFAULT_BLOCK_SIZE, DEFAULT_CARS,
    DEFAULT_DIFFICULTY_LABEL, DEFAULT_LAPS, DEFAULT_MIN_STRAIGHT, DEFAULT_REPAIR_BUDGET,
    DEFAULT_SEED, DEFAULT_SEED_BUDGET, DEFAULT_V_TARGET, LAPS_MAX, LAPS_MIN, MIN_STRAIGHT_MAX,
    MIN_STRAIGHT_MIN, REPAIR_BUDGET_MAX, REPAIR_BUDGET_MIN, SEED_BUDGET_MAX, SEED_BUDGET_MIN,
    V_TARGET_MAX, V_TARGET_MIN,
};

/// Parses `raw` case-insensitively against [`DIFFICULTY_LABELS`], so
/// `gp-game` restates no difficulty spelling of its own.
fn parse_difficulty(raw: &str) -> Result<Difficulty, String> {
    DIFFICULTY_LABELS
        .iter()
        .position(|label| label.eq_ignore_ascii_case(raw))
        .and_then(Difficulty::from_index)
        .ok_or_else(|| {
            format!(
                "expected one of {} (case-insensitive)",
                DIFFICULTY_LABELS.join(", ")
            )
        })
}

/// The raw `clap`-derived argument struct — `pub(super)` to `config`, never
/// escapes into the mapping logic beyond it.
#[derive(Debug, Parser)]
#[command(name = "graphite-gp", version)]
pub(super) struct Cli {
    /// Number of cars on the grid.
    #[arg(
        long,
        default_value_t = DEFAULT_CARS,
        value_parser = clap::value_parser!(u32).range(i64::from(CARS_MIN)..=i64::from(CARS_MAX)),
    )]
    pub(super) cars: u32,
    /// Number of laps in the race.
    #[arg(
        long,
        default_value_t = DEFAULT_LAPS,
        value_parser = clap::value_parser!(u32).range(i64::from(LAPS_MIN)..=i64::from(LAPS_MAX)),
    )]
    pub(super) laps: u32,
    /// Pilot difficulty, one of Rookie, Pro, or Ace (case-insensitive).
    #[arg(long, default_value = DEFAULT_DIFFICULTY_LABEL, value_parser = parse_difficulty)]
    pub(super) difficulty: Difficulty,
    /// Design speed target, in whole cells per turn.
    #[arg(
        long,
        default_value_t = DEFAULT_V_TARGET,
        value_parser = clap::value_parser!(i32).range(i64::from(V_TARGET_MIN)..=i64::from(V_TARGET_MAX)),
    )]
    pub(super) v_target: i32,
    /// Master seed, expanded into every RNG source unless overridden per
    /// source below.
    #[arg(long, default_value_t = DEFAULT_SEED)]
    pub(super) seed: u64,
    /// Overrides the derived car-collision-resolution seed.
    #[arg(long)]
    pub(super) seed_collision: Option<u64>,
    /// Overrides the derived track-generation seed.
    #[arg(long)]
    pub(super) seed_generation: Option<u64>,
    /// Overrides the derived AI-learning seed.
    #[arg(long)]
    pub(super) seed_ai_learning: Option<u64>,
    /// Overrides the derived AI-inference seed.
    #[arg(long)]
    pub(super) seed_ai_inference: Option<u64>,
    /// Minimum straight run before a corner.
    #[arg(
        long,
        default_value_t = DEFAULT_MIN_STRAIGHT,
        value_parser = clap::value_parser!(i32).range(i64::from(MIN_STRAIGHT_MIN)..=i64::from(MIN_STRAIGHT_MAX)),
    )]
    pub(super) min_straight: i32,
    /// Coarse-block corridor width.
    #[arg(
        long,
        default_value_t = DEFAULT_BLOCK_SIZE,
        value_parser = clap::value_parser!(i32).range(i64::from(BLOCK_SIZE_MIN)..=i64::from(BLOCK_SIZE_MAX)),
    )]
    pub(super) block_size: i32,
    /// Maximum number of seeds to try before giving up on generation.
    #[arg(
        long,
        default_value_t = DEFAULT_SEED_BUDGET,
        value_parser = clap::value_parser!(u32).range(i64::from(SEED_BUDGET_MIN)..=i64::from(SEED_BUDGET_MAX)),
    )]
    pub(super) seed_budget: u32,
    /// Maximum number of local-repair iterations per seed.
    #[arg(
        long,
        default_value_t = DEFAULT_REPAIR_BUDGET,
        value_parser = clap::value_parser!(u32).range(i64::from(REPAIR_BUDGET_MIN)..=i64::from(REPAIR_BUDGET_MAX)),
    )]
    pub(super) repair_budget: u32,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::super::{kind, parse};
    use super::*;

    // ---- AC4: per-flag bounds, player flags ----

    #[test]
    fn ac4_cars_bounds() {
        assert_eq!(parse(&["--cars", "2"]).race.cars, 2);
        assert_eq!(parse(&["--cars", "6"]).race.cars, 6);
        assert_eq!(
            kind(&["--cars", "1"]),
            clap::error::ErrorKind::ValueValidation
        );
        assert_eq!(
            kind(&["--cars", "7"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn ac4_laps_bounds() {
        assert_eq!(parse(&["--laps", "1"]).race.laps, 1);
        assert_eq!(parse(&["--laps", "9"]).race.laps, 9);
        assert_eq!(
            kind(&["--laps", "0"]),
            clap::error::ErrorKind::ValueValidation
        );
        assert_eq!(
            kind(&["--laps", "10"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn ac4_v_target_bounds() {
        assert_eq!(parse(&["--v-target", "3"]).race.v_target, 3);
        assert_eq!(parse(&["--v-target", "10"]).race.v_target, 10);
        assert_eq!(
            kind(&["--v-target", "2"]),
            clap::error::ErrorKind::ValueValidation
        );
        assert_eq!(
            kind(&["--v-target", "11"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn ac4_difficulty_rejects_unknown_spelling() {
        assert_eq!(
            kind(&["--difficulty", "wizard"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    // ---- AC9: drift guard against `SetupScreen`'s bound constants ----

    #[test]
    fn ac9_bound_constants_match_setup_screen() {
        use gp_render::screens::setup::assemble;
        assert_eq!(assemble(i32::MIN, 1, 3.0, Difficulty::Pro).cars, CARS_MIN);
        assert_eq!(assemble(i32::MAX, 1, 3.0, Difficulty::Pro).cars, CARS_MAX);
        assert_eq!(assemble(2, i32::MIN, 3.0, Difficulty::Pro).laps, LAPS_MIN);
        assert_eq!(assemble(2, i32::MAX, 3.0, Difficulty::Pro).laps, LAPS_MAX);
        assert_eq!(
            assemble(2, 1, -1000.0, Difficulty::Pro).v_target,
            V_TARGET_MIN
        );
        assert_eq!(
            assemble(2, 1, 1000.0, Difficulty::Pro).v_target,
            V_TARGET_MAX
        );
    }

    // ---- AC14 (partial, subtask 3): the five rows that need no `--seed` ----

    #[test]
    fn ac14_empty_string_argument_is_unknown() {
        assert_eq!(kind(&[""]), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn ac14_bare_dash_is_unknown() {
        assert_eq!(kind(&["-"]), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn ac14_repeated_flag_is_argument_conflict() {
        assert_eq!(
            kind(&["--cars", "4", "--cars", "5"]),
            clap::error::ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn ac14_stray_after_end_of_flags_is_unknown() {
        assert_eq!(
            kind(&["--", "stray"]),
            clap::error::ErrorKind::UnknownArgument
        );
    }

    #[test]
    fn ac14_bare_end_of_flags_marker_parses_to_defaults() {
        assert_eq!(parse(&["--"]), parse(&[]));
    }

    // ---- AC14 (final, subtask 4): the two rows `--seed` unlocks ----

    #[test]
    fn ac14_seed_overflow_is_value_validation() {
        assert_eq!(
            kind(&["--seed", "18446744073709551616"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn ac14_lone_seed_flag_is_invalid_value() {
        assert_eq!(kind(&["--seed"]), clap::error::ErrorKind::InvalidValue);
    }

    // ---- AC16 (final, subtask 5): all thirteen flags + nine defaults ----

    #[test]
    fn ac16_help_lists_all_thirteen_flags_and_nine_defaults() {
        let help = Cli::command().render_long_help().to_string();
        for flag in [
            "--cars",
            "--laps",
            "--difficulty",
            "--v-target",
            "--seed",
            "--seed-collision",
            "--seed-generation",
            "--seed-ai-learning",
            "--seed-ai-inference",
            "--min-straight",
            "--block-size",
            "--seed-budget",
            "--repair-budget",
        ] {
            assert!(help.contains(flag), "{help}");
        }
        assert_eq!(help.matches("[default: ").count(), 9, "{help}");
    }

    #[test]
    fn ac16_version_reports_crate_version() {
        assert_eq!(
            Cli::command().get_version(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    // ---- AC4: tuning-flag bounds, incl. min_straight 2/64 accept, 1/65 reject ----

    #[test]
    fn ac4_min_straight_bounds() {
        assert_eq!(parse(&["--min-straight", "2"]).min_straight, 2);
        assert_eq!(parse(&["--min-straight", "64"]).min_straight, 64);
        assert_eq!(
            kind(&["--min-straight", "1"]),
            clap::error::ErrorKind::ValueValidation
        );
        assert_eq!(
            kind(&["--min-straight", "65"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn ac4_block_size_bounds() {
        // Paired with --cars 2 so the cross-field floor is 1, keeping this
        // a per-flag boundary test rather than a cross-field one.
        assert_eq!(parse(&["--cars", "2", "--block-size", "1"]).block_size, 1);
        assert_eq!(parse(&["--block-size", "32"]).block_size, 32);
        assert_eq!(
            kind(&["--block-size", "0"]),
            clap::error::ErrorKind::ValueValidation
        );
        assert_eq!(
            kind(&["--block-size", "33"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn ac4_seed_budget_bounds() {
        assert_eq!(parse(&["--seed-budget", "1"]).seed_budget, 1);
        assert_eq!(parse(&["--seed-budget", "1024"]).seed_budget, 1024);
        assert_eq!(
            kind(&["--seed-budget", "0"]),
            clap::error::ErrorKind::ValueValidation
        );
        assert_eq!(
            kind(&["--seed-budget", "1025"]),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn ac4_repair_budget_bounds() {
        assert_eq!(parse(&["--repair-budget", "1"]).repair_budget, 1);
        assert_eq!(parse(&["--repair-budget", "1024"]).repair_budget, 1024);
        assert_eq!(
            kind(&["--repair-budget", "0"]),
            clap::error::ErrorKind::ValueValidation
        );
        assert_eq!(
            kind(&["--repair-budget", "1025"]),
            clap::error::ErrorKind::ValueValidation
        );
    }
}
