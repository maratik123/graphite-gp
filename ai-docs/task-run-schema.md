# `task-runs.jsonl` — schema and operating rules

Reference page for `ai-docs/metrics/task-runs.jsonl`, the longitudinal record of
`/task` run cost. One JSON object per line, one line per completed `/task` run,
appended at Step 12 by `.claude/skills/task/scripts/append-task-run.sh`.

Source spec: `ai-docs/plans/done/2026-07-31-task-run-telemetry.spec.md` (issue 186).

## Single writer, append-only, never hand-edited

- **Single writer.** `/task` Step 12 sub-step 5a is the only writer. Nothing else
  — not `/pr-commented`, not `/pr-ci-failed`, not `/main-ci-failed`, not
  `/project-review`, not `/bugfix` — appends to this file.
- **Append-only.** Lines are appended and never rewritten, reordered, or deleted.
- **Hand-edits are forbidden.** The JSON-Lines format is hand-edit-hostile: one
  malformed line breaks any `jq` read of the whole file, so a single stray
  keystroke destroys every record, not just the one touched. This mirrors the
  rationale behind the `ai-docs/deferred/_inbox.jsonl` prohibition; the two files
  are separate surfaces with separate single writers.
- **Repairing a bad line** means appending a corrected record, not editing the
  bad one — same discipline as `ai-docs/learnings.md`.

## Field table

18 fields. Nine are **fallback-required** — the orchestrator MUST emit them by
hand when the script exits non-zero; nine are **fallback-optional** and MAY be
omitted on that path. The fallback MUST NOT emit any key absent from this table.

| Field | Type | Source | Fallback class |
|---|---|---|---|
| `schema_version` | int (`1`) | literal | fallback-required |
| `date` | string `YYYY-MM-DD` | `date -u +%F` | fallback-required |
| `branch` | string | `git branch --show-current` | fallback-required |
| `issue` | int or null | progress-file `**Issue:** #N` (script) / ambient (fallback) | fallback-required |
| `spec_base` | string | `basename <progress-path> .progress.md` | fallback-required |
| `incomplete` | bool | derived — see the trigger table | fallback-required |
| `files_changed` | int | `git diff --shortstat <base>..HEAD`, by keyword | fallback-required |
| `insertions` | int | same, by keyword | fallback-required |
| `deletions` | int | same, by keyword | fallback-required |
| `rounds` | int | count of `^## Self-Review \(Round [0-9]+\)$` headings | fallback-optional |
| `hit_round_cap` | bool | `rounds >= 3 && verdicts[2] == "REJECT"` | fallback-optional |
| `verdicts` | array\<string\> | first `^\*\*Verdict:\*\*` line per section, in round order | fallback-optional |
| `findings` | object `{blocker,major,minor,nit}` int | severity cell of every `^\| [0-9]` row inside a Self-Review section | fallback-optional |
| `findings_first_seen` | object `{blocker,major,minor,nit}` int | same rows, restricted to those absent from the preceding round | fallback-optional |
| `objections` | int | cells containing `⚠️ Objected` (substring match) | fallback-optional |
| `objections_reopened` | int | cells containing `🔁 Re-opened` (substring match) | fallback-optional |
| `files_touched` | array\<string\> | `` ^- `<path>` `` lines under `## Files touched` | fallback-optional |
| `instruction_corpus_lines` | int | the pinned `:(glob)` command below | fallback-optional |

**`issue` is `#N`-only.** The canonical progress template specifies
`**Issue:** [#number or URL]`, so the `#N` form is not guaranteed. The parser
extracts `#N` only; a URL — or an absent line — yields JSON `null` **and** trips
`incomplete`. Widening the parser to accept a URL was rejected: `null` is honest
and total, whereas a URL regex would invent an issue number from any path segment
that happens to be digits.

**Substring, never whole-cell equality.** Live status tokens include
`✅ Fixed (design amended)`, `✅ Fixed (spec amended)`, `⚠️ Objected: <reason>`
and `⬜ Open 🔁 Re-opened` — two carry a parenthesised suffix, one a trailing
free-text reason, one an appended marker. A whole-cell compare misses all four.

**Section bounding.** All Self-Review parsing is bounded to the text between a
`^## Self-Review \(Round [0-9]+\)$` heading and the next `^## ` heading. This
keeps `## AC Status` rows, `## Decisions log` prose, and any later
`## Comment cycle round M` table out of `findings` / `objections`.

## Counting units — read this before comparing two records

Stated per field, in words, because the unit is not inferable from the number:

