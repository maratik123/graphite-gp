#!/usr/bin/env bash
#
# Delete the local progress files belonging to the merged branch passed as $1.
#
# Run by `/pr-merged` skill (.claude/skills/pr-merged/SKILL.md, step 3) after
# `git checkout main && git pull`, before `git branch -d <previous-branch>`.
#
# Derivation (PR linkage):
#   1. `gh pr list --state merged --head <branch>` -> merged PR number
#   2. `gh pr view <PR_NUM> --json body` -> PR body
#   3. `grep -oiE '(Close[sd]?|Fix(es|ed)?|Resolve[sd]?) #[0-9]+'` on the body
#      -> the first Closes/Fixes/Resolves #N reference. ISSUE_NUM is the
#      trailing digits. (Per /task + /interview convention every spec carries
#      `**Tracked in:** #<ISSUE_NUM>`, not #<PR_NUM>.)
#   4. `grep -lE "^\*\*Tracked in:\*\* #<ISSUE_NUM>"` in ai-docs/plans/done/
#      + ai-docs/plans/ -> matching /task spec file. The anchor on `^**Tracked
#      in:** ` prevents prose lines that mention "Tracked in:" alongside a
#      different #N (e.g. spec narrative discussing another spec's tracking
#      issue) from false-matching.
#   5. `basename <spec>.spec.md` -> spec-base name
#   6. Delete ai-docs/plans/<spec-base>.progress.md (if it exists)
#   6a. Branch-name fallback -- only runs when step 6 did NOT delete a file
#      (ISSUE_NUM empty, or SPEC_PATH empty): `${PREV_BRANCH#*/}` strips the
#      first `<prefix>/` (the /task branch convention is `<prefix>/<spec-base>`,
#      single slash) to recover <spec-base>, then deletes
#      ai-docs/plans/<spec-base>.progress.md if it exists. Covers /task PRs
#      that have no tracking issue at all.
#   7. Delete ai-docs/pr-comments/pr-<PR_NUM>.progress.md (the fallback path
#      used by /pr-commented for PRs not produced by /task)
#   8. Delete ai-docs/ci-fixes/pr-<PR_NUM>.progress.md (the fallback path
#      used by /pr-ci-failed for PRs not produced by /task)
#   9. `rmdir ai-docs/pr-comments ai-docs/ci-fixes ai-docs/main-ci`
#      -- opportunistic cleanup; non-fatal if the directories still have
#      unrelated files or do not exist.
#
# Secondary derivation (main-ci run linkage):
#   A. `gh pr view <PR_NUM> --json body` -> PR body (same body fetch as
#      step 2 -- the secondary probe runs against the same `${PR_BODY}`).
#   B. `grep -oE '^\*\*Tracked in run:\*\* [0-9]+'` on the body -> the
#      first `**Tracked in run:** <run-id>` reference. RUN_ID is the
#      trailing digits. /main-ci-failed writes this line into the body
#      of the fresh PR it opens; the per-run progress file lives at
#      `ai-docs/main-ci/<run-id>.progress.md` (gitignored).
#   C. Delete ai-docs/main-ci/<RUN_ID>.progress.md (if it exists).
#
# A PR may carry both `Closes/Fixes/Resolves #N` (primary path) and
# `**Tracked in run:** <run-id>` (secondary path) -- both probes run
# unconditionally and independently. They are not mutually exclusive: a
# /pr-ci-failed run on a /task-produced PR may still surface a fresh
# /main-ci-failed sibling PR carrying the run-id line.
#
# Failure modes (all exit 0 -- workflow step 4 proceeds regardless):
# - `PR_NUM` empty (branch merged outside `gh`, or PR is closed-not-merged):
#   prints a one-line note to stdout and exits 0 (nothing reliable to derive
#   paths from).
# - `ISSUE_NUM` empty (PR body has no Closes/Fixes/Resolves #N line -- e.g.
#   manual non-/task PR, or `/task` PR whose body convention drifted, or a
#   /main-ci-failed-produced PR which has no upstream tracking issue):
#   prints a one-line warning to STDERR naming the PR and branch, skips the
#   issue/spec-driven progress-file deletion, then attempts the
#   branch-name fallback (step 6a) before proceeding to the pr-comments /
#   ci-fixes fallback + secondary probe paths.
# - `SPEC_PATH` empty (issue number derived but no spec carries
#   `Tracked in: #<ISSUE_NUM>` -- e.g. PR closes a satellite issue without
#   its own /task spec): prints a one-line warning to STDERR naming the
#   issue number, skips the issue/spec-driven progress-file deletion, then
#   attempts the branch-name fallback (step 6a) before proceeding to the
#   pr-comments / ci-fixes fallback + secondary probe paths.
# - `RUN_ID` empty (PR body has no `**Tracked in run:** <run-id>` line --
#   the common case: every /task PR, every /pr-ci-failed PR, every manual
#   PR. Only /main-ci-failed-produced PRs carry the line.): silent
#   no-op; the main-ci secondary probe simply does not match. No
#   warning -- absence of this line is the norm, not an anomaly.
# - rm -f is silent on missing files (intentional -- files may not exist;
#   idempotent re-runs are safe).
# - rmdir failing on non-empty directory is expected and ignored.
#
# Multi-close PRs (rare) -- e.g. maratik123/quartzite#295 closes #289 + #277:
# the FIRST closing-keyword match wins. By convention the first-listed
# issue is the primary /task tracking issue; satellite closures are
# bundled cleanup items without their own spec. The same FIRST-match rule
# applies to multi-`**Tracked in run:**` PR bodies (rare; a single PR
# rarely references more than one main CI run).
#
# Deferred-task progress files (ai-docs/plans/deferred/) are intentionally
# NOT touched -- deferral is its own workflow and a deferred task has no
# merged PR to drive cleanup.

