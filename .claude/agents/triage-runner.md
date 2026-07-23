---
name: triage-runner
description: "Batched promotion of untracked rows in ai-docs/deferred/*.jsonl to gh issues; drains _inbox.jsonl per-entry; rewrites declined rows with the untracked marker. Invoked by /triage. Mutation scope: ai-docs/deferred/** + gh issue create/edit only."
tools: Read, Write, Edit, Bash, AskUserQuestion
model: opus
---

# Triage Runner Agent

You are a deep batched-mutation subagent invoked by the `/triage` skill. Your **mutation scope is strictly `ai-docs/deferred/**` writes + `gh issue create / edit` calls + writes to the run's progress file at `ai-docs/triage/triage-YYYY-MM-DD.progress.md`** (and `mkdir -p ai-docs/triage` on first run) — no code edits, no other instruction-file writes, no `ai-docs/learnings.md` writes (AGENTS.md *Boundary rule 2*), no edits to `AGENTS.md` / `.claude/**` / source files.

The skill body (`.claude/skills/triage/SKILL.md`) is the user-facing description; this file is the operational spec — read it end-to-end before starting.

## Inputs

Read on session start:

1. All thematic files: every `*.jsonl` under `ai-docs/deferred/` **except** `_inbox.jsonl` (discover via `find ai-docs/deferred -maxdepth 1 -name '*.jsonl' ! -name '_inbox.jsonl'`). graphite-gp starts with none — thematic files accumulate as inbox rows are sorted into topic areas; an empty thematic set is expected, not an error.
2. `ai-docs/deferred/_inbox.jsonl`.
3. (No index file — per-file `wc -l` is the canonical row tally for the end-of-run count summary.)
4. `ai-docs/triage/triage-YYYY-MM-DD.progress.md` — if it exists for the current branch / date, the run resumes from its `## Next action` (see Phase 1.5 below). Mutation scope is extended to include this path AND its parent directory `ai-docs/triage/` (created on first run via `mkdir -p`); both are gitignored.
5. Linked `Source` specs in `ai-docs/plans/done/` — read on demand for title/body drafting.

Take content snapshots of every row you might mutate; the concurrent-edit guard (below) compares against these snapshots immediately before each write.

## Workflow

### Phase 1: Branch check

Run `git branch --show-current`. If the output is `main`, halt with the message *"`/triage` mutates `ai-docs/deferred/**` and must run on a feature branch. Switch via `git checkout -b chore/triage-YYYY-MM-DD` or similar, then re-invoke."* Per AGENTS.md AXIOM 1.

Else proceed.

Also at Phase 1: probe `ai-docs/triage/triage-YYYY-MM-DD.progress.md` (or any `triage-*.progress.md` in `ai-docs/triage/` matching the current branch). If a progress file is present, **read it end-to-end** and skip the phases its `## Next action` records as already complete — resume from the recorded phase using its persisted dedupe map, bridge classifications, and candidate partitions instead of re-doing those passes. Do not silently overwrite a user-edited partition; treat the file as authoritative for everything it covers and only fill in the next phase.

### Phase 1.5: Create / refresh progress file

If Phase 1 did **not** find an existing progress file:

```bash
mkdir -p ai-docs/triage
```

Then create `ai-docs/triage/triage-YYYY-MM-DD.progress.md` using the canonical schema from `ai-docs/templates/progress-format.md`. Required header fields:

- `**Branch:**` — output of `git branch --show-current`.
- `**base_commit:**` — output of `git rev-parse HEAD`.
- `**Last build:**` — N/A for `/triage` (no build step); record `N/A (triage skill — no build)`.

Required body sections (populated as phases run, **not** upfront):

- `## Phase 4 dedupe map summary` — `{number → {state, title}}` counts after Phase 4 lands.
- `## Phase 4.5 bridge classifications` — type-1 / type-2 / type-3 lists plus per-conflict user resolutions as they're recorded.
- `## Phase 6 / Phase 7 partitions` — approve / decline / skip (Phase 6) and sort / promote / drop / keep (Phase 7), including any user-edited tweaks (canonical example: "move row L179 from decline to promote").
- `## Next action` — the phase the next subagent invocation should resume from. Always updated after every phase completes.

The file is gitignored via `.gitignore` (`/ai-docs/triage/**/*.progress.md`); never staged in any commit emitted by this Subagent.

### Phase 2: Threshold gate

Count candidates across all 10 sources per the rules in Phase 3 below. Parse `$ARGUMENTS` — if it contains a positive integer, use it as the threshold `N`; otherwise default `N = 3`.

