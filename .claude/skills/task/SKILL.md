---
name: task
description: "Full task workflow from a user description OR a GitHub issue number: interview → spec → design → design-review → impl → verify → self-review. Steps are strictly ordered and cannot be skipped."
disable-model-invocation: true
argument-hint: "[issue-number | task description]"
allowed-tools: Bash(cargo build) Bash(cargo test *) Bash(cargo clippy *) Bash(cargo fmt *) Bash(cargo doc *) Bash(git diff *) Bash(git rev-parse *) Bash(git checkout *) Bash(git branch *) Bash(git add *) Bash(git commit *) Bash(git push *) Bash(gh issue list *) Bash(gh issue view *) Bash(gh issue create *) Bash(gh issue comment *) Bash(gh pr create *) Bash(gh pr view *)
---

Full workflow for a task. Steps execute **strictly in sequence** — proceeding to N+1 before N is complete is FORBIDDEN.

> **Commit authorization.** The default rule "only commit when the user explicitly asks" does **not** apply inside this workflow. Commits at Step 8 (per subtask) and the commit + `git push` + `gh pr create` at Step 12 are pre-authorized by `/task` itself — perform them without an extra prompt. Pause to confirm only if the situation is ambiguous beyond the prescribed step (e.g., commits would touch main, files outside the task scope, or sensitive paths).

The task may originate from either:
- a **GitHub issue number** (e.g. `/task 42` or `/task #42`) — `/interview` reads the issue body during Steps 1–5
- a **user description** (e.g. `/task add foo to bar`) or empty (`/interview` interviews the user)

> **⚡ Compaction recovery check — read FIRST on every invocation.**
> If you are re-entering this skill after auto-compaction (a
> summary/compaction block appears at the top of context, or workflow
> context feels thin), STOP before any tool call and:
>
> 1. **Locate the durable-state file via this skill's active-state probe**
>    — run the preamble glob (`ls ai-docs/plans/*.progress.md 2>/dev/null`) and apply the validation it
>    documents (stale-merge, branch-match, or PR-linkage as the preamble
>    prescribes). The probe both finds the path AND decides whether to
>    RESUME, delete, park, or treat the situation as fresh.
> 2. Once the probe identifies the correct durable-state file
>    (the matched `ai-docs/plans/<spec-base>.progress.md`), read it **top-to-bottom in one
>    pass** — every line, including older sections and the `## Decisions
>    log` section. Do not skim. The recorded `current_step` is a
>    cross-check, never an instruction to skip the read.
> 3. **Then re-enter this skill from the top of its body** — let the
>    preamble's probe / validation / RESUME sequence route control. Do
>    NOT jump to a numbered Step directly; the preamble owns the routing.
>    The probe will land you at the right next action without re-doing
>    completed work.
>
> If the probe finds no matching durable-state file (or returns a
> validated "no active task" result), this is a fresh invocation —
> proceed normally.
>
> See `.claude/skills/context-reset/SKILL.md` § **Compaction recovery
> (re-entry)** for the canonical handoff rationale.

## ⚡ First: check for active task

```bash
ls ai-docs/plans/*.progress.md 2>/dev/null
```

