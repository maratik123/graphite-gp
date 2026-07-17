---
name: pr-merged
description: "After a PR merge: switch to main, pull, delete the merged branch's local progress files, and delete the local PR branch."
disable-model-invocation: true
allowed-tools: Bash(git checkout main) Bash(git pull) Bash(git pull *) Bash(git branch -d *) Bash(git branch --show-current) Bash(git status) Bash(git status --porcelain) Bash(.claude/skills/pr-merged/scripts/cleanup-progress.sh *)
---

> Near-stateless: no `.progress.md` discipline applies; re-entry consists of re-invoking the skill.

Current branch: !`git branch --show-current`

Working tree:
```!
git status --porcelain
```

If the current branch is `main`, stop and tell the user this skill must be run while standing on the merged PR branch.

If `git status --porcelain` above is non-empty (any modified, staged, or untracked entries), stop and ask the user how to proceed (commit, stash, discard, ignore). Do not run any further commands until the user answers.

Otherwise, run in order. **Capture `<previous-branch>` from the `Current branch:` value above before step 1** — once `git checkout main` runs, `git branch --show-current` no longer returns it.

1. `git checkout main`
2. `git pull`
3. **Delete the merged branch's local progress files** (gitignored Subagent artefacts; no longer needed). Run the cleanup script:

   ```bash
   ${CLAUDE_SKILL_DIR}/scripts/cleanup-progress.sh <previous-branch>
   ```

   The script encapsulates the PR-linkage derivation (PR number → spec lookup → progress-file paths) and handles the failure modes:
   - **`PR_NUM` empty** (branch merged outside `gh`, or PR is closed-not-merged): prints `pr-merged: no merged PR found for <previous-branch>; skipping progress-file cleanup.` and exits 0.
   - **No matching `/task` spec** (manual PR without `/task`): skips the `/task`-progress-file deletion silently; the `/pr-comments` fallback path is still attempted.
   - **`rmdir` on `ai-docs/pr-comments`** is opportunistic — non-fatal if the directory has unrelated files or doesn't exist; exit code ignored.

   Deferred-task progress files (`ai-docs/plans/deferred/`) are intentionally NOT touched — deferral is its own workflow and a deferred task has no merged PR to drive cleanup. Source: [`scripts/cleanup-progress.sh`](scripts/cleanup-progress.sh).

4. `git branch -d <previous-branch>` — always `-d`, never `-D`. If `-d` refuses because the branch is not fully merged, stop and report the message; do not force-delete.

## Patterns

### Auto-delete the merged local branch without a confirmation pause

*Default to* running `git branch -d <previous-branch>` immediately at step 4 of the workflow without pausing for user confirmation. *Prefer* the silent auto-delete over an `AskUserQuestion` prompt — `git branch -d` is the safe form (refuses to delete unmerged branches), and the user has already invoked this skill specifically to perform cleanup; pausing for a yes/no on a guaranteed-safe operation is friction without protection. If `-d` refuses (branch not fully merged), stop and report; do NOT escalate to `-D` without an explicit user instruction.

Validated by user feedback recorded in the sibling **quartzite** project's memory namespace — `~/.claude/projects/-home-syt-RustroverProjects-quartzite/memory/feedback_remove_merged_branches.md` (`type: feedback`): "Remove merged local branches without asking — `git branch -d` merged branches after `git pull`; do not pause to confirm". That file also carries the `-d`-not-`-D` rationale above. Surfaced by a **quartzite** `/improve` run (2026-05-22, Step 1c) — not a run in this repo, whose log begins 2026-07-13. The memory is user-local to that project: this skill's namespace resolves from `pwd`, so a graphite-gp sweep will never see it, by design.
