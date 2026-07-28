//! CLI argument parsing and validated game configuration (issue #41).
//!
//! Owns the bound/default constants, the seed resolution, the validated
//! [`GameConfig`], and the [`GenParams`]/temperature mapping. The
//! [`clap`]-derived raw `Cli` struct lives in `cli` (stays private to the
//! whole `config` tree — it never escapes into the mapping logic); the
//! cross-field [`ConfigError`] lives in `error`; the startup-echo formatter
//! ([`render_startup_echo`]) lives in `echo` — all three split out of
//! this file to keep it under the workspace's 800-line soft cap (AGENTS.md §
//! Code Style).
//!
//! Lives in the **lib** target (design `2026-07-28-game-loop-orchestration`
//! § *Module decomposition*, KD15) — both `src/main.rs` (the bin) and
//! `src/app/session.rs` need [`GameConfig`], and the lib is where headless
//! tests reach it. `src/main.rs` reaches it as `gp_game::config::…`.

mod cli;
mod echo;
mod error;

use clap::Parser;
use gp_core::rng::Seeds;
use gp_gen::GenParams;
use gp_render::screens::RaceConfig;

use cli::Cli;
pub use echo::render_startup_echo;
pub use error::ConfigError;

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
/// Default `--difficulty` spelling, routed through `cli::parse_difficulty`.
const DEFAULT_DIFFICULTY_LABEL: &str = "Pro";
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

/// The replay-mode `--replay` selects (issue #43, C3, spec § Replay CLI).
///
/// `Headless` drives the record through a spawned process with no window
/// and prints final standings (`AC21`), `Gui` plays it back on screen at one
/// turn per fixed interval (`AC21c`). The binary is a GUI app, so `Gui` is
/// the RESOLVED default when `--replay-mode` is not given at all — that
/// default is independent of `AC21d`'s cross-field rule, which rejects an
/// **explicitly given** `--replay-mode` when `--replay` is absent (spec §
/// Replay CLI: "CI passes `--replay-mode headless` explicitly").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayMode {
    /// `--replay-mode headless` — no window; prints final standings and
    /// exits non-zero on divergence (`AC21`).
    Headless,
    /// `--replay-mode gui` — plays back on screen (`AC21c`). The resolved
    /// default.
    Gui,
}

impl ReplayMode {
    /// Case-insensitively parses `raw` as `"headless"` or `"gui"`, mirroring
    /// `cli::parse_difficulty`'s label-parse idiom (spec § Replay CLI).
    /// FORCED `const fn` (`clippy::missing_const_for_fn`, nursery = deny).
    const fn from_label(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("headless") {
            Some(Self::Headless)
        } else if raw.eq_ignore_ascii_case("gui") {
            Some(Self::Gui)
        } else {
            None
        }
    }
}

/// The validated game configuration assembled from `Cli` (issue #41).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameConfig {
    /// The player-facing race configuration, reused from `gp-render` — the
    /// same type `AppShell::new` and the Setup screen already speak.
    pub race: RaceConfig,
    /// The resolved per-source seeds — the master expanded via
    /// `Seeds::from_master`, with any supplied per-source override applied.
    pub seeds: Seeds,
    /// The raw `--seed` master value (issue #43 § Seed policy) — request
    /// `k`'s effective seeds are `Seeds::from_master(master.wrapping_add(k))`
    /// for `k > 0`; `k = 0` uses `seeds` verbatim (preserving this field's
    /// own per-source override contract above).
    pub master: u64,
    /// `L_min` — minimum straight length before a corner.
    pub min_straight: i32,
    /// `k` — coarse-block size / nominal corridor width.
    pub block_size: i32,
    /// Outer-loop seed budget.
    pub seed_budget: u32,
    /// Inner-loop repair budget.
    pub repair_budget: u32,
    /// `--record <PATH>` — persist the race to this path at race end (spec
    /// § Replay CLI, `AC21`).
    pub record: Option<std::path::PathBuf>,
    /// `--replay <PATH>` — replay a persisted record from this path
    /// instead of an interactive race (spec § Replay CLI, `AC21`).
    pub replay: Option<std::path::PathBuf>,
    /// The resolved replay mode (spec § Replay CLI) — meaningless when
    /// `replay` is `None`.
    pub replay_mode: ReplayMode,
}

