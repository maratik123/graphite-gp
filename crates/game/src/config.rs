//! CLI argument parsing and validated game configuration (issue #41).
//!
//! Owns the whole CLI surface: the [`clap`]-derived raw [`Cli`] struct, the
//! bound/default constants, the seed resolution, the validated [`GameConfig`],
//! the [`gp_gen::GenParams`]/temperature mapping, and the startup-echo
//! formatter ([`render_startup_echo`]). [`Cli`] stays private to this module —
//! it never escapes into the mapping logic.

use clap::Parser;
use gp_core::rng::Seeds;
use gp_render::screens::{DIFFICULTY_LABELS, Difficulty, RaceConfig};
use thiserror::Error;

/// Minimum accepted `--cars` (mirrors `SetupScreen`, AC9).
const CARS_MIN: u32 = 2;
/// Maximum accepted `--cars` (mirrors `SetupScreen`, AC9).
const CARS_MAX: u32 = 6;
/// Minimum accepted `--laps` (mirrors `SetupScreen`, AC9).
const LAPS_MIN: u32 = 1;
/// Maximum accepted `--laps` (mirrors `SetupScreen`, AC9).
const LAPS_MAX: u32 = 9;
/// Minimum accepted `--v-target` (mirrors `SetupScreen`'s `f32` slider bound,
/// AC9).
const V_TARGET_MIN: i32 = 3;
/// Maximum accepted `--v-target` (mirrors `SetupScreen`'s `f32` slider bound,
/// AC9).
const V_TARGET_MAX: i32 = 10;
/// Default `--cars`, mirroring the deleted `main.rs` `STARTUP_CONFIG`.
const DEFAULT_CARS: u32 = 4;
/// Default `--laps`, mirroring the deleted `main.rs` `STARTUP_CONFIG`.
const DEFAULT_LAPS: u32 = 5;
/// Default `--v-target`, mirroring the deleted `main.rs` `STARTUP_CONFIG`.
const DEFAULT_V_TARGET: i32 = 7;
/// Default `--difficulty` spelling, routed through [`parse_difficulty`].
const DEFAULT_DIFFICULTY_LABEL: &str = "Pro";
/// Decimal places the startup echo renders the pilot temperature at.
const TEMPERATURE_DECIMALS: usize = 2;
/// Default `--seed` master, continuity with the deleted `main.rs`
/// `FIXTURE_SEED` (the value the `LabScreen` header already shows).
const DEFAULT_SEED: u64 = 7;

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

/// The raw `clap`-derived argument struct — private to this module, never
/// escapes into the mapping logic.
#[derive(Debug, Parser)]
#[command(name = "graphite-gp", version)]
struct Cli {
    /// Number of cars on the grid.
    #[arg(
        long,
        default_value_t = DEFAULT_CARS,
        value_parser = clap::value_parser!(u32).range(i64::from(CARS_MIN)..=i64::from(CARS_MAX)),
    )]
    cars: u32,
    /// Number of laps in the race.
    #[arg(
        long,
        default_value_t = DEFAULT_LAPS,
        value_parser = clap::value_parser!(u32).range(i64::from(LAPS_MIN)..=i64::from(LAPS_MAX)),
    )]
    laps: u32,
    /// Pilot difficulty, one of Rookie, Pro, or Ace (case-insensitive).
    #[arg(long, default_value = DEFAULT_DIFFICULTY_LABEL, value_parser = parse_difficulty)]
    difficulty: Difficulty,
    /// Design speed target, in whole cells per turn.
    #[arg(
        long,
        default_value_t = DEFAULT_V_TARGET,
        value_parser = clap::value_parser!(i32).range(i64::from(V_TARGET_MIN)..=i64::from(V_TARGET_MAX)),
    )]
    v_target: i32,
    /// Master seed, expanded into every RNG source unless overridden per
    /// source below.
    #[arg(long, default_value_t = DEFAULT_SEED)]
    seed: u64,
    /// Overrides the derived car-collision-resolution seed.
    #[arg(long)]
    seed_collision: Option<u64>,
    /// Overrides the derived track-generation seed.
    #[arg(long)]
    seed_generation: Option<u64>,
    /// Overrides the derived AI-learning seed.
    #[arg(long)]
    seed_ai_learning: Option<u64>,
    /// Overrides the derived AI-inference seed.
    #[arg(long)]
    seed_ai_inference: Option<u64>,
}

