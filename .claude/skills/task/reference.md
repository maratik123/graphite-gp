# /task — Reference

Reference material extracted from `SKILL.md` to keep the SKILL body under the per-skill 5,000-token (~20,000-char) truncation cap. The SKILL body owns the workflow steps; this file owns reference / troubleshooting / detail material.

## ⚡ First — validation sequence (detail)

The ⚡ First preamble's glob `ls ai-docs/plans/*.progress.md` is a flat match — it ignores branch and merge state. Two failure modes have already burned cycles in this repo:

1. **Stale-merge.** The matched progress file's task already merged via a GitHub-UI merge that bypassed `/pr-merged` (gitignored `.progress.md` survived). RESUME-ing into this points at a completed task instead of starting the new one. _See the sibling **quartzite** project's `ai-docs/learnings.md` 2026-05-13 stale-`.progress.md` entry._
2. **Wrong-branch parallel PR.** The matched progress file belongs to an unrelated in-flight PR on a different feature branch. RESUME-ing here cross-contaminates the two flows. _See quartzite's `ai-docs/learnings.md` 2026-05-14 branch-aware-probe entry._

**Validation sequence (run before the RESUME jump):**

1. Read `**Branch:**` and `**base_commit:**` from the matched `.progress.md`.
2. **Stale-merge check.** `git merge-base --is-ancestor <base_commit> origin/main` — if exit code is `0`, the task's base commit is now an ancestor of `origin/main`, meaning the work merged. Stale candidate.
3. **Branch-match check.** Compare the progress file's `**Branch:**` against `git branch --show-current`. If they differ, the user is on a different branch from the progress file's owner. Wrong-branch candidate.
4. If **either** check signals a mismatch, surface the situation to the user with three options and wait for direction — do NOT jump to RESUME:
   - **delete** — `rm ai-docs/plans/<base>.progress.md`, then proceed with the new task (Steps 1–7 or whichever applies).
   - **park** — `mv ai-docs/plans/<base>.progress.md ai-docs/plans/<base>.progress.md.parked`; the `.parked` suffix takes it out of the glob, allowing the new `/task` to start cleanly. Restore via the reverse `mv` later.
   - **RESUME anyway** — user explicitly chooses to ignore the mismatch (rare; typically only when reviving an interrupted task whose branch happens to be re-checked-out).
5. If both checks pass (base_commit NOT in origin/main AND branch matches), proceed to the RESUME flow defined in the SKILL body.

## ⚡ Second — deferred plan activation sequence

If `$ARGUMENTS` contains words like "activate", "start", "proceed" **and** a matching plan exists in `ai-docs/plans/deferred/`:

1. Identify the matching `*.spec.md` (and `*.design.md` if present) in `ai-docs/plans/deferred/`.
2. Move them to `ai-docs/plans/`:
   ```bash
   mv ai-docs/plans/deferred/YYYY-MM-DD-name.spec.md ai-docs/plans/
   mv ai-docs/plans/deferred/YYYY-MM-DD-name.design.md ai-docs/plans/     # if exists
   mv ai-docs/plans/deferred/YYYY-MM-DD-name.progress.md ai-docs/plans/   # if exists
   ```
3. Update `ai-docs/plans/INDEX.md`: move the plan row from the **Deferred plans** table to the **Active plans** table and mark its status as `🟢 ready` (or `🟡 spec-only` if no design).
4. Tell the user: "Activated plan [name] — moved spec (and design) to `ai-docs/plans/`."
5. **Verify the spec carries `**Tracked in:**`** — if missing, run `/interview`'s tracking-issue resolution to find or create one and add it to the spec header before continuing.
6. If a `.progress.md` was moved: treat it as an active task — read it and resume from `## Next action` (same as the RESUME path above).
7. Otherwise (no progress file): skip Steps 1–7 and jump directly to Step 8 (spec + design already exist).

## ⚡ Third — bare-issue activation sequence (full)

Activation sequence (bare-issue → matching deferred spec):

