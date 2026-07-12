# `/pr-ci-failed` reference

Static reference content extracted from `SKILL.md`. Loaded on demand when:

- The Spec Amendment recipe's detection trigger fires (`git diff --name-only <round-M-base-sha>..HEAD | grep '.spec.md$'` returns ≥ 1 file).
- An edge case appears (consult the table for the matching row's action).
- A round boundary needs the re-invocation contract (no failing checks, resolved-without-fix, etc.).

Step 2 log-fetch + classification + reproducer tables + fallback bash stay in `SKILL.md` — they are workflow-time, consulted on every Step-2 execution.

## Spec Amendment recipe (pr-ci-failed surface)

Fires BEFORE Step 5 when the fix diff touches `ai-docs/plans/*.spec.md` (or `done/*.spec.md`). Mirrors the `/task` Step 7 *Spec Amendment recipe* rule — the same root cause surfaces here whenever a CI-fix needs to reconcile spec text with the implementation.

| Detection trigger | Action |
|---|---|
| `git diff --name-only <round-M-base-sha>..HEAD \| grep -E '^ai-docs/plans/(done/)?.*\.spec\.md$'` returns ≥ 1 file | The fix is **spec-amending**. PAUSE before Step 5. |
| Diff contains no `.spec.md` files | Proceed straight to Step 5 (self-review). The recipe does not fire. |

**When spec-amending, run this sub-flow instead of going straight to Step 5:**

1. Re-run **`/task` Step 6 (`design` Subagent)** against the amended spec — spawn the `design` Subagent with `(amended spec, current design)` and prompt: *"the spec was amended during `/pr-ci-failed` Round M; verify the decomposition + ACs still hold against the new spec, and update the design accordingly. The CI-fix implementation has already landed in commit `<round-M-fix-SHA>`."*
2. Re-run **`/task` Step 7 (`design-review` Subagent)** with `(amended spec, refreshed design, round-M-fix diff)`. On NEEDS-CHANGES → loop back to sub-flow Step 1 (cap 3 design rounds total). On REQUEST-USER → surface and stop.
3. Only on a GO verdict: resume `/pr-ci-failed` Step 5 (self-review).

**Why:** A CI-fix that also amends `.spec.md` is, by definition, no longer a pure CI fix — it has reclassified the spec contract. `self-review` checks code-against-spec, not spec-against-design; the design-review re-entry is the only gate that catches contradictions, unresolved decomposition items, or new ACs introduced by the amendment.

**FORBIDDEN reasoning for skipping this recipe:** *"the spec amendment is just to mirror the new value"* / *"only the lint output changed"* / *"self-review will catch it"* / *"the CI failure is the real fix; the spec edit is incidental"*. All forbidden — the recipe fires on **any** `.spec.md` line in the round's diff. Same FORBIDDEN-reasoning principle as [`ai-docs/corrections-log.md` → FORBIDDEN reasoning for skipping a `learnings.md` write](../../../ai-docs/corrections-log.md#forbidden-reasoning-for-skipping-a-learningsmd-write).

## Re-invocation semantics

Each invocation:

- Reads the current failing check(s) and acts only on the FIRST failing check that is not in any prior round's progress note.
- Empty actionable set (no failing checks remaining) → no-op. Print `No failing checks on PR #<N>; exiting.` and stop. Do not append an empty round to the progress file.
- A previously-failing check that the next CI run resolves silently (no new commit, just a runner re-spin) → record as `resolved-without-fix` in the progress file for traceability if the user invokes `/pr-ci-failed` and finds nothing red.

## Edge cases

| Case | Action |
|---|---|
| Multiple failing checks with diverging root causes | Handle the first failing check this invocation; advise the user to re-invoke for the next |
| Same class on multiple runners (Linux + macOS + Windows lane) | Handle as one round — the local reproducer is the same; the fix usually lands on all runners simultaneously |
| Failure reproduces locally but the fix is non-trivial (cross-cutting) | Delegate to `/bugfix` (see Step 4 delegation criteria) |
| Reviewer requested force-push or rebase in a PR comment AND CI is red | This skill does not handle reviewer comments — surface the comment exists, bail; the user runs `/pr-commented` first, then re-invokes `/pr-ci-failed` |
| Main moved ahead of branch (merge conflict) | Bail at preconditions; surface for user decision (merge / rebase / defer) |
| CI is red on main itself (not just this PR) | Bail; recommend `/main-ci-failed` for the main-side red and re-invoke `/pr-ci-failed` after main is green |
| Failure on a closed/merged PR | Bail at preconditions (PR state ≠ OPEN) |
| Self-review REJECTs 3 times | Surface verdict and stop; do not push |
| Workflow YAML failure where the fix changes runner behaviour beyond the failing job | Bail with "expand scope to /task — workflow restructure exceeds CI-fix scope" |
| Bot-side check failure (e.g. a coverage/lint bot delta) endorsed by a human reviewer | Re-classify as a fix-needed (usually `other` for project-policy questions) |
