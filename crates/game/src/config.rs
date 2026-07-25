//! CLI argument parsing and validated game configuration (issue #41).
//!
//! Owns the whole CLI surface: the [`clap`]-derived raw [`Cli`] struct, the
//! bound/default constants, the seed resolution, the validated [`GameConfig`],
//! the [`gp_gen::GenParams`]/temperature mapping, and the startup-echo
//! formatter ([`render_startup_echo`]). [`Cli`] stays private to this module —
//! it never escapes into the mapping logic.

use clap::{CommandFactory, Parser};
use gp_core::rng::Seeds;
use gp_gen::GenParams;
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
/// Minimum accepted `--min-straight` — `gp-gen`'s `l_min` domain floor.
/// Values below this are silently clamped up by the generator
/// (`clamp_l_min`), so the CLI rejects them instead of advertising a value
/// the generator would quietly rewrite.
const MIN_STRAIGHT_MIN: i32 = 2;
/// Maximum accepted `--min-straight` — `gp-gen`'s `l_min` domain ceiling.
const MIN_STRAIGHT_MAX: i32 = 64;
/// Minimum accepted `--block-size`. The *real* floor is the cross-field
/// invariant `block_size ≥ ⌈cars/2⌉`; this per-flag floor only excludes
/// non-positive values.
const BLOCK_SIZE_MIN: i32 = 1;
/// Maximum accepted `--block-size` — an allocation/typo guard (Ф2 allocates
/// a corridor sized roughly `(coarse bbox) × block_size`), not a
/// performance promise.
const BLOCK_SIZE_MAX: i32 = 32;
/// Minimum accepted `--seed-budget` / `--repair-budget`. `0` is accepted by
/// `GenParams` but makes generation fail immediately — a CLI must not hand
/// the user that footgun.
const SEED_BUDGET_MIN: u32 = 1;
/// Maximum accepted `--seed-budget` — 16× the heaviest measured
/// configuration in this repo, an allocation/typo guard rather than a
/// wall-clock promise.
const SEED_BUDGET_MAX: u32 = 1024;
/// Minimum accepted `--repair-budget`. See [`SEED_BUDGET_MIN`].
const REPAIR_BUDGET_MIN: u32 = 1;
/// Maximum accepted `--repair-budget`. See [`SEED_BUDGET_MAX`].
const REPAIR_BUDGET_MAX: u32 = 1024;
/// Default `--min-straight` — `gp-gen`'s own proven pair.
const DEFAULT_MIN_STRAIGHT: i32 = 3;
/// Default `--block-size` — `gp-gen`'s own proven pair.
const DEFAULT_BLOCK_SIZE: i32 = 6;
/// Default `--seed-budget` — `gp-gen`'s cheap always-on e2e case.
const DEFAULT_SEED_BUDGET: u32 = 1;
/// Default `--repair-budget` — `gp-gen`'s cheap always-on e2e case.
const DEFAULT_REPAIR_BUDGET: u32 = 8;

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
    /// Minimum straight run before a corner.
    #[arg(
        long,
        default_value_t = DEFAULT_MIN_STRAIGHT,
        value_parser = clap::value_parser!(i32).range(i64::from(MIN_STRAIGHT_MIN)..=i64::from(MIN_STRAIGHT_MAX)),
    )]
    min_straight: i32,
    /// Coarse-block corridor width.
    #[arg(
        long,
        default_value_t = DEFAULT_BLOCK_SIZE,
        value_parser = clap::value_parser!(i32).range(i64::from(BLOCK_SIZE_MIN)..=i64::from(BLOCK_SIZE_MAX)),
    )]
    block_size: i32,
    /// Maximum number of seeds to try before giving up on generation.
    #[arg(
        long,
        default_value_t = DEFAULT_SEED_BUDGET,
        value_parser = clap::value_parser!(u32).range(i64::from(SEED_BUDGET_MIN)..=i64::from(SEED_BUDGET_MAX)),
    )]
    seed_budget: u32,
    /// Maximum number of local-repair iterations per seed.
    #[arg(
        long,
        default_value_t = DEFAULT_REPAIR_BUDGET,
        value_parser = clap::value_parser!(u32).range(i64::from(REPAIR_BUDGET_MIN)..=i64::from(REPAIR_BUDGET_MAX)),
    )]
    repair_budget: u32,
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
    /// `L_min` — minimum straight length before a corner.
    pub(crate) min_straight: i32,
    /// `k` — coarse-block size / nominal corridor width.
    pub(crate) block_size: i32,
    /// Outer-loop seed budget.
    pub(crate) seed_budget: u32,
    /// Inner-loop repair budget.
    pub(crate) repair_budget: u32,
}

