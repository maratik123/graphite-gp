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
  bad one — same discipline as the append-only corrections log.

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
| `rounds` | int | count of `^## Self-Review \(Round [0-9]+\)[[:space:]]*$` headings — trailing whitespace tolerated | fallback-optional |
| `hit_round_cap` | bool | `rounds >= 3 && verdicts[2] == "REJECT"` | fallback-optional |
| `verdicts` | array\<string\> | first `^\*\*Verdict:\*\*` line per section, in round order | fallback-optional |
| `findings` | object `{blocker,major,minor,nit}` int | severity cell of every `^\|[[:space:]]*[0-9]` row inside a Self-Review section — the leading pipe may be followed by any amount of whitespace, so column-aligned (`\|  1 \|`) and tight (`\|1\|`) rows both count | fallback-optional |
| `findings_first_seen` | object `{blocker,major,minor,nit}` int | same rows, restricted to those absent from the preceding round — **coupled** to its degeneracy signature, see § *Counting units* | fallback-optional |
| `objections` | int | **status** cells containing `⚠️ Objected` (substring match) — the marker is counted only in the row's status cell, never elsewhere in the row | fallback-optional |
| `objections_reopened` | int | **status** cells containing `🔁 Re-opened` (substring match) — same restriction | fallback-optional |
| `files_touched` | array\<string\> | `` ^-[[:space:]]+`<path>` `` lines under `## Files touched` — one or more spaces after the dash | fallback-optional |
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
`^## Self-Review \(Round [0-9]+\)[[:space:]]*$` heading and the next `^## ` heading. This
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
- **Identity key: the `File:line` cell, after trimming surrounding whitespace.**
  Two rows in consecutive rounds are "the same finding" iff their `File:line`
  cells are identical once leading and trailing whitespace is stripped
  (`gsub(/^[[:space:]]+|[[:space:]]+$/, "", key)`). Cell **padding** therefore
  does not drift the key — `\| src/a.rs:9 \|` and `\|src/a.rs:9\|` are the same
  finding. Nothing else is normalised: the path and line number are compared
  byte-for-byte, which is why a line-number shift *does* drift the key.
- **Known bias, direction fixed: `findings_first_seen` is biased upward.** If a
  finding's location moves between rounds — a fix shifted line numbers, or the
  reviewer cites a different line for the same defect — the key changes and the
  row re-counts as first-seen. Path-only matching was rejected as a mitigation:
  it merges genuinely distinct findings in the same file, under-counting instead.
  The series stays usable because the direction of the bias is known: read
  `findings_first_seen` as an **upper bound** on the defect population.