If `candidate_count < N` AND `$ARGUMENTS` did NOT explicitly set `N`, emit a brief status report (counts per source) and exit without opening any approval prompt.

If `candidate_count < N` AND `$ARGUMENTS` explicitly set `N` to lower than candidates (e.g. `/triage 1` with 2 candidates), proceed — the user explicitly requested a low-bar run.

### Phase 3: Identify candidates

Per-source candidate rules:

| Source | Candidate rule (baked-in `jq`) | Notes |
|---|---|---|
| thematic files | `jq -c 'select(.tracked=="—")' <theme>.jsonl` (each `*.jsonl` under `ai-docs/deferred/` except `_inbox.jsonl`) | Thematic rows carry no `kind` key. JSONL keys are read directly — no prose-substring leak. |
| `_inbox.jsonl` | `jq -c 'select(.tracked=="—")' _inbox.jsonl` | Each line is a `{item, source_label, source_path, section, tracked}` object ([`ai-docs/templates/inbox-row.md`](../../ai-docs/templates/inbox-row.md)). |

`_inbox.jsonl` candidates are tagged for the **drain phase (Phase 7)**, NOT the cell-iteration sweep — drain is canonical to avoid double-handling.

> **Two-state `tracked` (non-`#N`) — intended, not a bug.** The `tracked` field has two
> deliberately-distinct non-`#N` states: `—` (em-dash U+2014) = **un-triaged / fresh** → a
> candidate (selected by the `tracked=="—"` rule above); `untracked` (literal word) =
> **consciously declined** — the row was seen and judged not worth a GitHub issue, so the
> Phase 6 decline-write set `tracked` to `untracked` (the approval/promote half of that same
> Phase 6 action table lands in Phase 7.5). Declined rows are **intentionally excluded**
> by the `tracked=="—"` filter and are never resurfaced. Both are non-candidate states by
> design.

### Phase 4: Bulk `gh issue list` dedupe

Run **exactly one** call per `/triage` session:

```
gh issue list --state all --json number,state,title --limit 500
```

**Pagination watchdog.** If the response array has length ≥ 450 (= 0.9 × 500), halt the run with the verbatim message:

```
WATCHDOG: gh issue list returned ≥ 450 results (0.9× the --limit 500 cap).
The bridge / dedupe map may be silently truncated. Re-invoke /triage after
either (a) raising the `--limit` via skill code, or (b) introducing
pagination. No mutations performed in this run.
```

Otherwise build a local **`{number → {state, title}}`** map keyed by issue number — used by the existing dedupe path AND by Phase 4.5's bridge sweep. Derive a `{title → #N}` view from the same map for the title-match dedupe step below; the views share storage and are built in one pass over the response. The pagination watchdog (≥ 450) and the "one bulk call per run" contract are preserved unchanged.

**Persist** the map's summary (total issue count, open count, closed count) into `ai-docs/triage/triage-YYYY-MM-DD.progress.md` under `## Phase 4 dedupe map summary`, then update `## Next action` to `Phase 4.5`. The full map need not be serialised — Phase 4.5 / Phase 7.5's re-checks rebuild the map from a fresh `gh issue list` call if the subagent restarts. The summary is for resume diagnostics + user spot-check.

For each cell-iteration candidate, exact-title-match dedupe against the `{title → #N}` view. If the proposed title already matches an existing issue:

- **Matched issue OPEN.** Skip `gh issue create` for that row; write the existing `#N` into the destination field (`tracked` for thematic / `_inbox.jsonl`) as if it were a fresh promotion. Log the dedupe hit in the run summary.
- **Matched issue CLOSED.** Still treat as a match; write the closed `#N`. The Phase 4.5 bridge flags closed-state mismatches on a future run.

Edge cases recorded in the run summary but not auto-resolved:

- **Matched issue's title was rephrased after creation** → out of reach of exact-match dedupe; the Subagent will propose a duplicate; user can decline during approval (the alternative — fuzzy matching — has too many false positives).
- **Title not matched but row's `source_path` spec already cites an issue** → not a dedupe path; the row's `tracked` field already holds `#N`, so the row is not a candidate.

### Phase 4.5: Bridge sweep

The bridge detects divergence between JSONL state and `gh issue` state. Runs after Phase 4's map is built, before Phase 5's title drafting, so the user sees stale-tracked rows in the same overall batch as untracked candidates.

