# gp-gen generate() pipeline orchestration

**Source:** issue #34
**Date:** 2026-07-24
**Tracked in:** #34

## Scope

Implement `gp_gen::generate` — the Block-1 capstone that wires the already-landed
phases Ф1→Ф7 into the outer generation loop described in `docs/design.md` §2
(`generate_track` pseudocode). Every phase function already exists and is unit
tested; the task's core is the orchestration + `TrackArtifact` assembly. The
current stub is `pub fn generate(_params: GenParams) -> TrackArtifact { todo!(...) }`
in `crates/gen/src/lib.rs`.

Two amendments have widened this beyond pure orchestration. **Amendment 1**
(superseded) added a Ф7 empty-centerline fix after `generate()` was found to
accept correct tracks whose `centerline` was empty on 7/7 accepted draws.
**Amendment 2** (current, product-owner-approved) replaces amendment 1's
diagnosis — measurement refuted the `MAX_BRIDGE_GAP` explanation — and moves the
fix to its true source, `gp_core::geom::medial_axis` (Scope items 7–8, AC8–AC9).

In scope:

1. **Seed-budget outer loop (N4).** A bounded `repeat seed_budget times` loop —
   never an infinite loop. Each iteration draws a fresh track from the *same*
   continuing RNG stream (see Technical constraints — determinism), runs Ф1→Ф3,
   then the repair loop.
2. **Repair-budget inner loop.** A bounded `repeat repair_budget times` loop
   that each iteration runs the cheap checks and, only when green, the expensive
   oracle; feeds issues to Ф6 (`phase6_local_repair`); breaks out to reseed when
   Ф6 makes no progress (`RepairOutcome::Failed`).
3. **Cheap-then-expensive gating.** Every repair iteration runs Ф4 static checks
   (`phase4_static_checks`) plus the V=1 liveness (`oracle_liveness_v1`) first;
   the expensive Vmax oracle (`phase5_full_oracle`) runs *only* when the cheap
   checks are clean.
4. **Oracle routing.** On `OracleResult::NotLappable { stall_walls }`, route the
   stall diagnostic to Ф6. On `OracleResult::Lappable(metrics)`, run the run-out
   budget check (`phase5_runout_checks`, which needs the oracle metrics); if it
   yields `NoBraking` issues, route them to Ф6; if it is clean, the track is
   accepted and assembled.
5. **`GENERATION_FAILED` return path.** When the seed budget is spent without an
   accepted track, return the generation-failure value (no panic, no infinite
   loop).
6. **Ф7 artifact assembly.** On success, build and return a fully populated,
   self-consistent `TrackArtifact` (all fields — see AC4).
7. **`gp_core::geom::medial_axis` connected-ridge fix (amendment 2).** Change
   `medial_axis` (`crates/core/src/geom/distance.rs:112`) so it returns a
   **connected, thin (≈1-cell) ridge** on wide corridors: morphological thinning
   (or an equivalent skeletonisation) replacing or augmenting the current strict
   axis-local-max test, so a corridor's medial set is a connected skeleton
   rather than scattered singleton cells. `crates/core/src/geom/distance.rs` is
   explicitly IN SCOPE for #34. This supersedes amendment 1, whose stated root
   cause (`MAX_BRIDGE_GAP = 6` being annulus-fixture-tuned) was **measured false**
   — see Technical constraints for the evidence table. The four exact-output
   `medial_axis` unit tests in `distance.rs` are to be **updated** to the new
   correct expected output; that rewrite is sanctioned, not a regression (see
   Key decisions and AC9). `medial_axis`'s own rustdoc — which currently
   documents the strict-local-max definition and defers thinning/bridging to Ф7
   — must be rewritten to state the new contract in the same change.
