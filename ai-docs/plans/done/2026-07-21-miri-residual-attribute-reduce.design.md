# Design: Attribute & classify the residual Miri wall-clock (investigation only)

**Issue:** #107
**Date:** 2026-07-21

## Approach

This is a **pure investigation task**: it edits no `crates/**/src`, no
`.github/workflows/**`, and no instruction file. The single work-product is a
**comment posted on issue #107** carrying (AC1) a ranked crate/module Miri
`exec_time` attribution table, (AC2) the isolated interpret/compile baseline,
(AC3) the irreducible-vs-candidate-reducible classification + one achievable
steady-state wall-clock figure + a GO/NO-GO, and (AC6) the two deferred
follow-ups. The only tracked-file movement is the spec/design docs relocating to
`ai-docs/plans/done/` at task close (AC5).

### Chosen method

**One serial workspace Miri run (WARM, after a separately-timed cold compile),
JSON per-test timing captured to a log file, crate identity recovered from
cargo's `Running` markers, aggregated with an awk-pretag + jq pipeline, baseline
built additively as `compile_floor` (cold `--no-run`) + `interpret_startup_floor`
(warm-run wall − Σexec_time), and the cold-equivalent wall reconstructed for
comparison to the 17m02s figure of record.**

**Why `--report-time` — consistent with the amended spec.** The spec was amended
(Step 7, user-approved) so every command string now carries `--report-time` in
the ordering `-- -Z unstable-options --report-time --format json
--test-threads=1`. The design matches that ordering exactly. The reason
`--report-time` is mandatory: bare `--format json` emits **no** `exec_time` field
on the `ok`/`failed` events — libtest only attaches `exec_time` when
`--report-time` is also present:

- `-- -Z unstable-options --format json --test-threads=1` (no `--report-time`) → `{ "type": "test", "name": "geom::graph::tests::all_helpers_are_deterministic", "event": "ok" }` (no timing)
  `[measured: cargo +nightly test -p gp-core --lib -- -Z unstable-options --format json --test-threads=1 → ok events carry no exec_time]`
- `-- -Z unstable-options --report-time --format json --test-threads=1` → `{ … "event": "ok", "exec_time": 0.00008444 }`
  `[measured: cargo +nightly test -p gp-core --lib with --report-time → "exec_time": 0.00008444 present]`

