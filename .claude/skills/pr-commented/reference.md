# `/pr-commented` reference

Static reference content extracted from `SKILL.md`. Loaded on demand when:

- The Spec Amendment recipe's detection trigger fires (`git diff --name-only <round-M-base-sha>..HEAD | grep '.spec.md$'` returns ≥ 1 file).
- An edge case appears (consult the table for the matching row's action).
- A round boundary needs the re-invocation contract (no actionable threads, reviewer-resolved threads, etc.).

Step 0 GraphQL `reviewThreads` snapshot + REST `issues/<N>/comments` / `pulls/<N>/reviews` / `git log` recipes stay in `SKILL.md` — they are workflow-time, run on every Step-0 execution. Step 1 `## Comment cycle round M` template stays in `SKILL.md` — it is round-template scaffolding the orchestrator writes into during each round. Step 2 classification table + pause-trigger list stay in `SKILL.md` — they are workflow-time, consulted for every unresolved thread on every Step-2 execution.

## Spec Amendment recipe (pr-commented surface)

Fires BEFORE Step 5 when the round's diff touches `ai-docs/plans/*.spec.md` (or `done/*.spec.md`). Mirrors the `/task` Step 7 *Spec Amendment recipe* rule — same root cause, same remedy in every downstream "fix" skill.

| Detection trigger | Action |
|---|---|
| `git diff --name-only <round-M-base-sha>..HEAD \| grep -E '^ai-docs/plans/(done/)?.*\.spec\.md$'` returns ≥ 1 file | The round is **spec-amending**. PAUSE before Step 5. |
| Diff contains no `.spec.md` files | Proceed straight to Step 5 (self-review). The recipe does not fire. |

**When spec-amending, run this sub-flow instead of going straight to Step 5:**

1. Re-run **`/task` Step 6 (`design` Subagent)** against the amended spec — spawn the `design` Subagent with `(amended spec, current design)` and prompt: *"the spec was amended during `/pr-commented` Round M; verify the decomposition + ACs still hold against the new spec, and update the design accordingly. The implementation has already landed in commit `<round-M-fix-SHA>`."* Expected output: a refreshed design doc (`ai-docs/plans/*.design.md` if extant, otherwise an inline analysis).
2. Re-run **`/task` Step 7 (`design-review` Subagent)** with `(amended spec, refreshed design, round-M-fix diff)`. Expected verdict: GO, NEEDS-CHANGES, or REQUEST-USER. On NEEDS-CHANGES → loop back to sub-flow Step 1 (cap 3 design rounds total, matching `/task` Step 7's cap). On REQUEST-USER → surface and stop.
3. Only on a GO verdict from design-review: resume `/pr-commented` Step 5 (self-review). `self-review` operates on a code-vs-spec diff; it cannot validate that the spec → design → implementation chain still holds after a spec amendment — that's what the design-review re-entry does.

**Why:** a spec amendment can introduce contradictions, unresolved decomposition items, or new ACs that only a fresh design-review pass against the amended spec catches; `self-review` checks code-against-spec, not spec-against-design. Recurrence root cause: the `/task` Step 7 rule was not propagated to `/pr-commented` until the second incident; this recipe block closes that gap.

**FORBIDDEN reasoning for skipping this recipe:** *"the spec amendment is just a wording fix"* / *"the spec change is mechanical"* / *"self-review will catch it"* / *"only 3 lines changed"*. All forbidden — the recipe fires on **any** `.spec.md` line in the round's diff, regardless of size. The same FORBIDDEN-reasoning principle as [`ai-docs/corrections-log.md` → FORBIDDEN reasoning for skipping a `learnings.md` write](../../../ai-docs/corrections-log.md#forbidden-reasoning-for-skipping-a-learningsmd-write).

## Re-invocation semantics

Each invocation:

- Reads all threads but acts only on those `isResolved:false` AND not in any prior round's `Diff SHA` column of the progress file.
- An objection thread that the reviewer has now replied to may re-classify on this round:
  - Reviewer accepted → reply "Thanks, resolving." + resolve.
  - Reviewer pushed back → re-classify as `fix` or `clarify` per the new wording.
- Empty actionable set → no-op. Print `No new actionable threads on PR #<N>; exiting.` and stop. Do not append an empty round to the progress file.

## Edge cases

| Case | Action |
|---|---|
| Reviewer requested force-push or rebase | Bail at Step 2 (treat as a special pause); surface request to user — autonomous force-push is forbidden by AGENTS.md |
| Main moved ahead of branch (merge conflict) | Bail at preconditions; surface for user decision (merge / rebase / defer) |
| CI is red on current HEAD | Bail at preconditions; recommend fixing CI before review-comment work |
| Reviewer-resolved a thread mid-round | Detect via `isResolved:true` at Step 0; record as `resolved-by-reviewer` in progress file; no further action |
| Thread anchored to a line that no longer exists (`isOutdated:true`) | If body is still actionable → classify as usual; if anchor is meaningless without context → `clarify` (reply asking for re-anchor) |
| Comment requests architectural rework | Bail at Step 3; route to fresh `/task` design-review |
| Comment from the PR author themselves | Treat as a note; usually `already-fixed` after the author's later commit, or `ignore-bot`-equivalent (no-action) if it's a TODO note for themselves |
| Multiple commenters disagree on the same thread | Pause at Step 2; user decides |
| Self-review REJECTs 3 times | Surface verdict and stop; do not push |
| Bot comment endorsed by a human reviewer ("Codecov is right, please add a test") | Re-classify as `fix` (or `clarify` if the human's endorsement is itself ambiguous) — bot endorsement counts |