- **`findings`, `objections` and `objections_reopened` are summed across all rounds.**
  They therefore **inflate with `rounds`**: `.claude/agents/self-review.md`
  round>1 rules tell the reviewer to "Focus on remaining `⬜ Open` items plus
  anything newly introduced", and § *Findings format* says an APPROVE table "is
  empty (no rows) or contains only already-resolved items" — so rows **carry
  forward**, and one finding is counted once per round that saw it. `findings` is
  a *review-effort* measure (how much finding-handling the loop did), explicitly
  **not** a defect count. `objections` / `objections_reopened` count status
  **cells**, not distinct findings: a finding objected in round 2 and re-objected
  in round 3 counts twice. That is deliberate — an objection is an *event*.
- **`findings_first_seen` counts only rows absent from the immediately preceding round's table.**
  Round 1 contributes all of its rows. This is the
  defect-population measure, and the field to correlate against a landed
  `/improve` escalation.
- **Identity key: the `File:line` cell, verbatim.** Two rows in consecutive
  rounds are "the same finding" iff their `File:line` cells are byte-identical.
- **Known bias, direction fixed: `findings_first_seen` is biased upward.** If a
  finding's location moves between rounds — a fix shifted line numbers, or the
  reviewer cites a different line for the same defect — the key changes and the
  row re-counts as first-seen. Path-only matching was rejected as a mitigation:
  it merges genuinely distinct findings in the same file, under-counting instead.
  The series stays usable because the direction of the bias is known: read
  `findings_first_seen` as an **upper bound** on the defect population.

## The diff-size trio — base commit and parsing

**Two-source base rule.** The **script** uses the progress file's
`**base_commit:**` header. The **fallback** uses `git merge-base main HEAD`,
which needs no progress file. In the normal `/task` flow the two coincide — the
branch is cut at Step 1 and the progress file created at Step 8 with no
intervening commits. The script shares the fallback's last resort: when
`**base_commit:**` is absent or unparseable it computes the trio off
`git merge-base main HEAD` **and sets `incomplete: true`**, so the looser base is
never passed off as the precise one.

**Parse by keyword, never positionally.** Five shapes, all verified on
purpose-built commits:

| # | Diff | `git diff --shortstat` output |
|---|---|---|
| 1 | deletions only | ` 1 file changed, 3 deletions(-)` — insertions clause absent |
| 2 | one insertion only | ` 1 file changed, 1 insertion(+)` — deletions clause absent, and `insertion` is singular |
| 3 | no changes | *empty output*, exit 0 |
| 4 | pure rename | ` 1 file changed, 0 insertions(+), 0 deletions(-)` |
| 5 | both | ` 1 file changed, 2 insertions(+), 1 deletion(-)` — `deletion` singularises too |

A positional parse breaks on shapes 1–3. Match each number by its own keyword —
`([0-9]+) files? changed`, `([0-9]+) insertions?\(\+\)`,
`([0-9]+) deletions?\(-\)` — each defaulting to `0` when its clause is absent,
which also makes shape 3 fall out correctly as `0/0/0` with no special case.

## `incomplete: true` triggers (exhaustive)

| Condition | Effect |
|---|---|
| Progress file absent or unreadable | all optional fields omitted; the trio still emitted, off the `git merge-base main HEAD` base |
| `**Issue:**` line absent, or present in URL form rather than `#N` | `issue: null` — the field is fallback-required, so it is always emitted; `null` is its defined "present but unparseable" value, not an omission |
| `**base_commit:**` absent or unparseable | trio computed off `git merge-base main HEAD` instead — the looser base is flagged, never silently substituted |
| Both bases unobtainable — no `main` ref, detached HEAD, shallow clone, or not a git work tree | trio emitted as `0/0/0`. Reachable, not theoretical: a `git init` sandbox has no `main`. `0/0/0` is indistinguishable from a genuine no-change diff, so `incomplete: true` is what carries the difference |
| Zero `## Self-Review (Round N)` sections | `rounds: 0`, `verdicts: []` |
| A section has no `**Verdict:**` line, or a token that is neither `APPROVE` nor `REJECT` | `"UNKNOWN"` pushed into `verdicts` |
| A row's severity cell is not one of `blocker` / `major` / `minor` / `nit` | that row contributes to no bucket |
| `## Files touched` section absent | `files_touched` omitted |
| The pinned corpus command fails or yields a non-integer | `instruction_corpus_lines` omitted |

Any trigger → `"incomplete": true`, **exit 0**. A parse problem is never an
error; only *cannot append at all* is.

## Exit codes

| Code | Meaning | Orchestrator action |
|---|---|---|
| `0` | Appended — full **or** degraded | continue Step 12 |
| `2` | Usage error (no argument) | write the fallback record by hand, continue |
| `3` | `jq` not on `$PATH` | same |
| `4` | `jq` failed to compose a record | same |
| `5` | Could not append to the target | same |

