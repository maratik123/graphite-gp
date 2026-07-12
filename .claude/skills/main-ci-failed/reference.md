# `/main-ci-failed` reference

Static reference content extracted from `SKILL.md`. Loaded on demand when:

- The Spec Amendment recipe's detection trigger fires (`git diff --name-only main..HEAD | grep '.spec.md$'` returns ≥ 1 file on the new `fix/main-ci-<run-id>` feature branch).
- An edge case appears (consult the table for the matching row's action).
- A round boundary needs the re-invocation contract (no failing checks, resolved-without-direct-fix, etc.).

Step 2 log-fetch + classification + reproducer tables stay in `SKILL.md` — they are workflow-time, consulted on every Step-2 execution.

## Spec Amendment recipe (main-ci-failed surface)

Fires BEFORE Step 5 when the fix diff touches `ai-docs/plans/*.spec.md` (or `done/*.spec.md`). Mirrors the `/task` Step 7 *Spec Amendment recipe* rule — it fans out to every downstream "fix" skill whose round can touch `ai-docs/plans/*.spec.md`, including this main-branch surface.

| Detection trigger | Action |
|---|---|
| `git diff --name-only main..HEAD \| grep -E '^ai-docs/plans/(done/)?.*\.spec\.md$'` returns ≥ 1 file on the new `fix/main-ci-<run-id>` feature branch | The fix is **spec-amending**. PAUSE before Step 5. |
| Diff contains no `.spec.md` files | Proceed straight to Step 5 (self-review). The recipe does not fire. |

**When spec-amending, run this sub-flow instead of going straight to Step 5:**

1. Re-run **`/task` Step 6 (`design` Subagent)** against the amended spec — spawn the `design` Subagent with `(amended spec, current design)` and prompt: *"the spec was amended during `/main-ci-failed` for run `<run-id>`; verify the decomposition + ACs still hold against the new spec, and update the design accordingly. The CI-fix implementation has already landed in commit `<fix-SHA>` on branch `fix/main-ci-<run-id>`."*
2. Re-run **`/task` Step 7 (`design-review` Subagent)** with `(amended spec, refreshed design, fix diff)`. On NEEDS-CHANGES → loop back to sub-flow Step 1 (cap 3 design rounds total). On REQUEST-USER → surface and stop.
3. Only on a GO verdict: resume `/main-ci-failed` Step 5 (self-review).

**Why:** A main-CI-fix that also amends `.spec.md` has reclassified the spec contract — the failure is no longer purely a post-merge regression but a partial spec rewrite. `self-review` checks code-against-spec, not spec-against-design; the design-review re-entry is the only gate that catches contradictions or new ACs introduced by the amendment. **Main-specific consideration:** the amended spec will land on the new feature branch's eventual PR (per Step 7), not directly on main — so the design-review re-entry runs against the feature branch's tree, exactly like the `/pr-commented` and `/pr-ci-failed` flows.

**FORBIDDEN reasoning for skipping this recipe:** *"the spec amendment is just to mirror the new value"* / *"only the lint output changed"* / *"self-review will catch it"* / *"the CI failure is the real fix; the spec edit is incidental"* / *"main CI is on fire; we need to ship the fix fast"*. All forbidden — the recipe fires on **any** `.spec.md` line in the diff. Same FORBIDDEN-reasoning principle as [`ai-docs/corrections-log.md` → FORBIDDEN reasoning for skipping a `learnings.md` write](../../../ai-docs/corrections-log.md#forbidden-reasoning-for-skipping-a-learningsmd-write).

## Re-invocation semantics

Each invocation:

- Acts on the FIRST failing required check that does not already have an `ai-docs/main-ci/<run-id>.progress.md` file open.
- Empty actionable set → no-op. Print `No failing checks on latest main commit; exiting.` and stop. Do not create an empty progress file.
- A previously-failing main check that the next main push resolved silently (e.g., a force-merge of an unrelated fix) → record as `resolved-without-direct-fix` in the progress file's Decisions log; the file then waits for `/pr-merged` cleanup OR can be removed manually.

## Edge cases

| Case | Action |
|---|---|
| Multiple failing checks on the same main run | Handle the first failing check this invocation; advise user to re-invoke for the next |
| Local `cargo build` on main HEAD also fails before any fix attempt | Delegate to `/bugfix` for the deeper regression first; re-enter `/main-ci-failed` once local is green |
| Failure reproduces locally but the fix is cross-cutting / architectural | Delegate to `/bugfix` (see Step 4) — `/bugfix` carries this to a PR through its own end-to-end flow |
| `gh run list --commit <sha>` returns zero rows but `gh run list --branch main` shows a failure | `gh` `--commit` filter quirk on older versions — prompt user to pass run-id directly via `$ARGUMENTS`, or upgrade `gh` |
| Failing main run is older than `git rev-parse origin/main` (a fix has already landed) | Print `Main is currently GREEN; the failing run #<run-id> applies to an older commit.` and ask the user whether to fix retroactively (for build reproducibility) or skip |
| Self-review REJECTs 3 times | Surface verdict and stop; do not push; do not open PR |
| `gh pr create` fails (auth, network) | Push succeeded but PR was not opened — surface the branch URL and the failing `gh pr create` command for the user to retry manually |
| Reviewer wants the failing main commit reverted instead of forward-fixed | Out of scope — surface and bail; the user runs `git revert <sha>` manually and opens a revert PR |