1. Parse `$ARGUMENTS` — strip leading `#`, confirm it's a positive integer `N`.
2. Load issue body: `gh issue view <N> --json title,body,state,labels`. The `labels` field feeds `⚡ Fourth` (blocked-label reconciliation) on the next preamble.
3. Grep deferred specs for the tracking reference:
   ```bash
   grep -l "^\*\*Tracked in:\*\* #<N>\b" ai-docs/plans/deferred/*.spec.md
   ```
   If grep returns **zero matches**: fall through to Steps 1–5 (interview-driven flow). The issue body loaded in step 2 is available as context for the interview.
   If grep returns **one match**: continue with step 4.
   If grep returns **multiple matches** (unexpected — `**Tracked in:**` should be 1:1 with an issue): surface the list to the user and ask which spec to activate before proceeding.
4. Move the matched spec (and its `*.design.md` / `*.progress.md` siblings if present) from `ai-docs/plans/deferred/` to `ai-docs/plans/`:
   ```bash
   mv ai-docs/plans/deferred/YYYY-MM-DD-name.spec.md ai-docs/plans/
   mv ai-docs/plans/deferred/YYYY-MM-DD-name.design.md ai-docs/plans/     # if exists
   mv ai-docs/plans/deferred/YYYY-MM-DD-name.progress.md ai-docs/plans/   # if exists
   ```
5. Update `ai-docs/plans/INDEX.md`: move the plan row from **Deferred plans** to **Active plans**; status `🟢 ready` (or `🟡 spec-only` if no design exists yet).
6. Surface the spec's existing `## Acceptance Criteria` table to the user verbatim and ask: *"Confirm these ACs, or revise before continuing?"* — wait for the user's response before proceeding. Any revisions must be applied to the spec file before Step 6 launches.
7. **Do NOT run the interview.** **Do NOT create an `*.state.md` interview state file.** **Do NOT re-resolve the tracking issue** — the spec's existing `**Tracked in:** #<N>` is authoritative.
8. If a `.progress.md` was moved (rare — a prior `/task` run on this spec was interrupted): treat the activated task as a resume and jump to the RESUME path's `## Next action`.
9. Otherwise jump directly to **Step 6** (design phase) — the spec exists, ACs are confirmed, the interview phase is satisfied.

## ⚡ Third — bare-issue activation decision table (detail)

The keyword trigger in `⚡ Second` ("activate", "start", "proceed") does NOT fire on a bare integer, so `/task 47` would otherwise enter the interview machinery and create a spurious `*.state.md` file even when `ai-docs/plans/deferred/2026-05-01-paint-style.spec.md` already carries `**Tracked in:** #47`. The `⚡ Third` preamble catches this case.

