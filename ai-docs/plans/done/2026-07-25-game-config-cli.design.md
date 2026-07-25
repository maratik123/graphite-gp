# Design: gp-game config / CLI — m, laps, difficulty, V_target, seed → GenParams

**Issue:** [#41](https://github.com/maratik123/graphite-gp/issues/41)
**Spec:** [`ai-docs/plans/2026-07-25-game-config-cli.spec.md`](2026-07-25-game-config-cli.spec.md)
(round-4 amended — Scope 1..9, AC1..AC18)
**Date:** 2026-07-25
**Design round:** 3 + **GO write-back**. Round 2 reworked the owner's round-4
spec amendments (AC14's `--` exception, the `min_straight ∈ [2,64]` domain, the
startup echo replacing the `dead_code` allow). Round 3 folded the ITERATE
findings: `ConfigError::BlockSizeBelowWidthFloor` moved to subtask 5 with its
construction site; the decomposition's floor claim became a per-boundary consumer
table; AC14 split partial/final across subtasks 3 and 4; the `Cargo.lock` delta
became the measured 14 crates; AC18 asserts `Debug`-labelled forms. Round 3 also
found and fixed a further red gate on its own (`clippy::float_cmp` on the
`assert_eq!` over `f32` that round 2's AC10 row prescribed).

**This revision is the Step-7 GO write-back** (`/task` Step 8's first action
requires every GO note resolved *in the design*, not "in code later"): the
`main.rs` import trim that deleting `STARTUP_CONFIG` forces (§ Decomposition
subtask 3 + the new § Risks cross-crate-gap row); an accurate **nine**-class
lint enumeration (§ Risks); a `parse_from` / `TryFrom<Cli>` row in the consumer
table; the in-repo `float_cmp` precedent at `crates/render/src/test_util.rs:15-34`
(§ hand-off #3 + § Rejected alternatives + AC10); and the `syn` double-entry
citation plus a **measurement of the previously unmeasured subtask-4 slice**
(§ Risks + the consumer table). No change to `## Handoff plan`, the file list,
the AC mapping, or any model assignment — the reviewer re-verified all eight
handoff sub-points and full AC1..AC18 coverage after the subtask-3/5 reshuffle.

## Approach

One new module `crates/game/src/config.rs` owns the whole CLI surface: the
`clap`-derived raw struct, the bound/default constants, the seed resolution, the
validated `GameConfig`, the `GenParams`/temperature mapping, and the startup-echo
formatter. `gp-core` gains one small public helper (`Seeds::from_master`).
`main.rs` parses before `eframe::run_native`, echoes the resolved configuration
on the success path, and exits non-zero without a window on failure. One
integration test file proves the process-level exit contract.
**No library target for `gp-game`** (§ Resolved spec hand-offs #1).

Every fact below about a tool, a lint, a crate version, or an existing source
file was **executed**, not recalled. Three compile-verified prototypes back this
design, all under the workspace lint table (`pedantic` + `nursery` +
`arithmetic_side_effects` = deny) and all in scratch only — nothing in the
project tree was mutated by any design pass:

| Prototype | What it pins |
|---|---|
| `…/scratchpad/clapproto` | round 1: bin-crate unit tests run, `tests/` + `CARGO_BIN_EXE_*` on a bin-only crate, exit codes, Miri |
| `…/scratchpad/clapproto2` | the **final** shape: 13 flags, seed resolution, cross-field check, echo formatter, AC14 `ErrorKind`s, AC18 labelled assertions |
| `…/scratchpad/st3check` | the **subtask-3 and subtask-4** shapes: single-variant `ConfigError`, always-`Ok` `TryFrom`, echo v1, test-only `CommandFactory`; then the seed family, the resolution, and echo **v2**'s standalone `{seeds:?}` |

**What no prototype can reach.** All three stand in for `gp-render`/`gp-gen` with
local mock types, so **no lint that depends on the real cross-crate surface can
appear in any of them** — see the § Risks row of the same name. That blind spot
is why the GO's issue 1 (`unused_imports` on the imports `STARTUP_CONFIG`'s
deletion orphans) could not have been caught by prototyping, and it lands almost
entirely on subtask 3, the only subtask that edits `main.rs`.

### Resolved spec hand-offs

#### 1. `gp-game` gets **no** library target

`gp-game` stays a bin-only crate; unit tests live in
`crates/game/src/config.rs`'s `#[cfg(test)] mod tests`, and the two
process-level tests live in `crates/game/tests/cli.rs` (which needs no lib
target — it spawns the built binary).

1. **Bin-crate unit tests do run**, under CI's exact command, so a lib target
   buys no test reach for AC1–AC14/AC16/AC18.
   `[measured: cd <scratch>/clapproto2 && cargo test --tests --quiet → "test result: ok. 5 passed" for the bin target; CI runs `cargo test --workspace --tests --no-fail-fast` (`.github/workflows/ci.yml:106`)]`
2. **`--help` / `--version` are testable in-crate** through the `CommandFactory`
   impl that `#[derive(Parser)]` generates —
   `Cli::command().render_long_help().to_string()` and `.get_version()` — no
   process spawn, no lib target.
   `[measured: scratch `ac16_help_and_version` asserts all thirteen `--flag` names, exactly nine `[default: ` markers, and `get_version() == Some(env!("CARGO_PKG_VERSION"))` → passes]`
3. **A lib target's only unique reach is a liability.** The one thing a
   `gp-game` lib would add is a `tests/` file calling `parse_from` — but the
   *success* path of `main` now both echoes **and** opens a window, so an
   end-to-end success test would need a display. The two paths worth testing at
   process level (invalid argument, `--help`) terminate before
   `eframe::run_native` and are reachable via `env!("CARGO_BIN_EXE_graphite-gp")`,
   which is available to an integration test of a **bin-only** package. AC18 is
   pinned on the pure formatter precisely so no test needs the process.
   `[measured: clapproto has no [lib]; its tests/cli.rs used env!("CARGO_BIN_EXE_graphite-gp-proto") and both tests passed — exit code 2 with the clap message on stderr for `--cars 9`, exit code 0 for `--help`]`

#### 2. The derivation helper lives beside `Seeds` in `gp_core::rng`

```rust
impl Seeds {
    /// Derives all four source seeds from one master seed …
    pub fn from_master(master: u64) -> Self
```

- **`Seeds` is already documented as "one place to configure every source"**
  (`crates/core/src/rng.rs:1-10`, issue #49); expanding one master into that
  group is a `Seeds`-construction concern, not a CLI concern.
- **Zero new dependency edges.** `rand` and `rand_xoshiro` are already *direct*
  dependencies of `gp-core` and already imported by that exact file
  (`crates/core/src/rng.rs:19-21`).
- **`gp-render` (also a `gp-core` dependent) is the plausible second consumer**
  of a master→`Seeds` expansion (a "randomise seed" affordance on Setup);
  placing it in the shared crate now avoids a later lift.
- The AC11 draw-order pinning test then sits next to the code it pins, in a
  module that already has an RNG-stream test idiom
  (`stream(…)`, `crates/core/src/rng.rs:96-98`).

Binding-constraint checks on this placement:

- **`clippy::missing_const_for_fn` (nursery = deny) does NOT fire** — the body's
  first call is `Xoshiro256PlusPlus::seed_from_u64`, a trait method, so the fn is
  not const-eligible and stays a plain `pub fn`.
  `[measured: the scratch crate mirrors gp-core/src/rng.rs's shape under the workspace lint table → `cargo clippy --all-targets` clean, no missing_const_for_fn diagnostic]`
- **Zero new panic-index rows** — `seed_from_u64` + four `next_u64()` into named
  locals + a struct literal; no `unwrap`/`expect`/arithmetic operator/index.
  `[measured: ai-docs/panic-index.md's 6 rows are in gp-render (4), gp-game (1), gp-gen (1); gp-core has none]`
- **Trait-name correction to the spec.** Spec § Technical constraints says
  "`RngCore::next_u64`"; in `rand` 0.10 the trait is **`rand::Rng`** (`RngCore`
  does not exist). The import is `use rand::{Rng, SeedableRng};`.
  `[measured: ~/.cargo/registry/src/*/rand-0.10.2/src/lib.rs:59 → `pub use rand_core::{CryptoRng, Rng, SeedableRng, TryCryptoRng, TryRng};`; ~/.cargo/registry/src/*/rand_core-0.10.1/src/lib.rs:49-55 → `pub trait Rng: TryRng<Error = Infallible>` declaring `fn next_u64(&mut self) -> u64`]`
  The existing `use rand::Rng;` inside that file's test module needs **no**
  change — the added module-level import does not make it a reportable
  redundancy.
  `[measured: the scratch seeds module carries both the module-level `use rand::{Rng, SeedableRng};` and a test-module `use rand::Rng;` → clippy clean]`

#### 3. Raw struct ↔ validated config split

Three types in `crates/game/src/config.rs`, with `clap` confined to the first:

```rust
/// private to the module — never escapes into the mapping logic
#[derive(Debug, Parser)]
#[command(name = "graphite-gp", version)]
struct Cli { /* 13 fields: 9 defaulted + 4 Option<u64> */ }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameConfig {
    pub(crate) race: RaceConfig,      // gp_render::screens::RaceConfig — reused, not redefined
    pub(crate) seeds: Seeds,          // derived from the master, then overridden per field
    pub(crate) min_straight: i32,
    pub(crate) block_size: i32,
    pub(crate) seed_budget: u32,
    pub(crate) repair_budget: u32,
}

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error(transparent)]
    Cli(#[from] clap::Error),
    // lands in SUBTASK 5, with its construction site — see § Decomposition (issue 1)
    #[error("--block-size {block_size} is below the corridor-width floor \
             ceil(cars/2) = {floor} implied by --cars {cars}")]
    BlockSizeBelowWidthFloor { cars: u32, block_size: i32, floor: u32 },
}
```

Entry points — note **every method takes `self` by value**, see the
`wrong_self_convention` finding below:

```rust
pub(crate) fn parse_from<I, T>(args: I) -> Result<GameConfig, ConfigError>
where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone;   // AC1

impl TryFrom<Cli> for GameConfig { type Error = ConfigError; }           // seeds + cross-field check

impl GameConfig {
    pub(crate) const fn to_gen_params(self) -> GenParams;                // FORCED const fn, by value
    pub(crate) const fn temperature(self) -> f32;                        // FORCED const fn, delegates
    pub(crate) fn total_laps(self) -> i32;                               // total conversion, no expect
}

pub(crate) fn render_startup_echo(config: &GameConfig) -> String;        // AC18 — pure, no I/O

impl ConfigError { pub(crate) fn exit(self) -> !; }                      // clap-formatted, non-zero
```

**There is no `#[allow(dead_code)]` anywhere in this design**, in the final state
*and* at every subtask boundary (round 3 fixed the one boundary where that was
false — issue 1). Round 1 proposed an allow on the forward-looking `impl`; the
owner's round-4 ruling replaced it with the startup echo, which is a *production*
consumer of every field and both mapping methods. Confirmed by compiling, not by
reasoning:

`[measured: clapproto2 wires main → render_startup_echo → to_gen_params/temperature and carries NO dead_code allow on GameConfig/ConfigError/Cli/the constants; `cargo clippy --all-targets` is completely clean — no "never read", no "never used". The only allow in that prototype sits on its *stand-in* `GenParams`, an artifact of the prototype having no real gp-gen: in the real tree those fields are read inside the gp-gen library.]`
`[measured: st3check reproduces the subtask-3 slice (single-variant `ConfigError`, no cross-field check, echo v1) → `cargo clippy --all-targets` clean, 3 tests pass natively and under `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test` in 7.49s]`

Liveness chain in the final state: `main` → `parse_from` → `Cli` + all
constants; `main` → `AppShell::new(config.race)`; `main` → `config.total_laps()`;
`main` → `render_startup_echo(&config)` → `to_gen_params()` → `seeds`,
`min_straight`, `block_size`, `seed_budget`, `repair_budget`, and →
`temperature()`; `TryFrom` → `to_gen_params().min_width()` + the
`BlockSizeBelowWidthFloor` construction; `main` → `ConfigError::exit`. The
per-subtask version of this chain is § Decomposition's consumer table.

Other binding constraints, each measured:

- **Build-then-validate.** `TryFrom<Cli>` assembles the `GameConfig`, then reads
  the floor off `config.to_gen_params().min_width()`. This satisfies the spec's
  "⌈cars/2⌉ comes from `GenParams::min_width()`, not a hand-rolled division"
  *and* makes AC8 hold by construction — the object validated is the object
  handed to `gp-gen`.
  `[measured: crates/gen/src/lib.rs:72-74 `pub const fn min_width(&self) -> u32 { self.cars.div_ceil(2) }`; :77-79 `start_finish_width` returns `self.cars`]`
- **`clippy::float_cmp` (pedantic = deny) forbids `assert_eq!` on an `f32`** —
  which is exactly what round 2's AC10 row prescribed, and it would have red-ed
  subtask 3's gate. Compare **bit patterns** instead:
  `assert_eq!(config.temperature().to_bits(), Difficulty::X.temperature().to_bits())`.
  That is both lint-clean and semantically stronger — the design's claim about
  `temperature()` *is* bit-identical delegation, not approximate equality. (An
  epsilon form would also pass — `arithmetic_side_effects` does not lint float
  ops — but it asserts something weaker than the claim.)
  `[measured: `assert_eq!(parse(&[…]).temperature(), expected.temperature())` in st3check produced "error: strict comparison of `f32` or `f64` … `-D clippy::float-cmp` implied by `-D clippy::pedantic`"; switching both sides to `.to_bits()` cleared it and the test passes]`
  **In-repo precedent (GO issue 4), which this design should have cited from the
  start:** `crates/render/src/test_util.rs:15-34` is the workspace's existing
  answer to the same problem — `assert_f32`, "The ONLY float-comparison site in
  the crate", carrying a *declared* `#[allow(clippy::float_cmp, reason = …)]`
  whose reason argues this design's case almost verbatim (values "expected to be
  bit-identical by construction"; "an epsilon would mask real drift"). Two things
  follow. (a) `gp-game` cannot reuse it — the module is `#[cfg(test)]`-gated and
  private to `gp-render` — so a local solution is required, not a lift.
  (b) `to_bits()` is *better* than the precedent under this workspace's
  no-blanket-allow posture, because it needs **no** `#[allow]` at all. It is also
  immune to the trap that file documents: a helper named `*_eq` / `eq_*` makes
  `float_cmp` **silently skip** it, turning a declared suppression into an
  accidental one. If the implementor ever wraps the AC10 comparison in a helper,
  that naming rule carries over — though with `to_bits()` the lint is never
  engaged, so there is nothing to render inert.
  `[measured: crates/render/src/lib.rs:24-25 → `#[cfg(test)] mod test_util;` (private, test-only); crates/render/src/test_util.rs:15-19 → "The ONLY float-comparison site in the crate." + "NOTE: must NOT be named `*_eq` / `eq_*` — `clippy::float_cmp` silently skips such fns, which would make the `#[allow]` below inert and the suppression accidental rather than declared."; :20-34 → the `#[allow(clippy::float_cmp, reason = …)]` on `pub(crate) fn assert_f32`]`
- **`clippy::wrong_self_convention` (pedantic = deny) forces `to_gen_params` to
  take `self` BY VALUE**, because `GameConfig` is `Copy`. `temperature` and
  `total_laps` follow the same by-value shape for uniformity (and it matches
  `gp-render`'s `RaceConfig::temperature(self)`).
  `[measured: `pub(crate) const fn to_gen_params(&self)` produced "warning: methods with the following characteristics: (`to_*` and `self` type is `Copy`) usually take `self` by value"; switching to `self` cleared it]`
- **`to_gen_params` and `temperature` are FORCED `const fn`** by
  `missing_const_for_fn` (nursery = deny): a struct literal over `Copy` fields,
  and a delegation to `Difficulty::temperature`, itself `const fn` and itself
  documented "FORCED `const fn`" (`crates/render/src/screens/mod.rs:64-68`).
  `total_laps` is **not** const-eligible (`i32::try_from` is not const-callable
  on stable), so it stays a plain fn.
  `[measured: a non-const `temperature` produced "error: this could be a `const fn` … missing_const_for_fn"; both are `const fn` in the clean prototypes]`
- **`clippy::match_wildcard_for_single_variants` (pedantic = deny) applies from
  subtask 5 onward, not before.** Once `ConfigError` has two variants, the
  non-`Cli` arm of `exit` and of the `kind` test helper must be written
  `other @ Self::BlockSizeBelowWidthFloor { .. }`, never `other =>`. While the
  enum has a single variant (subtasks 3–4), `match self { Self::Cli(err) =>
  err.exit() }` needs no wildcard at all and trips nothing —
  including `infallible_destructuring_match`, since the arm body is a call and
  not a binding.
  `[measured: `other =>` in `exit` and again in the test helper each produced "error: wildcard matches only a single variant … help: try: `other @ ConfigError::BlockSizeBelowWidthFloor { .. }`" in clapproto2; the single-arm form in st3check is clippy-clean]`
- **An always-`Ok` `TryFrom` does NOT trip `clippy::unnecessary_wraps`** —
  relevant because subtasks 3–4 have no fallible check yet, so `TryFrom<Cli>`
  can stay `TryFrom` from the start rather than churning from `From` to `TryFrom`
  at subtask 5. The lint exempts trait impls, whose signature is not the author's
  to change.
  `[measured: st3check's `impl TryFrom<Cli> for GameConfig` body is a bare `Ok(Self { … })` → clippy clean, no unnecessary_wraps]`
- **`use clap::CommandFactory;` belongs INSIDE `mod tests` at subtask 3**, and
  moves to module scope at subtask 5. Until `exit`'s second arm calls
  `Cli::command()`, the only user is the AC16 test, and a module-level import
  would be an unused import on the non-test target — a `-D warnings` red.
  `[measured: st3check imports `CommandFactory` inside `mod tests` and is clippy-clean on all targets]`
- **`clippy::similar_names` (pedantic = deny) constrains the test helpers.** A
  local named `argv` beside an `args` parameter is rejected; name it `full`.
  `[measured: `let mut argv = vec!["graphite-gp"]; argv.extend_from_slice(args);` produced "error: binding's name is too similar to existing binding" twice; renaming to `full` cleared it]`
- **`clippy::doc_markdown` (pedantic = deny) splits doc prose into two
  registers.** A `Cli` field's `///` *is* its `--help` text, so it must name no
  Rust identifier — backticks would render literally in `--help`. A *constant's*
  `///` never reaches `--help`, so there `l_min` must be backticked.
  `[measured: `/// … the min_straight run-out seed.` and `/// l_min floor` both produced "error: item in documentation is missing backticks"; the identifier-free flag prose plus backticked constant docs are clean, and the rendered help reads "Number of cars on the grid" + "[default: 4]"]`
- **`name = "graphite-gp"` is mandatory** in `#[command(...)]`: the derive
  defaults the command name to `CARGO_PKG_NAME` (`gp-game`), not the binary name
  (`crates/game/Cargo.toml:14`).
  `[measured: the scratch packages are `clapproto2`/`st3check` with bin `graphite-gp-proto`, and `#[command(name = "graphite-gp")]` made the rendered help read `Usage: graphite-gp [OPTIONS]`]`
- **Assertion idiom:** `assert!(matches!(err, …), "…{err:?}")`, `assert_eq!` on
  `ErrorKind`, and `assert_eq!` on `f32::to_bits()` — never `assert_matches!`,
  which nothing in the workspace provides, and never bare `assert_eq!` on a
  float.
  `[measured: rg -U 'assert_matches' --type rust -l → no output; only crates/render has a [dev-dependencies] table]`

#### 4. The startup echo (Scope 8 / AC18)

```rust
/// Renders the resolved configuration for the startup echo (AC18).
pub(crate) fn render_startup_echo(config: &GameConfig) -> String
```

A **pure formatter** — builds a `String`, performs no I/O, so AC18's test never
touches the process or the window. Two lines: a human-readable player-facing
line including the temperature, then the `Debug` rendering of the resolved
`GenParams`, which carries all seven fields *including* the four `Seeds` values:

```text
graphite-gp: cars 6, laps 5, V_target 7, difficulty Pro (temperature 1.00)
graphite-gp: GenParams { cars: 6, min_straight: 3, v_ceiling: 7, block_size: 6, seeds: Seeds { collision: 10201931350592234856, generation: 3780764549115216544, ai_learning: 999, ai_inference: 3237956550421933520 }, seed_budget: 1, repair_budget: 8 }
```

`[measured: clapproto2's `ac18_echo_contains_every_resolved_value` renders exactly these two lines for `--cars 6 --seed 12345 --seed-ai-learning 999` and asserts all eleven labelled forms → passes]`

Design points:

- **`Debug` for the `GenParams` half, deliberately.** `GenParams` derives `Debug`
  (`crates/gen/src/lib.rs:49`) and `Seeds` derives `Debug`
  (`crates/core/src/rng.rs:30`), so a `{params:?}` dump cannot silently omit a
  field — and it auto-follows the deferred `v_ceiling → v_target` rename instead
  of drifting. A hand-written field-by-field renderer was rejected for exactly
  that drift risk.
- **The two halves are separately pinned, which is why AC18 asserts *labelled*
  forms** (issue 4). `Debug`'s `field: value` shape gives `min_straight: 3`,
  `collision: 10201931350592234856`, …, while the player line gives
  `temperature 1.00` — no label appears in both halves, so the assertions cannot
  be satisfied by the wrong line, and a bare `text.contains("3")` vacuity (any
  stray digit inside a 20-digit seed) is impossible.
- **Temperature precision is a named constant** (`TEMPERATURE_DECIMALS: usize =
  2`, used as `{:.*}`), so AC18's assertion is
  `format!("temperature {:.*}", TEMPERATURE_DECIMALS, Difficulty::Pro.temperature())`
  — the *value* still comes from `gp-render`, the single source of truth, so
  AC10's "no temperature literal restated in `gp-game`" holds in the test as well
  as in production.
- **`main` writes it with `let _ = writeln!(std::io::stdout(), …)`, NOT
  `println!`.** `println!` panics on a broken pipe (`graphite-gp | head -0`),
  which would be a new production panic path and an AC14 violation in spirit.
  `clap` itself takes the same precaution in the very code path this design
  already calls.
  `[measured: ~/.cargo/registry/src/*/clap_builder-4.6.0/src/error/mod.rs:245-248 → `pub fn exit(&self) -> ! { // Swallow broken pipe errors  let _ = self.print(); std::process::exit(self.exit_code()) }`]`
  `[measured: both scratch mains use `let _ = writeln!(std::io::stdout(), "{}", config::render_startup_echo(&config));` with `use std::io::Write;` → clippy clean]`
- **stdout, not stderr** — informational output about a successful start, not a
  diagnostic. Emitted **after** validation and **before** `eframe::run_native`,
  so `--help`/`--version`/any rejection never reach it (those all terminate
  inside `ConfigError::exit`).

#### 5. Bounds

`min_straight ∈ [2, 64]` is now **pinned by the amended spec** (Key-decisions
bounds row, AC4, and the narrowed Open-questions entry), not proposed by this
design; round 1's derivation is unchanged and independently re-verified:

`[measured: crates/gen/src/phase1.rs:20 `const MIN_COARSE_STRAIGHT: i32 = 2;`, :23 `const MAX_COARSE_STRAIGHT: i32 = 64;`, :264-266 `fn clamp_l_min(l_min: i32) -> i32 { l_min.clamp(MIN_COARSE_STRAIGHT, MAX_COARSE_STRAIGHT) }` — outside `[2,64]` the generator silently rewrites the value]`
`[measured: under `value_parser!(i32).range(2..=64)`, `--min-straight 2` and `--min-straight 64` parse, while `1` and `65` return `Err(ErrorKind::ValueValidation)` — clapproto2 `ac4_range_kinds`]`

The two ceilings the amended spec still leaves to this design:

| Flag | Domain | Basis |
|---|---|---|
| `--block-size` | `[1, 32]` | Floor `1` because the *real* floor is the cross-field `⌈cars/2⌉ ∈ [1,3]`, and AC5's own example (`--cars 6 --block-size 2`) requires `2` to clear the per-flag range and fail the cross-field check. Ceiling `32` is a **typo/allocation guard, not a performance promise**: Ф2 allocates a fine `Corridor` of roughly `(coarse bbox) × k` cells per axis, so an unbounded `k` is an unbounded allocation. `[measured: crates/gen/src/phase2.rs:52-79 `corridor_for_ring` multiplies each coarse extent by `k` (saturating) and hands the result to `Corridor::new`; `stage1_baseline` then sets `block_points(c, k)` per ring cell]` |
| `--seed-budget` / `--repair-budget` | `[1, 1024]` each | Floors per the spec (`0` is accepted by `GenParams` but makes generation fail immediately). Ceiling `1024` = 16× the heaviest configuration with a *measured* cost in this repo. `[measured: crates/gen/src/generate.rs:299-302 → `#[ignore = "heavy: ~467s debug wall time for a 64-seed/32-repair-budget sweep …"]`]` |

Stated limits, not papered over: these ceilings bound *allocation and typos*,
not wall-clock — nothing in this task can bound generation time because nothing
here calls `generate()` (spec § Out of scope). A cross-field work budget
(`seed_budget × repair_budget ≤ N`) is **rejected** as YAGNI: AC5 names exactly
one cross-field invariant, and #43 — which first runs the pipeline — can
*measure* a work budget instead of guessing one.

#### 6. No flag is hidden from `--help`

AC16 is honoured verbatim: all thirteen flags listed, `hide` used nowhere. The
four seed overrides are exactly the flags a user needs discoverable to reproduce
a run. The nuance the implementor must not trip over: the **nine defaulted**
flags render `[default: …]`; the **four `Option<u64>` overrides render none**,
because the spec's decision is that they default to *absent*. AC16's test
asserts thirteen flag names and exactly nine `[default: ` markers.
`[measured: clapproto2 `ac16_help_and_version` asserts `help.matches("[default: ").count() == 9` → passes]`

### Constants (all in `crates/game/src/config.rs`)

Naming uses the **`<FLAG>_MIN` / `<FLAG>_MAX` suffix** convention rather than
`gp-render`'s `MIN_CARS` prefix, because the `min_straight` flag would otherwise
produce `MIN_MIN_STRAIGHT`. The suffix form is mechanically derivable from every
flag name, including that one.

| Constant | Value | Lands in | Note |
|---|---|---|---|
| `CARS_MIN` / `CARS_MAX` | `2` / `6` (`u32`) | 3 | mirrors `SetupScreen` (AC9) |
| `LAPS_MIN` / `LAPS_MAX` | `1` / `9` (`u32`) | 3 | mirrors `SetupScreen` (AC9) |
| `V_TARGET_MIN` / `V_TARGET_MAX` | `3` / `10` (`i32`) | 3 | mirrors `SetupScreen`'s `f32` slider bounds (AC9) |
| `DEFAULT_CARS` / `DEFAULT_LAPS` / `DEFAULT_V_TARGET` | `4` / `5` / `7` | 3 | mirrors the deleted `STARTUP_CONFIG` |
| `DEFAULT_DIFFICULTY_LABEL` | `"Pro"` | 3 | a `&str`, routed through `parse_difficulty` |
| `TEMPERATURE_DECIMALS` | `2` (`usize`) | 3 | the echo's `{:.*}` precision |
| `DEFAULT_SEED` | `7` (`u64`) | 4 | continuity with `main.rs`'s existing `FIXTURE_SEED: i32 = 7`, the value the Lab header already shows |
| `MIN_STRAIGHT_MIN` / `MIN_STRAIGHT_MAX` | `2` / `64` (`i32`) | 5 | `gp-gen`'s documented `l_min` domain (spec-pinned) |
| `BLOCK_SIZE_MIN` / `BLOCK_SIZE_MAX` | `1` / `32` (`i32`) | 5 | cross-field check owns the real floor |
| `SEED_BUDGET_MIN` / `SEED_BUDGET_MAX` | `1` / `1024` (`u32`) | 5 | |
| `REPAIR_BUDGET_MIN` / `REPAIR_BUDGET_MAX` | `1` / `1024` (`u32`) | 5 | |
| `DEFAULT_MIN_STRAIGHT` / `DEFAULT_BLOCK_SIZE` | `3` / `6` | 5 | `gp-gen`'s proven pair |
| `DEFAULT_SEED_BUDGET` / `DEFAULT_REPAIR_BUDGET` | `1` / `8` | 5 | `gp-gen`'s cheap always-on e2e case |

Each constant lands in the same subtask as the `#[arg]` attribute that reads it —
that is what keeps every boundary `dead_code`-clean (§ Decomposition's table).

`default_value_t = DEFAULT_CARS` and `default_value = DEFAULT_DIFFICULTY_LABEL`
both accept a named const, and
`value_parser!(u32).range(i64::from(CARS_MIN)..=i64::from(CARS_MAX))` types
correctly — `i64::from`, never `as`, because `pedantic` polices casts.
`[measured: the scratch `Cli`s use named consts in `default_value_t` / `default_value` and `i64::from(..)` range bounds for all seven bounded flags → clippy clean, help renders `[default: 4]` / `[default: Pro]` / `[default: 7]`]`
`value_parser!(u32)`/`(i32)` produce `RangedI64ValueParser<T>` whose `range`
takes `RangeBounds<i64>`; `value_parser!(u64)` produces an unrestricted
`RangedU64ValueParser<u64>`, exactly right for the five seed flags (spec: every
`u64` is a valid seed).
`[measured: ~/.cargo/registry/src/*/clap_builder-4.6.0/src/builder/value_parser.rs:2362-2383 and :1327]`

### Difficulty parsing — no restated spellings

`fn parse_difficulty(raw: &str) -> Result<Difficulty, String>` matches `raw`
case-insensitively against `gp_render::screens::DIFFICULTY_LABELS` (`position` +
`eq_ignore_ascii_case`), maps the index through `Difficulty::from_index` (a total
`const fn` returning `Option`), and on failure returns a message naming the
accepted domain. `gp-game` restates **no** label string and **no** temperature
value.

A local `ValueEnum`-deriving mirror enum is **rejected**: `ValueEnum` is `clap`'s
trait and `Difficulty` is `gp-render`'s type, so it cannot be implemented for the
real type (orphan rule), and a mirror would restate the three spellings
`DIFFICULTY_LABELS` owns. `clap` accepts a plain `fn(&str) -> Result<T, E>` as a
`value_parser`, so no mirror is needed.
`[measured: ~/.cargo/registry/src/*/clap_builder-4.6.0/src/builder/value_parser.rs:870-876 → `impl<F, T, E> TypedValueParser for F where F: Fn(&str) -> Result<T, E> + …`; the scratch crates parse `--difficulty ACE` → `Ace`, and `--difficulty wizard` → `Err(ValueValidation)` rendering `error: invalid value 'wizard' for '--difficulty <DIFFICULTY>': expected one of Rookie, Pro, Ace (case-insensitive)`]`

### Seed resolution (normative, per spec)

```rust
let derived = Seeds::from_master(cli.seed);
let seeds = Seeds {
    collision:    cli.seed_collision.unwrap_or(derived.collision),
    generation:   cli.seed_generation.unwrap_or(derived.generation),
    ai_learning:  cli.seed_ai_learning.unwrap_or(derived.ai_learning),
    ai_inference: cli.seed_ai_inference.unwrap_or(derived.ai_inference),
};
```

Per-field, mutually independent, `Option::unwrap_or` (total — not a panic path).
`from_master` binds its four `next_u64()` draws to named locals *before* the
struct literal, so the mapping cannot depend on field-evaluation order.

### AC9's drift guard is a real cross-crate guard, not a tautology

`gp-render`'s bound constants are private, but
**`gp_render::screens::setup::assemble` is `pub`** and clamps with exactly those
constants, so the guard probes them behaviourally instead of restating them:

```rust
assert_eq!(assemble(i32::MIN, 1, 3.0, Difficulty::Pro).cars,  CARS_MIN);
assert_eq!(assemble(i32::MAX, 1, 3.0, Difficulty::Pro).cars,  CARS_MAX);
assert_eq!(assemble(2, i32::MIN, 3.0, Difficulty::Pro).laps,  LAPS_MIN);
assert_eq!(assemble(2, i32::MAX, 3.0, Difficulty::Pro).laps,  LAPS_MAX);
assert_eq!(assemble(2, 1, -1000.0, Difficulty::Pro).v_target, V_TARGET_MIN);
assert_eq!(assemble(2, 1,  1000.0, Difficulty::Pro).v_target, V_TARGET_MAX);
```

A change to `gp-render`'s bounds now **fails a `gp-game` test**. The compared
values are `u32`/`i32`, so `float_cmp` is not in play; the `f32` arguments are
finite sentinels (`±1000.0`), never `NaN` — `f32::clamp`'s NaN behaviour is not
what this guard is about.
`[measured: crates/render/src/screens/setup.rs:216-234 `pub fn assemble(cars: i32, laps: i32, v_target: f32, difficulty: Difficulty) -> RaceConfig` whose body clamps with the private `MIN_CARS`/`MAX_CARS`/`MIN_LAPS`/`MAX_LAPS`/`MIN_V_TARGET`/`MAX_V_TARGET`; `pub mod screens` (crates/render/src/lib.rs:23) and `pub mod setup` (crates/render/src/screens/mod.rs:22) make the path reachable — independently re-verified by the coordinator]`

### `main.rs` rewiring

```rust
fn main() -> eframe::Result {
    let config = match config::parse_from(std::env::args_os()) {
        Ok(config) => config,
        Err(err) => err.exit(),        // `!` coerces; nothing after this runs
    };
    // `let _ = writeln!` not `println!` — see § hand-off #4 (broken-pipe panic)
    let _ = writeln!(std::io::stdout(), "{}", config::render_startup_echo(&config));
    eframe::run_native(/* … */ Box::new(move |cc| { /* … */ Ok(Box::new(GraphiteGpApp::new(config))) }))
}
```

- `STARTUP_CONFIG` is deleted; `GraphiteGpApp::new(config: GameConfig)` passes
  `config.race` to `AppShell::new` — already the call site — and stores
  `total_laps: i32` computed **once** in `new()`.
  `[measured: crates/render/src/app.rs:138 → `pub const fn new(config: RaceConfig) -> Self` (the spec's ":137" citation points one line up, at that fn's `#[must_use]`)]`
- The `i32::try_from(STARTUP_CONFIG.laps).expect(…)` in `eframe::App::ui`
  disappears. `GameConfig::total_laps` uses
  `i32::try_from(self.race.laps).unwrap_or(i32::MAX)` — a *total* form whose
  sentinel is unreachable given the validated `[1, 9]` domain, and the same
  saturating idiom `gp-gen` already uses. This **removes** the one `gp-game` row
  from the panic index rather than relocating it.
  `[measured: ai-docs/panic-index.md row `crates/game/src/main.rs:111` | `i32::try_from(STARTUP_CONFIG.laps).expect("…")`, whose live call sits at crates/game/src/main.rs:119-120; the precedent is crates/gen/src/generate.rs:105 `let phase2_n = i32::try_from(n_u32).unwrap_or(i32::MAX);`]`
- **Deleting `STARTUP_CONFIG` orphans two imports** (GO issue 1). `main.rs:29` is
  `use gp_render::screens::{Difficulty, PhaseStatus, RaceConfig, RaceSummary, StandingEntry};`,
  and after the deletion neither `Difficulty` nor `RaceConfig` is named anywhere
  in the file: their only non-doc uses are the const's type (`:35`) and its
  `difficulty:` field value (`:39`), both of which go away, and
  `AppShell::new(config.race)` names no type. Trim the import to
  `use gp_render::screens::{PhaseStatus, RaceSummary, StandingEntry};` in the same
  subtask, or `-D warnings` reds on two `unused_imports`. The remaining mention at
  `:52` is doc prose in backticks, not an intra-doc link, so the rustdoc gate is
  unaffected either way.
  `[measured: grep -n "Difficulty\|RaceConfig" crates/game/src/main.rs → exactly four hits: :29 (the import), :35 `const STARTUP_CONFIG: RaceConfig = RaceConfig {`, :39 `difficulty: Difficulty::Pro,`, :52 `/// The router owning `Screen`/`RaceConfig`/`Overlays`/`has_generated`.` — independently confirmed by the coordinator]`
- Two doc comments in `main.rs` reference the deleted const and must be reworded
  in the same subtask (they are *caused* by the deletion, not scope creep): the
  `FIXTURE_CAR_COUNT` doc ("only the first 4 are used (`STARTUP_CONFIG.cars`)")
  and the module header's fixture paragraph, which must now say the fixture
  car/track set is fixed at 4 cars **regardless of `--cars`** until #43 wires
  generation. The header's other stale claim (`generate` is a `todo!()`) stays
  for #43 per spec § Deferred.
- `FIXTURE_SEED: i32 = 7` stays: feeding the `u64` master into
  `ShellSession.seed: i32` is explicitly deferred (spec § Deferred).

### Rejected alternatives

| Rejected | Why |
|---|---|
| **`#[allow(dead_code, reason = …)]` on the forward-looking config `impl`** (round 1's choice) | Overridden by the owner's round-4 ruling in favour of the startup echo — and the echo is measurably sufficient in the final state *and* at every boundary once `BlockSizeBelowWidthFloor` moves to subtask 5. Round 3's per-boundary table exists so the implementor never meets a red gate whose easiest escape is reinstating this allow. |
| A hand-written field-by-field `GenParams` renderer for the echo | Can silently omit a field, and would need editing when the deferred `v_ceiling → v_target` rename lands; `{params:?}` cannot omit and auto-follows. |
| `println!` for the echo | Panics on a broken pipe — a new production panic path. `let _ = writeln!(stdout(), …)` is total, and is what `clap`'s own `Error::exit` does. |
| An epsilon comparison for AC10 (`(a - b).abs() < f32::EPSILON`) | Lint-clean, but asserts something weaker than the design's claim. `to_bits()` equality is exact and states the delegation invariant directly — and the workspace has already ruled the same way: `crates/render/src/test_util.rs:20-32`'s declared `#[allow(clippy::float_cmp)]` reason says the compared values are "expected to be bit-identical by construction" and that "an epsilon would mask real drift". `to_bits()` reaches that outcome with no `#[allow]` at all. |
| Lifting `gp-render`'s `assert_f32` helper for AC10 | Not reachable: `mod test_util` is `#[cfg(test)]`-gated **and private** to `gp-render` (`crates/render/src/lib.rs:24-25`), so `gp-game` cannot import it. Making it `pub` to serve one `gp-game` assertion would widen a test-only surface across a crate boundary for less than `to_bits()` already gives. |
| Starting with `From<Cli>` and switching to `TryFrom` at subtask 5 | Unnecessary churn — an always-`Ok` `TryFrom` does not trip `unnecessary_wraps` (measured), so the trait can be `TryFrom` from subtask 3 on. |
| A `gp-game` library target + `tests/` tests for the config module | Buys nothing measurable, and its only unique reach — an end-to-end success test — now both echoes *and* opens a window. |
| A local `CliDifficulty` enum deriving `ValueEnum` | Orphan rule blocks implementing `ValueEnum` for the real `Difficulty`; a mirror restates the spellings `DIFFICULTY_LABELS` owns. |
| `clap` with `default-features = false` | The spec's reason for choosing `clap` is `help`/`usage`/`error-context`/`suggestions`; the default feature set is measured Miri-clean (§ Risks). |
| Hand-rolled per-flag bounds checking instead of `value_parser!(…).range(…)` | `clap`'s range error already names flag + received value + accepted domain — exactly AC6 — at zero code cost. |
| A cross-field `seed_budget × repair_budget` work budget | YAGNI — unmeasurable in this task (no `generate()` call); #43 owns it. |

## Decomposition

**The granularity constraint (restated per-boundary — issues 1 and 5).** Round 2
asserted globally that "every item has a production consumer by its own
subtask's gate" and that "anything finer than M = 6 would strand a field". The
first was **false at subtask 3** and the second was wrong in the other
direction: M = 6 as written stranded `ConfigError::BlockSizeBelowWidthFloor`,
whose only construction site sat in subtask 5.
`[measured: the reviewer's st3 reproducer emits "warning: variant BlockSizeBelowWidthFloor is never constructed" on both the bin and bin-test targets, with rustc noting the derived `Debug` impl "is intentionally ignored during dead code analysis" — a red gate under -D warnings; independently reproduced by the coordinator]`

A global assertion is the wrong instrument, so it is replaced by a table with one
row per item and a per-row tag. The rule the table enforces: **an item and its
first production consumer land in the same subtask** — the sole exception being a
`pub` item of a *library* crate, which is externally reachable and therefore
never `dead_code`.

| Item | Lands in | First production consumer | Same subtask? |
|---|---|---|---|
| `Seeds::from_master` | 2 | `TryFrom<Cli>` (subtask **4**) | **No — and legal.** `[derived → rustc dead_code reachability: a `pub` item of a lib target is externally reachable, so it is never reported dead; the round-2 measurement that a test-only use does not silence `dead_code` applies to bin targets]` |
| `Cli`'s four player flags; `CARS_*`/`LAPS_*`/`V_TARGET_*`/`DEFAULT_CARS`/`DEFAULT_LAPS`/`DEFAULT_V_TARGET`/`DEFAULT_DIFFICULTY_LABEL`; `parse_difficulty` | 3 | the `#[arg]` attributes + `parse_from` (3) | Yes `[measured: st3check clippy-clean]` |
| `ConfigError::Cli`; `ConfigError::exit` | 3 | `parse_from`'s `?` (via `#[from]`) and `main` (3) | Yes `[measured: st3check clippy-clean with the single-variant enum and single-arm `match`]` |
| `GameConfig { race }`; `temperature`; `total_laps`; `render_startup_echo` v1; `TEMPERATURE_DECIMALS` | 3 | `main` → `AppShell::new(config.race)` / `total_laps()` / `writeln!(… render_startup_echo …)`, and echo v1 → `temperature()` (3) | Yes `[measured: st3check clippy-clean]` |
| `parse_from`; `impl TryFrom<Cli> for GameConfig` (GO issue 3 — previously present only in consumer columns, never as rows) | 3 | `main` calls `parse_from`; `parse_from` calls `GameConfig::try_from` (3) | Yes `[measured: st3check clippy-clean, including the always-`Ok` `TryFrom` body]` |
| `Cli`'s five seed flags; `DEFAULT_SEED` | 4 | the `#[arg]` attributes (4) | Yes `[measured: st3check extended to the subtask-4 slice → clippy-clean, and its `ac14_partial_at_subtask_three` now asserts the two `--seed` rows]` |
| `GameConfig.seeds` (+ the resolution) | 4 | echo **v2**, which appends the resolved `Seeds` (4) | Yes `[measured: st3check's `echo_v2_renders_seeds_standalone` asserts the four labelled seed forms against a standalone `{seeds:?}` line → passes, clippy-clean]` |
| `Cli`'s four tuning flags; `MIN_STRAIGHT_*`/`BLOCK_SIZE_*`/`SEED_BUDGET_*`/`REPAIR_BUDGET_*`/`DEFAULT_MIN_STRAIGHT`/`DEFAULT_BLOCK_SIZE`/`DEFAULT_SEED_BUDGET`/`DEFAULT_REPAIR_BUDGET` | 5 | the `#[arg]` attributes (5) | Yes `[measured: clapproto2 clippy-clean]` |
| `GameConfig`'s four tuning fields; `to_gen_params` | 5 | `to_gen_params` reads the fields; echo **v3** and the cross-field check read `to_gen_params()` (5) | Yes `[measured: clapproto2 clippy-clean]` |
| **`ConfigError::BlockSizeBelowWidthFloor`; `exit`'s second arm; module-level `use clap::CommandFactory;`** | **5** (moved from 3 — issue 1) | the cross-field check constructs the variant; `exit`'s arm formats it via `Cli::command()` (5) | Yes `[measured: clapproto2 has both variants, the construction site and the `other @ …` arm, and is clippy-clean]` |
| `crates/game/tests/cli.rs` | 6 | n/a — a test target, outside `dead_code`'s bin-target analysis | n/a |

What the echo contributes is *satisfiability*, not laxity: it is an
**incrementally extensible production consumer**, so each config subtask can
consume the fields it just added by extending `render_startup_echo`. Without it,
subtasks 4 and 5 would each strand their new fields; with it, the only remaining
sequencing rule is the one the table encodes.

Every subtask is TDD-ordered internally: write the `#[cfg(test)]` assertions
first, then the production code that satisfies them, then gate, then commit.
Tests **accrete** across subtasks 3→5 (the defaults, AC14 and echo tests grow as
flags land); they are extended, not rewritten.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Dependency wiring.** Add `clap = { version = "4", features = ["derive"] }` to `[workspace.dependencies]`; add `clap = { workspace = true }` **and** `thiserror = { workspace = true }` to `gp-game` (`thiserror` is a workspace entry already in the lock via `gp-gen`, but **not** yet a `gp-game` dependency). Then `cargo update` → `cargo build` (a dep-graph edge changed, so the lock refresh is required), and check `git diff --stat Cargo.lock` against the 14-crate list in § Risks. | `Cargo.toml`, `crates/game/Cargo.toml`, `Cargo.lock` | — |
| 2 | **`Seeds::from_master` in `gp-core`.** Tests first: AC11's draw-order pinning (build `Xoshiro256PlusPlus::seed_from_u64(M)` in-test, take four `next_u64()` draws, compare the whole `Seeds` value) + distinct-masters pairwise distinctness. Then the helper + its `///` doc, adding `Rng` to the module's `use rand::{…}`. | `crates/core/src/rng.rs` | — |
| 3 | **Config module, player slice + `main.rs` rewiring + echo v1.** `mod config;`; module doc; the subtask-3 constants (see § Constants); `parse_difficulty`; the four player flags on `Cli`; `ConfigError` with **only** the `Cli` variant + `exit` as a single-arm `match`; `parse_from`; `TryFrom<Cli>` (always `Ok` — no cross-field check yet); `temperature`; `total_laps`; `render_startup_echo` **v1** (player line + temperature). `use clap::CommandFactory;` goes **inside `mod tests`**. Rewire `main.rs`: delete `STARTUP_CONFIG`, `GraphiteGpApp::new(config)`, `move` closure, the `let _ = writeln!` echo call, drop the `expect`, reword the two stale doc comments, **and trim `main.rs:29` to `use gp_render::screens::{PhaseStatus, RaceSummary, StandingEntry};` — deleting `STARTUP_CONFIG` orphans BOTH `Difficulty` and `RaceConfig`, and `AppShell::new(config.race)` re-introduces neither, so leaving line 29 as-is is two `unused_imports` and a red gate** (GO issue 1). Tests: AC9, AC4 (player flags), AC6 (clap-rendered messages + kinds), AC10 (via `to_bits()`), AC13 (player defaults), **AC14 (partial — `""`, `-`, repeated `--cars`, `-- stray`, bare `--`)**, AC16 (partial). | `crates/game/src/config.rs` (new), `crates/game/src/main.rs` | 1 |
| 4 | **Seed family + echo v2.** `--seed` + the four `--seed-*` overrides; `DEFAULT_SEED`; the seed resolution; `GameConfig.seeds`; echo **v2** appends the resolved `Seeds`. Tests: AC3 (the five seed flags), AC11 (parse-level clauses), AC12, AC13 (`DEFAULT_SEED`), AC2 (seeds half), **AC14 (final — the two `--seed` rows: `--seed 18446744073709551616` → `ValueValidation`, lone `--seed` → `InvalidValue`)**. | `crates/game/src/config.rs` | 2, 3 |
| 5 | **Tuning family + cross-field error + `GenParams` mapping + echo v3.** The four tuning flags and their constants; `GameConfig`'s four tuning fields; `to_gen_params`; **`ConfigError::BlockSizeBelowWidthFloor` together with the cross-field check that constructs it**, `exit`'s second arm (`other @ Self::BlockSizeBelowWidthFloor { .. }`), and the module-level `use clap::CommandFactory;`; echo **v3** = the full `GenParams` `Debug` line (AC18 complete). Tests: AC1, AC2 (final), AC3 (final), AC4 (tuning flags, incl. `min_straight` 2/64 accept and 1/65 reject), AC5, AC6 (cross-field variant + `Display` substrings), AC7, AC8, AC13 (final), AC16 (all thirteen + nine defaults), AC18 (labelled forms). The `kind` test helper gains its `other @ …` second arm here. | `crates/game/src/config.rs` | 4 |
| 6 | **Process-level exit contract.** New `crates/game/tests/cli.rs` (needs its own `//!` crate doc under `missing_docs = deny`): `--cars 9` → exit code `2`, stderr containing `--cars` / `9` / `2..=6`, and **no** window; `--help` → exit code `0`. Both tests MUST carry `#[cfg_attr(miri, ignore = "<why>")]` **in the same commit**, with a reason naming that test's own cause (spawning the built binary via `std::process::Command` — process spawning is unsupported under Miri), never a sibling's. **Do NOT run Miri locally** — Miri verification is left to CI's `miri-pass` gate. | `crates/game/tests/cli.rs` | 5 |

`M = 6`.

**Not subtasks** (owned by later `/task` steps, recorded so they are not lost):
removing the now-stale `crates/game/src/main.rs` panic-index row is a **Step 9
panic-index-sync obligation**, and it edits `ai-docs/panic-index.md` — an
instructions/harness file whose inclusion in a code subtask would violate the
change-type homogeneity rule. Doc updates (`ai-docs/context-status.md`,
`ai-docs/context.md`, `README.md`) are Step 9.5.

## Handoff plan

Grouping per `.claude/agents/design.md` § Rules → handoff-grouping and
`.claude/skills/task/SKILL.md` Step 8 (item 5, every-group handoff): a
`## Handoff plan` is mandatory for **every `M ≥ 1`** (a); a group holds **at most
10** consecutive subtasks (b); every boundary — including the entry into the
first group — spawns `/context-reset` per
`.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) (c);
the terminal group's size must land in `1..=10` (d); each group is homogeneous by
change-type (e); groups are minimized (f); each group carries its implementor
model + effort mark (g); the default maximum is 4 groups (h).

All six subtasks are the **code** change-type (`*.rs`, `Cargo.toml`,
`Cargo.lock` — no `*.md`, no `.claude/**`, no `ai-docs/**`), and `6 ≤ 10`, so
homogeneity (e) and the size cap (b) are both satisfied by a single group.
Minimization (f) therefore yields **one** group; splitting would be the
least-desirable interleaving fallback with no dependency forcing it. One group is
within the 4-group default (h), so no user gate applies.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent (its `model: sonnet` + `effort: medium` are
  frontmatter-pinned; pass no inline `model=`/effort override, since there is no
  per-invocation `effort` parameter), 1M-token window — subtasks **1–6** (code
  change-type: `crates/core/src/rng.rs`, `crates/game/src/config.rs`,
  `crates/game/src/main.rs`, `crates/game/tests/cli.rs`, `Cargo.toml`,
  `crates/game/Cargo.toml`, `Cargo.lock`). **Terminal group** (6 subtasks;
  within the `1..=10` range).
- **Handoff into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry)
  before subtask 1 — the every-group handoff binds on the **first** group too.
  There is no second group, so there is no inter-group handoff; Group A
  completes Step 8 inside its own `/context-reset` subagent.
- The `design`, `design-review`, `self-review` and `spec-writer` gates stay on
  Opus regardless of Group A's `sonnet` marker.

## Risks

- **`Cargo.lock` delta — the measured 14 new crates** (issue 3). Subtask 1 must
  compare `git diff --stat Cargo.lock` against exactly this closure, because the
  same row instructs it to revert *unrelated* transitive bumps and an
  under-enumerated list would invite reverting legitimate clap-closure crates:
  `clap`, `clap_builder`, `clap_derive`, `clap_lex`, `anstream`, `anstyle`,
  `anstyle-parse`, `anstyle-query`, `anstyle-wincon`, `colorchoice`,
  `is_terminal_polyfill`, `once_cell_polyfill`, `strsim`, `utf8parse`.
  **`heck` is NOT new** — it is already in the lock at 0.5.0 — and
  `proc-macro2` / `quote` / `syn` / `unicode-ident` / `windows-sys` /
  `windows-link` are already present too, so they should produce no new entry.
  **The `syn` case needs both of its lock entries cited, or the conclusion looks
  unsupported** (GO issue 5a): the lock carries `syn` **twice** — `2.0.119` and
  `3.0.3` — and `clap_derive` 4.6.4 requires `syn ^3.0.2`, which the existing
  `3.0.3` entry already satisfies. So "no new `syn` entry" is correct *because of
  the second entry*, not the first; a code-writer who sees two versions and one
  citation would rightly be suspicious.
  `[measured: grep -n '^name = "<crate>"$' Cargo.lock for all 21 names → the 14 above ABSENT; heck PRESENT (line 1795, 0.5.0), proc-macro2 (3091, 1.0.107), quote (3172, 1.0.47), **syn (3836, 2.0.119) AND syn (3847, 3.0.3)**, unicode-ident (4113, 1.0.24), windows-link (4732, 0.2.1), windows-sys (4766, 0.52.0); independently confirmed by the coordinator]`
  `[measured: ~/.cargo/registry/src/*/clap_derive-4.6.4/Cargo.toml:92-94 → `[dependencies.syn]` `version = "3.0.2"` `features = ["full"]` → satisfied by the lock's existing syn 3.0.3]`
  `[measured: crates.io live max_stable_version for clap = 4.6.4 (curl -sS -H "User-Agent: graphite-gp-agent (marat.buharov@gmail.com)" https://crates.io/api/v1/crates/clap | jq -r '.crate.max_stable_version'), so `version = "4"` follows AGENTS.md § Dependency Versions (`x` for `x.y.z`)]`
- **`clap`'s default features could have aborted the Miri gate** (`color` pulls
  `anstream`, and terminal detection is an FFI call). Measured clean: parsing,
  error `Display`, `ErrorKind` inspection, `render_long_help`, the seed
  derivation and the echo formatter all run under Tree Borrows.
  `[measured: MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --bin graphite-gp-proto → clapproto2 "ok. 5 passed" in 26.92s; st3check "ok. 3 passed" in 7.49s]`
- **The new integration-test target could have red-dened Miri** (its target is
  built and collected even though both tests are Miri-ignored). Measured clean in
  the prototype; the real binary additionally links `eframe`/`wgpu`, which is
  already compiled under Miri today as part of `gp-game`'s bin test harness. The
  mitigation is the mandatory per-test `#[cfg_attr(miri, ignore = "…")]` in the
  same commit as the test (§ Test Design); **the workspace Miri command is NOT run
  locally** — with the fresh `clap` edge it must rebuild the tree under the
  interpreter, which blocks the implementor for many minutes with no output and
  reads as a hung subagent.
  `[measured: clapproto, MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test → the tests/cli.rs target reported "2 ignored" with the reason string and exited clean]`
  `[derived → gate: CI's `miri-pass` aggregator on the PR (AGENTS.md § Rust Test Conventions). If CI's Miri cannot build/collect the new target, surface to the orchestrator with the recommendation to drop `tests/cli.rs` — AC15's structural half and AC16's in-crate half still hold — rather than absorbing a red gate.]`
- **Subtask 4 was the one boundary with no prototype; its novel surface is now
  measured** (GO issue 5b). `st3check` pinned subtask 3 and `clapproto2` pins the
  final (subtask-5) shape, leaving subtask 4 — the seed family plus echo **v2** —
  covered only by derivation. The specific unmeasured surface was echo v2's
  **standalone `{seeds:?}` rendering**, since in `clapproto2` the four seeds
  appear only *nested* inside the `GenParams` `Debug` output. `st3check` was
  extended to that slice, so the surface is now measured rather than reasoned
  about; `similar_names` on the `seed`/`seeds` binding pair in `TryFrom`'s body
  was a plausible trigger and does **not** fire.
  `[measured: st3check extended with `Seeds`, `from_master`, the five seed flags, the resolution and echo v2 → `cargo clippy --all-targets` clean; `echo_v2_renders_seeds_standalone` asserts `collision: …`/`generation: …`/`ai_learning: …`/`ai_inference: …` against the rendered line `graphite-gp: Seeds { collision: 10201931350592234856, generation: 3780764549115216544, ai_learning: 999, ai_inference: 3237956550421933520 }`; 4 tests pass]`
  `[derived → gate: subtask 4's own clippy run still governs the full slice — the prototype pins the rendering and the resolution, not the exact final file]`
- **No test may exercise the success path**, which now echoes *and* calls
  `eframe::run_native`. Only the invalid-argument (exit 2) and `--help` (exit 0)
  paths terminate before it; AC18 is pinned on the pure formatter precisely so
  the echo is verified without a process or a window.
  `[measured: clapproto's tests/cli.rs asserts exit code 2 with stderr "error: invalid value '9' for '--cars <CARS>': 9 is not in 2..=6", and exit code 0 for --help]`
- **The echo is a stdout contract.** Anything later parsing `graphite-gp`'s
  stdout (a future harness, a shell pipeline) sees these two lines. It is emitted
  only on the success path, only after validation, and the write is
  broken-pipe-safe (`let _ = writeln!`), never `println!`.
  `[derived → gate: AC18's formatter test, plus the AC15 integration test which asserts the *error* path writes to stderr]`
- **A `-D warnings` gate aborts on the first diagnostic, masking later ones —
  and this design has been bitten three times by it.** **Nine** lint classes are
  in play (GO issue 2 — round 3 said "eight", listed seven names, and double-counted
  a second `dead_code` *instance* as a class while omitting two real classes).
  The accurate enumeration, split by how each was established:

  | Class | Lint owner | How it was established here |
  |---|---|---|
  | `missing_const_for_fn` | clippy (nursery) | **Observed** on a non-const `temperature` |
  | `match_wildcard_for_single_variants` | clippy (pedantic) | **Observed** on `exit`'s and the `kind` helper's second arm |
  | `doc_markdown` | clippy (pedantic) | **Observed** on flag help prose and on a constant's doc |
  | `wrong_self_convention` | clippy (pedantic) | **Observed** on `to_gen_params(&self)` |
  | `similar_names` | clippy (pedantic) | **Observed** on `argv` beside `args` |
  | `float_cmp` | clippy (pedantic) | **Observed** on `assert_eq!` over `f32` (AC10) |
  | `unnecessary_wraps` | clippy (pedantic) | **Probed, negative** — does not fire on an always-`Ok` trait-impl `try_from` |
  | `dead_code` | rustc | **Observed twice** — (a) unread fields/methods before the echo existed, (b) an unconstructed `BlockSizeBelowWidthFloor` at the subtask-3 boundary |
  | `unused_imports` | rustc | **Not observed in any prototype** — the class behind GO issue 1; a prototype cannot reach it (see the cross-crate-gap row below), and it is the class § hand-off #3's `CommandFactory`-inside-`mod tests` bullet was already avoiding for an *added* import |

  Three of these (`dead_code` on the variant, `float_cmp`, `unused_imports` on
  the orphaned imports) surfaced only after an *earlier* class was cleared or
  only under outside review. Re-run the gate after each cleanup pass; a red gate
  mid-group is **not** licence to add `#[allow(dead_code)]` — surface it instead.
  `[measured: successive `cargo clippy --all-targets` runs across clapproto / clapproto2 / st3check each surfaced a different class until the run came back clean; `unnecessary_wraps` was confirmed silent on st3check's always-`Ok` `TryFrom`]`
  `[derived → gate: subtask 3's clippy run, for the one class (`unused_imports`) no prototype can reproduce]`
- **The prototypes cannot see the real cross-crate surface, and subtask 3
  concentrates that gap** (the through-line behind all three late defects). All
  three scratch crates stand in for `gp-render`/`gp-gen` with local mock types, so
  **no lint whose trigger lives in the real cross-crate surface can appear in
  any of them** — `unused_imports` on `gp_render::screens::{…}` being the exact
  case that reached GO review instead of a prototype. Subtask 3 is the only
  subtask that edits `main.rs`, i.e. the only one touching real `gp-render`
  imports, real `AppShell::new`, real `eframe` wiring and the real
  `ShellSession`, so it carries essentially all of this residual risk. Mitigation
  is the same pass-by-pass gate discipline as the row above, applied with extra
  attention at subtask 3: after the `main.rs` edit, re-run
  `cargo clippy --workspace --all-targets -- -D warnings` and read *every*
  diagnostic before moving on.
  `[derived → gate: subtask 3's clippy + `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` run — the first point at which the real gp-render surface is compiled against this design's code]`
- **`--cars` above the fixture car count is a visible inconsistency** until #43:
  the Setup screen will show `--cars 6` while the Race canvas still draws the
  four hand-built fixture cars — and the echo now prints `cars 6` too, making the
  gap *more* visible. In scope to **document** (subtask 3's doc reword), out of
  scope to fix (spec § Out of scope assigns real wiring to #43).
  `[measured: crates/game/src/main.rs:47 `const FIXTURE_CAR_COUNT: usize = 4;` and :223 a literal 4-cell `FIXTURE_CELLS` array, independent of any config]`
- **The shipped defaults remain unproven to generate an accepted track.**
  `DEFAULT_SEED = 7` expands to a *derived* `generation` seed that is neither of
  `gp-gen`'s two evidenced values (6, 9), and the default `v_target = 7` exceeds
  the `v_ceiling = 5` every proven `gp-gen` configuration uses. Recorded in spec
  § Deferred and assigned to #43; AC13 pins values only. The echo makes the
  unproven values *visible*, which mitigates the reporting gap, not the gap
  itself.
  `[measured: crates/gen/src/generate.rs:169-181 `params()` uses `v_ceiling: 5`, `min_straight: 3`, `block_size: 6`; :338-373 the always-on cheap e2e cases use generation seeds 6 and 9 with seed_budget 1 / repair_budget 8]`
- **Cross-field validation prevents a downstream debug-build panic**, which
  raises its value above UX polish: Ф2 asserts the same invariant with
  `debug_assert!(n <= k)`, so an unvalidated `block_size < ⌈cars/2⌉` reaching
  `generate()` in #43 would abort a debug build.
  `[measured: crates/gen/src/phase2.rs:39 `debug_assert!(n <= k, "n (min width) must not exceed k (block size)");`]`
- **File size.** `crates/game/src/config.rs` is projected at ≈260 production
  lines + ≈430 test lines, inside AGENTS.md's soft 500 (excl.) / 800 (incl.
  tests) caps. If the test module pushes the total past 800, split into
  `config/mod.rs` + a submodule rather than trimming coverage.
  `[derived → gate: wc -l crates/game/src/config.rs at subtask 5]`

## Test Design

All in-crate tests live in `crates/game/src/config.rs`'s `#[cfg(test)] mod
tests` (bin-crate unit tests, run by `cargo test --workspace --tests`); the two
process-level tests live in `crates/game/tests/cli.rs`; AC11's draw-order pinning
test lives in `crates/core/src/rng.rs`'s existing test module. No new
dev-dependency — plain loops, `assert_eq!`, `assert!(matches!(…))`.

**Fixtures / helpers** (in `config.rs`'s test module — note the `full` naming,
forced by `clippy::similar_names`):

```rust
fn parse(args: &[&str]) -> GameConfig;      // prepends "graphite-gp", .expect on the happy path
fn parse_err(args: &[&str]) -> ConfigError; // the .expect_err counterpart
fn kind(args: &[&str]) -> clap::error::ErrorKind;  // unwraps ConfigError::Cli
fn rendered(args: &[&str]) -> String;       // parse_err(args).to_string(), for AC6 substrings
```

`kind`'s shape is subtask-dependent: at subtasks 3–4 the enum has one variant, so
`match … { ConfigError::Cli(err) => err.kind() }` is exhaustive and needs no
second arm; at subtask 5 it gains
`other @ ConfigError::BlockSizeBelowWidthFloor { .. } => panic!("…{other:?}")`,
which `clippy::match_wildcard_for_single_variants` requires be written in that
`other @ …` form rather than as a bare wildcard.
`[measured: st3check's single-arm helper is clippy-clean; clapproto2's two-arm helper needed the `other @ …` binding to clear the lint]`

| AC | Location | Entry point | Scenarios |
|---|---|---|---|
| AC1 | `config.rs` | `parse_from` | Every test drives `parse_from` with an explicit `&[&str]` iterator, exercising the iterator-parameter contract throughout. The "never reads `std::env::args` internally" half is **structural**, not a test: the signature takes the args as a parameter and `main` is the sole `env::args_os` caller — a reviewable one-line grep at Step 9. |
| AC2 | `config.rs` | `parse_from([])` | One test naming each of the nine defaults and asserting `seeds == Seeds::from_master(DEFAULT_SEED)` as a whole value. Grows across subtasks 3→5 as flags land. |
| AC3 | `config.rs` | `parse_from` | Thirteen assertions: the nine in-range values appear verbatim in `GameConfig`/`race`; the four overrides appear in their own `Seeds` field. |
| AC4 | `config.rs` | `parse_from` / `kind` | Per bounded flag: lowest accepted, highest accepted, one step below, one step above → `Err(ErrorKind::ValueValidation)`. For `min_straight` explicitly: **2 and 64 accept, 1 and 65 reject**. **`--block-size 1` must be paired with `--cars 2`** so the cross-field floor is 1 — otherwise the per-flag boundary test fails for the wrong reason. `[measured: clapproto2 `ac4_range_kinds` asserts exactly this for `--cars 7`, `--min-straight 1`, `--min-straight 65`, `--min-straight 2`, `--min-straight 64`, `--difficulty wizard` → passes]` |
| AC5 | `config.rs` | `parse_from` | Subtask 5. `--cars 6 --block-size 2` → `Err(BlockSizeBelowWidthFloor { cars: 6, block_size: 2, floor: 3 })`; `--cars 6 --block-size 3` → `Ok`. `[measured: clapproto2 `ac5_cross_field` → passes]` |
| AC6 | `config.rs` | `parse_from` + `Display` | clap-sourced (unknown flag, missing value, unparseable value, out-of-range value, bad difficulty): assert the variant is `ConfigError::Cli(_)`, assert the `ErrorKind`, **and** assert the rendered string contains the flag, the received value and the domain (`"2..=6"` for ranges, `"Rookie, Pro, Ace"` for difficulty). Cross-field (subtask 5): variant match + rendered string contains `"--block-size"`, `"2"`, `"3"`, `"--cars"`, `"6"`. `[measured: the rendered forms are `error: invalid value '7' for '--cars <CARS>': 7 is not in 2..=6` and `error: invalid value 'wizard' for '--difficulty <DIFFICULTY>': expected one of Rookie, Pro, Ace (case-insensitive)`]` |
| AC7 | `config.rs` | `to_gen_params` | All seven fields asserted individually for a fully-specified config — `GenParams` has no `PartialEq` (it derives `Clone, Copy, Debug` only). `[measured: crates/gen/src/lib.rs:49]` |
| AC8 | `config.rs` | `to_gen_params` | Loop `cars in CARS_MIN..=CARS_MAX`: `block_size >= min_width()` and `start_finish_width() == cars`, with `--block-size` at its default (6 ≥ 3 for every accepted `cars`). |
| AC9 | `config.rs` | `gp_render::screens::setup::assemble` | The six behavioural probes in § *AC9's drift guard*. |
| AC10 | `config.rs` | `GameConfig::temperature` | All three difficulties, looping over `DIFFICULTY_LABELS` zipped with `Difficulty::from_index`: `assert_eq!(parse(&["--difficulty", label]).temperature().to_bits(), expected.temperature().to_bits())` — **`to_bits()` on both sides**, because `clippy::float_cmp` (pedantic = deny) rejects `assert_eq!` on an `f32`. Compared against the delegate, so no temperature literal is restated. The workspace's existing float-comparison site (`crates/render/src/test_util.rs:15-34`) reaches the same judgement via a declared `#[allow(clippy::float_cmp)]`; it is `#[cfg(test)]`-private to `gp-render` and so unusable here, and `to_bits()` needs no allow — see § hand-off #3. `[measured: the bare-`f32` form produced "error: strict comparison of `f32` or `f64` … float_cmp" in st3check; the `to_bits()` form is clean and passes]` |
| AC11 | `crates/core/src/rng.rs` | `Seeds::from_master` | Draw-order pinning with expectations **computed in-test** from a fresh `Xoshiro256PlusPlus` (no magic literals; a reordered assignment fails) + two distinct masters give pairwise-distinct fields. `[measured: `Seeds::from_master(42)` = { collision: 15021278609987233951, generation: 5881210131331364753, ai_learning: 18149643915985481100, ai_inference: 12933668939759105464 } — recorded as evidence the derivation is well-defined, NOT to be hard-coded]` |
| AC11 (parse level) | `config.rs` | `parse_from` | Same master via two independent parses → identical `Seeds`; two distinct masters → four pairwise-distinct field values. |
| AC12 | `config.rs` | `parse_from` | Per override: `--seed M --seed-<src> V` → that field `== V`, the other three `== Seeds::from_master(M)`'s. All four at once → `Seeds` independent of `M` (equal for two different masters). An override with `--seed` omitted → the other three derive from `DEFAULT_SEED`. |
| AC13 | `config.rs` | the constants | One test asserting each default constant equals its documented literal, growing across subtasks 3→5. **No `gp_gen::generate()` call** (spec § Deferred). |
| AC14 **(partial, subtask 3)** | `config.rs` | `kind` / `parse` | The five rows that need no `--seed`: `""` → `UnknownArgument`; `-` → `UnknownArgument`; `--cars 4 --cars 5` → `ArgumentConflict`; `-- stray` → `UnknownArgument`; and the one exception, bare `--` → `Ok`, asserted as `parse(&["--"]) == parse(&[])`. `[measured: st3check `ac14_partial_at_subtask_three` asserts all five at the subtask-3 slice → passes; the same run confirms `--seed 1` is `UnknownArgument` there, i.e. the two seed rows genuinely cannot be asserted yet]` |
| AC14 **(final, subtask 4)** | `config.rs` | `kind` | The two rows the `--seed` flag unlocks: `--seed 18446744073709551616` (2⁶⁴) → `ValueValidation`; a lone `--seed` with no value → `InvalidValue`. `[measured: clapproto2 `ac14_pathological_inputs_pinned_by_kind` asserts all seven rows with `assert_eq!` on `err.kind()` → passes, confirming the coordinator's independently-reproduced table including `-- stray`; st3check re-asserts these two at the subtask-4 slice specifically, where the flag first exists]` |
| AC15 | `crates/game/tests/cli.rs` | the built binary | `--cars 9` → exit code 2, stderr contains `2..=6`, no window; `--help` → exit code 0. Both Miri-ignored. The structural half (parse + echo precede `run_native`) is a review item, not a test. |
| AC16 | `config.rs` + `tests/cli.rs` | `Cli::command()` / the built binary | Subtask 3 (partial): the four player flags appear in `render_long_help()`, and `get_version() == Some(env!("CARGO_PKG_VERSION"))`. Subtask 5 (final): all thirteen `--flag` names and **exactly nine** `[default: ` markers. Process level: `--help` exits 0. `[measured: st3check `ac16_partial_help_and_version` and clapproto2 `ac16_help_and_version` → both pass]` |
| AC17 | — | gates | `/task` Step 9's verify list, plus subtask 1's `git diff --stat Cargo.lock` check against the 14-crate list. |
| AC18 | `config.rs` | `render_startup_echo` | Subtask 5. For a known config (`--cars 6 --seed 12345 --seed-ai-learning 999`), assert the rendered `String` contains eleven **labelled** forms — never bare values (issue 4): `format!("cars: {}", params.cars)`, and likewise `min_straight: `, `v_ceiling: `, `block_size: `, `seed_budget: `, `repair_budget: `, `collision: `, `generation: `, `ai_learning: `, `ai_inference: ` (all guaranteed by the derived `Debug`), plus `format!("temperature {:.*}", TEMPERATURE_DECIMALS, Difficulty::Pro.temperature())` from the player line. The labels partition the two lines, so no assertion can be satisfied by the wrong half or by a stray digit inside a 20-digit seed. Pure function: no process, no window. `[measured: clapproto2 `ac18_echo_contains_every_resolved_value` asserts all eleven labelled forms → passes; a negative control in the same test confirms the `min_straight: …` and `collision: …` needles are NOT satisfied by the player-facing line alone, so dropping the `{params:?}` half would fail the test]` |

**Miri annotations required in this task** (AGENTS.md § Rust Test Conventions —
same commit as the test): the two `tests/cli.rs` tests get
`#[cfg_attr(miri, ignore = "spawns the built binary via std::process::Command; process spawning is unsupported under Miri")]`.
No other new test needs one — every in-crate test is pure computation and was
measured green under Tree Borrows.

## Open questions

Settled by the round-4 spec amendments and recorded here so the trail stays
readable, **removed from the open list**: AC14's `--` exception (amendment 1 —
this design's round-1 finding accepted and independently reproduced), the
`min_straight ∈ [2,64]` domain (amendment 2 — confirmed), and the startup echo in
place of a `dead_code` allow (amendment 3 — reworked in round 2 and, after the
Step-7 review, corrected at the subtask-3 boundary in round 3).

Still open:

- **`--seed-budget` sits inside the `--seed-*` namespace.** `--seed-budget` and
  `--seed-generation` read as one family although only the latter is a seed
  override. This design keeps the spec's flag names verbatim; a rename to
  `--generation-budget` or `--seeds-<source>` remains a one-attribute change if
  the collision grates in use.
- **Whether the tuning and override flags should be hidden from `--help`.** This
  design lists all thirteen (AC16 as written). `hide = true` per flag is the
  cheap alternative if a thirteen-flag help page proves noisy in practice.