- **Frequency, not only direction: key drift is the *expected* case whenever
  several findings share one file.** Direction alone ("upper bound", "biased
  upward") is insufficient, because it invites reading the series as merely
  conservative when on the informative runs it is close to uninformative. The
  reason is structural, not incidental: `/task` Step 11 applies fixes between
  rounds, any fix that changes a file's line count shifts every line below it, and
  `self-review` re-derives every location against the current tree each round — so
  a carried-forward finding arrives under a different key. The condition that makes
  the `findings` / `findings_first_seen` split worth anything (several findings in
  one file, some fixed, some carried forward) is *exactly* the condition that moves
  the lines, so `findings_first_seen` collapses toward `findings` on precisely the
  runs the split exists to illuminate. Consequence for an analyst: a **low
  `findings_first_seen`/`findings` ratio is ambiguous** — it may mean "few repeat
  findings" **or** "many line-shifting fixes", and the record **cannot distinguish**
  the two.
- **Degeneracy signature — how to tell a measured run from a collapsed one.** On a
  run with `rounds > 1`, `findings_first_seen == findings` means **no row matched
  between rounds**. The log **does not distinguish the cause**: either there
  genuinely were no repeat findings, or a `File:line` key drifted because a Step-11
  fix shifted line numbers. A **ratio below 1** is evidence the key held for at
  least one row. The signature is stated at that strength and no stronger — "a
  ratio of 1 almost certainly means drift" would be **false**, because `✅ Fixed`
  items are not re-raised, so a run whose round-1 findings were all fixed and whose
  round 2 raised different ones legitimately yields a ratio of exactly 1.
- **Coupling clause — binds future edits, not just the commit that added the
  field.** `findings_first_seen` and its degeneracy signature ship together or not at all.
  If the signature above is ever unsatisfied — the sentence deleted,
  softened, or its verification grep failing — the correct response is to **remove
  `findings_first_seen`** from this schema, from the extractor and from the
  fixture; never to keep the field and drop the sentence. The field's presence is
  conditional on a reader being able to identify the runs where it degenerated: an
  unlabelled degenerate measure is not conservative, it is silently wrong, and an
  edit that drops the label changes what every record means mid-series while
  leaving them all looking identical.

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
| `git branch --show-current` yields empty (detached HEAD, or not a git work tree) | `branch: ""` — fallback-required, and last-line-wins dedup keys on it |
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

Run verbatim; the `:(glob)` magic prefixes and the `:(exclude)` term are equally
load-bearing:

```bash
git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
  ':(exclude)ai-docs/learnings.md' \
  | xargs -0 cat | wc -l
```

**The exclusion criterion — a property, never a list of names.** The field counts
files whose length reflects **instruction content**. Files whose volume is instead
**driven by the codebase or by journaling** are excluded, because their growth
carries no signal about instruction density and would mask the signal that does.
That criterion *is* the definition; every file list below is **derived from it**
and may be refreshed without reopening the decision. A page naming a specific file
as the rule, rather than as current derived membership, has restated the defect the
criterion exists to avoid — a copy of membership whose owner is another document.

**Derived membership — a measurement with a date. Re-derive it at each pinning;
never transcribe it forward.** Measured 2026-07-31, five files inside the counted
set satisfy the criterion: `ai-docs/learnings.md` (610 lines — journaling, one
entry per correction by mandate), `ai-docs/library-survey.md` (55) and
`ai-docs/dependency-versions.md` (50) (volume set by the dependency count), and
`ai-docs/panic-index.md` (14) and `ai-docs/unsafe-index.md` (7) (volume set by the
codebase — every production panic and `unsafe` site must add a row). Only the first
is material by size, so the `:(exclude)` term above carries that one file. The
other four fall under the criterion **in principle** and are retained in v1 purely
as negligible; they will need excluding once they grow.

**Re-check threshold — 1% of the counted corpus**, the denominator being the
post-exclusion total this command itself produces at that commit, so the guard and
the number it guards are always measured together. The rule is **mechanical, not
advisory**: when a file that satisfies the criterion crosses 1%, it **is** excluded
**at the next pinning** of the command — not "should be considered for exclusion".
The criterion never changes; only the derived membership does.

**Crossings are one-way.** Falling back below the threshold **does not restore** a
file to the count. What excludes a file is the criterion, never its size; the
threshold decides only when acting on the criterion becomes material. A file that
grows past it, is excluded, and later shrinks still satisfies the criterion, so
keeping it excluded is correct and is not an under-count — and one-way membership
also stops a file oscillating around the threshold from injecting steps into the
very trend the field exists to show. The one case that would genuinely under-count
is a file that **stops satisfying the criterion** (repurposed, or its volume
becoming instruction-driven); v1 does not detect that and accepts it.

**Baseline: the first `instruction_corpus_lines` value recorded in
`ai-docs/metrics/task-runs.jsonl`**, over 59 files at the `:(glob)` pathspec's
scope. Not restated as a number here: the corpus is still growing while this
page's own commits land, so any figure pinned in prose decays before the log
it describes exists. Re-derive with the pinned command above, against the
commit the first corpus line was appended at, if a fixed reference point is
needed. Any pre-exclusion figure is **not comparable** with a post-exclusion
one — the two count different sets — so a series starts from the
post-exclusion baseline and never splices the older basis onto it.

**Why the obvious pathspec is wrong.** Git's default wildmatch lets `*` cross
`/`, so a plain `'ai-docs/*.md'` reaches below depth 1 into `ai-docs/plans/`,
`ai-docs/plans/done/` and `ai-docs/templates/` — the archive of completed specs
and designs, which has nothing to do with the instruction corpus. The durable
statement is a **re-runnable discriminator**, not a line-count pair, because any
count here decays with the archive:

```bash
git ls-files -- '<form>' | grep -vc '^ai-docs/[^/]*\.md$'   # bare -> 109 ; :(glob) -> 0
```

Scope matters: the probe is over the **`ai-docs` pathspec alone**. Run across the
whole corpus set it yields 147 / 38, because the 38 non-`ai-docs` members fail the
`^ai-docs/…$` pattern on *both* sides and are counted twice — the same finding,
but not the stated figure. The `:(glob)` prefix restores pathname semantics and
matches at depth 1 as well as deeper, so a future `.claude/foo.md` is not silently
dropped.

`xargs -0 cat | wc -l` is used deliberately over `xargs wc -l | tail -1`: the
latter emits multiple `total` lines whenever `xargs` splits the argument list.

## What this log does NOT measure

**Open questions, not caveats.** Every entry below ends undecided, and that framing
is deliberate: a caveat reads as a closed topic — acknowledged, therefore handled,
therefore nobody's problem — and decays into skimmed boilerplate, whereas an
undecided question stays a live agenda item. Entries (i)–(viii) are
*coverage gaps*: axes the record cannot see. Entry (ix) is different in kind —
a field the log *does* measure and **reports inverted**. Entry (x) is different
again, and upstream of both. **No fields are added for any of this**:
`spec_amended_during_impl`, `subtasks_reopened`, and any handoff-compliance flag
are out of scope for v1, and this section is prose by design.

**(i)** *Is the second axis — process and handoff soundness — worth measuring at
all?* Process and handoff failures are orthogonal to every field here. A run whose
durable state was never maintained, or whose handoff protocol was skipped, emits a
completely normal-looking line, because **no field encodes handoff state**. The
standing consequence, which every consumer of this file must carry: **a clean
`rounds` trend is not evidence that the surrounding process was sound, because the
log cannot report otherwise.** The claim is structural and needs no incident to
support it. **Undecided**: does that axis warrant instrumentation here, a separate
log, or nothing?

**(ii)** *Should the `## Decisions log` be parsed?* It is not parsed in v1, so spec
churn during implementation and reopened subtasks are invisible to the record even
though the progress file witnessed both. Note explicitly that this one is **not
near-free**: the event exists only as prose, so capturing it needs a parser or new
instrumentation, not a field. **Undecided**: is that prose worth parsing, or does
the event belong in a surface that records it structurally?

**(iii)** *Should post-Step-12 rounds be captured?* Reviewer rounds
(`/pr-commented`) and CI-fix rounds (`/pr-ci-failed`, `/main-ci-failed`) extend the
same progress file *after* this record has been written, so they lie outside it
entirely. Capturing them means a second writer at `/pr-merged` and a different
lifecycle. **Undecided**: is a second writer worth the drift risk two writers
introduce?

**(iv)** *Should the 10-findings-per-round truncation be corrected or merely
flagged?* The reviewer's cap silently truncates `findings` on high-finding rounds,
and nothing in the progress file distinguishes a capped round from a round that
genuinely had ten. **Undecided**: raise the cap, mark capped rounds, or accept the
under-count?

**(vi)** *Should the record count **planning** rounds, not just review rounds?*
`rounds` counts `/task` Step 10 self-review rounds **only**; `/interview` spec
rounds, `design` rounds and `design-review` rounds appear nowhere in the record.
Two runs with identical `rounds` may therefore have reached implementation at
wildly different cost, and **a flat `rounds` trend after an `/improve` escalation is
compatible with that escalation having doubled the design phase.** This is the
sharpest entry in the list, because it is the log's *own motivating question*: the
log exists to ask whether process density pays for itself, and a record that omits
most of the process cannot answer it — a run can spend the large majority of its
effort in spec and design and still emit a record whose only effort signal is a
self-review count that never moved. **Undecided**: per-phase round counts, a coarse
planning-effort proxy, or nothing?

### One property, three faces — (v), (vii) and (viii)

**The consequence first, because it is what an analyst needs: a run that took three
attempts at the fixture and a run that took one produce identical records.** That
sentence names the question this log cannot resolve; the three mechanisms below
only explain how the gap arose.

**The property beneath them:** the harness **destroys working state at the boundary
of the step that produced it, and versions only the product.** State that plainly
as a *real trade*, not an oversight — it is reasonable hygiene for human review,
keeping diffs clean and transient state out of history, with a genuine benefit. The
cost falls entirely on the harness's ability to measure *itself*, a use case the
design predates.

**(v)** *Does the harness need a durable surface that is **not** the repository?*
This log's own source, `ai-docs/plans/<spec-base>.progress.md`, is gitignored and
therefore has **no history**, so any question about *when* a field was written or
*by whom* — delegate, or orchestrator backfilling afterwards — is unanswerable once
the session ends. The only reconstruction available is file mtime, which does not
survive a copy, checkout, archive, or clone. The task persists *derived* telemetry
from a source that is itself unauditable. Be honest about the provenance: the repo
records a **classification, not a justification** — the ignore rule and the agent
docs both merely label the file "local-only", and the pattern arrived in a single
bulk harness-import commit, so no per-line rationale exists in this repository's
history. The plausible diff-noise argument (the file is rewritten at every step
boundary, so versioning it would put that churn in every PR) is a **conjecture, not
the recorded reason**, and must not be presented as one. The question is therefore
**not** "should the progress file be versioned?" — that would sacrifice the very
benefit that motivates destroying it. **Undecided**, and out of scope here: does the
harness need a durable append-only event journal *outside* the repository?

**(vii)** *Is the harness's working state systematically unrecoverable?* Second face
of the same property. Two artefacts recording how a run actually proceeded are
destroyed by design: `/interview`'s `.state.md`, created at the start of round 1 and
deleted on terminal exit, and `<spec-base>.progress.md`, gitignored and deleted
after merge. Both hold exactly the process history this log tries to summarise, so
spec round counts and handoff authorship are **unverifiable after the fact, not
merely unrecorded**. The two reach that outcome by **different mechanisms** — the
progress file is matched by an ignore rule; `.state.md` is matched by none and is
simply deleted outright. That they converge without sharing a mechanism is what
supports reading this as a property of the harness rather than one rule applied
twice, and here too the repo records no rationale for either deletion. **Undecided**:
a general fix (persisting working state), a per-artefact one, or nothing?

**(viii)** *Is uncommitted work invisible to the record?* Third face. This log is
derived at Step 12 from committed history plus the progress file, so work that
existed but was never committed — a drafted fixture, a half-finished edit, an
implementation discarded and redone — appears nowhere: not in `git log`, not
necessarily in the progress file, not in the record. Where (v) and (vii) concern
state *destroyed* after the fact, this concerns state that **never entered version
control at all**. Shared consequence: the harness's true working state is
recoverable only while the session is live, and only by inspecting the working tree
directly — never by reading a durable surface or trusting a report. A
Step-12-derived record describes what *landed*, and **cannot be read as a measure of
effort**. **Undecided**: is effort that did not survive to a commit worth
representing, or is committed history simply the right unit?

### Different in kind

**(ix)** *Do these fields report the sign backwards?* Unlike every entry above, this
is not a coverage gap: it is a field the log measures and **reports inverted**, and
it sits nearer in kind to the `findings` / `findings_first_seen` split than to
anything else here. The inversion: a **rigorous** self-review that finds real
defects across three rounds emits `rounds: 3`, high `findings`, non-zero
`objections`; a **perfunctory** one that APPROVEs on round 1 emits `rounds: 1`,
`findings: {}`. On every field, the thorough run reads worse than the careless one.
**The condition is the usable part, not a hedge:** the inversion holds while the
number of defects in the diff is roughly fixed and review *thoroughness* varies.
When it is instead **input code quality** that varies, the sign is normal and the
fields read correctly — better code genuinely produces fewer findings and fewer
rounds. The discriminator, plainly: *these fields are readable when comparing runs
of **comparable review thoroughness**, and unreadable when thoroughness itself
differs between the runs being compared.* Without it an analyst either trusts the
fields everywhere or distrusts them everywhere, and both are wrong. **Prohibition,
stated directly because the misuse is the obvious next step: do NOT optimise against
these fields.** Once `rounds` becomes a target, the cheapest way to improve it is to
review less thoroughly — Goodhart, and more dangerous here than usual because the
goal reads as virtuous ("reducing review-cycle cost" is exactly what a well-meaning
reader would adopt this log for). **A downward `rounds` trend is not self-evidently
an improvement and must never be adopted as an objective.** **Undecided**: a
thoroughness covariate, a norm against target-setting, or nothing?

**(x)** *What will cause this log to be read?* Where (i)–(viii) concern what the
record cannot say and (ix) a field whose sign is inverted, this asks whether the
record is **consulted at all** — which is **upstream of every other** question in
this section, because a log that accumulates correctly and is never read costs the
writing and returns nothing. The failure shape is **observed in this repository, not
hypothetical**: the corrections log has a write trigger that is an AXIOM (an entry
is compelled on any instruction violation) and a read trigger that fires only on
explicit human invocation — and it has accumulated an **order of magnitude past its
escalation threshold** without ever being read. `task-runs.jsonl` is being given the
same architecture: mandatory mechanised writes at `/task` Step 12, and no consumer
at all, since every one of them is deferred out of v1. (The precise counts behind
that precedent are deliberately *not* on this page — they are command outputs that
decay in one commit, and this is the last section that should carry a rotting
figure. They live in the source spec's § *Key decisions* with their reproducing
command.) **Undecided**: does this log need a read trigger, a consumer, or an
escalation threshold of its own?

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
  ':(exclude)ai-docs/learnings.md' \
  | xargs -0 cat | wc -l                                               # -> must equal the above
```

The second command must be **character-identical** to the pinned command above,
`:(exclude)` term included — that identity is the whole point of the comparison.
A block re-pinned on one side only computes a different corpus from the one the
script wrote and reports a guaranteed mismatch on a correct run.

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