The whole methodology depends on `exec_time`. The command of record (matching the
amended spec's flag ordering verbatim):

```
/usr/bin/time -v env MIRIFLAGS=-Zmiri-tree-borrows \
  cargo +nightly miri test --workspace -- \
  -Z unstable-options --report-time --format json --test-threads=1 \
  2>&1 | tee <scratch>/miri-timing.log
```

This satisfies the spec's binding constraints: exact CI `MIRIFLAGS`, whole
`--workspace` (never `-p`), `--test-threads=1` serial, `+nightly` for the
miri-bearing toolchain. `--report-time` is an additive `-Z unstable-options`
libtest flag, not a deviation from the CI aliasing/scope contract.
(`--report-time`/`--format json` are order-independent to libtest — the measured
evidence above ran them in the opposite order — but the design pins the spec's
ordering for an exact string match.)

### Crate attribution — recovered from `Running` markers, not from `name`

The libtest `name` field is `module::…::test_fn` with **no crate prefix**
(`[measured: name = "geom::graph::tests::all_helpers_are_deterministic"]` — no
`gp_core::`), so the JSON alone cannot attribute a row to a crate. Two facts make
this clean:

1. **Every crate produces exactly one test binary** — there are **no
   integration-test targets** in the workspace (`[measured: cargo metadata
   --format-version 1 --no-deps | jq 'select(.kind[0]=="test")' → empty]`); all
   tests are `#[cfg(test)]` modules in `src`. So each crate emits one
   `unittests src/lib.rs (…/deps/<crate>-<hash>)` binary and the `Running`
   marker names the crate directly.
2. **cargo prints the `Running` marker to stderr immediately before that
   binary's JSON**, and with `2>&1` the marker reliably precedes its JSON block
   under `--test-threads=1`
   `[measured: cargo +nightly test -p gp-gen --lib … 2>&1 → "     Running unittests src/lib.rs (target/debug/deps/gp_gen-b900441bc4751292)" then "{ "type": "suite", "event": "started", … }"]`.

So the pipeline segments the merged log by `Running …/deps/<crate>-<hash>`
lines, tags each subsequent `ok`/`failed` JSON line with the current crate, and
then jq-aggregates. Under Miri the crate binary names are `gp_core`, `gp_gen`,
`gp_render`, `gp_ai` (`[measured: cargo metadata … lib targets → gp_core/gp_gen/gp_render/gp_ai]`);
`gp-game` is a binary crate whose test target's `Running` marker resolves to its
**bin name `graphite_gp`** (`[measured: cargo metadata → gp-game bin target = graphite-gp]`),
emitting a near-empty `unittests src/main.rs` binary (0 tests → 0 `exec_time`,
harmless — labelled `graphite_gp` in the table).

### Aggregation pipeline shape (AC1)

```bash
# (a) Tag each ok/failed JSON line with its crate (awk tracks current crate from Running markers)
awk '
  /^[[:space:]]*Running / {
    if (match($0, /deps\/[A-Za-z0-9_]+-[0-9a-f]+/)) {
      s=substr($0,RSTART,RLENGTH); sub(/^deps\//,"",s); sub(/-[0-9a-f]+$/,"",s); crate=s
    }
    next
  }
  /"type": "test"/ && /"event": "(ok|failed)"/ { print crate "\t" $0 }
' <scratch>/miri-timing.log > <scratch>/tagged.tsv

# (b) BY CRATE — cumulative exec_time, test count, ranked
jq -Rn '
  [ inputs | split("\t") | {crate: .[0], j: (.[1] | fromjson)}
    | {crate, name: .j.name, t: .j.exec_time} ]
  | group_by(.crate)
  | map({crate: .[0].crate, tests: length, total: (map(.t) | add)})
  | sort_by(-.total)' <scratch>/tagged.tsv

# (c) BY MODULE — first 2 name components as module key, top contributors
jq -Rn '
  [ inputs | split("\t") | {crate: .[0], j: (.[1] | fromjson)}
    | {key: (.crate + "::" + ((.j.name | split("::"))[0:2] | join("::"))), t: .j.exec_time} ]
  | group_by(.key)
  | map({module: .[0].key, tests: length, total: (map(.t) | add)})
  | sort_by(-.total)' <scratch>/tagged.tsv
```

Module depth (first 2 components) is a starting granularity; deepen only for the
one or two crates that dominate. `sort_by(-.total)` yields the ranked table AC1
requires.

### Baseline isolation (AC2) — explicit warmth model

libtest `exec_time` is **only the test-function body time under the Miri
interpreter** — it excludes per-binary Miri startup (interpreting `std`
init/`lang_start` for each binary) and excludes the under-Miri compile
`[derived → subtask 4 cross-check: Σ per-test exec_time ≈ each binary's suite exec_time, which is strictly < that binary's wall share — the gap is the excluded startup]`.

**Warmth is pinned, not incidental.** The under-Miri compile is expensive and
must land in exactly one measured phase, so the ordering fixes it deterministically:

1. **`compile_floor` (COLD)** — subtask 2 cleans `target/miri`, then times
   `cargo +nightly miri test --workspace --no-run`: the full cold crate compile
   under Miri. This leaves `target/miri` **warm** (all test binaries built).
2. **`T_run_warm` (WARM)** — subtask 3's record run **depends on subtask 2** and
   therefore always runs against those warm binaries: `/usr/bin/time -v` around
   `cargo +nightly miri test --workspace -- …json --report-time` measures
   `T_run_warm` = per-binary interpret startup + Σexec_time, with **no** compile.
   The JSON from this same run is the clean per-test attribution source.

The record run is warm **by construction** (the 2→3 dependency), so its measured
wall (`T_run_warm`) deterministically excludes compile — never incidentally
warm/cold. Deriving the floors:

- `Σexec_time` = jq `add` over all per-test `t` (pipeline (b) total-of-totals).
- `interpret_startup_floor = T_run_warm − Σexec_time` — both terms come from the
  **same warm run**, so this is **always ≥ 0** (wall ≥ sum of test bodies); the
  earlier "`T_wall − compile_floor`" form that could go negative is gone.
- `baseline_total = compile_floor + interpret_startup_floor` — the fixed floor
  (cold compile + per-binary interpret startup + harness overhead), built
  **additively** rather than by subtraction.
- `sysroot_setup` = elapsed(`cargo +nightly miri setup`), warm — **one-time,
  EXCLUDED** from steady-state (CI caches it via `cache-shared-key
  nightly-miri-v1`, `[measured: ci.yml miri job → components: miri, rust-src; cache-shared-key ${{ runner.os }}-nightly-miri-v1]`).
  The Miri **sysroot lives in `~/.cache/miri`, OUTSIDE `target/miri`**
  (`[measured: ls -d ~/.cache/miri → present; separate tree from target/miri]`),
  so cleaning `target/miri` (subtask 2) does **not** disturb the warm sysroot —
  and therefore `compile_floor` measures **only** the crate/dep compile, never a
  sysroot rebuild. That exclusion is asserted in the #107 comment prose so the
  `compile_floor` figure is not misread as including the sysroot build.

**Cold-equivalent wall of record + anchor selection.** #108's 17m02s is the
figure to reconcile against, but **whether it was a cold (compile-included) or a
warm (compile-excluded) run is an assumption, not a measured fact** — #108 timed
`cargo +nightly miri test --workspace` without recording its `target/miri`
warmth `[assumption — unverifiable from #108's record; NOT a measured claim]`. The
design therefore computes **both** candidate anchors and lets the data pick:

- `T_wall_cold = compile_floor + T_run_warm = baseline_total + Σexec_time` — the
  cold-equivalent full-run wall (matches 17m02s **if** #108 was cold).
- `T_run_warm` — the warm-run wall (matches 17m02s **if** #108 was itself warm,
  e.g. a repeated local run over an already-built `target/miri`).

**Anchor rule:** whichever of `{T_wall_cold, T_run_warm}` reconciles **closer** to
17m02s is the anchor for the headline `achievable_steady_state`; if `T_run_warm`
is the closer match, the WARM figure is the anchor. The **full decomposition**
(`compile_floor`, `interpret_startup_floor`, `Σexec_time`, both candidate walls)
is reported regardless, so the deliverable is robust to either reading of #108 —
only which single number leads the headline shifts. The #107 comment states the
cold-vs-warm assumption explicitly in prose (Decomposition subtask 7) so a reader
can re-anchor if #108's warmth is later pinned.

Cross-check the JSON capture against libtest's own rollup: each binary's
`suite` `ok` event carries `passed`/`failed`/`ignored` counts and an aggregate
`exec_time` (`[measured: gp-gen suite ok → "passed": 2, "failed": 0, "ignored": 0, "exec_time": 0.00023362]`);
Σ(per-crate captured `ok`+`failed` rows) must equal that binary's suite
`passed`+`failed`, and Σ(per-crate test `exec_time`) must ≈ its suite
`exec_time` — a self-consistency guard that the awk segmentation lost or
mis-tagged no rows. (Using the suite `ok` event's own `passed`/`failed`/`ignored`
fields avoids any ambiguity about whether `started.test_count` counts ignored
tests.)

### Classification & steady-state (AC3)

Map each attribution row to one of two buckets:

- **Irreducible-under-Miri** = gp-core integer-physics/geom tests (the exact
  deterministic-sim UB coverage Miri exists to provide — non-negotiably kept,
  spec Key-decisions + Out-of-scope) **+ `baseline_total`** (compile + interpret
  startup: gating tests never removes it — a binary with ≥1 remaining test still
  pays its full startup, so the baseline is irreducible by construction
  `[derived → per-binary compile+startup is test-count-independent]`).
- **Candidate-reducible** = any *surprising* heavy non-physics contributor whose
  `exec_time` could be gated with `#[cfg_attr(miri, ignore)]` **without** trading
  away UB coverage Miri provides (e.g. a heavy pure-logic gp-render test that
  Miri gives no extra safety signal on).

Then (all anchored to the **selected anchor** — `T_wall_cold` or `T_run_warm`,
whichever reconciles closer to #108's 17m02s per the anchor rule above; call it
`T_anchor`):

- `achievable_steady_state = T_anchor − Σexec_time(candidate-reducible tests)`,
  i.e. the anchor minus only the gate-able cost. Equivalently, when the anchor is
  `T_wall_cold`, this is `baseline_total + Σexec_time(irreducible tests)`. The
  reducible ceiling is bounded by `Σexec_time(gate-able tests)` **only** — never
  by the baseline (`compile_floor + interpret_startup_floor` survives any gating,
  since a binary with ≥1 remaining test still compiles and pays full startup
  `[derived → per-binary compile+startup is test-count-independent]`).
  State this bound explicitly in the comment.
- **NO-GO path** (fully valid, per spec Open-questions): if no candidate-reducible
  heavy contributor exists, `achievable_steady_state ≈ T_anchor ≈ 17 min`,
  recommend NO-GO, and that stands as the attributed steady-state (this hands the
  deferred B.2 guard its threshold input).
- **GO path**: if a reducible contributor is found, `achievable_steady_state =
  T_anchor − Σexec_time(that contributor)`; recommend GO as a **deferred**
  future `/task` (no gate applied here).

gp-core dominance is a *finding to report*, not a reduction target (spec).

### Rejected alternatives

- **Per-crate `-p` runs for clean crate attribution** — rejected: the spec
  binds "never a narrower `-p` run; a single clean workspace run is the source of
  the exec_time data." The `Running`-marker segmentation recovers crate identity
  from one workspace run, so `-p` is unnecessary anyway.
- **A bash `time`-per-test wrapper / pipe harness** — rejected: the spec's Key
  decision picks built-in libtest per-test timing, "no bash-pipe wrapper."
  libtest `exec_time` is the timing of record.
- **Parsing `name` for the crate** — impossible: `name` has no crate prefix
  (measured). Segmentation by `Running` markers is the only workspace-single-run
  source of crate identity.
- **A `--report-time`-less command** — moot after the Step-7 amendment folded
  `--report-time` into the spec; measured, that form emits no `exec_time` and
  would waste the ~17-min pass, which is exactly why the amendment added the flag.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Confirm flag pass-through under Miri.** Run the amended-spec flag set on the 2-test `gp-gen` crate under Miri (`cargo +nightly miri test -p gp-gen --lib -- -Z unstable-options --report-time --format json --test-threads=1`) and confirm `ok` events carry `exec_time` **through `cargo miri test`** (not just stable `cargo test`). This one narrow `-p` call is a *pre-flight flag check on a 2-test crate*, not the attribution run — the attribution run (subtask 3) is the mandated single `--workspace` pass. | none (scratch only) | — |
| 2 | **Measure baseline components (COLD compile).** Warm `cargo +nightly miri setup` (time it → `sysroot_setup`, one-time/excluded); **confirm the sysroot cache `~/.cache/miri` is present/warm BEFORE cleaning** (it lives outside `target/miri`, so the clean must not force a sysroot rebuild — if absent, re-run setup first). Then **clean `target/miri`** and time `cargo +nightly miri test --workspace --no-run` (→ `compile_floor`, the cold crate/dep compile only, **excluding** the sysroot build). This deliberately leaves `target/miri` **warm** so subtask 3's record run is warm-by-construction. Record all three. | none (scratch only) | 1 |
| 3 | **Record run (WARM, depends on subtask 2's build).** Execute the amended-spec workspace Miri command (`-- -Z unstable-options --report-time --format json --test-threads=1`) wrapped in `/usr/bin/time -v`, `2>&1 | tee <scratch>/miri-timing.log`; capture `T_run_warm` (interpret startup + Σexec, **no** compile — the binaries are already built by subtask 2). Confirm every `suite` event is `ok` (no Miri abort dropped a crate; spec technical constraint — all 170 passing tests must complete). **Depends on 2** so warmth is deterministic, not incidental. | none (scratch only) | 2 |
| 4 | **Aggregate.** Run the awk-pretag + jq BY-CRATE and BY-MODULE pipelines over the log → ranked attribution table; run the suite-`passed`/`failed`-vs-captured-rows + suite-`exec_time`-vs-Σ self-consistency cross-check. | none (scratch only) | 3 |
| 5 | **Isolate baseline + select anchor.** Compute `Σexec_time`; `interpret_startup_floor = T_run_warm − Σexec_time` (≥ 0 by construction); `baseline_total = compile_floor + interpret_startup_floor` (compile is crate/dep only — sysroot excluded, subtask 2); form BOTH candidate anchors `T_wall_cold = baseline_total + Σexec_time` and `T_run_warm`, and pick `T_anchor` = whichever reconciles closer to #108's 17m02s (per the anchor rule; cold-vs-warm of #108 is an assumption). Note `sysroot_setup` excluded. | none (scratch only) | 2, 4 |
| 6 | **Classify + steady-state + GO/NO-GO.** Bucket rows into irreducible (gp-core physics + baseline) vs candidate-reducible; compute `achievable_steady_state = T_anchor − Σexec(reducible)`; form the GO/NO-GO recommendation (NO-GO if no lever — valid). | none (scratch only) | 4, 5 |
| 7 | **Draft the #107 comment body** to `<scratch>/107-comment.md`: attribution table (by crate + by module) → baseline isolation (**stating `compile_floor` EXCLUDES the sysroot build — sysroot lives in `~/.cache/miri`, outside `target/miri`**) → the **cold-vs-warm anchor assumption** (which of `T_wall_cold`/`T_run_warm` was used and why, so a reader can re-anchor) → classification → steady-state figure → GO/NO-GO → the two Deferred items stated as future `/task` runs (AC6). **State explicitly that a future B.2 CI wall-clock threshold must be derived from `T_wall_cold` (CI runs cold), NOT `T_run_warm`** — if the headline anchor landed on the warm figure it understates the CI wall by `compile_floor`, so the deferred B.2 guard consumes the cold decomposition component regardless of which figure led the headline. | `<scratch>/107-comment.md` (scratch, untracked) | 6 |
| 8 | **Post + verify.** `gh issue comment 107 --body-file <scratch>/107-comment.md` (never `--body`); then confirm AC5 with a whole-tree `git status --porcelain` showing no change (catches `Cargo.lock` and any repo-root file, not just `crates/` `.github/` `AGENTS.md` `.claude/` `ai-docs/*.md`). | none (gh + git verify only) | 7 |

No `crates/**/src` edit, no workflow edit, no instruction-file edit at any step
(AC5 / Out-of-scope). The scratch log and comment body live under the session
scratchpad, outside the repo.

## Handoff plan

**M = 8.** All eight subtasks are the **same change-type — instructions/harness
(analysis/harness)**: they run measurement commands and post a GitHub comment;
none edits a tracked `*.rs` file (no code change-type is present anywhere in this
task — the spec is explicit that there is no source-code implementation). Per the
spec framing, this is analysis/harness work, so it routes to
**`general-purpose` + `opus`**, **not** `code-writer` (there is no code to
implement).

- **Group A (terminal)** — model **`opus`**, effort **inherited from the
  orchestrator (typically xHigh) — NOT pinned**, 1M-token window, via the
  **`general-purpose`** subagent — subtasks **1–8**.
  - **Entry handoff:** at the start of Group A, spawn `/context-reset` per
    `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry)
    (the every-group handoff contract fires on the first group too).
  - **Single terminal group** (8 subtasks; within `1..=10`). One group is
    **mandated by group-minimization** (§ Rules (f)): all 8 subtasks share one
    change-type and one model, the dependency chain is linear and forces no
    change-type alternation, and 8 ≤ the size cap of 10 — so the fewest-groups
    packing is exactly one group. Splitting the data-gathering (1–6) from the
    deliverable (7–8) into two *same-model* groups would be an avoidable
    non-minimized group-count (a `major` design defect), so despite the
    natural data→deliverable phasing they stay in one group. The large JSON
    record run is kept out of context by `tee`-ing to a log file and reading it
    back, so one group does not bloat context.
  - No inter-group handoff (single group). Group A completes its Step 8 in its
    own `/context-reset` subagent.

Group count = **1** (≤ the default max of 4; no user gating needed).

## Risks

- **`--report-time` might not pass through `cargo miri test`** (only proven on
  stable `cargo test`) → timing-less JSON wastes the ~17-min run: confirmed
  cheaply on the 2-test gp-gen crate under Miri *before* the workspace run —
  `[measured: --report-time required + working on stable cargo test → exec_time appears; derived → subtask 1 confirms miri pass-through on gp-gen]`.
- **`2>&1` mis-orders `Running` vs JSON → mis-attribution**: mitigated by
  `--test-threads=1` (serial binaries) and the measured ordering (marker precedes
  its JSON block); the suite `test_count`-vs-captured-rows cross-check (subtask 4)
  catches any lost/mis-tagged row — `[measured: gp-gen 2>&1 → Running line precedes suite started]`.
- **Miri aborts mid-run, cargo fail-fast drops later crates → incomplete data**:
  suite is green (#108, 170 passing); subtask 3 asserts every `suite` event is
  `ok` and does **not** introduce a new abort (gating is out of scope) —
  `[derived → subtask 3 all-suites-ok check]`.
- **Wall-clock variance (warm vs cold caches) confounds the baseline split**:
  warm the sysroot first (`cargo miri setup`), measure `compile_floor` via
  `--no-run` and the run phase separately, and report the machine + method in the
  comment so the figure is reproducible — `[derived → subtask 2/3/5 record method + machine]`.
- **`gp-game` (binary crate) emits a `unittests src/main.rs` 0-test binary**:
  0 `exec_time`, harmless; its `Running` marker resolves to the **bin target
  name `graphite_gp`** (not `gp_game`), so the table labels that row `graphite_gp`
  accurately rather than dropping it — `[measured: cargo metadata → gp-game bin target name = graphite-gp → deps marker graphite_gp; 0 test files]`.
- **AC5 accidental tracked-file change** (e.g. a stray `Cargo.lock` touch from
  `cargo miri` / `cargo metadata` — `Cargo.lock` is at repo root, *outside* the
  spec's named `crates/`/`.github/`/`AGENTS.md`/`.claude/`/`ai-docs/*.md` paths):
  subtask 8's closing gate is a **whole-tree `git status --porcelain`** (not a
  path-scoped `git diff --stat`), so a root-level file is caught; scratch
  artifacts live outside the repo — `[derived → subtask 8 git status --porcelain clean]`.

## Test Design

No code is written, so there are no `#[cfg(test)]` units. The equivalent
verification plan validates the **measurement methodology**:

- **Flag pass-through (subtask 1)** — Location: gp-gen (2 tests) under Miri.
  Entry point: the corrected flag set. Assert: `ok` events carry `exec_time`.
  Scenario covered: the single riskiest unknown (does `--report-time` survive
  `cargo miri test`) is discharged on a ~seconds-scale run before the 17-min one.
  `[measured on stable; derived → subtask 1 discharges on miri]`
- **Segmentation correctness (subtask 4)** — cross-check: Σ(captured `ok`+`failed`
  rows per crate) == that binary's `suite` `ok` event `passed`+`failed` fields;
  Σ(per-test `exec_time`) ≈ binary `suite` `exec_time`. Uses the suite `ok`
  event's own counts (not `started.test_count`) to sidestep any ignored-count
  ambiguity. Fails loud if awk lost or mis-tagged a row.
  `[measured: suite ok event carries passed/failed/ignored + exec_time]`
- **Warmth determinism (subtasks 2→3)** — `interpret_startup_floor = T_run_warm
  − Σexec_time` must be ≥ 0; a negative value means the record run was not warm
  (subtask 2 didn't build, or `target/miri` was cleaned between 2 and 3) — the
  dependency ordering prevents it, this is the guard. `[derived → subtask 5 non-negativity check]`
- **Anchor reconciliation (subtask 5)** — at least one of `{T_wall_cold =
  compile_floor + T_run_warm, T_run_warm}` must land in the neighbourhood of
  #108's 17m02s; the closer one is `T_anchor`. If NEITHER is close, a warmth or
  machine-variance confound is flagged and explained in the comment (the cold-vs-warm
  state of #108 is an assumption, not a measured fact).
  `[derived → subtask 5 dual-anchor reconciliation against 17m02s]`
- **Completeness (subtask 3)** — every `suite` event is `ok` (no dropped crate).
- **AC5 no-change gate (subtask 8)** — a whole-tree `git status --porcelain` is
  empty (catches `Cargo.lock`/repo-root files, not just the spec's named paths);
  the only tracked movement (plan docs → `done/`) happens at task close, outside
  these subtasks. `[derived → subtask 8]`
- **Deliverable presence (AC4)** — after `gh issue comment 107 --body-file`, the
  comment carries all of: by-crate + by-module table, baseline isolation,
  classification, one steady-state figure, GO/NO-GO, and the two Deferred items.

No fixtures/helpers beyond the scratch log and comment body file.

## Open questions

- **Which crate/module dominates** is the finding this task produces, not
  pre-decidable — the three hypotheses (gp-core's 98 physics tests as the floor
  `[measured: gp-core lib suite test_count = 98]`; the Miri interpret/compile
  baseline; heavier-than-assumed gp-render pure-logic tests) are resolved by the
  attribution table, not by this design.
- **Whether any reduction lever exists at all** is an output of the
  classification (§6), not knowable now. "No lever → NO-GO, ~17 min is the
  steady-state" is a fully valid, non-shortfall result (spec Open-questions) that
  retires the reduction question and feeds the deferred B.2 guard its threshold.
