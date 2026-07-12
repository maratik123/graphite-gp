---
name: triage
description: "Batched promotion of untracked rows to gh issues; drains _inbox.jsonl; reconciles JSONL ↔ gh issue divergence via the bridge sweep. Default threshold ≥ 3 unhandled rows."
argument-hint: "[N — override default threshold]"
disable-model-invocation: true
allowed-tools: Bash(gh issue create *) Bash(gh issue edit *) Bash(gh issue close *) Bash(gh issue reopen *) Bash(gh issue list *) Bash(gh issue view *) Bash(gh api *) Bash(grep *) Bash(rg *) Bash(jq *) Bash(awk *) Bash(sort *) Bash(wc *) Read Edit Write
---

Launch the `triage-runner` subagent. The subagent reads `.claude/agents/triage-runner.md` for full instructions.

## Progress file

A multi-turn `/triage` run persists state to `ai-docs/triage/triage-YYYY-MM-DD.progress.md` (local-only / gitignored under `/ai-docs/triage/**/*.progress.md`). The file mirrors the canonical schema at `ai-docs/templates/progress-format.md` and stores:

- Phase 4 dedupe map summary (`{number → {state, title}}` counts).
- Phase 4.5 bridge classifications (type-1 / type-2 / type-3 lists + per-conflict user resolutions as they land).
- Phase 6 / Phase 7 candidate partitions (approve / decline / sort / promote / drop / keep — including any user-edited tweaks to the proposed split).
- `## Next action` — the phase the next subagent invocation should resume from.

Lifecycle (mirrors `/task` and `/pr-commented` progress files):

- **Created** by `triage-runner` at Phase 1.5 (after the branch check, before threshold gate). If the file already exists on the current branch when the subagent starts, it is read at Phase 1 and the run resumes from `## Next action` instead of restarting from scratch.
- **Extended** by the subagent as each phase produces durable state (dedupe map, classifications, partitions, per-conflict resolutions).
- **Deleted** by `triage-runner` after Phase 8's run summary emits successfully — same shape as `/pr-merged`'s `scripts/cleanup-progress.sh` mechanic for `/task` / `/pr-commented` files.

Subagent context isolation makes classification state unrecoverable across invocations unless persisted; the progress file is what makes a `/triage` run resumable across compaction or fresh-subagent spawn.

## Trigger and threshold

Default threshold is **≥ 3 unhandled rows** across all row sources. Tunable via `/triage [N]` — passing `N` overrides the default. Below the threshold the subagent exits with a brief status report; no approval prompt opens.

"Unhandled" counts rows with `tracked=="—"` across the thematic files + `_inbox.jsonl` (every `*.jsonl` under `ai-docs/deferred/`). `_inbox.jsonl` rows count individually toward the threshold. Count via `jq -c 'select(.tracked=="—")' <theme>.jsonl | wc -l`. (`tracked=="untracked"` rows are **consciously declined**, not un-triaged — they are intentionally excluded from this count and from `/next`, the deliberate non-`#N` counterpart of `—`; see `triage-runner.md` Phase 3. Not a bug.)

Manual invocation always proceeds regardless of threshold — the `[N]` argument can explicitly raise *or* lower the gate (e.g. `/triage 1` drains anything; `/triage 100` forces the threshold to skip a small batch).

## Cell-iteration sweep

The subagent walks the thematic files — every `*.jsonl` under `ai-docs/deferred/` except `_inbox.jsonl` (discovered via `find ai-docs/deferred -maxdepth 1 -name '*.jsonl' ! -name '_inbox.jsonl'`). `_inbox.jsonl` is **NOT** in this sweep — its rows are handled per-entry in the drain step below. graphite-gp starts with only `_inbox.jsonl`; thematic files accumulate as inbox rows are sorted into topic areas, so an empty sweep is expected, not an error.

- Candidates (baked-in `jq`): `jq -c 'select(.tracked=="—")' <theme>.jsonl`. Because the store is JSONL, the `tracked` key is read directly — there is no prose-substring leak.
- For each candidate, the subagent drafts a title + body from the row's `.item` text and the linked `.source_path` spec.
- **Single** bulk `gh issue list --state all --json number,state,title --limit 500` query upfront; proposed titles are deduped against existing open + closed issues by exact title match. The map shape is `{number → {state, title}}`. The pagination watchdog at ≥ 0.9 × the limit and the "one bulk call per run" contract are preserved unchanged.
- The subagent presents the full batch as a table; the user approves a subset (per-row decisions, but in one table).
- All approved creates from the sweep AND from the drain step's *promote* action are collected and run together in a single contiguous `gh issue create` pass at the end of the run (the spec's "one bulk call" contract).
- On approval: the row's JSON object is rewritten in place via a read-modify-write `Write` (read the file, replace the matching line, write it back — no `>` redirect) — `tracked` set to `#N`.
- On decline: an implicit-by-decline write — `tracked` set to `untracked`. Single user action per row; no separate write confirmation.

