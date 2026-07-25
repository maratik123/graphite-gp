# gp-game: config / CLI — m, laps, difficulty, V_target, seed → GenParams

**Source:** issue #41
**Date:** 2026-07-25
**Tracked in:** #41

Block 3b (`gp-game`, crate `crates/game`), build-order 38/40, size **M**. Both
dependencies are closed and merged: #34 (`gp_gen::generate` pipeline) and #6
(`TrackArtifact` contract).

## Scope

1. **A `clap`-derived CLI** for the `graphite-gp` binary parsing **thirteen**
   flags:
   - the five player/product inputs from the issue — `--cars` (design `m`),
     `--laps`, `--difficulty`, `--v-target` (design `V_target`), `--seed`;
   - four generation-tuning flags — `--min-straight` (design `L_min`),
     `--block-size` (design `k`), `--seed-budget`, `--repair-budget` (round-1
     Q3: *extra flags*, matching the design doc's own
     `generate_track(m, k, L_min, V_target, repair_budget, seed_budget, rng)`
     signature);
   - four optional per-source seed overrides — `--seed-collision`,
     `--seed-generation`, `--seed-ai-learning`, `--seed-ai-inference` (round-2
     Q4: *master + overrides*).

   Every flag has a default or is optional, so bare `graphite-gp` keeps
   working.
2. **A validated config type in `gp-game`** assembled from those flags,
   composing `gp_render::screens::RaceConfig` and adding the resolved
   `gp_core::rng::Seeds` plus the four tuning values.
3. **Validation** — per-flag range checks plus the cross-field invariant
   `block_size ≥ ⌈cars/2⌉` (design `k ≥ n`). Out-of-range input is *rejected*,
   never silently clamped, and never panics.
4. **Seed resolution** — a single `--seed` master expanded into all four
   `Seeds` fields by the normative derivation below, with any supplied
   per-source override taking precedence over the derived value for that field
   only.
5. **Mapping to `gp_gen::GenParams`** — all seven fields (`cars`,
   `min_straight`, `v_ceiling`, `block_size`, `seeds`, `seed_budget`,
   `repair_budget`) — and mapping `difficulty` to the AI pilot **temperature**
   (`f32`, the third argument of `gp_ai::policy_action`).
6. **Clear error reporting** — `clap`'s own diagnostics for tokenizing and
   per-flag ranges, a `thiserror` enum for the cross-field invariants; the
   binary prints the error and exits non-zero *before* opening a window.
7. **Wiring into `crates/game/src/main.rs`** (round-1 Q1: *seed the GUI*) — the
   parsed `RaceConfig` replaces the hard-coded `STARTUP_CONFIG` const and is
   passed to `gp_render::AppShell::new`, so the Setup screen opens pre-loaded
   with the CLI values. The player can still change them in the GUI.
8. **A startup echo** (round-4 amendment 3) — on the success path, before the
   window opens, the binary prints the resolved `GenParams` and the AI pilot
   temperature, so the configuration actually in force is visible from the
   terminal. This is user-visible behaviour, hence part of the contract (AC18),
   not merely a design detail; it also gives the seeds/tuning fields a real
   consumer before #43 lands.
9. **Deterministic unit tests** driven through `clap`'s `try_parse_from` over
   an explicit argument iterator — never the real `std::env::args`, so tests are
   hermetic and order-independent.

## Out of scope

- Calling `gp_gen::generate` / replacing the hand-built `fixture_track()` in
  `main.rs` with a generated track — that is #43 (game loop, full wiring). This
  task produces the `GenParams`; #43 consumes it.
