# Design: Miri-gate zero-production-UB-signal gp-render token/helper tests + codify the convention (B.1 of #107)

**Issue:** #107 (B.1 — partial resolve; **Refs #107**, not Closes)
**Date:** 2026-07-21

## Approach

Purely mechanical, test-attribute-only change plus ONE AGENTS.md convention
bullet. No production logic, no new dep, no new token, no `ci.yml` change (B.2 is
deferred). Two ideas drive the design:

1. **Gate mechanism — per-`#[test]` `#[cfg_attr(miri, ignore = "…")]` + a module
   `//!` "Miri:" note**, mirroring the already-merged render gates
   (`track/grid.rs:11-15`, `track/heatmap.rs:22`, etc.). This is the house
   pattern verified in-tree `[measured: rg -Un 'cfg_attr\(miri' crates/render/src
   → 11 module //! notes; grid.rs:206-212 shows the applied per-test attr form]`.
   The attribute is **inert off-Miri**, so native `cargo test` / clippy / fmt /
   doc are untouched. **Never** a crate-level `--exclude` (AGENTS.md § Rust Test
   Conventions — that would also drop the crate's Miri-clean tests).

2. **Cost-not-abort reason strings, ONE honest reason per cause-group.** Unlike
   the wgpu-golden gate (FFI *abort*) and the Context/painter gate (interpreted
   *cost*), these tests neither abort nor drive a painter — they assert
   constant/data parity or exercise `#[cfg(test)]`-only scaffolding. The reason
   must state **interpreted wall-clock cost, no production Miri UB signal** and
   **make no abort claim** (AC1). Same-cause siblings share ONE reason (spec Key
   decisions; AGENTS.md § Rust Test Conventions "don't copy a sibling's reason"
   targets *different* causes only). Three distinct causes exist, so **three**
   reason strings (see § Reason strings).

**Module `//!` note — YES, apply it.** The existing render gates all pair the
per-test attribute with a module `//!` "Miri:" note; applying it here keeps the
convention uniform and is what AC6's "mirror the #106/#108 bullet" implies at the
source level `[measured: rg -Un 'cfg_attr\(miri' crates/render/src → every gate
has a //! note]`. One note per file; for `tokens/mod.rs` a single note covers
both its `css` and `inventory` test submodules.

**One reason per module-group vs per-test — per-group is cleaner and is the
spec's decision.** 26 bespoke reasons would restate one identical cause 26 times;
the honest granularity is the *cause*, of which there are three. Adopted.

**Charter split (rejected alternative: delegate everything to `code-writer`).**
The AGENTS.md bullet + Propagation sweep is a **prose/instruction-file** edit to a
**protected** file. `code-writer`'s charter is *code* (AGENTS.md § Workflow
"Before delegating … charter fit"), and a **background** subagent cannot answer
the self-modification permission prompt a protected-file edit raises — it **fails
closed** regardless of any `Edit(...)` allow-list (AGENTS.md § Workflow
"environment fit"). Therefore the instruction work is authored **in-thread by the
orchestrator**, split into its own group (see § Handoff plan). The 6-file gate is
homogeneous code → `code-writer`.

### The 26 enclosing `#[test]` functions (verified by reading each `#[cfg(test)]` mod)

Per-file `#[test]` counts `[measured: rg -Uc '#\[test\]' <file> → color 5,
spacing 2, typography 3, effects 7, mod 7 (css 5 + inventory 2), test_util 2]`.
`mod.rs` = 7 = 5 css + 2 inventory `[measured: rg -Un '#\[test\]|pub\(crate\) mod
css|mod inventory' crates/render/src/tokens/mod.rs → css block :63, inventory
block :214, 5 tests :167-200, 2 tests :380-394]`. Σ = 5+2+3+7+5+2+2 = **26**.

