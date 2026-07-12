---
name: next
description: "Recommend one task to work on next — an open GitHub issue or a ready plan from ai-docs/plans/INDEX.md — with rationale and 2–3 runner-ups. Pass `small` to limit to quick wins / groundwork that prepares the codebase for larger milestones."
argument-hint: "[small]"
disable-model-invocation: true
---

## Open GitHub issues

```!
gh issue list --limit 50 --state open --json number,title,labels,updatedAt
```

## Plan index

```!
cat ai-docs/plans/INDEX.md
```

## Deferred-file backlog (thematic files)

The deferred store is canonical **JSONL** (one JSON object per line). Thematic
files accumulate under `ai-docs/deferred/` as `/triage` sorts `_inbox.jsonl`
rows into topic areas; the block below surfaces every *untracked candidate*
(`tracked=="—"`) across all of them via a baked-in `jq` scan (see *Deferred-file
rows* below for the field semantics). graphite-gp currently ships only
`_inbox.jsonl`, so this scan is empty until the first thematic file is created —
that is expected, not an error.

Thematic files — untracked rows are `tracked=="—"` (every `*.jsonl` under
`ai-docs/deferred/` except `_inbox.jsonl`, which is drained per-entry by
`/triage`, not listed here):

```!
find ai-docs/deferred -maxdepth 1 -name '*.jsonl' ! -name '_inbox.jsonl' -print0 | xargs -0 -r jq -c 'select(.tracked=="—")'
```

## Task

Mode: `$ARGUMENTS` — if this is the literal string `small`, apply **small mode** below; otherwise apply **default mode**.

### Default mode (no argument)

Pick ONE item to recommend next from the issues and plans above.

Selection rules:
- Prefer plans marked 🟢 ready (no blockers in the "Blocked by" column).
- Prefer items that unblock the most other plans — consult the "Dependency order" section of `INDEX.md`.
- A time-sensitive GitHub issue (bug, regression, security) outranks a plan of comparable readiness.
- Skip items marked 🔴 blocked or 🟡 spec-only without a design.
- Skip GitHub issues carrying the `blocked` label (see *Blocked-issues label* below) — body text like "Blocked by: #N" is not visible here, so the label is the canonical signal.

### Small mode (`/next small`)

Recommend ONE **small** item — the goal is to lay groundwork for upcoming larger milestones, not to start a milestone itself.

Selection rules:
- Prefer scope: bugfix, refactor, cleanup, docs polish, small dependency upgrade, or a single-crate change.
- Prefer items that unblock or de-risk a larger plan further down the dependency chain — consult the "Dependency order" section of `INDEX.md` and pick prerequisites of bigger blocked plans.
- Skip items marked 🔴 blocked or full-milestone plans (multi-crate, design-heavy).
- Skip GitHub issues carrying the `blocked` label (see *Blocked-issues label* below).
- 🟡 spec-only items qualify only if writing the design itself is the small task.
- If an issue bundles one small sub-item with larger ones, recommend it scope-narrowed to the small sub-item and call out that the issue should be split.

### Blocked-issues label

This skill fetches issues via `gh issue list --json number,title,labels,updatedAt` — labels are visible, **issue bodies are not.** A "Blocked by: #N" line in an issue body therefore has no effect on `/next`. The convention is:

- After opening or triaging a new issue that depends on another open issue, run `gh issue edit <N> --add-label blocked` (creating the label first via `gh label create blocked` if the repo doesn't have it yet).
- When the blocking dependency is resolved, run `gh issue edit <N> --remove-label blocked`.
- `/next` filters out any issue whose `labels` array contains `blocked` in both default and small modes.

### Deferred-file rows (thematic files)

The block above already filters to the candidate set via `jq`; this classification
documents the field semantics behind that filter. The deferred store is JSONL —
`_inbox.jsonl` (the triage queue) plus any thematic `*.jsonl` files `/triage`
creates as it sorts inbox rows into topic areas:

1. **Tracked vs. untracked.** Thematic rows carry no `kind` key. Field `tracked`:
   `#N` ⇒ tracked; `—` (em-dash) ⇒ **un-triaged / fresh** ⇒ candidate (the
   `jq 'select(.tracked=="—")'` filter above); `untracked` (literal word) ⇒
   **consciously declined**, intentionally NOT a candidate. These two non-`#N`
   states are a deliberate two-state model (not a bug) — see `triage-runner.md`
   Phase 3.
2. **Double-recommendation guard.** If a tracked row's `#N` is already in the `gh issue list` candidate set, the deferred-file row is **not** re-listed as a separate item — at most one supplementary one-liner under that issue's recommendation cites the deferred row.
3. **`jq` filters the candidate set precisely.** The `jq` block above already
   excludes every non-candidate row. Treat each emitted JSON object as one
   candidate; read `.item` for the title and `.source_path` for the source
   citation.
4. **Output the *Candidates needing `/triage`* section.** Untracked rows surface in a section titled **Candidates needing `/triage`** in the output (see *Output (both modes)* below). They are **never** the top-line recommendation or a runner-up — only listed for situational awareness, with a brief note that the user can promote them by running `/triage` (or act on one directly via `/interview`).

### Output (both modes)

- **Recommendation:** title + link or file path + a 2–4 sentence rationale (scope, readiness, why now; in small mode, also why it counts as small and which larger work it sets up).
- **Runner-ups (2–3):** one line each, with the reason each ranked lower.
- **Candidates needing `/triage` (informational):** any untracked rows from the deferred thematic files. Title each row with the row's `.item` text and cite the source via `.source_path`. **Items in this section are never the top-line recommendation or a runner-up** — they are listed for situational awareness only. End the section with a one-sentence reminder that the user can promote these by running `/triage`, or act on one directly via `/interview`.