- The controller abstraction and player input (#42) and the AI controller that
  actually consumes the temperature (#158). This task only produces the `f32`.
- Feeding the CLI seed to the `LabScreen`'s displayed `seed` header
  (`ShellSession.seed`) — see § Deferred for the type mismatch that blocks it.
- Config files, environment variables, and persisted user settings.
- Changing the **shape** of `gp_gen::GenParams` (its seven fields) or of
  `gp_core::rng::Seeds` (its four fields). Adding a *derivation helper* beside
  `Seeds` in `gp-core` is **not** excluded — where the master→`Seeds` derivation
  physically lives is a design-phase placement call (see § Key decisions).
- Any new screen or widget in `gp-render`.

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| **The shipped default config is NOT proven to produce an accepted track** | Round-2 Q5 chose *pin values only*: AC13 asserts the default constants equal their documented literals, with **no** `gp_gen::generate()` call. The defaults therefore carry no evidence of generating anything — and `v_target = 7` in particular has no supporting test anywhere in the repo (`gp-gen`'s only proven configs use `v_ceiling = 5`). Verifying that the shipped defaults actually yield `Ok(TrackArtifact)` belongs to **#43**, when generation is first wired up; if they do not, #43 retunes the default constants. | No — assign to #43 |
| Rename `gp_gen::GenParams::v_ceiling` → `v_target` | The field is documented as "`V` — oracle speed ceiling", but `generate()` binds it straight to `let v_target = params.v_ceiling;` (`crates/gen/src/generate.rs:108`) and passes it to `phase3_start_finish` / `phase5_runout_checks` as `V_target`. `docs/design.md` §2 explicitly warns that `V_target` and `V_ceil` must not be conflated (`V_ceil` is the oracle's iterative-deepening bound, **not** a geometry input). AGENTS.md § *API Stability* permits the clean rename. It is `gp-gen` surface, not `gp-game` surface. | Yes |
| Stale module doc in `crates/game/src/main.rs` | Its header still says "`gp_gen::generate` is an unimplemented `todo!()` stub … that would panic at startup". #34 landed; `generate()` is implemented. The paragraph also justifies `fixture_track()` on that false premise. | Fold into #43 |
| `ShellSession.seed` is `i32`, the master seed is `u64` | `main.rs` currently passes `FIXTURE_SEED: i32 = 7` to the `LabScreen` header. A `u64` CLI seed cannot round-trip through an `i32` display field, so surfacing the real seed in the GUI needs either a widened `ShellSession.seed` or a documented display-only truncation. | Yes |
| Exposing `gp-render`'s setup-screen bound constants (`MIN_CARS`/`MAX_CARS`/`MIN_LAPS`/`MAX_LAPS`/`MIN_V_TARGET`/`MAX_V_TARGET`) as `pub` | They are private to `crates/render/src/screens/setup.rs`, so `gp-game` cannot reuse them and must restate the same numbers. A drift-guard test is the cheaper fix (AC9). | No — AC9 covers it |
| Randomised default seed (fresh track per launch) | Needs an entropy source; the workspace pins `rand` with `default-features = false, features = ["alloc"]`, so `OsRng` is not currently reachable. Deterministic-by-default is also the posture `gp_core::rng` documents ("No OS entropy — the same seed always yields the same draw stream"). | Yes, if wanted |

## Key decisions

| Question | Decision |
|---|---|
| **CLI role / wiring** (round-1 Q1) | **Seed the GUI.** The parsed `RaceConfig` replaces `main.rs`'s `STARTUP_CONFIG` const at the `AppShell::new` call site; the Setup screen opens pre-loaded and stays editable. The seed and the four tuning values are parsed, validated and mapped now, and carried for #43 to consume. |
| **Argument-parsing approach** (round-1 Q2) | **`clap`, derive API.** `clap = { version = "4", features = ["derive"] }` in `[workspace.dependencies]`, referenced as `clap = { workspace = true }` by `gp-game` — the workspace-table convention every existing dependency follows. Version **`"4"`** per AGENTS.md § *Dependency Versions* (`x` for `x.y.z`), applied to the observed live max stable **4.6.4** (crates.io, checked 2026-07-25). Its declared MSRV **1.85** is below the workspace `rust-version = "1.97.1"`. `default-features` stay **on** — `help`/`usage`/`error-context`/`suggestions` are the reason this option was chosen over a hand-rolled parser, so switching them off would defeat the decision. |
| **Source of the four non-CLI `GenParams` fields** (round-1 Q3) | **Extra flags.** `--min-straight`, `--block-size`, `--seed-budget`, `--repair-budget`, each with a default. Nothing is derived from `cars`/`v_target`; the only coupling is the validated invariant `block_size ≥ ⌈cars/2⌉`. |
| **`--seed` surface** (round-2 Q4) | **Master + per-source overrides.** One `--seed <u64>` master expands to all four `Seeds` fields; four optional flags — `--seed-collision`, `--seed-generation`, `--seed-ai-learning`, `--seed-ai-inference` — each override their own field. Any subset may be supplied. |
| **Master → `Seeds` derivation** (normative) | Build one `Xoshiro256PlusPlus` via `SeedableRng::seed_from_u64(master)`, then take **four successive `next_u64()` draws into named locals, in this exact order**: draw 1 → `collision`, draw 2 → `generation`, draw 3 → `ai_learning`, draw 4 → `ai_inference`. Draws are bound to locals *before* the `Seeds` literal is constructed, so the mapping never depends on struct-literal field-evaluation order. `Xoshiro256PlusPlus` is chosen because `gp-core` already uses it for three of the four sources and it is a fixed, platform-stable algorithm — the derivation is therefore reproducible across machines, and AC11 pins it. |
| **Override precedence** (normative) | A supplied `--seed-<source>` flag replaces the derived value **for that field only**, verbatim; the other three fields keep their derived values. Overrides are mutually independent. Omitting `--seed` does not disable derivation — the master falls back to its default constant and the derivation still runs. Supplying all four overrides makes the master irrelevant to the resulting `Seeds`. |
| Where the derivation helper lives | Design-phase placement call. Two defensible homes: (a) `gp-game`'s config module, which then declares `rand` + `rand_xoshiro` (both already `[workspace.dependencies]` entries and already in `gp-game`'s tree transitively via `gp-core` — verified 2026-07-25 with `cargo tree --invert rand_xoshiro`); or (b) an associated fn beside `Seeds` in `gp_core::rng`, where those two crates are already direct dependencies and where `Seeds` itself is documented as "one place to configure every source". Either way the derivation is the one above and AC11 pins it. |
| **Whether the shipped defaults must be proven to generate** (round-2 Q5) | **Pin values only.** AC13 asserts the default constants equal their documented literals; no `gp_gen::generate()` call, so nothing slow enters the suite and no seed hunt is required. The consequence — the defaults are unproven — is recorded in § Deferred and assigned to #43. |
| **Startup echo vs a `dead_code` allow** (round-4 amendment 3) | **Startup echo.** The config's seeds and tuning fields have no consumer until #43, which would otherwise force a justified `#[allow(dead_code, reason = ...)]` on the forward-looking config `impl`. Printing the resolved `GenParams` + temperature at startup *uses* those fields, so the allow becomes unnecessary — and the echo is independently useful, since the configuration in force becomes observable. Pinned by AC18. |
| Config type — new, or reuse `gp_render::screens::RaceConfig`? | Reuse. `RaceConfig { cars: u32, laps: u32, v_target: i32, difficulty: Difficulty }` already exists, is `Copy`, is already imported by `main.rs`, and is exactly the four player-facing inputs. `gp-game`'s config composes it and adds `seeds` + the four tuning values. No second definition of the same four fields. |
| Where the `clap`-derived struct sits relative to the config type | The derive struct holds raw parsed flags (including the four overrides as `Option<u64>`); a fallible conversion resolves the seeds, runs the cross-field check, and produces the validated config that `to_gen_params()` and `temperature()` hang off. Keeps `clap` types out of the mapping logic. Exact split is the design phase's call. |
| Error-type split | `clap`'s `value_parser!(T).range(..)` carries the per-flag numeric ranges, so `clap` renders those errors (flag, received value, accepted range) in its standard format. The cross-field invariant `block_size ≥ ⌈cars/2⌉` cannot be expressed per-flag and is checked after parsing, reported via a `thiserror` enum (already a workspace dependency, used by `gp-gen`). |
| Test entry point | `try_parse_from(["graphite-gp", ...])` — returns `Result` instead of exiting the process, which `Parser::parse()` does. Production `main` uses the exiting form. |
| Difficulty → temperature table | Delegate to `gp_render::screens::Difficulty::temperature()` (currently `Rookie 1.5 / Pro 1.0 / Ace 0.6`, documented as tunable placeholders). `gp-game` must **not** restate the numbers — one source of truth. |
| Accepted difficulty spellings | `rookie` / `pro` / `ace`, case-insensitive, matched against `gp_render::screens::DIFFICULTY_LABELS`. |
| Bounds — player inputs | Mirror the `SetupScreen` steppers/slider exactly: `cars ∈ [2, 6]`, `laps ∈ [1, 9]`, `v_target ∈ [3, 10]`. A CLI that accepts values the GUI cannot represent would desync the two entry paths that now feed the same `RaceConfig`. |
| Bounds — tuning inputs | `min_straight ∈ [2, 64]` — **both** ends, because `gp-gen` *silently clamps* `l_min` into exactly that domain (`clamp_l_min`, `crates/gen/src/phase1.rs:264`); accepting `1` or `65` would have the CLI advertise a value the generator quietly rewrites, contradicting this spec's own "reject, never clamp" decision (round-4 amendment 2). `seed_budget ≥ 1` and `repair_budget ≥ 1` (`gp-gen`'s own `zero_seed_budget_fails_promptly` / `zero_repair_budget_fails_promptly` tests show `0` is accepted by `GenParams` but makes generation fail immediately — a CLI must not hand the user that footgun); `block_size ≥ ⌈cars/2⌉`. Upper bounds for `block_size` and the two budgets remain the design phase's call. |
| Bounds — seed inputs | None. Every `u64` is a valid seed, for the master and for all four overrides. |
| Defaults — player inputs | Mirror `main.rs`'s existing `STARTUP_CONFIG`: `cars = 4`, `laps = 5`, `v_target = 7`, `difficulty = Pro`. Unchanged startup appearance when the binary is launched bare. |
| Defaults — tuning inputs | Anchored on the `(min_straight = 3, block_size = 6)` pair that `gp-gen`'s own `generate()` tests use, with budgets from its cheap always-on e2e case (`seed_budget = 1`, `repair_budget = 8`) — the only parameter set in this repo with evidence of producing an accepted track. Note this is an *anchor*, not a proof: see § Deferred. |
| Defaults — seeds | One named `DEFAULT_SEED` constant for the master; the four overrides default to *absent*, not to a value. |
| Invalid input handling | Reject with an error — never clamp. (`gp-render`'s `assemble` clamps *defensively* because its inputs come from a widget that cannot produce out-of-range values; a CLI's inputs are arbitrary text, and a silent clamp would hide the user's mistake.) |
| `v_target` → `GenParams` | `GenParams.v_ceiling`, per issue AC2 and per `generate()`'s actual use of the field. See § Deferred for the naming defect. |
| `laps` consumer | `laps` has no `GenParams` field — it is race-loop configuration, validated here and carried to `ShellSession.total_laps` (already an `i32` in `main.rs`) for #43. |
| Bounds/derivation arithmetic | `clippy::arithmetic_side_effects` is `deny` workspace-wide; every derived quantity uses `checked_*` / `saturating_*` / `div_ceil` / `try_from`, or a documented, test-covered `#[allow(..., reason = "...")]`. `⌈cars/2⌉` comes from `GenParams::min_width()`, not a hand-rolled division. |

## Technical constraints

- `gp-game` today depends on `gp-core`, `gp-gen`, `gp-render`, `gp-ai`,
  `eframe`, `winit`, and consists of exactly one file,
  `crates/game/src/main.rs` (~300 lines). There is **no** `lib.rs` — a
  `#[cfg(test)] mod tests` inside a binary crate is run by `cargo test`, but if
  the config module is to be integration-tested from `tests/`, the crate needs a
  library target too. Design-phase call.
- The workspace dependency graph contains **no** argument-parsing crate today
  (verified 2026-07-25: `grep -r` over every `Cargo.toml`, plus
  `cargo tree --invert clap` and `cargo tree --invert argh`, both "did not match
  any packages"). Per AGENTS.md, after editing the version constraint run
  `cargo update` then `cargo build`, and confirm `git diff --stat Cargo.lock`
  shows only the intended edges before staging.
- `rand` and `rand_xoshiro` are already `[workspace.dependencies]` entries
  (`rand = { version = "0.10", default-features = false, features = ["alloc"] }`,
  `rand_xoshiro = { version = "0.8", default-features = false }`) and already
  reach `gp-game` transitively through `gp-core` (verified 2026-07-25:
  `cargo tree --invert rand_xoshiro` shows `rand_xoshiro v0.8.1 → gp-core →
  gp-game`). Declaring either directly on `gp-game` therefore brings no external
  crate into the graph. `RngCore::next_u64` is available under the pinned
  `default-features = false` configuration.
- `gp_gen::GenParams` (`crates/gen/src/lib.rs:50`) is
  `{ cars: u32, min_straight: i32, v_ceiling: i32, block_size: i32, seeds:
  gp_core::rng::Seeds, seed_budget: u32, repair_budget: u32 }`, with
  `min_width() = cars.div_ceil(2)` (design `n`) and
  `start_finish_width() = cars` (design `m`) already provided as methods —
  `gp-game` must not recompute either.
- `gp_core::rng::Seeds` has **four** `u64` fields, declared in this order:
  `collision`, `generation`, `ai_learning`, `ai_inference`. It derives
  `Clone, Copy, Debug, Default, PartialEq, Eq, Hash`, so `assert_eq!` on a whole
  `Seeds` value works directly in the AC11/AC12 tests. Issue #49 designed it as
  *one place* to configure every source.
- **Known-good generation parameters** (the only ones with in-repo evidence of
  acceptance, from `crates/gen/src/generate.rs`): `cars = 4, min_straight = 3,
  v_ceiling = 5, block_size = 6` with `seed_budget = 1, repair_budget = 8` at
  generation seeds **6** and **9**, and with `seed_budget = 64,
  repair_budget = 32` at seed 1 (that one measured at ~467 s debug and is
  `#[ignore]`d as heavy). Note `v_ceiling = 5`, whereas this CLI's `v_target`
  default is **7**.
- **`gp-gen` silently clamps `l_min`** — the basis of the `min_straight ∈ [2,64]`
  domain (verified 2026-07-25 by reading the source): `MIN_COARSE_STRAIGHT: i32
  = 2` (`crates/gen/src/phase1.rs:20`), `MAX_COARSE_STRAIGHT: i32 = 64` (`:23`),
  and `fn clamp_l_min(l_min) -> i32 { l_min.clamp(MIN_COARSE_STRAIGHT,
  MAX_COARSE_STRAIGHT) }` (`:264`), whose own doc-comment states "below this,
  `phase1_coarse_ring` clamps up". Values outside `[2,64]` are therefore rewritten
  by the generator without telling anyone.
- **Measured `clap` 4.6.4 parse behaviour** (verified 2026-07-25 by building a
  scratch crate against real `clap` 4.6.4, with `value_parser!` ranges and no
  positional arguments declared — the same shape this CLI has). `""` →
  `UnknownArgument`; `-` → `UnknownArgument`; **bare `--` → `Ok`, defaults**;
  `-- stray` → `UnknownArgument`; `--seed 18446744073709551616` →
  `ValueValidation`; lone `--seed` → `InvalidValue`; `--cars 4 --cars 5` →
  `ArgumentConflict`; out-of-range values (`--cars 7`, `--min-straight 1`,
  `--min-straight 65`) → `ValueValidation`, while `--min-straight 2` accepts.
  AC4 and AC14 are pinned to these observed outcomes, not to assumed ones.
- `docs/design.md` §2 constrains `k ≥ n` and describes `L_min` as the run-out
  seed, with the accel zone sized `~V_target²/2`.
- `gp_render::screens::setup`'s bound constants are **private**; `gp-game`
  cannot import them.
- `gp_render::AppShell::new(config: RaceConfig)` (`crates/render/src/app.rs:137`)
  is the single injection point for a startup config, and `main.rs` already
  calls it with `STARTUP_CONFIG` — the substitution is a one-line change.
- `temperature` is `f32` (`Difficulty::temperature`, `gp_ai::policy_action`).
  This is `gp-render`/`gp-ai` surface; the `gp-core` integer-only rule
  (`docs/design.md` §3a) constrains `geom`/`sim`, not `gp-game` config.
- Workspace lint posture applies: `missing_docs = deny`, clippy `pedantic` +
  `nursery` = deny, `arithmetic_side_effects` = deny.
- Miri: `gp-game` is inside the Miri gate (only `gp-gen` carries the sanctioned
  crate-level exclusion, #134). Any new test that aborts or is unaffordable
  under Miri needs `#[cfg_attr(miri, ignore = "<why>")]` **in the same commit**.
- AGENTS.md § *Rust Test Conventions*: a file with ~50+ lines of substantial
  logic needs a `#[cfg(test)] mod tests`; no `unwrap()` in production code
  without a justifying comment.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | A `clap`-derived argument struct in `gp-game` exposes all thirteen flags (the four seed overrides as optional); a validated config type composes `gp_render::screens::RaceConfig` with the resolved `Seeds`, `min_straight`, `block_size`, `seed_budget`, `repair_budget`. The parse entry point takes the arguments as an iterator parameter (`try_parse_from`) and never reads `std::env::args` internally. |
| AC2 | Parsing an empty argument list succeeds and yields the documented defaults: `cars = 4`, `laps = 5`, `v_target = 7`, `difficulty = Pro`, the named tuning default constants, and a `Seeds` derived from `DEFAULT_SEED` with no override applied. Asserted by a test naming each value. |
| AC3 | Every one of the thirteen flags is settable and round-trips: for each, a test asserts a supplied in-range value appears verbatim in the validated config (for the four overrides, in the corresponding `Seeds` field). |
| AC4 | Per-flag bounds are enforced by rejection, not clamping: `cars ∉ [2,6]`, `laps ∉ [1,9]`, `v_target ∉ [3,10]`, `min_straight ∉ [2,64]`, `seed_budget < 1`, `repair_budget < 1` each produce an `Err`. For each, tests cover both the lowest and highest accepted values (accept) and one step outside each bounded end (reject) — for `min_straight` that means **2 and 64 accept, 1 and 65 reject** (round-4 amendment 2; measured to behave exactly so under `value_parser!(i32).range(2..=64)`). |
| AC5 | The cross-field invariant is enforced: `--cars 6 --block-size 2` (i.e. `block_size < ⌈cars/2⌉`) is rejected with a dedicated error naming both values and the derived floor; `block_size == ⌈cars/2⌉` is accepted. |
| AC6 | Invalid inputs produce clear errors. `clap`-sourced errors (unknown flag, missing value, unparseable value, out-of-range value, unrecognised difficulty) render the flag, the received value and the accepted domain; the cross-field error is a `thiserror` variant whose `Display` does the same. Tests assert on the error variant **and** on the presence of those substrings in the rendered message. |
| AC7 | `to_gen_params()` (or equivalent) maps the config onto a `GenParams` with **all seven** fields populated — `cars = m`, `v_ceiling = v_target`, `min_straight`/`block_size`/`seed_budget`/`repair_budget` from their flags, and `seeds` from the resolution in AC11/AC12. A test asserts each field for a known config. |
| AC8 | Derived-invariant tests over the whole accepted `cars` domain `[2,6]`: `to_gen_params().block_size ≥ GenParams::min_width()` and `start_finish_width() == cars` hold for every value. |
| AC9 | A drift guard asserts `gp-game`'s player-input bound constants equal the `SetupScreen`'s `[2,6]` / `[1,9]` / `[3,10]` ranges, so the CLI and the GUI — which now feed the same `RaceConfig` — cannot silently diverge. |
| AC10 | `temperature()` on the config returns exactly `Difficulty::temperature()` for the parsed difficulty, for all three difficulties — by delegation, with no temperature literal restated in `gp-game`. |
| AC11 | **The master→`Seeds` derivation is pinned exactly.** For a known master value, a test asserts the four resolved fields equal the first four `next_u64()` draws of `Xoshiro256PlusPlus::seed_from_u64(master)` in the order `collision, generation, ai_learning, ai_inference` — so reordering the assignment fails the test. Plus: the same master yields an identical `Seeds` across two independent parses, and two distinct masters yield four pairwise-distinct field values. |
| AC12 | **Override precedence.** For each of the four override flags independently, `--seed <M> --seed-<source> <V>` yields that field `== V` while the other three remain exactly the values derived from `M`. Supplying all four overrides yields a `Seeds` independent of the master. Omitting `--seed` while supplying an override still derives the other three from `DEFAULT_SEED`. |
| AC13 | **Defaults are pinned by value.** A test asserts each default constant equals its documented literal (`cars 4`, `laps 5`, `v_target 7`, `difficulty Pro`, `min_straight 3`, `block_size 6`, `seed_budget 1`, `repair_budget 8`, `DEFAULT_SEED <constant>`). No `gp_gen::generate()` call is made — see § Deferred for the unproven-defaults consequence assigned to #43. |
| AC14 | Zero production panics on any argument input: no `unwrap()`, no indexing panic, no integer-overflow panic; every numeric conversion is `try_from` / `checked_*` / `saturating_*`. A test feeds the pathological inputs below and asserts each is **handled without panicking**, pinning the measured outcome for each: `""` → `Err(UnknownArgument)`; `-` → `Err(UnknownArgument)`; `--seed 18446744073709551616` (2⁶⁴, one past `u64::MAX`) → `Err(ValueValidation)`; a lone `--seed` with no value → `Err(InvalidValue)`; `--cars 4 --cars 5` (repeated flag) → `Err(ArgumentConflict)`; `--` followed by a stray value → `Err(UnknownArgument)`. **A bare `--` is the one exception and legitimately parses to the defaults** — it is `clap`'s end-of-flags marker and this CLI declares no positional arguments, so there is nothing for it to introduce; asserting `Err` there would be asserting a bug (round-4 amendment 1). All seven outcomes measured against real `clap` 4.6.4 — see § Technical constraints. |
| AC15 | `main.rs` is rewired: the `STARTUP_CONFIG` const is gone, `AppShell::new` receives the CLI-derived `RaceConfig`, and an invalid argument makes the binary report the error and exit non-zero **without** opening a window (the parse precedes `eframe::run_native`). |
| AC16 | `graphite-gp --help` lists all thirteen flags with their defaults, and `--version` reports the crate version. |
| AC17 | `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` are all clean; every new public item carries at least a one-line `///`; `Cargo.lock`'s diff contains only the intended new edges. |
| AC18 | **Startup echo** (round-4 amendment 3). A pure formatting function takes the validated config (or its resolved `GenParams` + temperature) and returns the rendered text; a test asserts that for a known config the rendered string contains every resolved value — all seven `GenParams` fields, including all four `Seeds` values, plus the AI pilot temperature. `main` calls that function on the **success path only**, before `eframe::run_native`, so the echo is exercised without opening a window and the test targets the formatter rather than the process. Because the echo consumes the seeds/tuning fields, no `#[allow(dead_code)]` appears on the config `impl`. |

## Open questions

- **`--seed-budget` sits inside the `--seed-*` namespace.** With the override
  family added, `--seed-budget` and `--seed-generation` read as members of one
  family although only the latter is a seed override. `clap` will not complain,
  but a user might. Cheap alternatives if it grates: rename the overrides to
  `--seeds-<source>`, or the budget to `--generation-budget`. Not
  design-blocking — a rename is a one-line attribute change.
- **Upper bounds for the remaining tuning flags.** `min_straight`'s ceiling is
  now settled at **64** (round-4 amendment 2 — it mirrors `gp-gen`'s
  `MAX_COARSE_STRAIGHT`, so it is a correctness bound, not a preference).
  Ceilings for `block_size` and the two budgets remain open and are left to the
  design phase — a `--seed-budget 4000000000` is a hang, not a crash, so those
  are UX guards rather than correctness bounds.
- **Whether the tuning and override flags should be hidden from `--help`.**
  `clap` supports `hide = true` for advanced flags. AC16 currently assumes all
  thirteen are listed.
- **Randomised default seed.** Deterministic-by-default is assumed; a fresh
  track per launch needs an entropy source the workspace's `rand` feature set
  does not currently expose (§ Deferred).
- **Placeholder temperature values.** `Rookie 1.5 / Pro 1.0 / Ace 0.6` are
  documented in `gp-render` as tunable placeholders pending real `gp-ai`
  training. This task inherits them unchanged; retuning is a `gp-ai` concern.