| If `$ARGUMENTS` resolves to... | Action |
|---|---|
| A bare issue number (`/task 47` or `/task #47`) AND a deferred spec exists with `**Tracked in:** #N` matching that number | Run `⚡ Third`'s activation sequence — do NOT launch the interview, do NOT create a state file. |
| A bare issue number AND no matching deferred spec | Fall through to the Steps 1–5 interview phase (the issue's body becomes the interview seed). |
| Free text / keyword-triggered activation / empty args | Skip this phase; the active-task probe above (if applicable) or Steps 1–5 cover those entry modes. |

## Design Amendment recipe (re-entrant — triggered from Step 8 or Step 11)

If implementation (Step 8) reveals a necessary deviation from the design, **or** a self-review finding (Step 11) requires a design change rather than a code fix:

1. **Stop** the current step immediately. Do not silently continue with the deviated approach.
2. **Surface to user:** describe what changed and why the design must be updated. Wait for approval.
3. **Spawn the `design` Subagent** to update `ai-docs/plans/YYYY-MM-DD-name.design.md` to reflect the new approach. The orchestrator MUST NOT edit `*.design.md` directly — the `design` Subagent owns ALL writes to `*.design.md` (per the AXIOM in `SKILL.md` above the Design Amendment header). Orchestrator-side direct edits are FORBIDDEN.
   ```
   Agent(subagent_type="general-purpose", prompt="
     Read .claude/agents/design.md and follow it.
     Spec: ai-docs/plans/YYYY-MM-DD-name.spec.md
     Existing design: ai-docs/plans/YYYY-MM-DD-name.design.md
     Context: design must be amended during implementation / self-review — describe what changed and the user-approved direction.
   ")
   ```
   On Subagent return, immediately verify the design file was written (`ls ai-docs/plans/YYYY-MM-DD-name.design.md`). If missing — re-spawn the Subagent; do NOT transcribe its text output into the file.
4. Re-run design review — same as Step 7 (max 3 rounds total across all design-review runs):
   ```
   Agent(subagent_type="general-purpose", prompt="
     Read .claude/agents/design-review.md and follow it.
     Design: ai-docs/plans/YYYY-MM-DD-name.design.md
     Spec: ai-docs/plans/YYYY-MM-DD-name.spec.md
     Context: design was amended during implementation / self-review — describe what changed.
   ")
   ```
5. **On GO** → resume from the step that triggered the amendment:
   - Triggered from Step 8 → resume Step 8 (continue the remaining subtasks)
   - Triggered from Step 11 → mark the finding `✅ Fixed (design amended)`, then return to Step 10
6. **On ITERATE** → fix the design and re-run design review (counts against the 3-round limit).
7. **On STOP** → surface to user; do not proceed until the design issue is resolved.

> Silently implementing a deviation without triggering Design Amendment — FORBIDDEN.

## Spec Amendment recipe (re-entrant — triggered from Step 7 GO-with-notes resolution)

If a Step 7 design-review GO verdict surfaces a `note` / `minor` / recommendation whose resolution requires a change to the **spec** (wording, AC, or technical-constraints edit) — not just an in-place design fold-in:

1. **Classify each note** at Step 7 close: **design-internal** (fold into the design doc in-place; no loop — current behaviour) vs **spec-amending** (the note implies a spec wording / AC / constraint change). Mixed batches are allowed; spec-amending notes trigger this recipe, design-internal notes proceed normally.
2. **Stop before Step 8.** Do not begin implementation against the pre-amendment spec, and do not fold spec-amending notes into the design alone — FORBIDDEN. The design doc is the implementation contract built **against the spec**; if the spec changes, the contract must be re-established and re-verified.
3. **Surface to user via `AskUserQuestion`** — describe the candidate spec amendment (which AC / which line / proposed new wording) and wait for explicit approval. Two paths are common: **Path A** — amend the spec to match the design's discovered shape; **Path B** — annotate the design with a "Spec amendment / supersession" subsection (still requires the re-loop below if the design's shape effectively changes the spec). Path A is the default.
4. **On user approval — amend the spec via the `spec-writer` Subagent.** The orchestrator MUST NOT edit `*.spec.md` directly (per the AXIOM in `SKILL.md` above the Design Amendment header). Spawn `spec-writer` with the user's approved amendment as a synthetic round (`extra_context` carries the amendment description); the Subagent re-writes the spec on disk. Orchestrator-side direct `*.spec.md` edits with `Edit` / `Write` are FORBIDDEN — mirrors `.claude/skills/interview/SKILL.md` § Anti-patterns ("Mutating the spec yourself").
5. **Re-enter Step 6 (`design` Subagent)** with explicit context: "spec was amended at Step 7 GO-with-notes resolution — re-verify decomposition and ACs against the new spec":
   ```
   Agent(subagent_type="general-purpose", prompt="
     Read .claude/agents/design.md and follow it.
     Spec: ai-docs/plans/YYYY-MM-DD-name.spec.md
     Existing design: ai-docs/plans/YYYY-MM-DD-name.design.md
     Context: spec was amended during Step 7 GO-with-notes resolution.
     Re-verify decomposition and ACs against the new spec. Update the design doc to reconcile any drift.
   ")
   ```