| File | Test submodule | `#[test]` fns (exact) | Cause |
|---|---|---|---|
| `tokens/color.rs` (`mod tests` :170) | tests | `base_colors_match_css`, `aliases_match_their_base`, `car_colors_and_accessor`, `heat_ramp_is_ordered_slow_to_fast`, `cross_identities_hold` (5) | (a) constant parity |
| `tokens/spacing.rs` (`mod tests` :93) | tests | `tokens_match_css`, `radius_pill_saturates_to_255` (2) | (a) constant parity |
| `tokens/typography.rs` (`mod tests` :87) | tests | `numeric_tokens_match_css`, `family_names_match_css`, `role_aliases_match_their_target` (3) | (a) constant parity |
| `tokens/effects.rs` (`mod tests` :135) | tests | `shadow_0_is_none`, `elevation_shadows_match_css`, `shadow_inset_matches_css`, `focus_shadow_matches_css_and_round_trips`, `durations_match_css`, `eases_match_css`, `bg_decomposition_matches_css` (7) | (a) constant parity |
| `tokens/mod.rs` (`pub(crate) mod css` :63 → `mod tests` :164) | css tests | `assert_token_parses_px_em_and_bare_numbers`, `value_of_does_not_match_a_prefix`, `value_of_ignores_comment_mentions`, `assert_cubic_bezier_parses_control_points`, `var_target_extracts_the_referenced_name` (5) | (b1) test-only CSS parser |
| `tokens/mod.rs` (`mod inventory` :214) | inventory | `per_file_counts_match_ac1`, `ported_and_deviations_partition_the_parsed_names` (2) | (a) constant parity |
| `test_util.rs` (`mod tests` :40; module is `#[cfg(test)] mod test_util`, `lib.rs:20`) | tests | `assert_f32_accepts_an_equal_value`, `assert_f32_slice_accepts_equal_arrays` (2) | (b2) test-only float-assert helpers |

Matches the spec's per-module counts exactly (color 5, spacing 2, typography 3,
effects 7, inventory 2, css 5, test_util 2 = 26). No rustfmt-split `#[cfg_attr]`
pre-exists on any of these tests `[measured: rg -Un 'cfg_attr\(miri'
crates/render/src/tokens crates/render/src/test_util.rs → no matches]` — all 26
are currently un-gated, so this is a clean add (no edit-over-existing).

### Reason strings (3 causes)

- **(a) constant/data parity** — `color`, `spacing`, `typography`, `effects`,
  `inventory` (19 tests). Shared verbatim (wording broadened to honestly cover
  `color::car_colors_and_accessor`, which also drives the production `car_color`
  accessor's totality — `CAR_COLORS.get(index).copied()`, a total safe combinator
  with zero UB; its behavioral coverage stays live under native `cargo test` per
  AC5):
  > `interpreted wall-clock cost, no production Miri UB signal: asserts constant/data parity against the include_str!'d design-system CSS (or a sibling const), or a total safe accessor over a static const table — safe-Rust comparisons, not an abort`
- **(b1) `#[cfg(test)]` CSS parser self-tests** — `mod.rs` `css` (5 tests):
  > `interpreted wall-clock cost, no production Miri UB signal: exercises the #[cfg(test)]-only CSS parser helpers, never linked into the game binary — not an abort`
- **(b2) `#[cfg(test)]` float-assert helper self-tests** — `test_util.rs` (2
  tests):
  > `interpreted wall-clock cost, no production Miri UB signal: exercises the #[cfg(test)]-only assert_f32/assert_f32_slice helpers themselves — not an abort`