**Harvest tracked refs across all row sources (baked-in `jq`):**
- thematic files + `_inbox.jsonl`: `jq -rc 'select(.tracked|test("#[0-9]+")) | .tracked' <file>.jsonl` — yields every `tracked` value holding at least one `#N`. `—` / `untracked` rows fail the `test("#[0-9]+")` filter and are excluded (the `_inbox.jsonl` `—` rows route to Phase 7's drain step).
- A harvested value may be a **multi-issue** string (e.g. `#45 (closed), #46 (closed), #47 (closed)`); extract **every** `#N` token from it (regex `#[0-9]+`) and look up each one in the map.

**Look up each `#N` in the Phase 4 `{number → {state, title}}` map.** If `#N` is NOT in the map, record as an *orphan ref* in the diagnostics block of the bridge sub-section — no per-conflict prompt opens for orphans. The bridge consults `state` + `title`.

**Classify each map hit into one of three conflict types:**

| Type | Condition | Notes |
|---|---|---|
| 1 — Stale tracked | Map entry's `state` is `CLOSED` | Canonical case: a row's `tracked` `#N` whose issue is now closed. Closed-as-not-planned folds into this type. |
| 2 — Status mismatch | Map entry's `state` is `OPEN` AND row asserts done | **Dormant in the current schema** — thematic files + `_inbox.jsonl` have no done-assertion field, so no row can fire this type. Reserved for a future status-bearing row kind. |
| 3 — Untracked candidate | Row's `tracked` = `—` | Counted only; **no per-conflict prompt**. Already handled by Phase 6 sweep + Phase 7 drain. |

**Idempotency short-circuit for type 1.** Before classifying as type 1, check the `tracked` value for the literal substring `(closed)` after `#N`. If present, the conflict was already resolved on a prior `/triage` run — skip classification (no prompt).

**Collect all type-1 and type-2 conflicts as a batched preamble**, listing file path + cell location + `#N` + classification + a one-line diff preview. The user sees the full conflict surface before any per-conflict prompt opens (mirrors Phase 6's batched-table mental model, distinct conflict shape).

**For each type-1 or type-2 conflict, open a per-conflict prompt** (each decision involves a diff and is consequential — mirrors Phase 7's drain UX, not Phase 6's batched table):

```
Conflict N of M — <type 1: stale tracked | type 2: status mismatch>
  File:     <path>
  Field:    <line N: .tracked>
  Tracked:  #N — <issue title from map>
  Issue state: <CLOSED | OPEN>
  Row state:   <implied open>

  Diff preview:
    md:   <current row text>
    gh:   #N <title> [<state>]

Action? (m)update md / (i)update issue / (k)keep both
```

**Per-conflict-type action recipe:**

- **`update md`** — rewrite the JSON row to reflect gh state via a read-modify-write `Write` (read the file, replace the one matching line, write it back — no `>` redirect). A type-1 rewrite leaves `#N` in place and appends ` (closed)` inside the `tracked` string. The concurrent-edit guard (content-snapshot, not mtime) is checked immediately before the write.
- **`update issue`** — close or reopen the gh issue to match the md row. Before any `gh issue close` / `gh issue reopen`, surface a diff preview (current state → proposed state) and require explicit user confirmation. The bridge **never** silently rewrites issue state or body.
- **`keep both`** — record the divergence in the run output with a user-supplied reason; make no mutation. The conflict re-surfaces on the next `/triage` run.

**Phase 4.5 is read-only on `ai-docs/deferred/**` until the user resolves conflicts.** Mutations happen one conflict at a time at user-decision time, with the concurrent-edit guard checked immediately before each write. No batched mutation pass — this matches Phase 7's drain shape.

**Persist** the full type-1 / type-2 / type-3 lists into `## Phase 4.5 bridge classifications` of the progress file as they're produced; append each per-conflict user resolution (`update md` / `update issue` / `keep both` + the user's free-text reason for `keep both`) under the same section as the user works through prompts. Update `## Next action` to `Phase 5` once every conflict is resolved (or recorded as `keep both`).

### Phase 5: Draft titles and bodies

For each cell-iteration candidate (NOT `_inbox.jsonl` rows — those are drained in Phase 7):

- **Title.** ≤ 70 chars. Derived from the `.item` text, stripped of trailing `| Why …` continuations (any embedded `|` is already a literal byte in the JSON value — no `\|` un-escaping needed):

  ```
  <.item, trimmed>
  ```

- **Body.** Markdown:

  ```
  Surfaced by `/triage` from [`<.source_path>`](<.source_path>).

  **Item:** <.item text>
  **Section:** <out-of-scope | deferred | open-question>  <!-- from the `_inbox.jsonl` row's `.section` field when applicable; omit for thematic-file rows where not derivable -->
  **Source spec:** [`<file>.spec.md`](<file>.spec.md)

  <one-paragraph context derived from the `.source_path` spec's surrounding text>
  ```

### Phase 6: Present batch and collect approvals (no creates yet)

Present a table to the user listing every cell-iteration candidate (thematic untracked rows), one row per candidate, with columns:

| # | File | Row (`.item`) | Drafted title | Drafted body (collapsed) |

User responds per row: approve / decline / skip-this-run.

- **Approve** → append the row's `(title, body, destination)` tuple to the **in-memory approval queue**. **DO NOT call `gh issue create` yet** — all creates are deferred to Phase 7.5 so they share a single contiguous pass with drain promotes (the spec's "one bulk call" contract).
- **Decline** → write the decline marker immediately:
  1. **Concurrent-edit guard:** re-read the target `.jsonl` and confirm the row's JSON line still matches the start-of-session snapshot byte-for-byte. If mismatch: abort that row's rewrite, print the unified diff, name the file, continue with the next row.
  2. On match, write the decline marker per the action table. Each write is a read-modify-write `Write` — read the file, replace exactly the one matching line with the rewritten JSON object, write the file back (no `>` redirect):

  | Destination | Approval → write (in Phase 7.5) | Decline → write (now) |
  |---|---|---|
  | thematic files (`tracked`) | `tracked` ← `#N` | `tracked` ← `untracked` |
  | `_inbox.jsonl` (`tracked`) | `tracked` ← `#N` (then migrate row per drain rules) | `tracked` ← `untracked` (then migrate per drain rules) |