set -uo pipefail

PREV_BRANCH="${1:-}"
if [ -z "${PREV_BRANCH}" ]; then
  printf 'cleanup-progress.sh: usage: cleanup-progress.sh <previous-branch>\n' >&2
  exit 2
fi

PR_NUM=$(gh pr list --state merged --head "${PREV_BRANCH}" --json number --jq '.[0].number // empty')

if [ -z "${PR_NUM}" ]; then
  printf 'pr-merged: no merged PR found for %s; skipping progress-file cleanup.\n' "${PREV_BRANCH}"
  exit 0
fi

PR_BODY=$(gh pr view "${PR_NUM}" --json body --jq '.body' 2>/dev/null)

ISSUE_NUM=$(printf '%s' "${PR_BODY}" \
  | grep -oiE '(Close[sd]?|Fix(es|ed)?|Resolve[sd]?) #[0-9]+' \
  | head -n1 \
  | grep -oE '[0-9]+$')

SPEC_DRIVEN_DELETE=0

if [ -z "${ISSUE_NUM}" ]; then
  printf 'pr-merged: PR #%s body has no Closes/Fixes/Resolves #N line; skipping spec-driven progress-file cleanup for %s.\n' "${PR_NUM}" "${PREV_BRANCH}" >&2
else
  SPEC_PATH=$(grep -lE "^\*\*Tracked in:\*\* #${ISSUE_NUM}\b" ai-docs/plans/done/*.spec.md ai-docs/plans/*.spec.md 2>/dev/null | head -n1)
  if [ -n "${SPEC_PATH}" ]; then
    SPEC_BASE=$(basename "${SPEC_PATH}" .spec.md)
    rm -f "ai-docs/plans/${SPEC_BASE}.progress.md"
    SPEC_DRIVEN_DELETE=1
  else
    printf 'pr-merged: no /task spec matches Tracked in: #%s (derived from PR #%s, branch %s); skipping spec-driven progress-file cleanup.\n' "${ISSUE_NUM}" "${PR_NUM}" "${PREV_BRANCH}" >&2
  fi
fi

# Fallback: PRs with no tracking issue (ISSUE_NUM empty) or no matching spec
# (SPEC_PATH empty) still have a /task-produced progress file on disk --
# derive its name from the branch instead of the issue/spec linkage. The
# /task branch convention is `<prefix>/<spec-base>` (single slash), and the
# progress file shares that <spec-base>. Only fires when the spec-driven
# path above did NOT already delete a file, so the primary success path is
# left undisturbed.
if [ "${SPEC_DRIVEN_DELETE}" -eq 0 ]; then
  BRANCH_BASE="${PREV_BRANCH#*/}"
  if [ -f "ai-docs/plans/${BRANCH_BASE}.progress.md" ]; then
    rm -f "ai-docs/plans/${BRANCH_BASE}.progress.md"
    printf 'pr-merged: deleted ai-docs/plans/%s.progress.md via branch-name fallback (PR #%s had no tracking-issue linkage).\n' "${BRANCH_BASE}" "${PR_NUM}"
  fi
fi

# Secondary probe: /main-ci-failed PR bodies carry a `**Tracked in run:**
# <run-id>` line pointing at ai-docs/main-ci/<run-id>.progress.md. Absence
# of the line is the norm for every non-/main-ci-failed PR -- silent no-op.
RUN_ID=$(printf '%s' "${PR_BODY}" \
  | grep -oE '^\*\*Tracked in run:\*\* [0-9]+' \
  | head -n1 \
  | grep -oE '[0-9]+$')

if [ -n "${RUN_ID}" ]; then
  rm -f "ai-docs/main-ci/${RUN_ID}.progress.md"
fi

rm -f "ai-docs/pr-comments/pr-${PR_NUM}.progress.md"
rm -f "ai-docs/ci-fixes/pr-${PR_NUM}.progress.md"

# Opportunistic cleanup -- fails non-fatally if directories have other files
# or do not exist. Exit code intentionally ignored.
rmdir ai-docs/pr-comments ai-docs/ci-fixes ai-docs/main-ci 2>/dev/null || true

exit 0