/// A `gp-game` config error — `clap`'s own diagnostics for tokenizing and
/// per-flag ranges, plus the cross-field invariant `block_size ≥
/// ⌈cars/2⌉`.
#[derive(Debug, Error)]
pub(crate) enum ConfigError {
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
}

impl ConfigError {
    /// Reports the error and exits the process non-zero — never returns.
    /// `clap`-formatted for every variant, including the cross-field one
    /// (via `Cli::command().error(..)`, so its rendering matches `clap`'s
    /// own diagnostics).
    pub(crate) fn exit(self) -> ! {
        match self {
            Self::Cli(err) => err.exit(),
            other @ Self::BlockSizeBelowWidthFloor { .. } => Cli::command()
                .error(clap::error::ErrorKind::ValueValidation, other.to_string())
                .exit(),
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

        // Build-then-validate (AC8 holds by construction): assemble the
        // config first, then read the corridor-width floor off
        // `to_gen_params().min_width()` — the object validated is the
        // object handed to `gp-gen`, never a hand-rolled `⌈cars/2⌉`.
        let config = Self {
            race: RaceConfig {
                cars: cli.cars,
                laps: cli.laps,
                v_target: cli.v_target,
                difficulty: cli.difficulty,
            },
            seeds,
            min_straight: cli.min_straight,
            block_size: cli.block_size,
            seed_budget: cli.seed_budget,
            repair_budget: cli.repair_budget,
        };

        let floor = config.to_gen_params().min_width();
        // `config.block_size` is validated into `[1, 32]` by `clap`, so this
        // total conversion always succeeds; a hypothetical failure compares
        // as `0 < floor`, which rejects rather than silently accepts.
        let block_size_u32 = u32::try_from(config.block_size).unwrap_or(0);
        if block_size_u32 < floor {
            return Err(ConfigError::BlockSizeBelowWidthFloor {
                cars: config.race.cars,
                block_size: config.block_size,
                floor,
            });
        }

        Ok(config)
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

    /// Maps this config onto a [`GenParams`], mapping `v_target` onto
    /// `v_ceiling` (AC7). FORCED `const fn` — a pure struct literal over
    /// `Copy` fields (`clippy::missing_const_for_fn`, nursery = deny).
    pub(crate) const fn to_gen_params(self) -> GenParams {
        GenParams {
            cars: self.race.cars,
            min_straight: self.min_straight,
            v_ceiling: self.race.v_target,
            block_size: self.block_size,
            seeds: self.seeds,
            seed_budget: self.seed_budget,
            repair_budget: self.repair_budget,
        }
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
    // v3 (subtask 5): the full `GenParams` `Debug` line — carries all seven
    // fields, including the four nested `Seeds` values. `Debug` cannot
    // silently omit a field, and auto-follows the deferred
    // `v_ceiling` -> `v_target` rename instead of drifting.
    format!("{player_line}\ngraphite-gp: {:?}", config.to_gen_params())
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

    /// Unwraps the `clap::error::ErrorKind` of a `ConfigError::Cli`. Panics
    /// on the cross-field variant — no test in this module calls `kind` for
    /// a case expected to produce it.
    fn kind(args: &[&str]) -> clap::error::ErrorKind {
        match parse_err(args) {
            ConfigError::Cli(err) => err.kind(),
            other @ ConfigError::BlockSizeBelowWidthFloor { .. } => {
                panic!("unexpected variant: {other:?}")
            }
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

    // ---- AC13 (final): tuning defaults ----

    #[test]
    fn ac13_tuning_defaults_match_documented_literals() {
        assert_eq!(DEFAULT_MIN_STRAIGHT, 3);
        assert_eq!(DEFAULT_BLOCK_SIZE, 6);
        assert_eq!(DEFAULT_SEED_BUDGET, 1);
        assert_eq!(DEFAULT_REPAIR_BUDGET, 8);
        let config = parse(&[]);
        assert_eq!(config.min_straight, DEFAULT_MIN_STRAIGHT);
        assert_eq!(config.block_size, DEFAULT_BLOCK_SIZE);
        assert_eq!(config.seed_budget, DEFAULT_SEED_BUDGET);
        assert_eq!(config.repair_budget, DEFAULT_REPAIR_BUDGET);
    }

    // ---- AC3 (final): tuning flags round-trip ----

    #[test]
    fn ac3_tuning_flags_round_trip() {
        let config = parse(&[
            "--min-straight",
            "5",
            "--block-size",
            "10",
            "--seed-budget",
            "3",
            "--repair-budget",
            "12",
        ]);
        assert_eq!(config.min_straight, 5);
        assert_eq!(config.block_size, 10);
        assert_eq!(config.seed_budget, 3);
        assert_eq!(config.repair_budget, 12);
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
            other @ ConfigError::Cli(_) => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn ac5_block_size_at_width_floor_is_accepted() {
        assert_eq!(parse(&["--cars", "6", "--block-size", "3"]).block_size, 3);
    }

    // ---- AC6: cross-field error message names flag, values and domain ----

    #[test]
    fn ac6_cross_field_error_names_flags_and_values() {
        let text = rendered(&["--cars", "6", "--block-size", "2"]);
        assert!(text.contains("--block-size"), "{text}");
        assert!(text.contains('2'), "{text}");
        assert!(text.contains('3'), "{text}");
        assert!(text.contains("--cars"), "{text}");
        assert!(text.contains('6'), "{text}");
    }

    // ---- AC7: to_gen_params maps all seven fields ----

    #[test]
    fn ac7_to_gen_params_maps_all_seven_fields() {
        let config = parse(&[
            "--cars",
            "6",
            "--v-target",
            "10",
            "--min-straight",
            "5",
            "--block-size",
            "10",
            "--seed-budget",
            "3",
            "--repair-budget",
            "12",
            "--seed",
            "42",
        ]);
        let params = config.to_gen_params();
        assert_eq!(params.cars, 6);
        assert_eq!(params.min_straight, 5);
        assert_eq!(params.v_ceiling, 10);
        assert_eq!(params.block_size, 10);
        assert_eq!(params.seeds, config.seeds);
        assert_eq!(params.seed_budget, 3);
        assert_eq!(params.repair_budget, 12);
    }

    // ---- AC8: derived invariants over the whole accepted `cars` domain ----

    #[test]
    fn ac8_block_size_and_start_finish_width_hold_over_cars_domain() {
        for cars in CARS_MIN..=CARS_MAX {
            let params = parse(&["--cars", &cars.to_string()]).to_gen_params();
            assert!(
                u32::try_from(params.block_size).unwrap_or(0) >= params.min_width(),
                "cars={cars}"
            );
            assert_eq!(params.start_finish_width(), cars);
        }
    }

    // ---- AC18: the startup echo contains every resolved value ----

    #[test]
    fn ac18_echo_contains_every_resolved_value() {
        let config = parse(&[
            "--cars",
            "6",
            "--seed",
            "12345",
            "--seed-ai-learning",
            "999",
        ]);
        let params = config.to_gen_params();
        let rendered = render_startup_echo(&config);

        assert!(
            rendered.contains(&format!(
                "temperature {:.*}",
                TEMPERATURE_DECIMALS,
                config.temperature()
            )),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("cars: {}", params.cars)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("min_straight: {}", params.min_straight)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("v_ceiling: {}", params.v_ceiling)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("block_size: {}", params.block_size)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("seed_budget: {}", params.seed_budget)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("repair_budget: {}", params.repair_budget)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("collision: {}", params.seeds.collision)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("generation: {}", params.seeds.generation)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("ai_learning: {}", params.seeds.ai_learning)),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("ai_inference: {}", params.seeds.ai_inference)),
            "{rendered}"
        );

        // Negative control (design § AC18): the player line alone does not
        // satisfy the `GenParams`-half needles.
        let player_line = rendered.lines().next().expect("player line present");
        assert!(!player_line.contains("min_straight: "), "{player_line}");
        assert!(!player_line.contains("collision: "), "{player_line}");
    }
}