/// The validated game configuration assembled from [`Cli`] (issue #41).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameConfig {
    /// The player-facing race configuration, reused from `gp-render` — the
    /// same type `AppShell::new` and the Setup screen already speak.
    pub(crate) race: RaceConfig,
    /// The resolved per-source seeds — the master expanded via
    /// `Seeds::from_master`, with any supplied per-source override applied.
    pub(crate) seeds: Seeds,
}

/// A `gp-game` config error — `clap`'s own diagnostics for tokenizing and
/// per-flag ranges, plus (from subtask 5 on) the cross-field invariant.
#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    /// A `clap` parsing/validation error (unknown flag, missing value,
    /// unparseable value, out-of-range value, unrecognised difficulty).
    #[error(transparent)]
    Cli(#[from] clap::Error),
}

impl ConfigError {
    /// Reports the error and exits the process non-zero — never returns.
    /// `clap`-formatted for every variant reachable so far.
    pub(crate) fn exit(self) -> ! {
        match self {
            Self::Cli(err) => err.exit(),
        }
    }
}

impl TryFrom<Cli> for GameConfig {
    type Error = ConfigError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        // Seed resolution (normative, per spec): four independent
        // `Option::unwrap_or` picks, one per field — never a panic path.
        let derived = Seeds::from_master(cli.seed);
        let seeds = Seeds {
            collision: cli.seed_collision.unwrap_or(derived.collision),
            generation: cli.seed_generation.unwrap_or(derived.generation),
            ai_learning: cli.seed_ai_learning.unwrap_or(derived.ai_learning),
            ai_inference: cli.seed_ai_inference.unwrap_or(derived.ai_inference),
        };

        Ok(Self {
            race: RaceConfig {
                cars: cli.cars,
                laps: cli.laps,
                v_target: cli.v_target,
                difficulty: cli.difficulty,
            },
            seeds,
        })
    }
}

impl GameConfig {
    /// This config's pilot temperature — delegates to
    /// [`RaceConfig::temperature`], the single source of truth for the
    /// mapping (AC10). FORCED `const fn` — a pure delegation over `Copy`
    /// fields (`clippy::missing_const_for_fn`, nursery = deny).
    pub(crate) const fn temperature(self) -> f32 {
        self.race.temperature()
    }

    /// The race's lap count as an `i32`, for `ShellSession::total_laps`. A
    /// *total* (non-panicking) conversion — `self.race.laps` is validated
    /// into `[1, 9]`, so the `i32::MAX` sentinel is unreachable in practice,
    /// but the form stays total rather than relying on that invariant.
    pub(crate) fn total_laps(self) -> i32 {
        i32::try_from(self.race.laps).unwrap_or(i32::MAX)
    }
}

/// Parses `args` (never `std::env::args` internally — the iterator is the
/// caller's responsibility) into a validated [`GameConfig`] (AC1).
pub(crate) fn parse_from<I, T>(args: I) -> Result<GameConfig, ConfigError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(args)?;
    GameConfig::try_from(cli)
}

