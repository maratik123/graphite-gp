# Gate zero-production-UB-signal gp-render tests under Miri + codify the convention (B.1 of #107)

**Source:** issue #107 (B.1)
**Date:** 2026-07-21
**Tracked in:** #107

## Scope

1. **(A) Source change — gate the constant-verification token tests.** Add
   `#[cfg_attr(miri, ignore = "…")]` to every `gp-render` `tokens::*`
   **constant-vs-CSS** unit test (kind-(a)), so they are skipped under the Miri
   interpreter while still running under native `cargo test`. These parse the
   `include_str!`'d design-system CSS and assert a Rust `const` equals the parsed
   CSS value — pure safe-Rust constant comparisons with **no production UB
   signal**. Reason strings are **cost-not-abort** ("interpreted wall-clock cost,
   no production Miri UB signal") — these tests do **not** abort Miri, they are
   merely slow under interpretation.

   Kind-(a) tests (19; Σ exec ≈ 193.5s / 87% of all Miri test-body time):
   - `tokens::color` — 5 tests / 126.72s (`color.rs`)
   - `tokens::spacing` — 2 tests / 26.01s (`spacing.rs`, incl.
     `radius_pill_saturates_to_255` — a saturating `u8` conversion of a `const`,
     still safe integer arithmetic with no UB signal)
   - `tokens::typography` — 3 tests / 25.55s (`typography.rs`)
   - `tokens::effects` — 7 tests / 9.24s (`effects.rs`)
   - `tokens::inventory` — 2 tests / 5.95s (`mod.rs`, `mod inventory`)

2. **(A) Source change — gate the `tokens::css` parser-helper self-tests.** Also
   gate the `tokens::css` submodule tests (`mod.rs`, `pub(crate) mod css` →
   `mod tests`; 5 tests / 0.40s). **Q1 = WHOLE TREE.** Rationale (verified,
   stronger than #107's literal "gate all tokens::*" wording): the entire `css`
   parser (`value_of` / `assert_token` / `assert_cubic_bezier` / `var_target` /
   `scan_declarations`) is declared under `#[cfg(test)] pub(crate) mod css`
   (`mod.rs:62`) — it compiles **only in test builds**, is **never linked into
   the game binary**, and every call site is inside a `#[cfg(test)] mod tests`
   block. Miri UB-checking over it covers only **test-only scaffolding** → **zero
   production-safety value**, so its Miri coverage is worthless and it is gated
   like the rest.

3. **(A) Source change — gate the `test_util` float-assert self-tests
   (SCOPE EXPANSION #1).** Gate the 2 self-tests in `crate::test_util`
   (`crates/render/src/test_util.rs:43-52`:
   `assert_f32_accepts_an_equal_value` + `assert_f32_slice_accepts_equal_arrays`)
   that verify the shared float-assert helpers `assert_f32` / `assert_f32_slice`
   themselves. `mod test_util` is declared `#[cfg(test)]` (`lib.rs:20`) — test-only
   machinery whose self-tests carry zero production UB signal. **Only these 2
   self-tests are gated.** Test-only helper modules that have **no** self-tests
   are NOT touched — their fixtures are consumed by **production-logic** tests
   that MUST stay under Miri: `crates/render/src/track/test_support.rs`,
   `gp-core` `geom::common`, `gp-core` `sim::common` (all verified to contain no
   `#[test]`).

   **Net gate scope = 26 tests across 6 files:** `tokens::{color 5, spacing 2,
   typography 3, effects 7, inventory 2, css 5}` (= 24, in
   `crates/render/src/tokens/{color,spacing,typography,effects,mod}.rs`) +
   `test_util` (= 2, in `crates/render/src/test_util.rs`).

4. **(B.1) Convention amendment (SCOPE EXPANSION #2).** Add ONE concise bullet to
   AGENTS.md § Rust Test Conventions codifying the **general** principle:
   Miri-gate unit tests that carry **zero production UB signal** — namely tests
   that verify either **(a)** constant/data parity against a static source (the
   `include_str!` CSS-token consts), or **(b)** **test-only helper machinery**
   (the `#[cfg(test)]` CSS parser, the `test_util` float-assert helpers) — with
   `#[cfg_attr(miri, ignore = "…")]` and **cost-not-abort** reason strings. Mirror
   the existing gp-render Context/painter Miri-gate bullet (added by #106/#108) in
   structure and grep-checkable-trigger style. Keep it **tight** — AGENTS.md must
   stay under the 40,000-char hard cap (35,000-char early-warning). This makes the
   task a **harness/instruction change, not code-only**.

5. **Measurement.** Verify with the exact workspace command CI runs
   (`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace`): the 26
   gated tests move `passed` → `ignored`, and the workspace stays green.

## Out of scope

- **B.2 (CI wall-clock budget guard, `ci.yml` backstop)** — a separate future
  `/task`; this task does NOT touch `ci.yml`.
- Any production source-**logic** change, new dependency, or new design token —
  this is **test-attribute-only** plus one AGENTS.md convention bullet.
- Gating `gp-core` integer-physics tests (9.82s) — they stay under Miri
  unconditionally (deterministic-sim UB coverage is why Miri exists).
- Gating any test-only helper module with **no self-tests** — its fixtures feed
  production-logic tests that must stay under Miri.
- Any reduction in native `cargo test` coverage — it must stay unchanged.
- Changing the aliasing model / `MIRIFLAGS` (`-Zmiri-tree-borrows` stays).
- Re-touching the already-merged #106/#108 Context/painter or wgpu-golden gates.

## Deferred

- **B.2 CI wall-clock budget guard** | a concrete threshold can only be derived
  from post-B.1 measured numbers, and it is a distinct `ci.yml` change with its
  own `actionlint` gate | separate issue: keep **#107 open** after this partial
  resolve (this task is **Refs #107**, NOT Closes).

## Key decisions

| Question | Decision |
|---|---|
| Q1 — `tokens::css` disposition | **WHOLE TREE** — gate all 24 `tokens::*` tests incl. the 5 `tokens::css` parser-helper tests. Rationale: the `css` parser is `#[cfg(test)]`-only machinery never linked into the game binary, so its Miri coverage is zero-production-value scaffolding (stronger than #107's literal "gate all tokens::*" wording). |
| Test-only helper self-tests (Expansion #1) | Gate `test_util`'s 2 `assert_f32` / `assert_f32_slice` self-tests. Helper modules with no self-tests (`track/test_support.rs`, gp-core `geom::common` / `sim::common`) are left untouched. |
| Gate mechanism | Per-`#[test]` `#[cfg_attr(miri, ignore = "…")]`, mirroring existing render gates (module `//!` note + per-test attribute). Never a crate-level `--exclude` (AGENTS.md § Rust Test Conventions). |
| Reason-string class | **cost-not-abort** — "interpreted wall-clock cost, no production Miri UB signal". These tests do NOT abort Miri (unlike the wgpu-golden FFI gates); the reason must not claim an abort. |
| Same-cause reason sharing | ONE honest reason per **module-group**; same-module siblings share it (all share one cause: interpreted wall-clock cost over test-only work). The "don't copy a sibling's reason" rule targets *different* causes only. |
| Convention home (B.1) | AGENTS.md § Rust Test Conventions, alongside the golden + Context/painter Miri-gate bullets. Exact wording/placement left to design; Propagation Rule grep sweep required in the same PR. |

## Technical constraints

- Gate per-test in the same commit (AGENTS.md § Rust Test Conventions). Here the
  cause is **cost**, not abort, but the per-test + same-commit mechanics are
  identical; the `#[cfg_attr(miri, …)]` attribute is **inert off-Miri** so native
  `cargo test` / clippy / doc are unaffected.
- Verify only with the CI workspace command
  (`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace`; add
  `+nightly` locally) — never a narrower `-p` run. Confirm the 26 gated tests
  report `ignored` and the workspace stays green.
- Miri is a required branch-protection gate via the `miri-pass` aggregator
  (context `Miri`, `ci.yml`, #76). A red Miri blocks merge.
- Editing AGENTS.md fires the **Propagation Rule**: run
  `grep -rn "<changed-keyword>" .claude/agents/ .claude/skills/ .claude/rules/ AGENTS.md ai-docs/`
  and update every match in the same PR (mirror to any enforcement sibling such
  as `self-review.md` / `review-findings.md`). The #106/#108 precedent's sweep
  was **negative** — a command-produced gate with no enforcement sibling — so
  **re-verify** the sweep is negative for this bullet rather than assuming it.
- AGENTS.md (and every instruction file Claude loads) must stay < 40,000 chars
  (35,000-char early-warning AXIOM) — keep the new bullet concise; check
  `wc -c AGENTS.md` after the edit.
- Grep-checkability: after gating, confirm each intended enclosing `#[test]` in
  the 6 files carries the attribute. A rustfmt-split `#[cfg_attr(\n    miri, …)]`
  is invisible to single-line grep — verify with `rg -U` or by reading the region.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Every kind-(a) constant-vs-CSS `tokens::*` test (color 5, spacing 2, typography 3, effects 7, inventory 2 = 19) carries a per-`#[test]` `#[cfg_attr(miri, ignore = "<cost-not-abort reason>")]`; the reason states interpreted wall-clock cost with no production Miri UB signal and makes no abort claim. |
| AC2 | The 5 `tokens::css` parser-helper tests are gated with the same cost-not-abort attribute (Q1 = whole tree). |
| AC3 | The 2 `test_util` float-assert self-tests (`assert_f32_accepts_an_equal_value`, `assert_f32_slice_accepts_equal_arrays`) are gated with the same cost-not-abort attribute; no test-only helper module lacking self-tests is modified. |
| AC4 | `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace` reports exactly the 26 gated tests as `ignored` (Δ `ignored` = 26) and the workspace stays green. |
| AC5 | Native `cargo test` (no Miri) still runs and passes ALL 26 gated tests — native coverage is unchanged (the gate is `miri`-only). |
| AC6 | AGENTS.md § Rust Test Conventions gains ONE concise bullet codifying the zero-production-UB-signal Miri gate — covering both (a) constant/data-parity tests and (b) test-only helper machinery — with a mechanically grep-checkable trigger and cost-not-abort reason strings, mirroring the #106/#108 Context/painter bullet. |
| AC7 | The Propagation Rule grep sweep is run for the AGENTS.md edit and every match updated in the same PR (re-verify whether the sweep is negative, as with #106/#108, rather than assuming). |
| AC8 | AGENTS.md stays under the 40,000-char hard cap after the edit (`wc -c AGENTS.md`). |
| AC9 | `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass. |
| AC10 | The PR references the issue as **Refs #107** (partial resolve — B.2 keeps #107 open), NOT Closes. |

## Open questions

- None. Q1 is resolved (whole tree); both scope expansions are incorporated.