impl TryFrom<Cli> for GameConfig {
    type Error = ConfigError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        // `AC21d`'s two cross-field rejections, checked before anything else
        // — neither depends on any derived value, only on which raw flags
        // were actually given (`cli.replay_mode`/`cli.record`/`cli.replay`
        // are `Option`, so "given" and "defaulted" are distinguishable,
        // unlike a `default_value_t` flag).
        if cli.replay_mode.is_some() && cli.replay.is_none() {
            return Err(ConfigError::ReplayModeWithoutReplay);
        }
        if cli.record.is_some() && cli.replay.is_some() {
            return Err(ConfigError::RecordWithReplay);
        }

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
            master: cli.seed,
            min_straight: cli.min_straight,
            block_size: cli.block_size,
            seed_budget: cli.seed_budget,
            repair_budget: cli.repair_budget,
            record: cli.record,
            replay: cli.replay,
            replay_mode: cli.replay_mode.unwrap_or(ReplayMode::Gui),
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
    /// fields (`clippy::missing_const_for_fn`, nursery = deny). Takes
    /// `&self` (not `self`) since C3's `PathBuf` fields cost `GameConfig`
    /// its `Copy` derive.
    pub const fn temperature(&self) -> f32 {
        self.race.temperature()
    }

    /// The race's lap count as an `i32`, for `ShellSession::total_laps`. A
    /// *total* (non-panicking) conversion — `self.race.laps` is validated
    /// into `[1, 9]`, so the `i32::MAX` sentinel is unreachable in practice,
    /// but the form stays total rather than relying on that invariant.
    pub fn total_laps(&self) -> i32 {
        i32::try_from(self.race.laps).unwrap_or(i32::MAX)
    }

    /// Maps this config onto a [`GenParams`], mapping `v_target` onto
    /// `v_ceiling` (AC7). FORCED `const fn` — a pure struct literal over
    /// `Copy` fields (`clippy::missing_const_for_fn`, nursery = deny).
    /// Takes `&self` — see [`Self::temperature`]'s note.
    pub const fn to_gen_params(&self) -> GenParams {
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
///
/// # Errors
/// Returns [`ConfigError::Cli`] for any `clap` tokenizing/validation
/// failure (unknown flag, missing/unparseable/out-of-range value), and
/// [`ConfigError::BlockSizeBelowWidthFloor`] when `--block-size` sits below
/// the corridor-width floor `--cars` implies (AC5).
pub fn parse_from<I, T>(args: I) -> Result<GameConfig, ConfigError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(args)?;
    GameConfig::try_from(cli)
}

/// Parses `args`, prepending the program name (AC1's iterator contract —
/// every test drives `parse_from` with an explicit `&[&str]`, never
/// `std::env::args`). Shared across this module's submodule test bodies
/// (`error`, `echo`) via `super::parse` — private, not `pub(crate)`: plain
/// module-privacy already reaches every descendant of `config`.
#[cfg(test)]
fn parse(args: &[&str]) -> GameConfig {
    let mut full = vec!["graphite-gp"];
    full.extend_from_slice(args);
    parse_from(full).expect("expected Ok")
}

/// The `Err` counterpart of [`parse`].
#[cfg(test)]
fn parse_err(args: &[&str]) -> ConfigError {
    let mut full = vec!["graphite-gp"];
    full.extend_from_slice(args);
    parse_from(full).expect_err("expected Err")
}

/// Unwraps the `clap::error::ErrorKind` of a `ConfigError::Cli`. Panics on
/// any cross-field variant — no test in this crate calls `kind` for a case
/// expected to produce one.
#[cfg(test)]
fn kind(args: &[&str]) -> clap::error::ErrorKind {
    match parse_err(args) {
        ConfigError::Cli(err) => err.kind(),
        other @ (ConfigError::BlockSizeBelowWidthFloor { .. }
        | ConfigError::ReplayModeWithoutReplay
        | ConfigError::RecordWithReplay) => {
            panic!("unexpected variant: {other:?}")
        }
    }
}

/// The rendered `Display` form of `parse_err(args)`, for AC6 substring
/// assertions.
#[cfg(test)]
fn rendered(args: &[&str]) -> String {
    parse_err(args).to_string()
}

#[cfg(test)]
mod tests {
    use gp_render::screens::{DIFFICULTY_LABELS, Difficulty};

    use super::*;

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

    // ---- Fix 2 (self-review round 1, major): `total_laps` had zero
    // coverage — gutting its body to `0` left every other test green. Covers
    // the default (`DEFAULT_LAPS`) and an explicit value at each accepted
    // `[LAPS_MIN, LAPS_MAX]` bound. ----

    #[test]
    fn total_laps_returns_parsed_laps_as_i32() {
        // Asserted against `i32` LITERALS, not `i32::try_from(DEFAULT_LAPS)`:
        // re-deriving the expectation through the same conversion
        // `total_laps` performs would let a conversion bug cancel out on both
        // sides. The literals match AC13's documented defaults/bounds.
        assert_eq!(parse(&[]).total_laps(), 5);
        assert_eq!(parse(&["--laps", "1"]).total_laps(), 1);
        assert_eq!(parse(&["--laps", "9"]).total_laps(), 9);
    }
}
