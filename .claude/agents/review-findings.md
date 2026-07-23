---
name: review-findings
description: "Walks the entire codebase on the current branch (no diff, no spec) and produces a findings table written to a progress file. Invoked by /project-review at the start of a whole-branch review."
---

# Review Findings Subagent

Reviews the entire codebase on the current branch. No diff, no spec — reads source files directly. Produces a findings table and writes it into the progress file.

The self-review push-gate that validates the post-fix state — and its applicability matrix (ad-hoc / out-of-skill fix → review over `git diff <merge-base>..HEAD`; docs-only / instruction-only commit → self-review optional) — is defined in [`.claude/agents/self-review.md` § When self-review applies](self-review.md); this Subagent only produces the findings table that gate consumes.

## Mindset: maximally skeptical, but justified

**Presumption of guilt.** Your job is to find real problems before they reach production.

Every suspicion — investigate via Read/grep, don't guess. Don't invent problems.

## Instructions

1. Read `AGENTS.md` — current project rules
2. Read every `*.spec.md` and `*.design.md` in `ai-docs/plans/done/` — these document **intentional** decisions. Do not raise findings for anything explicitly described there.
3. Walk the source tree:
   ```bash
   find . -name "*.rs" -not -path "*/target/*" | sort
   ```
4. Read each source file. For large files (>300 lines) read in sections; do not skip.
5. Run through the checklist below.
6. Write the progress file (path passed in prompt) in the format below. Create it — do not append.

## Checklist

### 0. Design conformance (when designs exist in `done/`)

- **AC-verification-grep re-run (mandatory).** Re-run every AC-verification grep / shell check documented in any `ai-docs/plans/done/*.design.md` against the shipped artefact (the files currently on the branch). The design's "AC<N> verified by: <command>" lines are NOT optional — each command MUST be executed during this review against the live tree, and the result quoted in the findings (PASS / FAIL). "Confirmed during drafting" is NOT sufficient; that was the failure mode in `maratik123/quartzite#295` (spec-writer tools-line regression — see quartzite's `ai-docs/learnings.md` 2026-05-15 tooling entry on spec-writer `tools:` frontmatter). Any AC-verification grep that fails against the shipped artefact → `major` finding with the failing command and its actual output.

### 1. Safety and correctness
- `unsafe` blocks: each justified with a comment explaining the invariant?
- **Panic-index sync.** For every public fn / method with a `# Panics` doc section, **and** every production `.unwrap()` / `.expect(…)` / `panic!` outside `#[cfg(test)]`, verify there is a corresponding entry in `ai-docs/panic-index.md` (location, trigger, invariant, why not `Result`, preferred fix). Production panic site missing from the index → `major`. The `# Panics` doc-section signal is the primary trigger; the grep below is the secondary catch-net.
- **Unsafe-index sync.** For every public fn / method with a `# Safety` doc section, **and** every production `unsafe { … }` block / `unsafe fn` declaration outside `#[cfg(test)]`, verify there is a corresponding entry in `ai-docs/unsafe-index.md` (location, why unsafe, safety invariant, why not safe Rust, preferred fix). Production unsafe site missing from the index → `major`. The `# Safety` doc-section signal is the primary trigger; the `rg '\bunsafe\s*\{|\bunsafe\s+fn\b'` recipe is the secondary catch-net.
- **`unwrap()` / `expect()` / `panic!()` audit:** grep changed files for these outside `#[cfg(test)]` modules. A reason string does NOT make a panicking call acceptable — ask "is there a non-panicking form?" Mandatory substitutions:
  - `Mutex::lock().expect(...)` → `.lock().unwrap_or_else(|e| e.into_inner())`
  - `Condvar::wait*().expect(...)` → `.wait*(...).unwrap_or_else(|e| e.into_inner())`
  - `Option::expect(...)` in logically-guaranteed positions → `if let` or `let Some(...) = ... else { ... }`
  Flag any `.expect()` whose reason string explains the invariant but not why recovery is impossible.
- Integer casts (`as`): could they silently wrap or truncate?
- Arithmetic: overflow/underflow possible on plausible inputs?
- Error handling: silenced errors (`let _ = ...`)? Missing `?`?
- Logic: off-by-one, wrong comparison direction, always-true conditions?

### 2. API design
- Public items missing validation or easy to misuse?
- `pub` where `pub(crate)` would suffice?
- `no_std` compatibility: any `::std::` paths in crates that declare `#![no_std]`?
- Lifetime / ownership: surprising footguns for callers?
- Naming (see AGENTS.md "API Naming"): is every `_unchecked` fn `unsafe` with a `# Safety` doc section? Is the unsuffixed name the safe/ergonomic default? Any safe fn carrying `_unchecked` (or `_checked` used for non-safety variants) → finding.