## `_inbox.jsonl` drain

`_inbox.jsonl` rows are handled per-entry — **not** routed through the cell-iteration sweep above (drain is canonical to avoid double-handling). Read inbox rows via `jq -c '.' _inbox.jsonl`; each line is one `{item, source_label, source_path, section, tracked}` object.

One prompt per row, four actions:

- **sort** — remove row from `_inbox.jsonl`; append a thematic-shaped JSON line (`{item, source_label, source_path, status:"", tracked:"—"}`) to a user-chosen thematic `.jsonl` (numbered menu). The row remains untracked at the thematic-file level and can be promoted on a future `/triage` run via the standard sweep. (The `section` key is dropped on migration — thematic rows have no `section`.)
- **promote** — queue the row into the same combined `gh issue create` pass as the sweep; on approval the row migrates to a user-chosen thematic `.jsonl` with `tracked:"#N"`; on decline migrates with `tracked:"untracked"`. Either way the row leaves `_inbox.jsonl`.
- **drop** — physically remove the row's line from `_inbox.jsonl`. No migration. Reserved for legitimately-bad rows (wrong shape, duplicate that dedupe missed, etc.). Distinct from `untracked`, which records legitimate review-and-decline.
- **keep** — leave the row in `_inbox.jsonl` unchanged for a later `/triage` session.

All `_inbox.jsonl` line removals / appends use a read-modify-write `Write` (read the file, edit the line set, write it back) — no `>` redirect.

## Bridge

After the bulk `gh issue list` call and before the cell-iteration sweep, the subagent harvests every tracked-ref across all row sources and looks up each `#N` in the local `{number → {state, title}}` map built in Phase 4. The tracked-ref harvest reads JSONL directly: `jq -rc 'select(.tracked|test("#[0-9]+")) | .tracked' <theme>.jsonl` (the thematic files + `_inbox.jsonl`). (`#N` may appear in a multi-issue `tracked` string like `#45 (closed), #46 (closed)` — extract every `#N` token from the harvested value.) The bridge consults `state` + `title`. Rows whose `tracked=="—"` are explicitly excluded by the `test("#[0-9]+")` filter — those route to the per-entry drain step (`_inbox.jsonl`) or the cell-iteration sweep (thematic).

Three conflict types reported (no silent overwrite — every type-1 and type-2 conflict surfaces a diff and asks the user):

- **Stale tracked.** Row's `tracked` holds `#N` and the map reports that issue is CLOSED. Canonical example: a `tracked` `#N` whose issue was later closed-as-not-planned.
- **Status mismatch.** A row asserts done but the linked `#N` issue is OPEN. **Dormant in the current schema** — thematic files + `_inbox.jsonl` carry no done-assertion field, so no row can fire this type; it is reserved for a future status-bearing row kind. (A thematic row with `#N` that closed folds into stale-tracked.)
- **Untracked candidate count.** Row's `tracked` = `—`. Reported as a count for situational awareness only — these rows are already handled by the cell-iteration sweep (thematic) and the `_inbox.jsonl` drain step.

For each detected type-1 or type-2 conflict, the user picks one of three actions (per-conflict prompt, mirroring the drain step's per-entry shape):

- **`update md`** — rewrite the JSON row to reflect gh state (read-modify-write `Write`, no `>` redirect). Type-1 rewrites leave `#N` in place and append ` (closed)` inside the `tracked` string. Concurrent-edit guard (content-snapshot, not mtime) inherited from the cell-iteration sweep.
- **`update issue`** — close or reopen the gh issue to match the md row. Before any `gh issue close` / `gh issue reopen` call, the bridge surfaces a diff preview (current state → proposed state) and requires explicit user confirmation. The bridge **never** silently rewrites issue state or body.
- **`keep both`** — record the divergence in the run output with a user-supplied reason; make no mutation. The conflict re-surfaces on the next `/triage` run.

Issues that exist in `gh` but have no md row anywhere are explicitly **not** flagged — asymmetric drift is by design.

The bridge appends a sub-section to the run-output summary listing every conflict, its type, the user's resolution, and any `gh issue close` / `gh issue reopen` calls made. See `.claude/agents/triage-runner.md` Phase 4.5 for the operational specification.

## Run-output summary

At the end of every `/triage` run the subagent emits:

- Status table covering all row sources with candidate counts (before / after).
- List of issues created (`#N` + one-line title each).
- List of rows declined (file path + `.item` content).
- List of inbox actions taken (sort / promote / drop, with destination thematic file when applicable).
- Concurrent-edit aborts (if any), listing the affected files + diff snippet.
- Per-file JSONL row-count diff (`wc -l ai-docs/deferred/*.jsonl` before / after the run; JSONL line counts are the canonical tally).

Context from user (if any): $ARGUMENTS