6. **Re-enter Step 7 (design-review)** against the new (spec, design) pair — same as the original Step 7 (counts against the 3-design-round-cap, which applies to the merged total of pre- and post-amendment iterations):
   ```
   Agent(subagent_type="general-purpose", prompt="
     Read .claude/agents/design-review.md and follow it.
     Design: ai-docs/plans/YYYY-MM-DD-name.design.md
     Spec: ai-docs/plans/YYYY-MM-DD-name.spec.md
     Context: spec was amended during a previous Step 7 GO-with-notes resolution — verify the design now matches the amended spec.
   ")
   ```
7. **On the new GO** → proceed to **Step 8**. Step 8's first-action GO-notes verification ("every note / minor / recommendation from the latest design-review GO has been written back into the design document") now references the **new** GO verdict; pre-amendment notes are no longer authoritative.
8. **On ITERATE** → fix the design (or re-amend the spec if a contradiction surfaces) and re-run design-review (counts against the 3-round cap).
9. **On STOP** → surface to user; do not proceed until the design / spec issue is resolved.

> Folding a spec-amending note into the design alone is FORBIDDEN — the design would be built against the old spec without ever being verified against the new one. _See quartzite's `ai-docs/learnings.md` 2026-05-15 (process) entry on spec amendment during GO-with-notes resolution._

## Steps 1–5 — spec creation delegation (detail)

`/task` does not duplicate the interview workflow. Scope extraction, key-decision confirmation, tracking-issue resolution, spec writing, and the cross-link comment are owned by `/interview` (`.claude/skills/interview/SKILL.md`). Treat these five steps as a single delegated phase.

**Already have a spec?** If a saved spec for this task already exists under `ai-docs/plans/` (e.g. the user previously ran `/interview` to draft the spec without implementing), confirm with the user that this is the spec to implement, then **skip to Step 6** — do not re-run the interview.

**Otherwise, run the interview** by invoking the `interview` Skill via the `Skill` Tool, passing the original `$ARGUMENTS` through:

```
Skill(skill="interview", args="$ARGUMENTS")
```

The interview will:

- detect entry mode (issue ref / free text / empty) and load the issue body if applicable
- extract and confirm scope (in / out / deferred)
- ask any clarifying questions (max 4 rounds, max 3 questions per round)
- resolve or create the tracking GitHub issue
- save the spec at `ai-docs/plans/YYYY-MM-DD-name.spec.md` with `**Tracked in:** #N` and an `## Acceptance Criteria` table
- post a cross-link comment on the tracking issue (unless `Tracked in: none`)

When the Skill call returns, `/interview`'s instructions and the saved spec are both in conversation context. Resume with the next paragraph of `/task` (the spec-only check, then Step 6).

**Spec-only run.** If the user wants to stop after the interview ("just draft the spec, defer the implementation"), move the spec to `ai-docs/plans/deferred/`, update `INDEX.md` (move the row to **Deferred plans**, status `🟡 spec-only`), and **do not proceed to Step 6**. The spec can be picked up later via the deferred-plan-activation preamble above.

**Before Step 6:** confirm `ai-docs/plans/YYYY-MM-DD-name.spec.md` exists and the user has approved its `## Acceptance Criteria`.

## Step 8 — progress-file creation template (detail)

Create `ai-docs/plans/YYYY-MM-DD-name.progress.md` at start using the canonical format spec at [`ai-docs/templates/progress-format.md`](../../../ai-docs/templates/progress-format.md). Required fields: `**Branch:**`, `**base_commit:**`, `**Last build:**`, `**current_step:**`, `**last_passed_gate:**`, `**entry_args:**`, plus a `## Decisions log` h2 section. Optional: `**parent_skill:**` (when `/task` itself was invoked from another skill — rare). For `/task` flows also include `**Issue:**` and `**Spec:**`.

Record base commit, branch, and `entry_args` in the progress file header immediately:

```
**Branch:** feat/YYYY-MM-DD-name
**base_commit:** <output of `git rev-parse HEAD`>
**current_step:** Step 8 — Implementation start
**last_passed_gate:** cargo build | <ISO-8601 UTC timestamp> | <commit SHA from git rev-parse HEAD>
**entry_args:** <original $ARGUMENTS at /task entry — bare issue ref (`#348`/`348`), `activate paint-style`, free text (`add foo to bar`), or `(none)` for empty entry>
```

The `**entry_args:**` field is recorded ONCE at Step 8 creation and **read-only thereafter** — Steps 9–12 do NOT touch it. On a lost-arguments re-entry (empty `$ARGUMENTS` after compaction), this recorded value is the canonical entry reference per `⚡ First`'s lost-arguments clause.

## Step 9.5 — documentation update (detail)

Update content files only — **do not move spec/design to `done/` yet** (that happens at Step 12):

1. **`ai-docs/context-status.md`** (detailed per-issue log) — append this task's implementation-status entry: the per-issue bullet capturing design decisions, traps, and invariants worth not rediscovering (the same shape as the existing `## Status` bullets there). This is where the growing per-issue log lives — **not** `context.md`, which stays a thin orientation page under the size cap.
2. **`ai-docs/context.md`** (orientation) — update only if a block's high-level state changed: bump the `## Status` summary bullet for the affected block, resolve open questions answered during implementation, keep the Architecture / Track-artifact orientation current.
3. **`README.md`** — update the status table if a new crate was implemented.

## Step 8 — first-action GO-notes verification (detail)

Verify both spec and design (with GO verdict) exist AND that **every note / minor / recommendation from the latest design-review GO verdict has been written back into the design document**. "Applied in code later" is NOT the same as "resolved in the design"; the design doc is the implementation contract. Scan the most recent `## Self-Review (Round N)` / `## Verdict: GO` block emitted by `.claude/agents/design-review.md` — for each `## Issues` row of `Severity: note` / `minor` and each `## Recommendations` bullet, confirm the corresponding API table / helper list / risk table / decomposition section of `ai-docs/plans/YYYY-MM-DD-name.design.md` was updated to match. If any note is unresolved → stop, edit the design doc to incorporate it (and re-run design-review if the change is non-trivial per the Design Amendment rule), and only then begin coding. Missing spec, missing design, missing GO verdict, OR unresolved GO-notes = previous steps incomplete.

## Step 9 — verify list (full)

1. `cargo build` — compiles clean
2. `cargo test` — all green
3. `cargo fmt -- --check` — no formatting drift
4. `cargo clippy --workspace --all-targets -- -D warnings` — clean (`--workspace --all-targets` is required so leaf crates outside the default dep tree are linted AND bench/test/example targets are covered; the bare form misses both)
5. `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` — no doc errors or warnings (matches the doc gate in AGENTS.md § *Build & Test*)
6. **actionlint gate** — if this task created or modified any `.github/workflows/*.yml` file, run `actionlint <file>` (or pass every changed workflow file in one invocation) and require a clean pass. Skip the gate only when no workflow file was touched. See AGENTS.md *Build & Test → Workflow files*.
7. **Panic-index sync** — see `## Step 9 — panic-index sync (detail)` below.
8. **Unsafe-index sync** — see `## Step 9 — unsafe-index sync (detail)` below.
9. For each AC — confirm covered by test or manual verification.
10. Show a `| # | Criterion | Test / Verification | Status |` summary table.
11. On ALL PASS → proceed to Step 9.5.

## Every-group handoff (rationale)

During Step 8 the orchestrator NEVER executes subtask code in its own context. Every group fans out through `/context-reset` — including the first group, and including designs whose total subtask count is one. The orchestrator's role during Step 8 is strictly *to spawn group handoffs, parse each subagent's progress-file delta, re-validate state, and spawn the next group's handoff* until the design's `## Handoff plan` is exhausted. Re-state the rule to yourself before deciding the next action: *"Did I just receive a group return? Then the next action is to spawn the next group's `/context-reset` handoff, until the design's `## Handoff plan` is exhausted. No exceptions for 'one more quick subtask in this turn' or 'the first group is small enough to do inline'."* See `.claude/skills/context-reset/SKILL.md` for the handoff protocol.

**Failure modes this prevents.** Two PR-level incidents in the sibling **quartzite** project (`maratik123/quartzite`, whence this harness was adapted) motivated the every-group redesign that replaced the prior runtime-gate regime:

- **`maratik123/quartzite#339`** — a long-lived orchestrator session hit auto-compaction mid-task. The compaction summary did not reproduce the strict step sequence faithfully and Step 10 (self-review) was silently omitted.
- **`maratik123/quartzite#374`** — a sonnet-model orchestrator session hit auto-compaction mid-Step-8. The post-compaction session showed *"context rot"*: skipped Step 9 verify gates, missed the Step 10 self-review spawn, and missed the runtime handoff trigger that should have fired after the 3rd subtask. The runtime trigger was load-bearing precisely when the compacted context could no longer reproduce it — the prior runtime-gate regime relied on the same context that compaction had just degraded.

The every-group fan-out removes the failure mode structurally: the orchestrator's own context never grows long enough to trip compaction (Step 8 subtask work runs in short-lived subagent invocations), and the `## Handoff plan` is the per-group spec the orchestrator reads at each return.

**Trigger source: design's `## Handoff plan` section.** As of the every-group redesign (quartzite's PR for `maratik123/quartzite#375`), the `design` Subagent produces a `## Handoff plan` section in the design document for **every** decomposition with M ≥ 1 (per `.claude/agents/design.md` § Rules → handoff-grouping). That section names the exact group boundaries and the per-group spawn order — pre-computed at design time. Single-subtask designs (M = 1) carry a `## Handoff plan` with one group, fanned out via one `/context-reset` invocation; M = 9 → 3 groups, fanned out via 3 `/context-reset` invocations. Every M ≥ 1 design now carries explicit per-group fan-out.

## Step 8 — local FAIL investigation before push (AGENTS.md workflow corollary)

When `cargo test` returns `FAILED`, identify the specific failing test (`grep "FAILED"` on the output) and reproduce it in isolation (`cargo test test_name_substring -- --nocapture`) before deciding the failure was transient. A subsequent green run is NOT proof of transience — different test-thread assignments or environment vars (DISPLAY, WAYLAND_DISPLAY) can flip the result. Only accept "transient" when the test is known flaky AND multiple reruns are consistently green. _See quartzite's `ai-docs/learnings.md` 2026-05-11 entry for the winit-`EventLoop::new()`-on-worker-thread case._

## Step 9 — panic-index sync (detail)

Scan new/modified production sources for documented or direct panic sites and update `ai-docs/panic-index.md` if any are introduced:

- `rg '^\s*///\s*#\s*Panics' <changed-files>` — documented panic behaviour (primary signal; always present when a panic exists)
- `rg '\.expect\(|\.unwrap\(|panic!' <changed-files>` filtered to lines outside `#[cfg(test)]` modules — direct panic call sites

For each new hit, add an entry to `ai-docs/panic-index.md` (location, trigger, invariant, why not `Result`, preferred fix). Stage `panic-index.md` with the implementation commit. Skip when this task added no new production panics.

## Step 9 — unsafe-index sync (detail)

Scan new/modified production sources for `# Safety` doc sections and direct `unsafe` call sites; update `ai-docs/unsafe-index.md` if any new production unsafe sites were introduced:

- `rg '^\s*///\s*#\s*Safety' <changed-files>` — `# Safety` doc-section signal on `unsafe fn` / `unsafe trait` declarations (primary signal; required by `#![deny(clippy::undocumented_unsafe_blocks)]` on every workspace crate)
- `rg '\bunsafe\s*\{|\bunsafe\s+fn\b' --type rust <changed-files>` — direct `unsafe { … }` blocks and `unsafe fn` declarations (secondary catch-net; walk hits and skip those inside `#[cfg(test)]` modules or under `tests/` / `benches/` / `examples/`)

For each new production hit, add an entry to `ai-docs/unsafe-index.md` (Location, Why unsafe, Safety invariant, Why not safe Rust, Preferred fix). Stage `unsafe-index.md` with the implementation commit. Skip when this task added no new production unsafe sites.

## Step 11 — review-fix narrative (detail)

For each `⬜ Open` finding in the latest `## Self-Review (Round N)` section of the progress file:

- **Fix it** → mark `✅ Fixed` in the progress file, implement the change.
- **Requires a design change** → trigger the **Design Amendment** recipe above (user approval required); on return mark `✅ Fixed (design amended)`.
- **Object to it** (finding is wrong or intentionally out of scope):
  - `nit` / `minor`: Subagent may object autonomously — write reason, mark `⚠️ Objected: <reason>`.
  - `major` / `blocker`: **surface to user first** before objecting. User must approve the objection.

After all findings are resolved (`✅ Fixed` or `⚠️ Objected`), run the **full** gate set — `cargo build`, `cargo test`, `cargo fmt -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, plus `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` if any `///`/`//!` changed — **after EVERY fix, including ones an agent (or you) calls trivial: "one character", "just a typo", "doc-comment only"**. A fix's *size* is not evidence of its *risk*; "fix is one character" is a claim about the edit, never about the gates. Doc comments are compiled input, so a prose edit can fail clippy — `too_long_first_doc_paragraph` (`nursery`), `doc_markdown` / `missing_panics_doc` (`pedantic`); both groups `deny` — while build/test/fmt **and the doc gate itself** stay green: `cargo doc` does NOT subsume `cargo clippy` for doc-comment lints, and vice versa. *Corollary when fixing a DOC-1 tense violation on a long summary:* prefer **restructuring to a short first paragraph + blank `///` + a detail paragraph** over inserting the verb suffix in place — the in-place insertion is the form that trips the length lint, and the restructure satisfies DOC-1's one-line-summary intent directly (the lint and the convention agree; only the original prose disagreed with both). See `ai-docs/learnings.md` 2026-07-16 DOC-1 entry. Then update `.progress.md`, re-read the PR body (`gh pr view <N> --json title,body`) and edit only if it contradicts new commits, and resolve every fixed review thread per the GraphQL recipe in AGENTS.md § *Workflow* (PR review comment resolution) — reply via REST, query unresolved thread IDs via `reviewThreads`, then `resolveReviewThread` each fixed thread and verify `isResolved: true`. Threads behind `⚠️ Objected` findings stay open. Skipping this resolution has caused the same correction twice (`ai-docs/learnings.md` entries #33 and #44).

**Design-marker `[Xn]` references MUST be backticked** (write `` `[X3]` ``, never bare `[X3]`). A bare `[Xn]` in a `///`/`//!` doc comment reads as a broken intra-doc link (`broken_intra_doc_links` = `deny`) and fails **only** the doc gate — `cargo build`/`clippy`/`test` all stay green, so it is invisible until `RUSTDOCFLAGS="-D warnings" cargo doc` runs. Same class as the DOC-1 corollary above: the doc gate is the sole catcher. See `ai-docs/learnings.md` 2026-07-16 `[Xn]`-backtick entry.

## Step 12 — PR-body template (detail)

`gh pr create --title "..." --body "$(cat <<'EOF' ... EOF)"` — body must include:

- **Summary** (bullet list of what changed)
- **Tracking** — reference the issue captured in the spec's `**Tracked in:**` field:
  - PR fully resolves the issue → `Closes #<N>` (auto-closes on merge)
  - PR partially addresses or is related (multi-PR effort, shared umbrella issue) → `Refs #<N>` (no `Closes`)
  - Spec was written with `Tracked in: none` → omit this section
- **Test plan** (checklist: one line per AC, plus clippy/build)

## Step 12 — inbox propagation (detail)

The Step 12 sub-step 4 parser specification lives in a dedicated reference file: **[inbox-propagation.md](inbox-propagation.md)**. It covers the six shape rules (NONE / TABLE / PIPEBULLET3 / PIPEBULLET2 / BOLDBULLET / PLAINBULLET), the unrecognised-shape warning behaviour, the per-row mapping format (one JSON line per item appended to `_inbox.jsonl` — canonical row shape: [`ai-docs/templates/inbox-row.md`](../../../ai-docs/templates/inbox-row.md)), and the file-level dedupe rule against the thematic `.jsonl` files. Load it on demand when implementing or modifying Step 12's propagation logic.