8. **Ф7 `racing_line` keeps its pipeline (amendment 2).** `racing_line`
   (`crates/gen/src/phase7.rs`, issue #33) retains its existing
   medial → bridge → prune → walk → orient → resample pipeline. With a connected
   ridge from the fixed `medial_axis`, `bridge_gaps` is expected to succeed and
   the centerline to come out non-empty on real `generate()`-produced corridors.
   Minor `phase7.rs` adjustments are permitted where the connected ridge demands
   them (including revisiting `MAX_BRIDGE_GAP`), but **no wholesale Ф7 redesign**.
   Where `crates/gen/src/phase7_tests.rs` fixture expectations encode the OLD
   `medial_axis` output, updating them is sanctioned. `phase7.rs`'s module doc —
   which asserts `medial_axis` "deliberately leaves a *thin but imperfect* ridge
   cell set" and that repairing it is `racing_line`'s job — is superseded by the
   new connected-ridge behaviour and must be corrected in the same change. The
   centerline remains render-only and is NOT part of the block1→block4 AI
   contract.

## Out of scope

- Any change to a Ф1–Ф6 phase function's behaviour or signature (they are landed
  and tested; treat them as fixed dependencies). Purely additive signature/return
  changes to `generate` itself are expected and fine. **Amendment-2 carve-out:**
  the `gp_core::geom::medial_axis` connected-ridge fix (Scope item 7) and the
  bounded `phase7.rs` follow-through (Scope item 8) ARE in-scope for #34; all
  Ф1–Ф6 phase functions remain fixed and out-of-scope. Ф4 consumes
  `DistanceTransform` but **not** `medial_axis` (`phase4.rs` / `phase4_defects.rs`
  reference it in doc-comments only), so its behaviour is unaffected.
- Redesigning Ф7 — no wholesale centerline-algorithm rework, no new render
  behaviour. The Ф7 change is limited to whatever the connected ridge makes
  necessary.
- Any other `gp_core::geom` primitive. `DistanceTransform::compute`,
  `component_count`, `flood_fill`, `geodesic_layers`, `walls_from_boundary` etc.
  keep their current behaviour; only `medial_axis` changes.
- Tuning the *quality* of generated tracks (aesthetics, difficulty balancing).
- Wiring `generate()` into `gp-game` / `gp-render` (they still use hand-built
  fixtures; the live game-screen turn loop is Block 3b, deferred).