> **Re-entry-after-compaction case (lost-arguments path).** If `$ARGUMENTS` is empty (lost to
> compaction) AND the glob above finds a matching file with `**entry_args:**` recorded, treat the recorded
> `entry_args` as the canonical entry reference. The `⚡ Second` and `⚡ Third` preambles must NOT fire on the **lost-arguments path** — they require a positive match against the live `$ARGUMENTS`, which by definition is unavailable on re-entry. Only `⚡ First` (active-task probe) is allowed to route a lost-arguments re-entry. If the probe finds no matching progress file, the re-entry is "fresh"; surface this to the user (do NOT proceed to Steps 1–5 silently, because the user's original task is unknown).

**If found → validate the match BEFORE jumping to RESUME.** The probe is a flat glob — it matches any `.progress.md` regardless of git branch or merge state. Run the four-step **stale-merge + branch-match** validation in `reference.md` § ⚡ First — validation sequence (detail). If either check fails, surface to user with **delete / park / RESUME anyway** options before continuing; do NOT auto-RESUME on a mismatch.

**RESUME flow (skip Steps 1–7) — only after validation passes:**
1. Read the `.progress.md` file
2. Read spec — only `## Acceptance Criteria`
3. Read only files from `## Files touched`
4. Jump to `## Next action`
5. Tell user: "Found active task [X], resuming from [subtask Y]"

---

## ⚡ Second: check for deferred plan activation

> **Guard sentence.** This preamble fires only when `$ARGUMENTS` is **non-empty**. On a lost-arguments re-entry (empty `$ARGUMENTS` after compaction), do NOT enter this preamble — fall through to `⚡ First`'s active-task probe instead. See `⚡ First`'s lost-arguments clause.

If `$ARGUMENTS` contains words like "activate", "start", "proceed" **and** a matching plan exists in `ai-docs/plans/deferred/`, run the deferred-plan activation sequence in `reference.md` § ⚡ Second — deferred plan activation sequence: move spec/design (and `.progress.md` if present) from `deferred/` to `ai-docs/plans/`, update `INDEX.md`, verify `**Tracked in:**`, then resume the moved `.progress.md` if any (else skip Steps 1–7 and jump to Step 8).

---

## ⚡ Third: bare-issue activation of a matching deferred spec

> **Guard sentence.** This preamble fires only when `$ARGUMENTS` is **non-empty** (a bare integer). On a lost-arguments re-entry (empty `$ARGUMENTS` after compaction), do NOT enter this preamble — fall through to `⚡ First`'s active-task probe instead. See `⚡ First`'s lost-arguments clause.

> **AXIOM — When `$ARGUMENTS` is a bare gh issue number, search deferred specs for `**Tracked in:** #N` BEFORE launching the interview.**
>
> | If gh issue #N has... | Action |
> |---|---|
> | Zero deferred specs with `Tracked in: #N` | Fall through to Steps 1–5 (fresh interview) |
> | Exactly one matching deferred spec | Move spec + design + progress into `ai-docs/plans/`, update INDEX.md, surface ACs for confirmation, jump to Step 6 |
> | Multiple matches | Surface to user |
>
> A bare integer does NOT trigger the keyword preamble (`⚡ Second`), so `/task 47` would otherwise spin up a spurious interview state file even when a deferred spec already carries `**Tracked in:** #47`. See `reference.md` § ⚡ Third — bare-issue activation decision table (detail) for the full resolution recipe.

Activation sequence (bare-issue → matching deferred spec): parse `$ARGUMENTS` (strip leading `#`), load `gh issue view <N> --json title,body,state,labels`, grep `ai-docs/plans/deferred/*.spec.md` for `**Tracked in:** #N`. **Zero matches** → fall through to Steps 1–5. **One match** → move the spec (and `*.design.md` / `*.progress.md` siblings if present) into `ai-docs/plans/`, update `INDEX.md`, surface the spec's ACs for user confirmation, do NOT re-run interview, do NOT create `*.state.md`, then jump to Step 6 (or RESUME if a `.progress.md` came along). **Multiple matches** → surface to user. Full sequence: `reference.md` § ⚡ Third — bare-issue activation sequence (full).

---

## ⚡ Fourth: blocked-label reconciliation

> **Guard sentence.** Fires when the resolved input is a gh issue — bare-number `/task <N>` (whether `⚡ Third` matched a deferred spec or fell through), OR `⚡ Second`/`⚡ Third` activated a deferred spec whose `**Tracked in:** #N` points to a real issue. Free-text `/task` invocations with no issue reference skip this preamble entirely. Runs AFTER `⚡ Third` and BEFORE Steps 1–5 / Step 6.

If the resolved issue's `labels` array (already fetched by `⚡ Third` step 2) contains `blocked`, reconcile before proceeding:

1. Enumerate blockers from the issue body — `Blocked by #M` / `Depends on #M` references. Treat any free-text blocker ("blocked on the auth rewrite") with no `#M` form as an unresolvable open blocker for step 4.
2. Query each `#M` blocker: `gh issue view <M> --json state`.
3. **All blockers CLOSED** → `gh issue edit <N> --remove-label blocked` (the stale label removal is part of the deliverable; it keeps `/next` / `/triage` filter accuracy honest), then continue with the normal flow.
4. **At least one blocker OPEN, or any unresolvable free-text reference** → pause and ask the user one of:
   - which blockers to wait on (do **not** start work);
   - which blockers to disregard for this issue (closed-as-not-planned, unrelated cross-ref) — if zero open blockers remain after the user's answer, proceed with step 3 (remove the label);
   - or whether to start work anyway accepting the risk — in this case do **NOT** remove the label (the issue is still semantically blocked; the user is overriding the gate explicitly).

Never proceed silently when any blocker is open. The `blocked` label is the project's gate signal; starting `/task` past it without reconciliation defeats the gate and risks producing a spec/design that hits the unresolved dependency mid-implementation.

---

### Steps 1–5: Spec creation (delegated to `/interview`)

`/task` does not duplicate the interview workflow. Treat Steps 1–5 as a single delegated phase. If a saved spec already exists under `ai-docs/plans/`, confirm with the user and skip to Step 6. Otherwise invoke `Skill(skill="interview", args="$ARGUMENTS")` — the interview handles entry-mode detection, scope confirmation, clarifying-question rounds, tracking-issue resolution, spec writing, and the cross-link comment. Spec-only runs move the spec to `ai-docs/plans/deferred/` and stop. **Before Step 6:** confirm the spec exists at `ai-docs/plans/YYYY-MM-DD-name.spec.md` and the user has approved its `## Acceptance Criteria`. Full delegation narrative: `reference.md` § Steps 1–5 — spec creation delegation (detail).

### Step 6: Design Subagent

First action: confirm the spec exists. Spawn the `design` Subagent (per `.claude/agents/design.md`) with the spec path; result: `ai-docs/plans/YYYY-MM-DD-name.design.md`.

### Step 7: Design review

Spawn the `design-review` Subagent (per `.claude/agents/design-review.md`) with the spec + design paths.

Verdict: GO / ITERATE / STOP.
- **GO** → proceed to Step 8. Spec-amending notes (AC/constraint changes) need Step 6 → Step 7 re-run, not a fold-in — see `reference.md` § Spec Amendment recipe.
- **ITERATE** → back to Step 6 (max 3 rounds total).
- **STOP** → fundamental flaw with the approach. Surface the verdict and `Issues` table to the user, do not start Step 8. Wait for direction (e.g., narrow scope, change approach, abandon).

> **AXIOM — `*.spec.md` and `*.design.md` writes are subagent-owned.**
> The `/task` orchestrator NEVER writes to `ai-docs/plans/*.spec.md` or `ai-docs/plans/*.design.md` (including `done/` siblings). All such writes go through the responsible Subagent: `spec-writer` for `*.spec.md`, `design` for `*.design.md`. Orchestrator-side direct edits with `Edit` / `Write` are FORBIDDEN. Recurrences (in the sibling **quartzite** project's log, 2026-05-24 — 4 process entries): orchestrator wrote design file when subagent's text response landed without a `Write` call; orchestrator transcribed design output instead of re-running the agent; orchestrator directly edited spec on user tweak after `status: ready`; orchestrator edited design doc during spec-amendment sub-flow.
>
> | If the orchestrator is tempted to... | Do this instead |
> |---|---|
> | `Edit` a `*.design.md` to apply a self-review finding | Spawn `design` Subagent with the finding text |
> | `Write` a `*.design.md` because the Subagent's text output didn't land on disk | Re-spawn the `design` Subagent; do NOT transcribe its text |
> | `Edit` a `*.spec.md` to apply a user tweak after `interview` returned `ready` | Spawn `spec-writer` with the tweak as a synthetic round |
> | `Edit` a `done/*.spec.md` during `/pr-commented` Spec Amendment | Same — route through `spec-writer` |

### Design Amendment (re-entrant — triggered from Step 8 or Step 11)

If implementation (Step 8) reveals a necessary deviation from the design, **or** a self-review finding (Step 11) requires a design change rather than a code fix: stop the step, surface to user for approval, **spawn the `design` Subagent to update the design doc** (orchestrator MUST NOT edit `*.design.md` directly — see the AXIOM above), re-run Step 7 design-review (max 3 rounds total). On GO → resume the triggering step. See `reference.md` § Design Amendment recipe for the full procedure.

> Silently implementing a deviation without triggering Design Amendment — FORBIDDEN.
> Orchestrator-side direct edits to `*.design.md` / `*.spec.md` — FORBIDDEN (per the AXIOM above).

---

### Step 8: Implementation

> First action: verify spec + design + GO verdict exist AND that every `note`/`minor`/recommendation from the latest design-review GO has been written back into the design document. "Applied in code later" is NOT the same as "resolved in the design"; the design doc is the implementation contract. See `reference.md` § Step 8 — first-action GO-notes verification (detail). Unresolved GO-notes = previous steps incomplete.

- **Create a feature branch immediately** — before writing any code or making any commits:
  ```bash
  git checkout -b feat/YYYY-MM-DD-name
  ```
  Use the same date-name as the spec file. Record the branch name in the progress file.
- **Before every `git commit` in this step:** run `git branch --show-current` and confirm it is NOT `main`. If it is — stop immediately, do not commit, apply the recovery procedure in AGENTS.md.
- **Before every `git commit` in this step:** check `git status` for `ai-docs/learnings.md`. If modified or untracked, stage it together with the related code changes — learnings are part of the task deliverable and must be visible in the PR diff. **After every push** (CI fix, reviewer-comment fix, self-review fix): if a learning entry was written *after* the last code commit landed, give it its own commit on the feature branch in the same turn — do not leave `learnings.md` as an unstaged working-tree change waiting to be bundled with the next code change. Order: write learning → `git add ai-docs/learnings.md` → commit → push.
- Create `ai-docs/plans/YYYY-MM-DD-name.progress.md` at start using the canonical schema at [`ai-docs/templates/progress-format.md`](../../../ai-docs/templates/progress-format.md). Required: `**Branch:**`, `**base_commit:**`, `**Last build:**`, `**current_step:**`, `**last_passed_gate:**`, `**entry_args:**`, plus a `## Decisions log` h2 section. For `/task` flows also include `**Issue:**` / `**Spec:**`. Record `**entry_args:**` (original `$ARGUMENTS` — bare ref, keyword phrase, free text, or `(none)`) ONCE and **read-only thereafter**; on lost-arguments re-entry it is the canonical entry reference. Full header template + write-once rule: `reference.md` § Step 8 — progress-file creation template (detail).
- After each subtask:
  1. `cargo build` — must compile
  2. `cargo test test_name` — if subtask adds tests
  3. `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings`
  4. Update `.progress.md` — **at this subtask boundary, rewrite `**current_step:**` to `Step 8 — subtask N of M complete`; rewrite `**last_passed_gate:**` to `cargo clippy --workspace --all-targets -- -D warnings | <ISO-8601 UTC timestamp> | <commit SHA from git rev-parse HEAD>`; append a `## Decisions log` bullet for any non-trivial choice made during this subtask (one line, prefixed `Step 8 subtask N:`; omit if none).**
  5. **Every-group handoff (binding, not optional).** During Step 8 the orchestrator NEVER executes subtask code in its own context — every group fans out through `/context-reset`, including the first group, and including designs whose total subtask count is M = 1. Spawn `/context-reset` at the start of each group per the design's `## Handoff plan` section (the per-group spec; required for every M ≥ 1 per `.claude/agents/design.md` § Rules → handoff-grouping). Between group returns the orchestrator reads the subagent's progress-file delta (`current_step`, `last_passed_gate`, tail of `## Decisions log`) and re-validates state — branch matches `**Branch:**`, `base_commit` unchanged in the progress header, no uncommitted dirt — `git diff --quiet && git diff --cached --quiet` (or an empty `git status --porcelain`), **never bare `git diff --quiet`**, which compares the working tree to the **index** and so reports a staged-only change as clean; that is exactly the pre-spawn state AGENTS.md § *Workflow* phase (2) forbids — before spawning the next group's handoff. If the runtime delta disagrees with the design's `## Handoff plan` (e.g. extra subtasks completed, group boundary moved), trigger the **Design Amendment recipe** (`reference.md` § Design Amendment recipe) — do NOT silently advance. Orchestrator model is per-invocation; pinning was considered and rejected (Key Decision Q3 of the every-group redesign). **Per-group implementor selection (distinct from that rejected orchestrator-pinning — do not conflate them).** When spawning each group's `/context-reset` handoff, the orchestrator READS that group's marked implementor model + effort from the design's `## Handoff plan` and routes accordingly: a **code** group (marked `sonnet`) → `subagent_type="code-writer"`, whose `model: sonnet` + `effort: medium` are **frontmatter-pinned** — pass NO inline `model=`/effort override, because there is no per-invocation `effort` parameter, so an inline `general-purpose` code spawn could never enforce a "medium (pinned)" tier; the file is the only lever. An **instructions/harness** group (marked `opus`) → `subagent_type="general-purpose"` with inline `model="opus"` and effort inherited — both in a 1M-token window. The *orchestrator* model itself is UNCHANGED (still per-invocation; the `design` / `design-review` / `self-review` gates stay Opus); only the **instructions/harness** *implementor* spawn takes an inline `model=` override, never the orchestrator or the quality-gate subagents. See `reference.md` § Every-group handoff (rationale) for the failure modes this prevents and `.claude/skills/context-reset/SKILL.md` for the handoff protocol.
- Unknown API → read sources → grep codebase → ask user. Don't guess.
- Bug report during impl → activate `/bugfix`, then return here.
- **Irreversible outward-facing deliverable in a Step-8 subtask → hold the publish for Step 12.** When the design places an irreversible outward-facing publish (a `gh issue comment` / PR comment, an external post) inside a Step-8 subtask, split it: the group implementor drafts the content to scratch and PREPARES — does NOT run — the publish command; the orchestrator runs it at Step 12 after self-review APPROVE (and, for a public post, explicit user confirm). Mirrors `/task`'s own outward-action-at-Step-12 structure — a wrong analysis must not go public before self-review can catch it.
- Implementation reveals design must change → trigger **Design Amendment** above, then resume here.
- **Local FAIL investigation before push (AGENTS.md workflow corollary).** When `cargo test` returns `FAILED`, isolate and reproduce the failing test before treating it as transient. See `reference.md` § Step 8 — local FAIL investigation before push for the recipe.

### Step 9: Verify

Run the full 11-step verify list in `reference.md` § Step 9 — verify list (full): `cargo build`, `cargo test`, `cargo fmt -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`, `actionlint` (only if workflows changed), panic-index sync (see `reference.md` § Step 9 — panic-index sync (detail) for the exact `rg` recipes), unsafe-index sync (see `reference.md` § Step 9 — unsafe-index sync (detail) for the exact `rg` recipes), then per-AC coverage check and a `| # | Criterion | Test / Verification | Status |` summary table. On ALL PASS → Step 9.5.

**Write progress at this step boundary** before further tool calls: rewrite `**current_step:**` to `Step 9 — Verify (ALL PASS)`; rewrite `**last_passed_gate:**` to `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace | <ISO-8601 UTC timestamp> | <commit SHA from git rev-parse HEAD>`; append a `## Decisions log` bullet recording panic-index additions and unsafe-index additions, if any (one line, prefixed `Step 9:`; omit if none).

### Step 9.5: Update documentation

Update content files only — **do not move spec/design to `done/` yet** (that happens at Step 12). Append this task's per-issue implementation-status entry to `ai-docs/context-status.md` (the detailed log), bump the affected block's summary bullet + resolve open questions in `ai-docs/context.md` (thin orientation page — keep it under the size cap), and update `README.md` (status table for newly implemented crates). Full detail: `reference.md` § Step 9.5 — documentation update (detail).

**Every number and every `file:line` you write here is a measurement, not a recollection.** A test count, a crate tally, a panic-index / unsafe-index row locator, a "N sites" figure — re-derive each one **in this turn, after the last edit**, with the command that produces it (`cargo test -p <crate> --lib` for a per-crate count; the locator `rg` re-run after the final `cargo fmt`). **NEVER** transcribe a figure from a subagent's return summary — it is a claim (AGENTS.md § *Workflow*), and a `code-writer` has already reported one crate's count for another's. If a new figure contradicts an existing durable baseline (README's test line, a panic-index row), the resolution is a **fresh measurement**, never picking one of the two.

**A diff that REMOVES something has a wider doc surface than one that adds.** Prose enumerates what exists, so a deleted dep edge / flag / module / panic-index row leaves assertions scattered through documents you are otherwise editing correctly. Adding a description of what is now true does **not** discharge the obligation to delete what is now false — and the two routinely live in the same file, paragraphs apart. Concrete trigger: for every name your diff removed, `grep -n '<removed-name>' <every doc you are touching>` **before** closing the edit. The salience of a removal that was an *acceptance criterion* is what makes this feel already-handled.

**Write progress at this step boundary** before further tool calls: rewrite `**current_step:**` to `Step 9.5 — docs updated`; append a `## Decisions log` bullet recording any open questions resolved in `context.md` (one line, prefixed `Step 9.5:`; omit if none).

Then proceed to Step 10.

### Step 10: Self-review loop (max 3 rounds)

Spawn the `self-review` Subagent with the spec, design, and progress paths (per `.claude/agents/self-review.md`).

**On APPROVE:** proceed to Step 12. The progress file is gitignored and **stays in the working tree** — `/pr-commented` extends it across reviewer rounds, `/pr-merged` deletes it post-merge. Do NOT `rm` it here. **Write progress at this step boundary** before further tool calls: rewrite `**current_step:**` to `Step 10 — self-review APPROVE (Round N)`; append a `## Decisions log` bullet recording the round count (one line, prefixed `Step 10:`).

**On REJECT:** proceed to Step 11. After Step 11, loop back here. **Write progress at this step boundary** before further tool calls: rewrite `**current_step:**` to `Step 10 — self-review REJECT (Round N), addressing findings`; append a `## Decisions log` bullet recording the finding count + severity breakdown (one line, prefixed `Step 10:`).

**After round 3 with REJECT:** surface all remaining `⬜ Open` findings to the user and ask how to proceed.

### Step 11: Review fixes

> **AXIOM — A self-review finding whose proposed fix diff touches `*.design.md` or `*.spec.md` in `ai-docs/plans/` is a Design/Spec Amendment trigger, NOT an ordinary Step-11 code-fix.**
> Mechanical detection at the top of every fix round; the trigger fires regardless of how small the doc edit appears. Recurrences (in the sibling **quartzite** project's log): 2026-05-13 (notes not folded back), 2026-05-15 GO-with-notes resolution, 2026-05-21 design doc change committed directly during self-review fix (the latest is the propagation gap — Step 11 self-review fix flow was missing from the prior escalation set; see quartzite's `ai-docs/learnings.md` 2026-05-21).
>
> | If the proposed Step 11 fix diff includes... | Action |
> |---|---|
> | A `*.design.md` file under `ai-docs/plans/` (active or `done/`) | **STOP.** Trigger the **Design Amendment recipe** above — surface to user, update the design, re-run Step 7 design-review on the amended design (max 3 rounds), then resume Step 11 from the GO verdict. Mark the originating finding `✅ Fixed (design amended)`. Do NOT commit the design-doc edit as a code-fix commit. |
> | A `*.spec.md` file under `ai-docs/plans/` (active or `done/`) | **STOP.** Trigger the **Spec Amendment recipe** (`reference.md` § Spec Amendment recipe) — surface to user, update the spec, re-run Step 6 design → Step 7 design-review on the amended (spec, design) pair, then resume Step 11. Mark the originating finding `✅ Fixed (spec amended)`. |
> | Only `*.rs` / `*.toml` / non-`ai-docs/plans/` `*.md` / other source files | Normal Step 11 code-fix path — apply, re-run gates, push. |

**Order the batch so every measurement is taken after the last content edit.** A Step-11 fix batch routinely contains both a correction to a *number about* file F and an edit *to* F; batch order does not naturally put them in the valid order. Before writing any figure into a durable surface, answer: **"does anything else in this batch touch the file I am measuring?"** If yes, that edit lands first and the measurement is re-derived after it. A figure that was correct when you measured it is not thereby correct when you commit it — this is the Step-11 instance of AGENTS.md § *Communication* ("never record-then-edit") and of Step 9.5's measurement rule, and it is where that rule has recurred.

For each `⬜ Open` finding in the latest `## Self-Review (Round N)` section of the progress file: **fix** (mark `✅ Fixed`), **design-amend** (trigger the Design Amendment recipe per the table above, mark `✅ Fixed (design amended)`), **spec-amend** (trigger the Spec Amendment recipe per the table above, mark `✅ Fixed (spec amended)`), or **object** (`nit`/`minor` autonomously; `major`/`blocker` only after user approval — mark `⚠️ Objected: <reason>`). See `reference.md` § Step 11 — review-fix narrative for the full procedure including the unconditional PR-body re-read and review-thread-resolution recipe.

After all findings are resolved, run gates (`cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`) and:

1. Update `.progress.md`.
2. **PR body sync (unconditional).** `gh pr view <N> --json title,body`, re-read, then `gh pr edit` only if the body contradicts the new commits. Never skip the read.
3. **Resolve fixed review threads (unconditional).** GraphQL recipe per `reference.md` and AGENTS.md § *Workflow* (PR review comment resolution).
4. **Write progress at this step boundary** before further tool calls: rewrite `**current_step:**` to `Step 11 — review fixes complete (Round N)`; rewrite `**last_passed_gate:**` to `cargo clippy --workspace --all-targets -- -D warnings | <ISO-8601 UTC timestamp> | <commit SHA from git rev-parse HEAD>`; append a `## Decisions log` bullet recording any `⚠️ Objected` rationale or Design-Amendment trigger (one line, prefixed `Step 11:`; omit if none).
5. Return to Step 10.

### Step 12: Finalise docs, commit, and create PR

1. **Step-skip gate (binding, not optional).** Read `**current_step:**` from `ai-docs/plans/<spec-base>.progress.md`. It MUST be one of `Step 10 — self-review APPROVE (Round N)` or `Step 11 — review fixes complete (Round N)`. If it is anything else (e.g. `Step 9 — Verify (ALL PASS)`, `Step 9.5 — docs updated`, `Step 8 — subtask N of M complete`) — **STOP**, do not proceed to commit, surface the gap to the user, and loop back to the missing step. Rationale: Step 10 (self-review) has been silently skipped on "simple" tasks and post-compaction. Recurrence pattern, all in the sibling **quartzite** project ([its log](https://github.com/maratik123/quartzite/blob/main/ai-docs/learnings.md)): 2026-05-07 design-skip; 2026-05-14 `maratik123/quartzite#339` Step-10 skip; 2026-05-14 `maratik123/quartzite#281` compaction-induced skip (an issue, not a PR); 2026-05-16 `maratik123/quartzite#374` context-rot. The gate fires regardless of how trivial the diff appears — no "too simple" exemption (quartzite's `ai-docs/learnings.md` 2026-05-07 entry). **Compaction-recovery exception:** if `**current_step:**` is unparseable or absent and the post-compaction summary suggests Step 10 ran, ask the user explicitly before treating Step 10 as satisfied — do NOT auto-pass the gate.
2. **Confirm `.progress.md` is NOT staged.** It is gitignored (`/ai-docs/plans/**/*.progress.md`); `git status` should not list it. If accidentally tracked or staged, unstage / `git rm --cached`. The file MUST stay in the working tree but MUST NOT enter the commit.
3. Confirm `git branch --show-current` is **not** `main`. If it is — stop, do not push, tell the user, apply the AGENTS.md recovery procedure.
4. **Finalise INDEX.md and move plan files:**
   - Change the plan row status to `✅ implemented (N tests)`
   - Move spec/design files to `ai-docs/plans/done/`
   - Update dependency tree and **Suggested next steps**
5. **Inbox propagation — parse the just-finalised spec (and its design if present) and append one JSON line per item to `ai-docs/deferred/_inbox.jsonl`** (row shape: [`ai-docs/templates/inbox-row.md`](../../../ai-docs/templates/inbox-row.md)). Apply the file-level dedupe rule against the thematic `.jsonl` files in `ai-docs/deferred/` (the `*.jsonl` siblings of `_inbox.jsonl`; dedupe over `.source_path` via `jq`); emit a `WARN:` line on unrecognised body shapes and continue. Full per-shape parser + dedupe rules in `reference.md` § Step 12 — inbox propagation (detail) and in [`inbox-propagation.md`](inbox-propagation.md). The Step 12 commit (sub-step 7 below) stages `_inbox.jsonl` alongside the existing artefacts.
6. `cargo build` — ensures `Cargo.lock` is refreshed and included if changed.
7. Stage all changed files: implementation files from `## Files touched`, `context-status.md` (+ `context.md` if its summary changed), `README.md`, `ai-docs/learnings.md` (if modified), updated `INDEX.md`, `ai-docs/deferred/_inbox.jsonl` (rows appended in sub-step 5), and spec/design now in `done/`.
8. Commit `feat(<crate>): <imperative summary>` with a 1–3 line body and `N new tests; all M tests green.`
9. `git push -u origin <branch>`
10. `gh pr create` with title + body — body must include **Summary** / **Tracking** (`Closes #N` for full-resolve or `Refs #N` for partial; omit if `Tracked in: none`) / **Test plan** (one line per AC + clippy/build). Full body template: `reference.md` § Step 12 — PR-body template (detail).
11. Post the PR URL to the user.
12. **Write progress at this step boundary** before further tool calls: rewrite `**current_step:**` to `Step 12 — PR opened (PR #<N>)`; append a `## Decisions log` bullet recording the PR number and the spec/design `done/` move (one line, prefixed `Step 12:`).

After the PR is created, the unconditional PR-body re-read rule (AGENTS.md *Workflow*) applies to any subsequent push on this branch: `gh pr view <N>` first, then `gh pr edit` only if the body now contradicts the diff.

**Reviewer comments arrive after Step 12** — run `/pr-commented` (one round per invocation, re-invocable). Do not re-enter `/task` for routine reviewer feedback; architectural-rework requests are the exception (`/pr-commented` bails → fresh `/task` design-review cycle).

---

## Patterns

### 1. Step 9's per-AC sweep is load-bearing, not ceremonial

*Prefer* running **each measurable AC's own command over the AC's own stated scope**,
and treating that result as authoritative. Two upstream readings do **not** override it:
`design-review`'s narrower operative reading of an AC (an AC saying "anywhere in
`crates/`, grep-clean" binds every subtask, including one a later design section adds for
a *different* AC's traceability), and a delegate's "flagged, left as-is per the design".

*Default to* **building** missing coverage over recording a PARTIAL. When a delegate
waves a clause through as "untestable on the fixture I used", check whether the coverage
is genuinely unachievable or just needs a purpose-built fixture — the delegate's local
fixture choice can silently narrow coverage below the approved spec.

Validated by [`ai-docs/learnings.md`](../../../ai-docs/learnings.md) 2026-07-23
(AC7 long-straight fixture) and 2026-07-24 (AC11 whole-`crates/` grep-clean past a
design-review GO). The delegate-skepticism half also lives in AGENTS.md § *Patterns* 1.

---

**Reference:** [`reference.md`](reference.md) — anti-patterns, gate checklist, Design Amendment recipe, validation procedures, FORBIDDEN list, Step-specific narrative detail (every-group handoff rationale, local-FAIL investigation, panic-index sync, Step 11 review-fix narrative, Step 12 inbox-propagation rules).