Under no path does Step 12 halt on this sub-step.

## Known truncation

`.claude/agents/self-review.md` § *Rules* caps the reviewer at **10 findings**
per round: *"Maximum 10 findings per round. If more exist, list the 10 most
severe."* `findings` and `findings_first_seen` therefore under-count any round
that hit that cap. There is no marker in the progress file distinguishing a
capped round from a 10-finding round, so the under-count is silent.

## Step-12 re-entry duplicates — the last line wins

Step 12 can run twice for one task (compaction recovery re-entry). The script
appends unconditionally, so a re-entry produces a second line with the same
(`spec_base`, `branch`). **Consumers take the last line per (`spec_base`,
`branch`) and ignore earlier ones.** Strict append-only is preserved, the script
stays trivial, and the duplicate is itself signal — it records that a re-entry
happened, which an in-place rewrite would erase.

## `instruction_corpus_lines` — the pinned command

Run verbatim; the `:(glob)` magic prefixes are load-bearing:

```bash
git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
  | xargs -0 cat | wc -l
```

**Why the obvious pathspec is wrong.** Git's default wildmatch lets `*` cross
`/`, so a plain `'ai-docs/*.md'` also matches everything under
`ai-docs/plans/done/**`. Measured on the same tree: **9,403** lines over 59 files
with `:(glob)`, **38,538** without it. The `:(glob)` prefix restores pathname
semantics, and it matches at depth 1 as well as deeper — a future
`.claude/foo.md` is not silently dropped.

`xargs -0 cat | wc -l` is used deliberately over `xargs wc -l | tail -1`: the
latter emits multiple `total` lines whenever `xargs` splits the argument list.

## Hosted blocks for `/task` Step 12 sub-step 5a

`.claude/skills/task/SKILL.md` carries one-line pointers to the three blocks
below rather than the blocks themselves — that file is close to the 35,000-char
instruction-file warning band and this page is not in the capped set.

### Precondition assertion — untracked corpus files

`git ls-files` enumerates *index* paths while `cat` reads the *working tree*, so
a corpus-set file created by this task but not yet staged would be omitted from
`instruction_corpus_lines` while still landing in the commit. Sub-step 5a's
**first action**:

```bash
git ls-files --others --exclude-standard -- \
  'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md'
```

Non-empty output → `git add` those paths (they enter the same commit at sub-step
7 regardless), then re-run until empty. Only then invoke the script. Every
*tracked* corpus file already carries its worktree edits, so after this assertion
the measured set equals the committed set.

### Step-12 verification block

Run immediately after the append returns, before sub-step 7 stages. Record both
results in the PR body under **Test plan**, PASS/FAIL with observed values:

```bash
tail -c1 ai-docs/metrics/task-runs.jsonl | xxd -p                      # -> 0a
tail -1  ai-docs/metrics/task-runs.jsonl | jq -r '.instruction_corpus_lines'
git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
  | xargs -0 cat | wc -l                                               # -> must equal the above
```

A mismatch on the second pair is a **stop-and-diagnose**, not a
re-measure-and-record: it means a corpus-set file changed between the script's
measurement and this check, which is exactly what the precondition assertion and
the sub-step ordering exist to prevent.

### Fallback recipe

Used only when the script exits non-zero. Emit the nine fallback-required fields
by hand and append one line; omit every fallback-optional field rather than
guessing at it.

- `schema_version` — literal `1`
- `date` — `date -u +%F`
- `branch` — `git branch --show-current`
- `issue` — the issue number as an int, or `null`
- `spec_base` — the progress file's basename minus `.progress.md`
- `incomplete` — literal `true` (the fallback path is itself a degradation)
- `files_changed`, `insertions`, `deletions` — parsed by keyword from
  `git diff --shortstat "$(git merge-base main HEAD)"..HEAD`, each defaulting to
  `0`; all three `0` if no base is obtainable

Compose with `jq -n` and append with `printf '%s\n' >> ai-docs/metrics/task-runs.jsonl`
so the trailing newline is preserved.

### Worked fallback example

```json
{"schema_version":1,"date":"2026-07-31","branch":"feat/2026-07-31-task-run-telemetry","issue":186,"spec_base":"2026-07-31-task-run-telemetry","incomplete":true,"files_changed":7,"insertions":812,"deletions":3}
```

Its key set is exactly the nine fallback-required fields: a **proper subset** of
the script's 18 keys and a superset of the required set. Both containments are
asserted mechanically by case 11 of
`.claude/skills/task/scripts/test-append-task-run.sh`, which re-derives the
required set from this page's own field table rather than hardcoding it.
