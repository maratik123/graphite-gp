# Gate egui-Context/tessellation-driving gp-render unit tests under Miri

**Source:** issue #106
**Date:** 2026-07-21
**Tracked in:** #106

## Scope

1. **(A) Source change — gate the Context/tessellation-driving tests.** Add
   `#[cfg_attr(miri, ignore = "…")]` to every `gp-render` unit test that stands
   up an `egui::Context` (directly or via a shared render/capture helper) and
   thereby drives egui's context/layout/tessellation machinery, so those tests
   are skipped under the Miri interpreter while still running under native
   `cargo test`. The per-test `ignore` reason must describe that test's own
   cost/abort cause (AGENTS.md § Rust Test Conventions: never copy a sibling's
   reason).
2. **Keep pure-logic tests under Miri.** The integer/float arithmetic and
   set-theory tests stay un-gated — this is exactly the logic Miri exists to
   check: `heatmap::{speed_bounds_*, normalize_*, ramp_color_*}`,
   `regions::{classify_loops_*, triangulate_*, asphalt_equals_corridor_contains,
   infield_hole_and_outfield_are_disjoint}`, `grid::line_coords_*`,
   `fastest_lap::catmull_rom_*`, `mod::layer_order_is_documented`, and siblings.
3. **(B.1) Convention rule** — codify in AGENTS.md § Rust Test Conventions
   (mirroring the existing wgpu-golden Miri-gate convention) that any
   `gp-render` unit test constructing an `egui::Context`/painter carries the
   Miri gate, naming the concrete, mechanically-checkable trigger. This is the
   full policy scope for this task (per Q1); the CI budget guard (B.2) is
   deferred to its own issue and this task does NOT touch `ci.yml`.
4. **Measurement.** Record Miri wall-clock before and after the source change
   using the exact workspace command CI runs
   (`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace`), to
   confirm the gate set (Δ newly-`ignored` = number of gated tests) and record
   the measured reduction per the amended AC4. The residual non-Context Miri
   cost is tracked separately in #107 — it is not a success gate for this task.

## Out of scope