### 3. Test coverage
- Every file with ~50+ lines of non-trivial code has a `#[cfg(test)] mod tests` block? (Exceptions: files under `examples/` are runnable demos — no test block required; files under `benches/` declared with `[[bench]] harness = false` are criterion bench binaries — `criterion_main!` replaces the test runner, so `#[cfg(test)]` items would never run — no test block required.)
- Tests cover edge cases and error paths, not just the happy path?
- Any test that would pass even if the production code were deleted (cosmetic test)?
- Integration tests for public-facing macro output?
- **`assert_matches!` scrutinee impls `Debug`?** `assert_matches!` formats the scrutinee with `{:?}` on mismatch (`Result` needs `T`+`E`; `Box<dyn Trait>` needs a `Debug` supertrait). A production `#[derive(Debug)]` added *only* to satisfy a test-only `assert_matches!` is an out-of-scope API change — flag it; `assert!(matches!(...))` imposes no such bound. (AGENTS.md § Rust Test Conventions.)
- **Golden-image threshold class** ([`ai-docs/code-style.md` → Golden-image thresholds](../../ai-docs/code-style.md#golden-image-thresholds))**:** any `egui_kittest` golden that renders text (glyphs, labels, numerals, icons) using exact compare (`threshold(0.0)`) instead of the measured text threshold (`threshold(1.0)`) → `major` — it schedules a near-certain red-CI round the in-tree precedent (four text goldens at `1.0`) already resolved. Flat / byte-stable goldens (`placeholder`) at `threshold(0.0)` are correct.
- **Shared-boundary fill/stroke consistency** ([`ai-docs/code-style.md` → Shared-boundary fill/stroke consistency](../../ai-docs/code-style.md#shared-boundary-fillstroke-consistency))**:** a render layering a fill and a stroke for the SAME boundary with different geometry — per-cell square `rect_filled` under a smoothed (Chaikin) `closed_line` stroke, or the reverse → `major`. Corner staircase / colour bleed that passes `image-check`, exact compare, and CI. The fill must recolour the **shared** smoothed mesh; a § Risks "convex-corner bleed" note does not clear it.
- **Golden setup fidelity** ([`ai-docs/code-style.md` → Golden setup fidelity](../../ai-docs/code-style.md#golden-setup-fidelity--fixture--harness-must-match-the-real-thing))**:** (a) a visual golden whose fixture is hand-rolled from a unit-test fixture when an owner-approved `scene_*`/gallery fixture of the same kind already exists; (b) a golden that forces runtime conditions the binary does not set (`.with_theme(...)`, window size, visuals) so it renders differently from the real binary → `major`. Both pass `image-check`, exact/measured compare, and CI. Prefer reusing the shared fixture and making the draw code self-sufficient (own background + own palette tokens).

### 4. Performance
- O(n²) or worse where O(n) is straightforward?
- Unnecessary clones or allocations in non-trivial code paths?

### 5. Style (AGENTS.md rules)
- `#[allow(clippy::...)]` / `#[allow(dead_code)]` without a justification comment?
- Public items undocumented (`///` missing on `pub` functions/types)?
- Dead code that clippy does not catch?
- **Error types** ([`ai-docs/code-style.md` → Error types](../../ai-docs/code-style.md#error-types))**:** any new error enum/struct with hand-rolled `Display` / `std::error::Error` impls that could use `thiserror` instead? The rule mandates `thiserror` for new error types unless the derive cannot express the required behaviour.
- **Crate-level lints:** every new crate's `lib.rs` carries both `#![deny(missing_docs)]` and `#![deny(clippy::undocumented_unsafe_blocks)]`?
- **File size** ([`ai-docs/code-style.md` → File size](../../ai-docs/code-style.md#file-size))**:** any non-exempt `.rs` file over the **hard limit** (1000 lines excl. `#[cfg(test)]` / 1500 incl. tests) → `major`, refactor required. Files over the **soft limit** (500 / 800) that visibly mix responsibilities → `minor` with a split suggestion. Exemptions: auto-generated / codegen output, single large state machine or `match`, `macro_rules!` definitions. Measure excl-tests with `awk '/^#\[cfg\(test\)\]/{exit} {n++} END{print n}' file.rs`. Do **not** flag cohesive small-to-medium files for being "monolithic" — one-struct-per-file is anti-idiomatic in Rust.
- **Magic numbers** ([`ai-docs/code-style.md` → Magic numbers](../../ai-docs/code-style.md#magic-numbers))**:** inline numeric literal carrying semantic meaning (grid sizes, tuning thresholds, timeouts, retry counts, cache limits, offsets) without an accompanying `const SCREAMING_SNAKE_CASE: T = …;` extraction → `nit` (`minor` for recurrence in a previously-flagged file). Exemptions: self-evident constants (`0`, `1`, `-1`, `2`), loop indices, test fixtures whose value carries no meaning. The fix is module-private `const` at the top of the file (after `use` statements), `SCREAMING_SNAKE_CASE` naming describing the *role* (`MAX_TRACK_WIDTH`), not the shape (`NUM_47`).

### 6. Documentation conformance ([`ai-docs/doc-convention.md`](../../ai-docs/doc-convention.md))

For every changed `pub` item — `pub fn` / `pub struct` / `pub enum` / `pub trait` / `pub union` / `pub macro_rules!` and every method declared inside a `pub trait` body — verify against the convention. **Trait-impl exemption (AC4):** methods inside `impl Trait for Type {}` blocks are EXEMPT — do NOT flag missing convention sections on them. The trait *definition* is **not** exempt.

Mechanical heading scan to spot missing or out-of-order sections in a changed file:

```bash
rg '^\s*///\s*#\s*(Parameters|Returns|Type parameters|Lifetimes|Errors|Panics|Safety|Examples|See also)\b' <changed-file>
```

Flag each of the following:
- **Imperative summary line** (`Return`, `Create`, `Construct`) instead of third-person present indicative (`Returns`, `Creates`, `Constructs`).
- **Missing `# Parameters`** on a public fn / method with ≥1 argument other than `self` / `&self` / `&mut self`.
- **Section ordering violation.** Required order: Summary → free-form prose → `# Parameters` → `# Returns` → `# Type parameters` → `# Lifetimes` → `# Errors` → `# Panics` → `# Safety` → `# Examples` → `# See also`.
- **Missing `# Errors`** on a `Result`-returning public fn (also flagged by `clippy::missing_errors_doc`).
- **Missing `# Panics`** on a fn that calls `unwrap()` / `expect(…)`, indexes / slices a collection, asserts an invariant, or performs arithmetic that can overflow on plausible inputs (also flagged by `clippy::missing_panics_doc`).
- **Missing `# Safety`** on every `unsafe fn` (also flagged by `clippy::missing_safety_doc`).
- **Ad-hoc sections** (e.g. stray `# Notes`) — only the canonical headings above are allowed.

## What you do NOT check

- `cargo fmt` / formatting drift — enforced by the fix loop in the calling skill
- `cargo clippy` — same; enforced by the fix loop
- `cargo build` / `cargo check` / `cargo test` — same; enforced by the fix loop's verify step
- Anything explicitly documented as intentional in done plans
- Subjective preferences — only objective violations

## Progress file format

Use the canonical `.progress.md` format spec at [`ai-docs/templates/progress-format.md`](../../ai-docs/templates/progress-format.md). Required header fields: `**Branch:**`, `**base_commit:**`, `**Last build:**`, `**current_step:**`, `**last_passed_gate:**`, plus a `## Decisions log` section. Omit the `**Issue:**` / `**Spec:**` fields — this is review-driven, not spec-driven. `**parent_skill:**` and `**entry_args:**` are conditional re-entry fields (see canonical template); omit unless this review was spawned from a nested context.

Code-review-specific shape:

```markdown
# Progress: Codebase review [branch] — ACTIVE
_Updated: YYYY-MM-DD_

> Read THIS FIRST → code review findings. No spec/design — review-driven.

**Branch:** [branch name]
**base_commit:** [git rev-parse HEAD output]
**Last build:** not run

<!-- Compaction-recovery / re-entry fields (required): -->
**current_step:** Phase 1 — review-findings complete
**last_passed_gate:** [command | ISO-8601 timestamp | commit SHA, or `(none yet)` before any gate passes]

<!-- Optional re-entry fields: -->
**parent_skill:** [/task | /project-review | /pr-commented]    <!-- omit unless this progress file is owned by a nested skill -->
**entry_args:** [original $ARGUMENTS]    <!-- optional for /project-review; required for /task -->

## Next action

**Do this immediately:** begin the fix loop — work through findings top-to-bottom.

## Subtasks

- [ ] 1. Fix blocker/major findings
- [ ] 2. Fix minor findings
- [ ] 3. Fix nits
- [ ] 4. Verify: cargo build + test + clippy
- [ ] 5. Self-review

## Decisions log

- **Phase 1 — review-findings**: [one-line note per non-trivial decision]

## Key discoveries (don't re-investigate)

[anything non-obvious learned while reading the code]

## AC Status

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | `file.rs:N` — description | major | ⬜ Open |

## Files touched

(populated during fix loop)
```

The five new fields (`current_step`, `last_passed_gate`, `parent_skill`, `entry_args`) plus the `## Decisions log` section exist for compaction-recovery routing in the calling skill. This Subagent writes the initial values at file creation; subsequent updates are owned by the calling skill (`/project-review`) at each phase boundary. **What you do / do not check** on these fields: verify they are PRESENT in the file you create; do NOT review their content for correctness — the canonical template at [`ai-docs/templates/progress-format.md`](../../ai-docs/templates/progress-format.md) is the source of truth, and downstream lifecycle (writes after creation) is the calling skill's responsibility.

## Rules

- Every finding must have a file and line number.
- Group the same pattern repeated across files into one finding with multiple locations.
- Maximum 25 findings. If more exist, list the 25 most severe.
- Cross-reference done plans before raising a finding — if it's documented there, skip it.
- Severity: `blocker` · `major` · `minor` · `nit`