**Per-step recap** of the Step 12 inbox-propagation sub-step:

- Run the parser against `ai-docs/plans/done/YYYY-MM-DD-name.spec.md` (and the matching `*.design.md` if it exists).
- Build the live dedupe set `H`: every `.source_path` value in the thematic `.jsonl` files in `ai-docs/deferred/` — every `*.jsonl` sibling of `_inbox.jsonl` (created by `/triage` as it drains the inbox into topic files; none may exist yet in a fresh repo, in which case `H` is empty), harvested via `jq -r '.source_path' <file>.jsonl | sort -u`.
- For each candidate row, dedupe at *file* granularity: if the candidate's `source_path` is in `H`, skip the entire file (all of its sections); otherwise append the JSON line to `ai-docs/deferred/_inbox.jsonl` below the existing body.
- Emit one `WARN: <spec-path> :: <section heading> — unrecognised body shape, no rows emitted` line to stdout for any section whose body matches none of the six shape rules; the row count for that section is zero and Step 12 continues normally.
- The Step 12 commit stages `_inbox.jsonl` alongside the existing artefacts.

## FORBIDDEN

- Declaring done with uncovered ACs.
- Skipping design review.
- Writing code before the spec is confirmed.
- `rm`ing `.progress.md` from within `/task` (it's gitignored and lives until `/pr-merged`).
- Staging `.progress.md` into a commit.
- Pushing from the main branch.
- Silently deviating from the design without triggering Design Amendment.

## Gate checklist

| Before | Check |
|---|---|
| Steps 1–5 | Spec saved at `ai-docs/plans/YYYY-MM-DD-name.spec.md`? `**Tracked in:** #N` present (or `none` with reason)? Cross-link comment posted on the tracking issue (unless tracking skipped)? ACs confirmed by user and verifiable? See `/interview` gate checklist for the full per-step list. |
| Step 6 | Spec exists? ACs confirmed? Not a "spec-only / defer" run? |
| Step 8 | Design doc with GO? Test Design section present? **Every note / minor / recommendation from the GO verdict written back into the design doc?** |
| Step 8 start | Feature branch created? Run `git branch --show-current` before every `git commit` — must not be `main`. `base_commit` + `branch` recorded in progress file? |
| Each subtask | `cargo build` ✅? Tests run? `.progress.md` updated? |
| Step 9 | `cargo build` ✅? `cargo test` green? `cargo fmt -- --check` clean? `cargo clippy --workspace --all-targets -- -D warnings` clean (note: `--workspace --all-targets`, not bare)? `cargo doc --no-deps --workspace` clean (with `RUSTDOCFLAGS="-D warnings"`)? `actionlint` clean on every changed `.github/workflows/*.yml` (skip if none changed)? Any new `# Panics` doc section / `.unwrap()` / `.expect()` / `panic!` outside `#[cfg(test)]` → `ai-docs/panic-index.md` updated and staged (skip when no new production panics)? Any new `# Safety` doc section / `unsafe { … }` / `unsafe fn` outside `#[cfg(test)]` → `ai-docs/unsafe-index.md` updated and staged (skip when no new production unsafe sites)? All ACs covered? |
| Step 9.5 | context-status.md entry appended + context.md summary/README.md updated? (spec/design NOT moved yet — happens at Step 12) |
| Step 10 | Self-review APPROVE? (Progress file persists in working tree — gitignored — until `/pr-merged`. Do NOT `rm` it here.) |
| Step 11 | `major`/`blocker` objections confirmed by user? Design change → Design Amendment triggered? `gh pr view <N>` re-read after every push (unconditional) — `gh pr edit` only if body contradicts new commits? |
| Design Amendment | User approved the amendment? Design review returned GO before resuming? |
| Step 12 | Branch ≠ main? INDEX.md ✅? spec/design moved to done/? `_inbox.jsonl` parsed and appended (or warning logged for unrecognised shape) and staged? `Cargo.lock` refreshed? PR body references the tracking issue (`Closes #N` or `Refs #N`)? PR created and URL posted? |