/// Renders the resolved configuration for the startup echo (AC18) — a pure
/// formatter, no I/O, so it is testable without a process or a window.
pub(crate) fn render_startup_echo(config: &GameConfig) -> String {
    let player_line = format!(
        "graphite-gp: cars {cars}, laps {laps}, V_target {v_target}, difficulty {difficulty} (temperature {temp:.prec$})",
        cars = config.race.cars,
        laps = config.race.laps,
        v_target = config.race.v_target,
        difficulty = config.race.difficulty.label(),
        temp = config.temperature(),
        prec = TEMPERATURE_DECIMALS,
    );
    // v2 (subtask 4): a standalone `Seeds` line — `Debug` cannot silently
    // omit a field. Subtask 5 replaces this with the full `GenParams`
    // `Debug` line, which nests the same `Seeds` fields.
    format!("{player_line}\ngraphite-gp: {:?}", config.seeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses `args`, prepending the program name (AC1's iterator contract —
    /// every test drives `parse_from` with an explicit `&[&str]`, never
    /// `std::env::args`).
    fn parse(args: &[&str]) -> GameConfig {
        let mut full = vec!["graphite-gp"];
        full.extend_from_slice(args);
        parse_from(full).expect("expected Ok")
    }

    /// The `Err` counterpart of [`parse`].
    fn parse_err(args: &[&str]) -> ConfigError {
        let mut full = vec!["graphite-gp"];
        full.extend_from_slice(args);
        parse_from(full).expect_err("expected Err")
    }

    /// Unwraps the `clap::error::ErrorKind` of a `ConfigError::Cli`.
    fn kind(args: &[&str]) -> clap::error::ErrorKind {
        match parse_err(args) {
            ConfigError::Cli(err) => err.kind(),
        }
    }

    /// The rendered `Display` form of `parse_err(args)`, for AC6 substring
    /// assertions.
    fn rendered(args: &[&str]) -> String {
        parse_err(args).to_string()
    }

    // ---- AC13: defaults (player + seed) ----

    #[test]
    fn ac13_defaults_match_documented_literals() {
        assert_eq!(DEFAULT_CARS, 4);
        assert_eq!(DEFAULT_LAPS, 5);
        assert_eq!(DEFAULT_V_TARGET, 7);
        assert_eq!(DEFAULT_DIFFICULTY_LABEL, "Pro");
        assert_eq!(DEFAULT_SEED, 7);
        let config = parse(&[]);
        assert_eq!(config.race.cars, DEFAULT_CARS);
        assert_eq!(config.race.laps, DEFAULT_LAPS);
        assert_eq!(config.race.v_target, DEFAULT_V_TARGET);
        assert_eq!(config.race.difficulty, Difficulty::Pro);
    }

    // ---- AC2 (seeds half): empty args derive Seeds from DEFAULT_SEED ----

    #[test]
    fn ac2_default_seeds_equal_derivation_from_default_seed() {
        assert_eq!(parse(&[]).seeds, Seeds::from_master(DEFAULT_SEED));
    }

    // ---- AC3: every player + seed flag round-trips ----

    #[test]
    fn ac3_player_flags_round_trip() {
        let config = parse(&[
            "--cars",
            "6",
            "--laps",
            "9",
            "--difficulty",
            "ace",
            "--v-target",
            "10",
        ]);
        assert_eq!(config.race.cars, 6);
        assert_eq!(config.race.laps, 9);
        assert_eq!(config.race.difficulty, Difficulty::Ace);
        assert_eq!(config.race.v_target, 10);
    }

    #[test]
    fn ac3_seed_override_flags_round_trip() {
        let config = parse(&[
            "--seed-collision",
            "1",
            "--seed-generation",
            "2",
            "--seed-ai-learning",
            "3",
            "--seed-ai-inference",
            "4",
        ]);
        assert_eq!(config.seeds.collision, 1);
        assert_eq!(config.seeds.generation, 2);
        assert_eq!(config.seeds.ai_learning, 3);
        assert_eq!(config.seeds.ai_inference, 4);
    }

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

    // ---- AC10: temperature delegates exactly, bit-identical ----

    #[test]
    fn ac10_temperature_delegates_exactly() {
        for (label, expected) in DIFFICULTY_LABELS.iter().zip(
            [Difficulty::Rookie, Difficulty::Pro, Difficulty::Ace]
                .iter()
                .copied(),
        ) {
            let config = parse(&["--difficulty", label]);
            assert_eq!(
                config.temperature().to_bits(),
                expected.temperature().to_bits()
            );
        }
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

    // ---- AC11 (parse level): master -> Seeds derivation, at parse_from ----

    #[test]
    fn ac11_same_master_two_parses_yield_identical_seeds() {
        assert_eq!(
            parse(&["--seed", "123"]).seeds,
            parse(&["--seed", "123"]).seeds
        );
    }

    #[test]
    fn ac11_distinct_masters_yield_pairwise_distinct_fields() {
        let a = parse(&["--seed", "1"]).seeds;
        let b = parse(&["--seed", "2"]).seeds;
        let a_fields = [a.collision, a.generation, a.ai_learning, a.ai_inference];
        let b_fields = [b.collision, b.generation, b.ai_learning, b.ai_inference];
        for (x, y) in a_fields.iter().zip(b_fields.iter()) {
            assert_ne!(x, y);
        }
    }

    // ---- AC12: override precedence ----

    #[test]
    fn ac12_collision_override_leaves_others_derived() {
        let derived = Seeds::from_master(5);
        let config = parse(&["--seed", "5", "--seed-collision", "999"]);
        assert_eq!(config.seeds.collision, 999);
        assert_eq!(config.seeds.generation, derived.generation);
        assert_eq!(config.seeds.ai_learning, derived.ai_learning);
        assert_eq!(config.seeds.ai_inference, derived.ai_inference);
    }

    #[test]
    fn ac12_generation_override_leaves_others_derived() {
        let derived = Seeds::from_master(5);
        let config = parse(&["--seed", "5", "--seed-generation", "999"]);
        assert_eq!(config.seeds.collision, derived.collision);
        assert_eq!(config.seeds.generation, 999);
        assert_eq!(config.seeds.ai_learning, derived.ai_learning);
        assert_eq!(config.seeds.ai_inference, derived.ai_inference);
    }

    #[test]
    fn ac12_ai_learning_override_leaves_others_derived() {
        let derived = Seeds::from_master(5);
        let config = parse(&["--seed", "5", "--seed-ai-learning", "999"]);
        assert_eq!(config.seeds.collision, derived.collision);
        assert_eq!(config.seeds.generation, derived.generation);
        assert_eq!(config.seeds.ai_learning, 999);
        assert_eq!(config.seeds.ai_inference, derived.ai_inference);
    }

    #[test]
    fn ac12_ai_inference_override_leaves_others_derived() {
        let derived = Seeds::from_master(5);
        let config = parse(&["--seed", "5", "--seed-ai-inference", "999"]);
        assert_eq!(config.seeds.collision, derived.collision);
        assert_eq!(config.seeds.generation, derived.generation);
        assert_eq!(config.seeds.ai_learning, derived.ai_learning);
        assert_eq!(config.seeds.ai_inference, 999);
    }

    #[test]
    fn ac12_all_four_overrides_make_seeds_independent_of_master() {
        let overrides = &[
            "--seed-collision",
            "10",
            "--seed-generation",
            "20",
            "--seed-ai-learning",
            "30",
            "--seed-ai-inference",
            "40",
        ];
        let mut full_a = vec!["--seed", "1"];
        full_a.extend_from_slice(overrides);
        let mut full_b = vec!["--seed", "2"];
        full_b.extend_from_slice(overrides);
        assert_eq!(parse(&full_a).seeds, parse(&full_b).seeds);
    }

    #[test]
    fn ac12_override_without_seed_still_derives_others_from_default() {
        let derived = Seeds::from_master(DEFAULT_SEED);
        let config = parse(&["--seed-collision", "999"]);
        assert_eq!(config.seeds.collision, 999);
        assert_eq!(config.seeds.generation, derived.generation);
        assert_eq!(config.seeds.ai_learning, derived.ai_learning);
        assert_eq!(config.seeds.ai_inference, derived.ai_inference);
    }

    // ---- AC16 (partial, subtask 3): the four player flags + version ----

    #[test]
    fn ac16_partial_help_and_version() {
        use clap::CommandFactory;
        let help = Cli::command().render_long_help().to_string();
        for flag in ["--cars", "--laps", "--difficulty", "--v-target"] {
            assert!(help.contains(flag), "{help}");
        }
        assert_eq!(
            Cli::command().get_version(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