- The O(n³) ear-clipping optimization (#104) and the single-`Galley` text reuse
  (#96) — the issue establishes both give ~0 Miri benefit (they target 60 fps
  runtime frame time, a different axis than CI Miri wall-clock). They remain
  independent runtime-perf issues.
- Changing the aliasing model / `MIRIFLAGS` (`-Zmiri-tree-borrows` stays).
- **Any `ci.yml` change**, including the B.2 CI wall-clock budget guard (Q1
  scoped this task to gate + convention only; B.2 is a separate deferred issue).
- Removing or altering the existing wgpu-golden Miri gates.
- **B.3 (scope the Miri job to exclude `gp-render` wholesale)** — rejected per
  the issue: it would drop UB coverage on the pure-logic render tests, which
  (A)+(B.1) preserves. Not pursued.
- Any reduction in native `cargo test` coverage — it must stay unchanged.

## Deferred

- **B.2 CI budget guard** — a wall-clock backstop on the Miri job | a concrete
  threshold can only be derived from the post-fix measured numbers this task
  produces, and the product owner scoped this task to gate + convention only
  (Q1) | separate issue: **yes**.
- **Residual non-Context Miri cost (~17 min after gating)** — investigate and
  reduce the Miri wall-clock that remains once every `egui::Context` test is
  gated | measurement (BEFORE 24m48s → AFTER 17m02s local, Δ +15 `ignored` =
  the exact gate set) showed the residual is non-Context cost (e.g. gp-core's
  integer-physics tests, which MUST stay under Miri, plus the Miri
  interpret/compile baseline), not the Context tests this task removes | separate
  issue: **#107**.

## Key decisions

| Question | Decision |
|---|---|
| Source-change scope: track-only or crate-wide? | **Crate-wide.** The trigger is "the test constructs an `egui::Context`/painter", not "lives in `track/`". #105's `screens/setup.rs` capture test is already implicated, and AC4 is measurement-driven (measured reduction, not a fixed wall-clock target) — so gating is scoped to whatever set the before/after numbers require, audited across all of `gp-render`. Candidate sites beyond the #101 track tests (`track/{heatmap,grid,fastest_lap,regions,mod}.rs`): `screens/setup.rs`, `icons.rs`, `placeholder.rs`, `widgets/card.rs`. |
| Gate mechanism | Per-item `#[cfg_attr(miri, ignore = "…")]`, mirroring the existing golden gates (module `//!` note + per-test attribute). Per-test reason string, never crate-level `--exclude` (AGENTS.md § Rust Test Conventions). |
| Convention home (B.1) | AGENTS.md § Rust Test Conventions, alongside the existing golden-test Miri-gate rule. Exact wording/placement left to design; Propagation Rule grep sweep required in the same PR. |
| Policy scope (B) | **A + B.1** (Q1, round 1): gate the tests AND codify the convention rule. B.2 CI budget guard deferred to its own issue; this task does NOT touch `ci.yml`. |

## Technical constraints

- Miri aborts on the first unsupported operation and cargo's fail-fast drops
  the rest — gate per-test, in the same commit (AGENTS.md § Rust Test
  Conventions). The reason string documents that test's own cost/abort cause.
- Verify only with the CI workspace command
  (`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace`), never
  a narrower `-p` run.
- Miri is a required branch-protection gate via the `miri-pass` aggregator
  (context `Miri`, ci.yml, #76). The raw `miri` job runs
  `cargo miri test --workspace` (ci.yml:188).
- If B.2 is taken, any `.github/workflows/ci.yml` edit MUST pass `actionlint`
  before `git add` (AGENTS.md AXIOM).
- If B.1/AGENTS.md is edited, the Propagation Rule fires: run the keyword grep
  sweep across `.claude/**`, `AGENTS.md`, `ai-docs/**` and update siblings
  (e.g. `self-review.md`, `review-findings.md`) in the same PR.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Every `gp-render` unit test that constructs an `egui::Context`/painter carries `#[cfg_attr(miri, ignore = "…")]`, with a reason describing that test's own cost/abort cause; verified by a grep audit over `crates/render/src/`. |
| AC2 | All pure-logic `gp-render` track/overlay tests (Scope §2) still run and pass under Miri (not ignored). |
| AC3 | Native `cargo test -p gp-render` and `cargo test --workspace` test count/coverage is unchanged; the full suite is green (issue cites 285+ workspace tests). |
| AC4 | Miri wall-clock measured before and after with `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace` and recorded in the PR. Success is the *measured* outcome: (a) the AFTER value is materially lower than BEFORE, and (b) the count of newly-`ignored` Miri tests equals the gate set (AFTER `ignored` count − BEFORE `ignored` count = number of gated tests). The ~5-min target from #106's original wording is **retired** — it rested on a refuted attribution (the residual ~17 min is non-Context cost — e.g. gp-core's integer-physics tests, which MUST stay under Miri, plus the Miri interpret/compile baseline — not the Context tests this task gates; note the ~5-min figure was itself a CI number not reproducible locally). Residual-cost investigation is tracked in #107. |
| AC5 | `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` are clean. |
| AC6 | *(B.1)* AGENTS.md § Rust Test Conventions states the gate rule naming the concrete trigger ("constructs an `egui::Context`/painter"), mirroring the existing golden-test gate; the Propagation Rule grep sweep is run and siblings updated as needed in the same PR. |
| AC7 | No `.github/workflows/**` file is modified by this task (B.2 stays deferred). |

## Open questions

- **Exact per-test gate list.** Whether `icons.rs` / `placeholder.rs` /
  `widgets/card.rs` Context tests must be gated is resolved by the implementing
  pass's before/after measurement (Δ newly-`ignored` = gate set), not answerable
  pre-measurement. (Not design-blocking — the crate-wide trigger + measurement
  drive it.)