All three state cost, name the zero-UB-signal cause, and make **no abort claim**
— distinct from the wgpu-golden (FFI abort) and Context/painter (painter-pass
cost) reasons already in-tree. Reasons are prose strings (may wrap with `\` line
continuations per `grid.rs:206-212` house form).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add per-`#[test]` `#[cfg_attr(miri, ignore = "<cause reason>")]` to all 26 tests + a module `//!` "Miri:" note per file (one note in `mod.rs` covering both submodules), using the 3 reason strings above. Then run AC4/AC5/AC9 gates. | `crates/render/src/tokens/{color,spacing,typography,effects,mod}.rs`, `crates/render/src/test_util.rs` | — |
| 2 | Add ONE AGENTS.md § Rust Test Conventions bullet (after the Context/painter bullet, line 309) codifying the zero-production-UB-signal gate for both sub-classes (a)/(b) with a grep-checkable trigger + cost-not-abort reason class; run the Propagation Rule sweep; verify `wc -c AGENTS.md` < 40000. | `AGENTS.md` | 1 |

M = 2. Task 2 depends on 1 only so the measured gate scope (26) the bullet's
trigger describes is already in place when the convention is written; the two are
independently applyable but this order keeps the bullet's grep-check example
truthful.

### Proposed AGENTS.md bullet (exact wording, to insert after line 309)

> - **gp-render zero-production-UB-signal token/helper tests carry the Miri gate — mechanical trigger.** Any `gp-render` unit test whose body asserts ONLY **(a)** constant/data parity against a static source (the `include_str!`'d design-system CSS consts — `tokens::{color,spacing,typography,effects}` + `mod inventory`) **or (b)** `#[cfg(test)]`-only helper machinery never linked into the game binary (the `pub(crate) mod css` parser, the `mod test_util` `assert_f32`/`assert_f32_slice` helpers) MUST carry `#[cfg_attr(miri, ignore = "<why>")]`. Its Miri UB coverage is worthless (pure safe-Rust parity, or test-only scaffolding), so gate it — grep-checkable (`rg -Un '#\[test\]' crates/render/src/tokens crates/render/src/test_util.rs`, then confirm each enclosing `#[test]` is gated), NOT "is slow": like the Context/painter gate these are interpreted wall-clock **cost**, not abort — the reason must make **no abort claim**. One honest reason per cause-group (constant-parity / css-parser / test_util); same-cause siblings MAY share it.

Addition measures **1048 chars** `[measured: awk 'NR==<bullet-line>' AGENTS.md |
wc -c on the drafted bullet → 1048]` → AGENTS.md ≈ 33856 + 1048 = **≈ 34,904
chars**, under the 40,000 cap but with only **≈ 95 chars of headroom below the
35,000 early-warning** `[measured: wc -c AGENTS.md → 33856 (pre-edit); derived
post-edit ≈ 34.9k → discharged by AC8 wc -c after edit]`. **Thin margin — flag:**
if the measured post-edit size reaches ≥ 35,000, tighten the bullet before commit
(the "tighten if ≥ 35k" branch is expected to be *close*, not comfortable; AC8
re-measure is load-bearing here, not a formality).

## Handoff plan

Per § Rules handoff-grouping (a)–(h). M = 2, two change-types → two homogeneous
groups, both terminal-eligible, count = 2 (≤ 4, minimized — a code subtask and an
instruction subtask cannot share a group per (e), so 2 is the floor).

- **Group A** — **code** change-type (`*.rs`); model `sonnet` (sonnet-5), effort
  **`medium` (pinned)** via the `code-writer` subagent, 1M-token window —
  subtask 1. All 6 `.rs` files are one uniform mechanical gate → one group. Group
  entry: spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md`
  § Compaction recovery (re-entry). (1 subtask; within `1..=10`.)
- **Handoff after Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
  Parent `/task` resumes in Group B with fresh context.
- **Group B** — **instruction/harness** change-type (`AGENTS.md`); model `opus`,
  effort **inherited from the orchestrator (typically xHigh) — NOT pinned** —
  subtask 2. **Authored IN-THREAD by the orchestrator, NOT spawned to a
  background `general-purpose` subagent:** `AGENTS.md` is a **protected
  self-modification target**, and a background subagent **fails closed** on the
  self-mod permission prompt (AGENTS.md § Workflow "environment fit"); the Group B
  marker (`opus`, inherited effort) records the model that authors it in-thread,
  overriding the default (g) instruction→`general-purpose` routing for this
  protected-file reason. Terminal group (1 subtask; within `1..=10`). This group
  completes Step 8 in its own `/context-reset` subagent.

## Risks

- **Miscount / missed test (AC1–AC4).** Mitigation: post-gate grep confirms
  every enclosing `#[test]` in the 6 files carries the attribute, then the Miri
  run's Δ`ignored` = 26 is the ground truth. — `[derived → AC4:
  MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace reports
  Δignored = 26; per-file re-scan rg -U '#\[test\]' vs cfg_attr(miri]`
- **rustfmt splits the `#[cfg_attr(...)]` across lines**, hiding it from a
  single-line grep at verify time. Mitigation: verify with `rg -U` (multiline) or
  read the region (AGENTS.md § Rust Test Conventions; spec Technical
  constraints). — `[derived → AC4 grep-check uses rg -U]`
- **`//!` note / backtick reason text trips the rustdoc broken-intra-doc-link
  gate.** The existing render `//!` Miri notes use the same backticked idioms
  (`egui::Context`, `#[cfg_attr(miri, ignore = "…")]`) and pass the doc gate, so
  mirror their exact phrasing. — `[derived → AC9: RUSTDOCFLAGS="-D warnings"
  cargo doc --no-deps --workspace]`
- **AGENTS.md crosses the 35,000 early-warning / 40,000 cap.** The bullet
  measures **1048 chars** → post-edit ≈ **34,904**, only **≈ 95 chars below the
  35,000 warning** (well under the 40k cap). **Thin margin.** Mitigation:
  re-measure after the edit; tighten the bullet if it reaches ≥ 35k. — `[measured:
  wc -c AGENTS.md → 33856; bullet 1048; derived post-edit ≈ 34.9k → AC8 wc -c
  after edit]`
- **Propagation sweep wrongly assumed negative** (spec calls this out
  explicitly). Mitigation: RUN the sweep and classify every hit; do not assume.
  See § Test Design AC7. — `[measured: grep -rln 'Miri' .claude/agents/self-review.md
  .claude/agents/review-findings.md .claude/agents/design-review.md
  .claude/skills/project-review/SKILL.md → (empty); derived → AC7 re-run at edit
  time]`
- **~17-min Miri wall-clock (AC4).** Not a correctness risk; budget the run. The
  gate's whole purpose is to *reduce* this (kind-(a) ≈ 193.5s / 87% of Miri
  test-body time per spec). — `[derived → AC4 single workspace Miri run]`

## Test Design

No new tests — this task **gates existing** tests; the "test design" is the
per-AC verification matrix (all run by Group A except AC6–AC8, run in-thread by
Group B; AC10 at PR-create).

| AC | Verification |
|---|---|
| AC1 | 19 kind-(a) tests carry the (a) reason (color 5, spacing 2, typography 3, effects 7, inventory 2). `[derived → rg -U -B2 '#\[test\]' on the 4 token files + mod.rs inventory → each `#[test]` immediately followed by the (a) `#[cfg_attr(miri, ignore …)]`]` |
| AC2 | 5 `tokens::css` tests carry the (b1) reason. `[derived → rg -U on mod.rs css block]` |
| AC3 | 2 `test_util` self-tests carry the (b2) reason; no no-self-test helper module (`track/test_support.rs`, gp-core `geom::common`/`sim::common`) is touched. **Primary evidence:** `[measured: git diff --name-only → only the 6 listed files + AGENTS.md]`. Note on the "no self-tests" claim: `track/test_support.rs` is a standalone file, so `rg -Uc '#\[test\]' → 0` proves it. But `geom::common` / `sim::common` are `pub(crate) mod common` **submodules** inside `crates/core/src/geom/mod.rs` (`:374`) / `sim/mod.rs` (`:437`), whose FILES also hold 29 / 24 physics `#[test]`s — so a file-level `rg -Uc '#\[test\]'` returns 29 / 24, NOT 0. To confirm the `common` submodules themselves have no self-tests, scope the scan to the `mod common` block (read the region, or `sed -n` the block then `rg`), not the whole file. `[measured: rg -Uc '#\[test\]' → geom/mod.rs 29, sim/mod.rs 24 (file-level, expected), test_support.rs 0]` The `git diff --name-only` guard is the authoritative AC3 evidence regardless. |
| AC4 | `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace` → Δ`ignored` = 26 (26 gated tests now `ignored`), workspace green. **~17-min run; run ONCE after all 6 files gated.** `[derived → single workspace Miri run; compare ignored count to a pre-gate baseline or read the 26 named tests in the `ignored` list]` |
| AC5 | Native `cargo test -p gp-render` runs and passes all 26 (attribute inert off-Miri). `[derived → cargo test -p gp-render → 26 named tests pass, 0 ignored]` |
| AC6 | AGENTS.md bullet present, both sub-classes (a)/(b), grep-checkable trigger, cost-not-abort. `[derived → read AGENTS.md line after 309]` |
| AC7 | Propagation sweep RUN, every match classified. Measured baseline: the enforcement-sibling grep is **empty** — no `self-review.md` / `review-findings.md` / `design-review.md` / `project-review/SKILL.md` references the Miri-gate convention `[measured: grep -rln 'Miri' <those 4 files> → (empty)]`; the broad sweep's other hits are **task artifacts / history**, not enforcement siblings `[measured: grep -rln 'Miri-gate\|zero production UB\|cfg_attr(miri\|Context/painter' .claude/ AGENTS.md ai-docs/ → matches only in ai-docs/plans/**, ai-docs/context.md, ai-docs/learnings.md, INDEX.md, _inbox.jsonl, AGENTS.md — none are enforcement siblings]`. **Expected NEGATIVE (no sibling edit), but RE-RUN at edit time** with the concrete new keywords (`grep -rn 'zero-production-UB-signal\|token/helper tests' .claude/agents/ .claude/skills/ .claude/rules/ AGENTS.md ai-docs/`) and classify each hit — do NOT assume, per spec Technical constraints + #106/#108 precedent. |
| AC8 | `wc -c AGENTS.md` < 40000 after edit. `[measured pre-edit: 33856; derived post-edit ≈ 34.8k]` |
| AC9 | `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass. `[derived → run all four]` |
| AC10 | PR body says **Refs #107** (B.2 keeps #107 open), not Closes. `[derived → gh pr view N --json body at Step-11/12]` |

## Open questions

- None. Q1 resolved (whole tree — all 24 `tokens::*` incl. css); both scope
  expansions (css whole-tree, `test_util` self-tests) incorporated; the
  charter-split (in-thread AGENTS.md group) is settled by the environment-fit
  rule.