- Re-enabling the `gp-gen` Miri gate (rides the #134 cost carve-out).

## Deferred

- Default numeric values for `seed_budget` / `repair_budget` beyond a defensible
  starting point | tuning needs real multi-seed generation data | no — tune in
  place once generation runs end-to-end.
- Diagnostic payload on the failure value (which seeds/issues were tried) | the
  design-doc sentinel `GENERATION_FAILED` carries no data; add later if a caller
  needs it | no.

## Key decisions

| Question | Decision |
|---|---|
| Return type (must express `GENERATION_FAILED`; current stub is infallible `-> TrackArtifact`) | `generate(params: GenParams) -> Result<TrackArtifact, GenerationError>`, a clean break of the stub signature. `GenerationError` is a new `thiserror` type per AGENTS.md § Code Style (thiserror for new error enum/struct); its sole variant represents seed-budget exhaustion (`SeedBudgetExhausted` / the `GENERATION_FAILED` sentinel). No callers exist yet (`generate` is `todo!`), so the break costs nothing. Design may choose `Option<TrackArtifact>` instead **only** with an explicit Design Amendment; the thiserror `Result` is the convention default. |
| Where `seed_budget` / `repair_budget` live | Add `seed_budget: u32` and `repair_budget: u32` fields to `GenParams`. Rationale: `GenParams` already folds every other `generate_track` pseudocode argument (`m`=`cars`, `k`=`block_size`, `L_min`=`min_straight`, `V`=`v_ceiling`, `rng`=`seeds`); the two budgets are the remaining `generate_track` arguments and belong in the same struct. This is also what makes the `GENERATION_FAILED` test controllable (set `seed_budget = 0`). Module-level `const` budgets are rejected — they are not test-settable. |
| `v_target` source for Ф3 / run-out | Pass `params.v_ceiling` as the `v_target` argument to `phase3_start_finish` and `phase5_runout_checks` (the pseudocode's `V_target`). `GenParams` carries a single speed input; `phase5_full_oracle` grows its own internal `V_ceil` from 1 and takes no ceiling argument. |
| Run-out (`NoBraking`) integration | On a `Lappable` oracle result, run `phase5_runout_checks(d, metrics, v_target)` before accepting; non-empty `NoBraking` issues are routed to Ф6 like any other issue. `NoBraking` is a run-out *budget* check (per-corner accel-zone rule), not a lappability verdict (context-status; #30 AC7 proof — the oracle never reports a dynamic-only stall). Acceptance requires both `Lappable` **and** an empty run-out check. |
| `RepairContext` assembly | Ф6 is driven via `phase6_local_repair(ctx: &RepairContext, issues: &[Issue])`. `generate` populates `RepairContext` each repair iteration: `d, skel, k, n, m, grid, sf, race_dir, v_target`, plus `metrics: Some(&TrackMetrics)` when the last oracle run was `Lappable` and `stall_walls: Some(&[Wall])` when it was `NotLappable` (mutually exclusive per iteration). |
| `s_field` builder | `SField::from_gate_bfs(&d, &sf.gate)` (gp-core; the gate-cut monotone BFS distance field). |
| `centerline` builder | `racing_line(&d, &sf.gate, race_dir)` (Ф7, `phase7.rs`; render-only, total, never panics). |
| `walls` builder | `gp_core::geom::walls_from_boundary(&d)`. |
| **Q-Ф7 — where to fix the empty centerline** (amendment 2, product-owner decision) | **Option A1: fix at the source, in gp-core.** `medial_axis` is changed to emit a connected thin ridge; `racing_line` keeps its pipeline. Rejected alternatives: bumping/deriving `MAX_BRIDGE_GAP` (refuted — the binding constraint is component *count*, not gap *width*: min cross-component gap measured 2–3, always ≤ `MAX_BRIDGE_GAP = 6`), and reconstructing a skeleton inside `racing_line` (duplicates a geometry primitive into `gp-gen` and leaves the gp-core primitive wrong for every future consumer). |
| Are the `medial_axis` exact-output test rewrites a regression? | **No — sanctioned.** The four tests `medial_axis_is_thin_centerline_on_straight_band`, `medial_axis_includes_neck_and_is_connected_across_it`, `medial_axis_forms_four_connected_strips_on_annulus`, `medial_axis_even_width_band_is_two_cell` pin the *old* strict-local-max output and MUST be updated to the new correct expected sets. They are the specification of the behaviour being deliberately replaced. Renaming them to match the new behaviour is permitted (e.g. the annulus test's "four connected strips" name is expected to become inaccurate once the ridge is one loop); AC9 is satisfied by the equivalent updated test regardless of its name, but no such test may be deleted or `#[ignore]`d. Two sibling tests are **invariants and must stay green unchanged**: `empty_corridor_has_zero_dt_and_empty_medial_axis` (totality on empty input) and `compute_and_medial_axis_are_deterministic` (determinism). |
| Blast radius of the `medial_axis` change (verified) | The **only** production consumer is `racing_line` (`crates/gen/src/phase7.rs:509`). `crates/gen/src/phase4.rs:8` and `crates/gen/src/phase4_defects.rs:109` mention it in doc-comments only; `crates/core/src/geom/mod.rs:13` is a re-export listing. No other crate consumes it. Test consumers: `crates/core/src/geom/distance.rs` unit tests and `crates/gen/src/phase7_tests.rs:32`. |
| Thinning algorithm choice | Left to the design phase — the spec-level requirement is the *property* (connected, ≈1-cell-thin, deterministic, integer-only, total on empty input), not a named algorithm. Any deterministic thinning that satisfies AC9 is acceptable. |

## Technical constraints

- **Determinism (replay).** Construct the RNG *once* via `params.generation_rng()`
  and thread a single `&mut Xoshiro256PlusPlus` through every seed-budget
  iteration. Each iteration's `phase1_coarse_ring(l_min, &mut rng)` advances the
  same stream, so "reseed on repair exhaustion" means *draw the next track from
  the continuing stream*, not re-seed with a new seed value. A fixed
  `seeds.generation` MUST yield a byte-identical `TrackArtifact` on every run
  (AGENTS.md: `gp-core` physics is deterministic; #49 single-RNG-path contract).
- **No floats / no non-determinism in the loop control.** The orchestration is
  integer-and-enum control flow over deterministic phases; do not introduce
  wall-clock, OS entropy, or `HashMap`-iteration-order dependence into the
  accept/reject decision.
- **Zero production panics.** `generate` must be total — no `unwrap`/`expect`/
  index-panic on any budget or geometry. Budget exhaustion is a value
  (`GenerationError`), not a panic. (`gp-gen` panic-index discipline.)
- **Phase signatures are fixed inputs.** Wire to the *actual* landed signatures,
  which differ from the §2 pseudocode in argument lists:
  `phase1_coarse_ring(l_min, &mut rng) -> CoarseSkeleton`;
  `phase2_rasterize(&skel, k, n) -> Corridor`;
  `phase3_start_finish(d, &skel, m, v_target) -> Phase3Output { d, sf, grid }`;
  `phase4_static_checks(&d, &skel, k, n, m, &sf) -> Vec<Issue>`;
  `oracle_liveness_v1(&d, &grid, &sf, race_dir) -> bool`;
  `phase5_full_oracle(&d, &grid, &sf, race_dir) -> OracleResult`;
  `phase5_runout_checks(&d, &metrics, v_target) -> Vec<Issue>`;
  `phase6_local_repair(&ctx, &issues) -> RepairOutcome`.
  Note `n` is `u32` (`GenParams::min_width()`), `m` is `u32`
  (`GenParams::start_finish_width()` = `cars`), `k`/`l_min`/`v_target` are `i32`.
- **`width_min` is a required artifact field.** The `TrackArtifact.width_min: u32`
  field (a Ф4 geometry output, consumed by the Lab screen — context-status #20)
  must be populated with the accepted corridor's measured minimum cross-section
  width. The design phase determines the exact producer (it derives from the same
  Ф4 / `DistanceTransform` machinery `phase4_static_checks` already runs); the
  spec-level requirement is only that the field is real, not a placeholder.
- **Ф7 empty-centerline defect — the VERIFIED diagnosis (amendment 2).**
  During Step-8 implementation `generate()` was found to accept valid,
  deterministic, correct tracks while `racing_line` returned an EMPTY centerline
  on **7/7** accepted generated corridors — systemic, not a fixture accident.
  Amendment 1 blamed `MAX_BRIDGE_GAP = 6` (`crates/gen/src/phase7.rs:28`) being
  annulus-fixture-tuned. **That is measured FALSE and is superseded.** An
  independent public-API probe (run twice, independently) over real
  `generate()`-produced corridors measured:

  | config | medial cells | 4-conn components | min cross-component gap | dt_peak | centerline |
  |---|---|---|---|---|---|
  | bs=6 seed=6 | 40 | 40 (all singletons) | 3 | 15 | 0 |
  | bs=6 seed=9 | 63 | 63 (all singletons) | 3 | 21 | 0 |
  | bs=7 seed=0 | 229 | 84 (76 singleton) | 2 | 18 | 0 |
  | bs=7 seed=10 | 141 | 44 (38 singleton) | 2 | 14 | 0 |

  The minimum cross-component gap is **2–3, never above 6** — so
  `MAX_BRIDGE_GAP = 6` is NOT the binding constraint. The real defect is that
  `gp_core::geom::medial_axis` (`crates/core/src/geom/distance.rs:112`) uses a
  strict axis-local-max ridge test that **shatters into 40–84 disconnected
  components** on the wide corridors real generation produces (`dt_peak` 14–21,
  i.e. bands well over 20 cells across). `bridge_gaps` therefore cannot assemble
  one loop from dozens of singletons, and `racing_line` falls back to
  `Centerline::default()` (empty). The fix must attack the fragmentation, not
  the gap bound.
- **`medial_axis` invariants that survive the change.** Determinism
  (`BTreeSet<Point>`, identical output for identical input — pinned by
  `compute_and_medial_axis_are_deterministic`) and totality on empty input
  (`empty_corridor_has_zero_dt_and_empty_medial_axis`) are contract invariants;
  both tests must stay green **unchanged**. `medial_axis` stays integer-only,
  panic-free, and `BTreeSet`-ordered (`gp-core` determinism rule, AGENTS.md
  § Code Style / `docs/design.md` §3a).
- **Miri gate (amendment 2).** `gp-core` **is** inside the Miri gate — only
  `gp-gen` rides the #134 cost carve-out (`.github/workflows/ci.yml:193`:
  `cargo miri test --workspace --exclude gp-gen`). Because the thinning change
  lands in `gp-core`, `MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace
  --exclude gp-gen` must stay green; a red Miri blocks merge (AGENTS.md
  § Rust Test Conventions, #76). Reproduce with the workspace command, never a
  narrower `-p` run.
- **Doc reconciliation is part of the change (amendment 2).** Three doc surfaces
  currently state the superseded contract and must be corrected in the same
  change: (a) `medial_axis`'s own rustdoc in `distance.rs` (the strict-local-max
  definition, the "strict inequality is load-bearing" rationale, and the closing
  paragraph deferring thinning + corner-bridging to Ф7); (b) `phase7.rs`'s module
  doc (`medial_axis` "deliberately leaves a *thin but imperfect* ridge cell
  set"); (c) `MAX_BRIDGE_GAP`'s doc-comment if the constant is touched. The doc
  gate (`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`) must stay
  green.
- **`Centerline` type note.** `Centerline` (`crates/core/src/track.rs`) is the
  render-only arc-length loop type
  (`samples: Vec<CenterlineSample>`, `CenterlineSample.s: f32`); its `s`
  is an existing render-only parameter, unrelated to the `gp-core` physics
  integer-only rule.
- **File-size discipline.** `crates/gen/src/lib.rs` currently hosts `GenParams` +
  the stub. If the orchestration + its tests push the file past the soft
  500/800 limits, split the loop into a dedicated module (e.g. `generate.rs`,
  `pub use`d) — the design phase owns the file layout.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The cheap checks (`phase4_static_checks` + `oracle_liveness_v1`) run every repair iteration; `phase5_full_oracle` is invoked only when the cheap checks are clean (green). A test observes the oracle is *not* consulted while static/liveness issues remain. |
| AC2 | Oracle `NotLappable.stall_walls` are routed into the `RepairContext.stall_walls` fed to Ф6; a global reseed (advancing to the next seed-budget iteration) happens only when `phase6_local_repair` returns `RepairOutcome::Failed` (repair-budget/progress exhaustion), never while Ф6 is still committing edits. |
| AC3 | `generate` returns the `GENERATION_FAILED` value (`Err(GenerationError::SeedBudgetExhausted)`) after the seed budget is exhausted, with no infinite loop. A test with `seed_budget = 0` (and/or a pathological budget) returns the failure value promptly. |
| AC4 | On success `generate` returns a `TrackArtifact` with **all** contract fields populated and self-consistent: `corridor`, `walls` (from boundary), `sf` (gate ahead of the start grid), `race_dir`, `s_field` (monotone `0..L`, single hole / gate-cut ring), `start_grid` (distinct positions, all behind `sf`), `centerline` (**NON-EMPTY and well-formed**: `centerline.samples` non-empty, one closed loop, monotone `s`, `samples[0].s == 0`), `metrics` (from the accepting oracle run), `width_min` (≥ `n = ⌈m/2⌉`). |
| AC5 | Seeded end-to-end determinism via TWO e2e tests. **(a) Heavy** (`#[ignore]`, run manually/nightly — too oracle-heavy for every CI run): a larger-budget config runs `generate` twice on identical `GenParams`, asserts identical results and the full artifact invariants — exactly one bounded hole in the complement; S/F chord width ≥ `m`; a lap exists / `oracle_liveness_v1` holds on the returned corridor+grid+sf; `s_field` monotone along `race_dir`; **and a non-empty, well-formed `centerline` per AC4**. **(b) Cheap** (default suite, runs in EVERY CI run): the smallest `(block_size, seed, seed_budget, repair_budget)` that reliably accepts on an early draw with a non-empty centerline (probe data: `bs=6 seed=6` accepts ~3s release at `seed_budget=1`; `bs=5` never accepts). The cheap test asserts acceptance, determinism (two runs identical), and a **NON-EMPTY** `centerline` — at minimal oracle cost. |
| AC6 | On a `Lappable` oracle result, `phase5_runout_checks` runs against the oracle metrics; a track is accepted (assembled and returned) only when the run-out check is empty. Non-empty `NoBraking` issues are routed to Ф6 (a repair iteration), not silently accepted. |
| AC7 | `generate` is total (zero production panics on any `GenParams`), integer/enum-deterministic, and adds no new dependency. `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and the doc gate stay green; the new loop logic carries a `#[cfg(test)] mod tests`. |
| AC8 | The fix yields a non-empty centerline on generated corridors, covered by a regression test that runs a **real `generate()`-produced corridor** (not just the hand-built annulus fixture) through `racing_line` and asserts `centerline.samples` is non-empty and well-formed. The `phase7` unit tests (`phase7_tests.rs`) all pass; where a fixture expectation encodes the OLD `medial_axis` output it is updated to the new correct value (sanctioned per Scope item 8) — every other `phase7` test stays green unmodified, and no `phase7` test is deleted or `#[ignore]`d to make the suite pass. |
| AC9 | **`medial_axis` returns a CONNECTED thin ridge.** A new `gp-core` unit test builds a wide-corridor fixture whose band is wide enough to have shattered under the old strict-local-max test (a corridor with `dt` peak comparable to the measured 14–21, i.e. a band over ~20 cells across, not the 3-cell annulus frame) and asserts the resulting medial set is a **single 4-connected component** (`component_count == 1` over the medial cells) and **thin** (≈1 cell — no 2×2 block of medial cells). The four exact-output tests (`medial_axis_is_thin_centerline_on_straight_band`, `medial_axis_includes_neck_and_is_connected_across_it`, `medial_axis_forms_four_connected_strips_on_annulus`, `medial_axis_even_width_band_is_two_cell`) pass against their updated expected outputs; `empty_corridor_has_zero_dt_and_empty_medial_axis` and `compute_and_medial_axis_are_deterministic` pass **unmodified**; and `MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace --exclude gp-gen` is green. |

## Open questions

- **Default budget magnitudes.** What `seed_budget` / `repair_budget` defaults
  best balance generation success-rate vs. worst-case latency? No principled
  answer exists before end-to-end generation data — the design phase picks a
  defensible starting pair (documented), tunable in place afterward. Not
  design-blocking (tests inject explicit budgets).
- **Failure diagnostics.** Whether `GenerationError` should eventually carry
  which seeds/issues were tried (for a "why did generation fail" UX). Deferred;
  the current design-doc contract is a bare sentinel.