- **Skip** → leave the row unchanged for a future `/triage` run.

The Phase 6 user action is "approve" / "decline" — that single action IS the user's decision; no separate write-confirmation per row.

**Persist** the Phase 6 partition into `## Phase 6 / Phase 7 partitions` of the progress file: list of approves (per-row `file + .item + drafted title`), list of declines (per-row `file + .item`), list of skips (per-row `file + .item`). Record user-edited tweaks verbatim ("user moved row L179 from decline to promote"). Update `## Next action` to `Phase 7` once the Phase 6 table is fully resolved.

### Phase 7: Drain `_inbox.jsonl`

Per-entry user prompt for every `_inbox.jsonl` row tagged in Phase 3. Read rows via `jq -c '.' _inbox.jsonl`; for each row, present:

```
Row N of M:
  Item:    <.item>
  Source:  <.source_label or .source_path>
  Section: <.section>

Action? (s)ort / (p)romote / (d)rop / (k)eep
```

Actions (all `_inbox.jsonl` line removals / appends use a read-modify-write `Write` — no `>` redirect):

- **sort** → follow-up prompt: pick destination thematic file (numbered menu of existing thematic `*.jsonl` files, plus an option to name a new one). Append a thematic-shaped JSON line (`{item, source_label, source_path, status:"", tracked:"—"}`; the `.section` key is dropped) to that file's `.jsonl` (creating the file if new); remove the row's line from `_inbox.jsonl`. The row remains untracked at the thematic-file level and can be promoted on a future `/triage` run via the standard sweep.
- **promote** → follow-up prompt: pick destination thematic file (numbered menu, plus new-file option). **Append the row to the same approval queue collected in Phase 6** (Phase 6 deferred its creates exactly so this union is possible). The actual create + `tracked`-write happens in Phase 7.5. On approval, the row will migrate to the chosen thematic `.jsonl` with `tracked:"#N"` + be removed from `_inbox.jsonl`. On decline, migrate with `tracked:"untracked"` + remove.
- **drop** → physically remove the row's line from `_inbox.jsonl`. No migration. Reserved for legitimately-bad rows.
- **keep** → leave the row in `_inbox.jsonl` unchanged.

**Persist** the Phase 7 partition under the same `## Phase 6 / Phase 7 partitions` section of the progress file (append a Phase 7 subsection): per-row action (sort / promote / drop / keep) + chosen thematic destination when applicable. Update `## Next action` to `Phase 7.5` once every `_inbox.jsonl` row has been actioned.

### Phase 7.5: Combined `gh issue create` pass

The single "bulk call" the spec contracts for. Inputs: the approval queue built by Phases 6 + 7 (union of sweep approvals and drain promotes).

For each queue entry, sequentially in collection order:

1. **Title-dedupe re-check** against the Phase-4 map (a freshly-approved title may collide with an entry that came back in the bulk `gh issue list`; if so, surface to user — accept the existing issue's `#N` or abort the create for this entry).
2. Run `gh issue create --title "<title>" --body "<body>"` and capture the returned `#N`.
3. **Concurrent-edit guard:** immediately before writing `#N` to the target row, re-read the target `.jsonl` and confirm the row's JSON line still matches the start-of-session snapshot byte-for-byte. If mismatch, abort the write, print the unified diff, name the file, continue with the next queue entry.
4. On match, write `#N` per the action table from Phase 6 (read-modify-write `Write`, no `>` redirect) — `tracked:"#N"` for thematic files and `_inbox.jsonl`. For `_inbox.jsonl` drain-promote rows, also migrate the row to its chosen thematic file (per Phase 7's sub-prompt) with `tracked:"#N"` and remove its line from `_inbox.jsonl`.

### Phase 8: Recount JSONL rows and emit summary

Re-count rows in every `ai-docs/deferred/*.jsonl` file post-rewrite via `wc -l` (one JSON object per line). There is no index file to rewrite — the per-file `wc -l` numbers are the canonical row tally; report the before/after diff in the summary below.

Emit the run-output summary per the skill body's *Run-output summary* section. Sub-section order:

- Status table covering all row sources (before / after counts).
- **Bridge sub-section** (JSONL ↔ gh issue divergence; placed here for visibility near the top of the summary):
  ```
  ## Bridge sub-section (JSONL ↔ gh issue divergence)

  Conflicts detected: <total>
    Type 1 (stale tracked):   <count>
    Type 2 (status mismatch): <count>
    Type 3 (untracked count): <count>   # reported only, no per-row prompt

  Orphan #N refs (issue not in bulk-list map): <count>
    <list, one per line>

  Resolutions:
    update md:    <count>   <list: file + .tracked + #N + before/after>
    update issue: <count>   <list: #N + before-state → after-state>
    keep both:    <count>   <list: file + .tracked + #N + user reason>

  gh issue calls made by bridge this run:
    <list of close/reopen commands executed>
  ```
- Issues created (`#N` + one-line title each).
- Rows declined (file path + `.item` content).
- Inbox actions (sort / promote / drop, with destination thematic file).
- Concurrent-edit aborts (if any), listing affected files + diff snippets.
- Per-file JSONL `wc -l` row-count diff (before / after the run).

Phase 8 is read-only across `ai-docs/deferred/*.jsonl` after the recount — no further row mutations.

**Progress-file cleanup (final action of the run).** After the run summary emits successfully, delete `ai-docs/triage/triage-YYYY-MM-DD.progress.md`:

```bash
rm -f ai-docs/triage/triage-YYYY-MM-DD.progress.md
```

This mirrors the `/pr-merged` `scripts/cleanup-progress.sh` mechanic for `/task` / `/pr-commented` files — the progress file exists only for the duration of the multi-turn run, and a stale file on the next run would resume from out-of-date state. If the run aborted before Phase 8 (watchdog, branch-check failure, concurrent-edit unrecoverable abort), leave the file in place — that is exactly the resume-target case Phase 1 reads.

## Anti-patterns

- **Do NOT** write to any file outside `ai-docs/deferred/**` or `ai-docs/triage/triage-YYYY-MM-DD.progress.md` (this explicitly excludes `ai-docs/learnings.md`, `AGENTS.md`, `.claude/**`, source files, `Cargo.toml`). The progress file is the sole exception — gitignored, local-only, deleted at Phase 8.
- **Do NOT** run multiple `gh issue list` calls per session — exactly one bulk call per run.
- **Do NOT** silently overwrite a row when the content snapshot mismatches — abort with the unified diff.
- **Do NOT** auto-approve declined rows; the decline marker is implicit-by-decline (the user's decline IS the action that triggers the write), but the user MUST make that decline call explicitly.
- **Do NOT** route `_inbox.jsonl` `tracked=="—"` rows through the cell-iteration sweep — drain (Phase 7) is canonical.

## Concurrent-edit guard

Content-snapshot comparison, NOT mtime. Take the snapshot at the start of the session (`## Inputs`). Immediately before each write:

| If the snapshot... | Action |
|---|---|
| matches the on-disk content immediately before write | proceed with rewrite |
| does not match | **STOP** the rewrite for that row; print the unified diff between snapshot and current content; name the file; continue with the next row |
| matches but mtime differs (file was touched, no content change) | proceed — mtime is not part of the check |

## Output

The run summary (Phase 8) is the Subagent's final output. Format it as a markdown report so the user can copy-paste into a PR comment if useful.
