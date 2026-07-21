# Attribute & classify the residual Miri wall-clock (investigation only)

**Source:** issue #107
**Date:** 2026-07-21
**Tracked in:** #107

## Scope

Follow-up to #106/#108. After gating every `gp-render` unit test that
constructs an `egui::Context`/painter (15 tests, merged in #108), the workspace
Miri job still runs ~17 min locally
(`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace`, measured
in #108: BEFORE 24m48s → AFTER 17m02s, Δ +15 `ignored`). That residual is
**non-Context cost** and was never attributed. This task **attributes and
classifies** it and posts the findings as a comment on #107. It is an
investigation task: **it makes no code, workflow, or instruction-file change.**

1. **Attribute the residual by crate/module.** Run the workspace Miri suite
   once under the exact CI command with per-test timing, capturing
   machine-readable durations:
   `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace -- -Z unstable-options --report-time --format json --test-threads=1`,
   then jq-aggregate each event's `exec_time` by crate and by module into a
   ranked attribution table (crate → cumulative Miri exec time, top module/test
   cost contributors). Isolate the fixed Miri interpret/compile baseline
   (sysroot build + per-crate interpret startup) from summed per-test time so
   the two floors are distinguished. Local Miri duration is the measurement of
   record — no CI-timing inspection.
2. **Classify the residual and emit the achievable steady-state number.**
   Partition the residual into *irreducible-under-Miri* (gp-core
   integer-physics tests — the deterministic sim Miri exists to check, which
   MUST stay under Miri; plus the interpret/compile baseline) vs.
   *candidate-reducible* (any surprising heavy cost that could be gated without
   trading away UB coverage Miri provides). Emit a single attributed
   **achievable steady-state wall-clock** figure and a GO/NO-GO recommendation
   on whether a future reduction is worth pursuing.
3. **Deliverable — the #107 comment.** Post the attribution table + baseline
   isolation + classification + steady-state number + GO/NO-GO recommendation
   as a **comment on issue #107**. That comment IS the primary artifact of this
   task. (The spec/design plan docs are the only files that change on disk, and
   only at task close when they move to `ai-docs/plans/done/`.)

## Out of scope

**No code, workflow, or instruction-file change of any kind** — this is a
pure investigation task. Specifically out of scope:

- Any Miri reduction — no `#[cfg_attr(miri, ignore = "…")]` added, no test
  re-gated, no `--exclude`. The GO/NO-GO recommendation is advice for a future
  task, not an action taken here.
- Any `.github/workflows/**` edit, including the B.2 CI wall-clock budget guard.
- Any `AGENTS.md` / `.claude/**` / `ai-docs/*.md` instruction or convention
  edit. No Propagation Rule fires (nothing is escalated).
- Any `crates/**/src/**` source change.
- Gating or excluding gp-core integer-physics tests from Miri — even as a
  future recommendation, this stays off the table (they are the exact
  deterministic-sim UB coverage Miri exists to provide; #106 Scope §2).
- Changing the aliasing model / `MIRIFLAGS` (`-Zmiri-tree-borrows` stays).
- Reproducing or targeting the retired ~5-min CI figure from #106 (a CI number,
  not reproducible locally; formally retired in #106's amended AC4).
- Runtime frame-time perf (#104 ear-clipping, #96 Galley reuse) — a different
  axis with ~0 Miri benefit.

## Deferred

Both action items below are deferred to **separate future `/task` runs** the
user will start from this task's findings (framed as "own /task run", not
necessarily new gh issues — file one only if judged warranted at that time):

- **Any warranted coverage-preserving Miri reduction** — gating a surprising
  heavy non-physics cost contributor if the classification (§2) flags a GO |
  requires this task's attribution table + GO/NO-GO to decide what, if anything,
  to gate | future `/task` run.
- **B.2 CI wall-clock budget guard** — a `ci.yml` backstop on the Miri job |
  its threshold can only be derived from this task's attributed achievable
  steady-state | future `/task` run.

## Key decisions

| Question | Decision |
|---|---|
| Task deliverable scope | **Attribution + classification ONLY** (Q2, round 2 — supersedes round 1's "+ reduce"). No code/workflow/instruction change. Both the conditional reduction and the B.2 guard are deferred to separate future `/task` runs. |
| Primary deliverable / artifact home | A **comment posted on issue #107** carrying the attribution table, baseline isolation, classification, steady-state figure, and GO/NO-GO. Not a committed doc, not the PR body as the primary home (the PR, if any, only moves plan docs to `done/`). |
| Timing methodology | Built-in libtest per-test timing (`-Z unstable-options --report-time --format json --test-threads=1` after the `--`), jq-aggregating `exec_time`. `--report-time` is required: libtest emits no `exec_time` field on ok/failed JSON events without it (`--format json` alone yields untimed events, measured live on nightly). Serial (`--test-threads=1`) for clean attribution. No bash-pipe wrapper. Exact jq shape left to design. |
| Measurement of record | Local `cargo +nightly miri test --workspace` wall-clock; no CI-timing inspection. |
| gp-core physics under Miri | Non-negotiable: stays under Miri regardless of its cost share. If it is the dominant floor, that is a *finding* reported in the comment, not a reduction target — now or in the deferred follow-up. |

## Technical constraints

- Verify only with the CI workspace command
  (`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace`), never
  a narrower `-p` run; `+nightly` selects the miri-bearing toolchain locally.
- The suite is currently green under Miri; the attribution run must complete all
  170 passing tests (Miri aborts on the first unsupported op and cargo's
  fail-fast drops the rest) — do not introduce a new abort. A single clean run
  under `--test-threads=1` is the source of the exec_time data.
- `--report-time` / `--format json` / `--test-threads=1` are nightly
  `-Z unstable-options`; the issue reports they pass through `cargo miri test`
  locally — the design/implementing pass confirms before relying on the JSON.
  `--report-time` is mandatory: without it libtest omits the per-test
  `exec_time` field the entire attribution methodology depends on (`--format
  json` alone yields untimed events, measured live on nightly).
- Post the #107 comment with `gh issue comment 107 --body-file <path>` (never
  inline `--body` if the body contains the substring `git commit`, per AGENTS.md
  — use `--body-file`).
- No `actionlint` gate applies (no `.github/workflows/**` edit); no Propagation
  Rule fires (no instruction-file edit).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The workspace Miri suite is run once under the CI command with `-Z unstable-options --report-time --format json --test-threads=1`; per-test `exec_time` is jq-aggregated into a ranked attribution table by crate and by module. |
| AC2 | The Miri interpret/compile baseline (sysroot build + per-crate interpret startup, independent of test count) is isolated and quantified separately from summed per-test time, so the two floors are distinguished. |
| AC3 | The residual is classified into irreducible-under-Miri (gp-core integer-physics + baseline) vs. candidate-reducible, and a single attributed **achievable steady-state wall-clock** figure plus a GO/NO-GO reduction recommendation is produced. |
| AC4 | AC1–AC3 (attribution table + baseline isolation + classification + steady-state + GO/NO-GO) are posted as a **comment on issue #107** via `gh issue comment 107 --body-file`; the comment is the deliverable. |
| AC5 | The repo source tree is unchanged: `git diff --stat` over `crates/`, `.github/`, `AGENTS.md`, `.claude/`, and `ai-docs/*.md` shows no change; the only tracked-file movement is the plan spec/design docs relocating to `ai-docs/plans/done/` at task close. |
| AC6 | Both action items — a warranted Miri reduction and the B.2 CI budget guard — are recorded in Deferred as future `/task` runs, and the #107 comment states they are deferred (so the user can start them from these findings). |

## Open questions

- **Which crate/module actually dominates** is the finding this task produces —
  not pre-decidable. The three hypotheses (gp-core 98 physics tests as the
  floor; Miri interpret/compile baseline; heavier-than-assumed gp-render
  pure-logic tests) are candidates the attribution table resolves.
- **Whether any reduction lever exists at all** is an output of the
  classification, not knowable now. "No lever found → NO-GO, ~17 min is the
  steady-state" is a fully valid, non-shortfall result — it retires the
  reduction question and hands the B.2 follow-up its threshold input.
