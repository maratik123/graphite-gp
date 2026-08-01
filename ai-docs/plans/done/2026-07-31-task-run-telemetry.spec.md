# Task-run telemetry JSONL log

**Source:** issue #186
**Date:** 2026-07-31
**Tracked in:** #186

Give the harness a durable, longitudinal record of its own behaviour. Today every
`/task` run measures itself — self-review rounds, per-round verdicts, finding
severities, objections — into `ai-docs/plans/<spec-base>.progress.md`, and that
file is gitignored and deleted after merge, so none of it survives. Without the
record, an `/improve` escalation into the instruction corpus cannot be shown to
have reduced, increased, or not moved the per-task review cost.

This task adds **extraction and persistence**: one JSON line per completed
`/task` run appended to `ai-docs/metrics/task-runs.jsonl` at Step 12, derived from
the progress file that exists at that moment — plus **one narrow, deliberate
addition to `self-review`'s status vocabulary** (see *Carve-out from issue AC3*).

## Scope

1. **New append-only log** at `ai-docs/metrics/task-runs.jsonl` — JSON Lines, one
   object per completed `/task` run, committed (not gitignored).
2. **Extractor script** `.claude/skills/task/scripts/append-task-run.sh` plus a
   companion fixture test, mirroring the in-tree
   `check-citations.sh` / `test-check-citations.sh` pair.
3. **Orchestrator fallback** — when the script cannot append at all, `/task`
   Step 12 writes a minimal record by hand. Its field set is a documented strict
   subset of the script's (see *The two-path contract*).
4. **`🔁 Re-opened` status marker** added to `.claude/agents/self-review.md`, so
   `objections_reopened` is an exact count rather than a heuristic.
5. **`/task` Step 12 integration** — a sub-step that produces and appends the
   record, ordered before the existing "stage all changed files" sub-step so the
   append reaches the PR diff.
6. **Schema documentation** in `ai-docs/`, referenced by link from
   `.claude/skills/task/SKILL.md` (not restated inline).
7. **Graceful degradation** — an absent, truncated, or unparseable progress file
   yields a record carrying whatever fields could be filled plus
   `"incomplete": true`. The append never fails the task.

**Amendment record — `findings_first_seen` is retained with known limits, not
repaired.** The `File:line` identity key's dominant failure mode (§ *Technical
constraints* 8) was caught **mid-implementation**, at the boundary before the
fixture subtask, with subtasks 1–2 already committed (`e7cbe54` empty tracked
log, `ca59991` schema page) and nothing yet baked into an extractor or test —
verified: `.claude/skills/task/scripts/` is still untracked. Two alternative keys
were weighed and **rejected**: path-only (merges distinct findings in one file,
under-counts) and path + normalised `Finding` text (nothing in `self-review.md`
obliges a carried-forward row to keep verbatim text, so it trades a near-certain
failure for a discretionary one). The owner chose to **document reality over
appearing to fix it**. A future reader must therefore not "repair" this by
switching the key or by softening `AC9a`'s assertion — that assertion encodes
measured behaviour, and changing the key silently would break the comparability
of every record written before the change.

**Scope-boundary disclosure added mid-implementation (`AC13b`).** Raised during
this run's implementation phase: the corpus measures a single axis — the cost of
getting a *review* to converge — and an entire second class of failure, process
and handoff breakdown, is invisible to it by construction, since no field in the
record encodes handoff state. The point holds structurally and needs no incident
to support it: a run whose durable state was never maintained emits a
normal-looking JSONL line, so a flat `rounds` trend can never be evidence that
the surrounding process was sound. `AC13b` requires this as prose on the schema
page; **no field is added**, deliberately — the fix for an unmeasured axis is to
say it is unmeasured, not to widen v1. The section is specified as **open
questions rather than caveats**: a caveat reads as a closed topic and decays into
boilerplate, while an undecided question stays a live agenda item. Note for
whoever writes the page: the structural claim needs no incident to support it, so
state it structurally — do **not** attach a "first observed instance" to a
specific run unless that run's handoff surface has actually been inspected and
found wanting.

**Record-only observation (no action taken, deliberately).** This spec carries a
large acceptance-criteria set for what the issue frames as "extraction and
persistence, not new measurement". Whether that density is proportionate is precisely the question
this corpus is being built to answer, and it cannot be answered from inside the
task that raises it — a spec cannot measure its own overhead. Noted here so the
first analysis pass over `task-runs.jsonl` has the observation on record rather
than reconstructing it. The AC set was **not** restructured in response.

## Out of scope

- Any dashboard, aggregation, trend computation, or automated reaction to the
  numbers. Collect first.
- Retroactive backfill of past `/task` runs. Not merely undesired — **impossible**:
  zero `*.progress.md` files exist on disk (`git status --porcelain --ignored`,
  `find` both empty) and zero were ever added in history
  (`git log --all --diff-filter=A --name-only --pretty=format: -- '*.progress.md'`
  empty). The source data for past runs does not exist anywhere.
- Emitting or updating records from `/pr-commented`, `/pr-ci-failed`,
  `/main-ci-failed`, `/project-review`, or `/bugfix`. v1 writes exactly one line,
  at `/task` Step 12.
- Any edit to `.claude/skills/pr-merged/scripts/cleanup-progress.sh` or to the
  progress-file lifecycle (creation, gitignore status, deletion point).
- Adding a third implementer disposition between `✅ Fixed` and `⚠️ Objected`
  ("the finding is real but the proposed fix is wrong") — a separate change to a
  Subagent contract, explicitly excluded by the issue. The `🔁 Re-opened` marker
  in scope is a **reviewer-side** annotation on an existing `⬜ Open` row, not a
  new implementer disposition.
- Changing which severities `self-review` may assign, or the `APPROVE` / `REJECT`
  verdict vocabulary.

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| Reviewer-round and CI-fix-round counts (`/pr-commented`, `/pr-ci-failed`, `/main-ci-failed` extend the same progress file *after* Step 12 has written the record) | The record must be in the PR diff (AC2), which forces Step 12; those rounds do not exist yet at that point. Capturing them means a second writer at `/pr-merged`, a different lifecycle. | Yes |
| Aggregation / trend tooling over the log | Explicitly out of scope in the issue; needs data first | Yes |
| Correlating a landed `/improve` escalation with the rounds that followed it | Consumer of this data, not part of collection | Yes |
| Telemetry for `/project-review` runs (its `review-findings` table has the same shape) | Different lifecycle and no PR diff to ride; the schema would need a `source` discriminator | Yes |
| Records for `/task` runs that abort before Step 12 | "One line per **completed** run" is the issue's framing; an abandoned run has no PR to carry the diff | No |

## Carve-out from issue AC3

Issue AC3 reads: *"The record is derived from the progress file only — no new
instrumentation in subagents, no change to `self-review`'s contract."*

**That clause is amended by this spec, deliberately and narrowly.**
`objections_reopened` is not derivable under it: `.claude/agents/self-review.md`
§ 7 instructs the reviewer to re-open an invalid `⚠️ Objected` item as plain
`⬜ Open` and prescribes **no** distinguishing token, so a re-opened row in
round N+1 is textually identical to a newly-raised one. The alternatives were a
best-effort `File:line` heuristic (miscounts silently) or dropping the field
(objection health stays half-visible). The product owner chose the exact count.

The amended criterion is **AC3** below. The carve-out is bounded to: adding one
status marker written by `self-review` § 7. No new severity, no new verdict, no
new implementer disposition, no new file written by any Subagent.

## The `🔁 Re-opened` marker is ADDITIVE, not a replacement

The marker **appends to** the existing status, producing `⬜ Open 🔁 Re-opened`.
It does **not** replace `⬜ Open`.

This is not cosmetic. A replacement token would silently drop every re-opened
finding out of the loops that consume `⬜ Open` — the exact findings the reviewer
judged most in need of attention.

**Complete enumeration — a pre-implementation baseline, not a live invariant.**
This task's own subtask 5 renders the marker as `⬜ Open 🔁 Re-opened`, which
itself contains `⬜ Open`, so these figures move once it lands; they are evidence
for the additive design at the time of writing, and `AC4` is deliberately stated
as a property so no acceptance test depends on them. As measured before
implementation, `grep -rn '⬜ Open' .claude/` returns **14 occurrences
across 6 files**; `grep -rn '⬜ Open' AGENTS.md ai-docs/` (excluding `learnings.md`
and `plans/`) returns none, so `.claude/` is the whole population.

**Classification basis** (mechanical, so a later reader can re-derive it): an
occurrence is a **template** if the line is a markdown table *data row* inside a
fenced skeleton block — `grep -rn '⬜ Open' .claude/ | grep -E ':\| [0-9]'`, which
selects rows beginning `| <digit> |`. All other occurrences are **consumers**:
prose that reads the status and branches on it. The split is 3 template / 11
consumer.

### Consumers (11) — these are what the additive form protects

| File:line | Site | What breaks under a replacement token |
|---|---|---|
| `.claude/skills/task/SKILL.md:211` | Step 11 "For each `⬜ Open` finding…" | Re-opened findings never fixed |
| `.claude/skills/task/SKILL.md:196` | "surface all remaining `⬜ Open` findings" after round 3 | Re-opened findings never surfaced |
| `.claude/skills/task/reference.md:247` | Step 11 narrative, same loop | Same |
| `.claude/skills/project-review/SKILL.md:81` | Phase-2 loop over the `## AC Status` table | Re-opened findings skipped in `/project-review` |
| `.claude/skills/project-review/SKILL.md:135` | "Fix each `⬜ Open` finding from the self-review section" | Same — this is the site a `/project-review`-hosted § 7 re-open lands in |
| `.claude/skills/project-review/SKILL.md:138` | Post-round-3 surface | Re-opened findings never surfaced |
| `.claude/skills/bugfix/SKILL.md:276` | On-REJECT fix loop | Re-opened findings never fixed in `/bugfix` |
| `.claude/skills/bugfix/SKILL.md:277` | Post-round-3 STOP + surface | Re-opened findings never surfaced |
| `.claude/agents/self-review.md:164` | "For REJECT: at least one … row with `⬜ Open` status" | A round containing only re-opened findings could not produce REJECT |
| `.claude/agents/self-review.md:175` | **The re-open WRITE site** — "Vague reasons … → re-open as `⬜ Open`" | This is the line that must emit the marker; it is both writer and vocabulary statement |
| `.claude/agents/self-review.md:177` | "Focus on remaining `⬜ Open` items" | Next round stops focusing on re-opened items |

`self-review.md:175` is the load-bearing one: it is the only occurrence that
*writes* a status rather than reading one. Its siblings at l.128–130 and l.176 say
"→ re-open" without naming a token, so they inherit whatever l.175 defines.

### Templates (3) — illustrative rows, different treatment

| File:line | Block | Treatment |
|---|---|---|
| `.claude/agents/self-review.md:157` | § *Findings format* example row | The block is the **vocabulary definition**, so the marker must be documented here (a vocabulary line; an example row is optional) |
| `.claude/agents/self-review.md:158` | Same block, second example row | Same |
| `.claude/agents/review-findings.md:155` | Progress-file skeleton `review-findings` writes at creation | **Must NOT show the marker** — see the reconciliation below |

### Reconciliation with § *Propagation scope*

`review-findings.md:155` carries `⬜ Open` yet § *Propagation scope* rules that
file "no token needed". Both are correct, for a reason worth stating rather than
waving through: l.155 is the template for a **freshly created** findings table,
and a re-opened row cannot exist at creation time — re-opening happens only in
`self-review` § 7, on a later round. `review-findings` never re-reviews an
objection, so it never writes the marker, and showing it in a creation-time
skeleton would misrepresent the initial state. The row is correct unchanged.

### Does the "no sync group" argument survive?

Yes, and it is stronger than round 2 stated. Of the **11 consumer** sites, the
Review sync group (`project-review/SKILL.md`, `review-findings.md`) reaches only
**3** — all in `project-review/SKILL.md`; `review-findings.md` contributes a
template, not a consumer. Three more are inside `self-review.md` itself (the file
being edited). The remaining **5 consumer sites live in three files —
`task/SKILL.md` (2), `task/reference.md` (1), `bugfix/SKILL.md` (2) — that share
no sync group with `self-review.md`**, so the Propagation Rule would not point at
any of them. The additive form is what keeps all 11 working untouched.

## Propagation scope — measured, not assumed

The round-1 answer flagged the Review sync group and
`ai-docs/templates/progress-format.md` as candidates. Both were read. Result:

| File | Needs the token? | Evidence |
|---|---|---|
| `.claude/agents/self-review.md` | **Yes — the only file that must change.** | Sole definer of the findings-table status vocabulary (§ *Findings format*, l.146–164) and sole site of § 7 objection-quality re-opening (l.124–130) and the round>1 rules (l.172–177). |
| `.claude/skills/project-review/SKILL.md` (Review sync group) | **No token needed** — but the Propagation-Rule check is still required and its outcome recorded. | It reuses `self-review` (so § 7 can fire in its flow), but its own text only *consumes* `⬜ Open` (l.81, 135, 138), which the additive form preserves. It defines no status vocabulary. |
| `.claude/agents/review-findings.md` (Review sync group) | **No token needed** — check still required. | It **does** carry one `⬜ Open` occurrence (l.155), but as a *creation-time template* row, not a consumer — see § *The marker is ADDITIVE* → *Reconciliation*. It creates the initial whole-codebase findings table and never re-reviews an objection; § 7 exists only in `self-review`, so a re-opened row cannot exist at creation time. Its severity list (l.170) is unchanged by this task. |
| `ai-docs/templates/progress-format.md` | **No.** | Read in full: it does **not** define the findings-table status vocabulary at all. Its `## AC Status` table is `\| AC \| Status \|` carrying `PASS / FAIL / NOT_TESTED` — a different table for a different purpose. It references `## Self-Review (Round N)` only in the lifecycle prose (l.86). The round-1 premise that it is "the canonical status-vocabulary home" does not hold. |

## The two-path contract (drift constraint)

The script and the orchestrator fallback both emit into one schema. The accepted
drift risk is constrained mechanically:

- **Every field is classified in the schema doc as `fallback-required` or
  `fallback-optional`.**
- **`fallback-required`** (the orchestrator MUST emit these): `schema_version`,
  `date`, `branch`, `issue`, `spec_base`, `incomplete`, `files_changed`,
  `insertions`, `deletions`.
- **`fallback-optional`** (the orchestrator MAY omit): `rounds`, `hit_round_cap`,
  `verdicts`, `findings`, `findings_first_seen`, `objections`,
  `objections_reopened`, `files_touched`, `instruction_corpus_lines`.
- **The fallback MUST NOT emit any key absent from the script's key set.** Strict
  subset, enforced by AC10 against a worked example in the schema doc — not by
  prose alone.

> **The diff-size trio needs a base commit, and that forces a two-source rule.**
> The natural base is `**base_commit:**` — a *required* progress-file header
> (`ai-docs/templates/progress-format.md:12`), immutable after creation (`:73`).
> But `files_changed` / `insertions` / `deletions` are `fallback-required`, and the
> fallback fires precisely when the progress file could not be read, so a
> progress-file-only base would make them unobtainable exactly when they are
> mandatory. Rule: the **script** uses `**base_commit:**`; the **fallback** uses
> `git merge-base main HEAD`, which needs no progress file. In the normal `/task`
> flow the two coincide (the branch is cut at Step 1 and the progress file is
> created at Step 8 with no intervening commits); when they differ the record is
> already flagged `"incomplete": true`, so the looser base is not passed off as the
> precise one. Both sources are stated in the schema doc.

**Exit-code contract** (resolves what would otherwise be two contradictory
requirements — "never fails the task" vs "fallback fires on non-zero"):

| Situation | Script behaviour | Orchestrator |
|---|---|---|
| Progress file present and parseable | Append full record; **exit 0** | Nothing further |
| Progress file absent, truncated, or its `## Self-Review` sections malformed | Append degraded record with `"incomplete": true`; **exit 0** | Nothing further |
| Cannot append at all (unwritable path, missing `jq`, unexpected internal error) | Emit a one-line diagnosis to stderr; **exit non-zero** | Write the `fallback-required` record by hand, then continue Step 12 |

Under no path does Step 12 halt on this sub-step.

## Key decisions

| Question | Decision |
|---|---|
| Where does the append happen? | `/task` Step 12, before the existing staging sub-step (currently sub-step 7). Step 12 runs INDEX finalisation → inbox propagation → `cargo build` → stage → commit → push → `gh pr create`; the progress file is in the working tree throughout. |
| Is the progress file available at Step 12? | Yes. It is deleted by `/pr-merged` step 3 (`cleanup-progress.sh`), i.e. **after merge** — not at `/task` finalise. The issue's "deleted at finalise" wording is imprecise; nothing in the proposal depends on it. |
| Does the new log need a `.gitignore` change? | **No.** `git check-ignore -v ai-docs/metrics/task-runs.jsonl` exits 1 — no rule matches. (`ai-docs/plans/**/*.progress.md` is ignored via `.gitignore:11`, but that pattern does not reach `ai-docs/metrics/`.) |
| Format | JSON Lines, append-only, consistent with `ai-docs/deferred/_inbox.jsonl`. `jq` is allow-listed in `.claude/settings.json` (`"Bash(jq *)"`). |
| Is the `_inbox.jsonl` single-writer AXIOM affected? | No — that AXIOM names `ai-docs/deferred/_inbox.jsonl` specifically. The new file is a separate surface with its own single writer and its own hand-edit prohibition, stated in the schema doc. |
| `schema_version` field | Included, `1` in v1. One integer now avoids an un-versioned corpus later when the deferred fields land. |
| `objections` / `objections_reopened` counting unit | **Status cells, not distinct findings**, summed across all `## Self-Review (Round N)` sections: `objections` = cells containing `⚠️ Objected`; `objections_reopened` = cells containing `🔁 Re-opened`. Both inherit the carry-forward inflation described in the row below; they are retained as-is because an objection is an *event* (a push-back was made in round N), which is the thing being measured, not a defect population. |
| **Why `findings` alone is not a cost signal** | `self-review.md:177` tells round>1 to "Focus on remaining `⬜ Open` items **plus** anything newly introduced", and § *Findings format* says an APPROVE table "is empty (no rows) **or contains only already-resolved items**" — rows **carry forward**. An all-rounds sum therefore counts one finding once per round that saw it, so it scales with `rounds` and correlates with it near-tautologically. Both fields are kept, with distinct meanings (below). |
| `findings` (retained) | Per-severity counts over **all** rounds' table rows — a *review-effort* measure (how much finding-handling the loop did), explicitly **not** a defect count. |
| `findings_first_seen` (new) | Per-severity counts of rows **absent from the immediately preceding round's table** — the defect-population measure. Round 1 contributes all its rows. This is the field to correlate against an `/improve` escalation. |
| Row identity across rounds | The **`File:line` cell, verbatim**, is the key: two rows in consecutive rounds are the same finding iff their `File:line` cells are byte-identical. **The key drifts in the common case, not at the margin — and the spec states frequency, not just direction.** Mechanism: `/task` Step 11 fixes findings between rounds; any fix that changes a file's line count shifts the numbering of everything below it, and `self-review` re-derives line numbers each round against the current tree (`.claude/agents/self-review.md:169` — "every violation must have an exact file and line number"). A carried-forward finding therefore arrives in round N+1 under a *different* key and re-counts as first-seen. **The sharp part is structural:** the condition that makes the `findings` / `findings_first_seen` split worth anything — several findings in one file, some fixed, some carried forward — is *exactly* the condition that moves the lines. So `findings_first_seen` approaches `findings` precisely on the runs the field was introduced for, and agrees with it mainly on the trivial ones (findings spread one-per-file, or no fix changed line counts above them). This is an **anti-correlation with the field's usefulness**, not a bias to be averaged out. Path-only matching was rejected: it merges genuinely distinct findings in the same file, under-counting instead. Switching the key to path + normalised `Finding` text was **weighed and rejected by the owner** — nothing in `self-review.md` requires a carried-forward row to keep verbatim `Finding` text, so it trades a near-certain failure for a discretionary one; documenting reality beat appearing to fix it. **Coupling clause (repeated here deliberately, so it is reachable from the identity decision as well as from `AC13a`):** because this key degrades in the common case, `findings_first_seen` is defensible **only** while a reader can tell which runs degenerated. The field and its degeneracy signature **ship together or not at all** — if the signature requirement in `AC13a` is ever unsatisfied, remove the field from the schema rather than keeping it unlabelled. A later edit that drops the sentence and leaves the field standing converts a labelled-degenerate measure into a silently-wrong one. |
| Keep or drop `findings_first_seen` in v1? | **KEEP — decided, do not re-open.** Dropping it was weighed on the grounds that it degenerates on exactly the informative runs (§ *Technical constraints* 8), and rejected: `findings` alone scales with `rounds` near-tautologically, so dropping the split returns the spec to the very problem that motivated it. A field that degenerates on *some* runs carries strictly more information than no field **provided the degeneracy is labelled** — which the degeneracy signature below supplies. That proviso is the whole basis of the decision, so the signature is not optional garnish. |
| **Degeneracy signature** (how an analyst tells a measured run from a collapsed one) | **Settled wording**, stated here so `AC13a` can require it on the schema page: *On a run with `rounds > 1`, `findings_first_seen == findings` means **no row matched between rounds**. The log does not distinguish the cause — either there genuinely were no repeat findings, or the `File:line` key drifted because a Step-11 fix shifted line numbers. A ratio below 1 is evidence the key held for at least one row.* The wording is deliberately weaker than "a ratio of 1 almost certainly means a shift", which was **withdrawn as overstated**: `self-review.md:173` says `✅ Fixed` items are **not** re-raised, so a run whose round-1 findings were all fixed and whose round 2 raised different ones legitimately has zero carry-forward and a ratio of exactly 1. Asserting a diagnosis the data cannot support would reintroduce the unreproducible-claim class this task has already had to remove. |
| `hit_round_cap` (new) | Boolean. `/task` Step 10 caps the loop at 3 rounds (`.claude/skills/task/SKILL.md:188`) and round 3 with REJECT takes the forced-surface path (`:196`), so `rounds` has almost no dynamic range. Derived as `rounds >= 3 && verdicts[2] == "REJECT"` (round 3 rejected). Distinguishes "closed on round 2" from "ran out of rounds" — the distinction that carries the signal. |
| `files_changed` / `insertions` / `deletions` (new) | Task-size normaliser, from `git diff --shortstat <base>..HEAD`. Without it a downward `rounds` trend after an escalation is indistinguishable from a run of smaller tasks, so the correlation this corpus exists to support is unavailable even in principle. `fallback-required`; base-commit sourcing per § *The two-path contract*. |
| Do the size fields widen the AC3 carve-out? | **No — confirmed, not implied.** They add no Subagent instrumentation and no new Subagent write: `**base_commit:**` is an *existing required* header field that `/task` already writes at Step 8, and `git diff --shortstat` is an ambient git query. § *Carve-out from issue AC3* stays bounded to the `🔁 Re-opened` marker. |
| `verdicts` source | The `**Verdict:** APPROVE \| REJECT` line in each `## Self-Review (Round N)` section, in round order. |
| `rounds` source | Count of `## Self-Review (Round N)` sections (`/task` Step 10 caps at 3). |
| `files_touched` source | The `## Files touched` section, canonical line shape ``- `path` — what changed``. Path only; the description is dropped. |
| Step-12 re-entry (compaction recovery) | Duplicate lines are permitted; consumers take the **last** line per (`spec_base`, `branch`). Strict append-only is preserved and the script stays trivial. See *Open questions* for the in-place-rewrite alternative. |
| `instruction_corpus_lines` set | Broad: `AGENTS.md` + `CLAUDE.md` + all `.claude/**/*.md` + top-level `ai-docs/*.md`, **minus files excluded by the criterion below**. |
| **Exclusion criterion — definitional, stated as a property, never as a list of names** | `instruction_corpus_lines` counts files whose length reflects **instruction content**. Files whose volume is instead determined by the **codebase** or by **journaling** are **excluded**, because their growth carries no signal about instruction density and would mask the signal that does. The criterion is the definition; any file list is **derived from it** and may be refreshed without reopening the decision. |
| Why not "Broad minus `learnings.md`"? | That hardcodes a filename into the pinned command — **a copy of membership whose owner is another document**, the defect class of issue #187. The criterion is definitional; membership is derived and must be presented as derived. |
| Why not "append-only" as the criterion? | **It is the wrong property, and only looks principled.** `learnings.md`'s problem is not its write mechanics but that its volume is driven by something other than instruction content. Verified: `AGENTS.md:280` declares append-only for `learnings.md` **alone** — a `grep -niE 'append-only'` over `AGENTS.md` returns exactly that one hit — so an "append-only as declared in AGENTS.md" test would catch **one** file and structurally miss the four others sharing the real property. A hardcoded name in disguise. |
| Derived membership (measured 2026-07-31; all five confirmed in-Broad) | `ai-docs/learnings.md` **610** (journaling — one entry per correction, by mandate); `ai-docs/library-survey.md` **55** and `ai-docs/dependency-versions.md` **50** (dependency count); `ai-docs/panic-index.md` **14** and `ai-docs/unsafe-index.md` **7** (codebase — every production panic / `unsafe` site must add a row). |
| What v1 actually excludes | **`learnings.md` only** — the sole member material by size. The other four **fall under the criterion and are excluded in principle**, retained in v1 purely as negligible (≤ 55 lines against ~9,700). They will need excluding once they grow; `panic-index` / `unsafe-index` are small only because the Rust codebase is ~44k lines today, and both grow by mandate as it does. The criterion is therefore correct **now** and needs no revisiting when the numbers move — only the derived membership list gets refreshed. |
| **Provenance for `AC13b` entry (x) — the write-mandatory / read-optional precedent** | Measured 2026-07-31 on this repo, reproducing command stated so the figures are re-derivable rather than transcribed: `awk` over `ai-docs/learnings.md` pairing each `### ` entry's `**Kind:**` (absent ⇒ `correction`) with its `**Escalated?**` value, then counting `kind=correction, escalated=no`. Result: **28 unescalated correction entries** against the `AGENTS.md:320` threshold of **≥3** — `/improve` overdue by **9.3×**. A second trigger on the same line is **also** met and is easy to miss: **3 unescalated `validation` entries** against its **≥2** threshold. Recurrence within the class this spec keeps re-learning (a derived count decaying in durable prose) is real and repeated across independent entries — `:326` (2026-07-20), `:398` (07-24), `:480` (07-25), `:516` and `:558` (07-28), plus further instances dated 07-31 — but the **exact tally depends on how the class is delimited**, so it is deliberately given as "repeated across independent entries" rather than as a number. Per its own lesson, the schema page carries none of this: `AC13b` (x) requires the **qualitative** form there. |
| **Re-check trigger — lives on the threshold, not in anyone's memory** | "Currently negligible" is a claim with a shelf life: `panic-index.md` / `unsafe-index.md` grow by mandate with the Rust codebase, so the statement is *guaranteed* to expire silently. The guard is therefore co-located with the figure it protects. **Threshold: 1% of the counted corpus** — the denominator being the post-exclusion total the pinned command itself produces at that commit, so the guard and the number it guards are always measured together. **The rule is mechanical, not advisory:** when an excluded-in-principle file crosses 1%, it **is** excluded at the next pinning of the command — not "should be considered for exclusion". The criterion does not change; only the derived membership does. **This figure is owned by the schema page and stated in the spec once, here** — `AC14` references it rather than restating it. |
| **Directionality — one-way by design, stated so it is not left to omission** | Crossing the threshold is a promotion that **does not reverse**: falling back below it does **not** restore a file to the count. What excludes a file is the **criterion** (volume driven by codebase or journaling), never its size; the threshold decides only *when acting on the criterion becomes material*. A file that grows past it, is excluded, and later shrinks **still satisfies the criterion**, so keeping it excluded is correct and is **not** an under-count. One-way also protects the series: a file oscillating around the threshold would otherwise flip membership repeatedly, injecting steps into the very trend the field exists to show. **The one case that would genuinely under-count** is a file that **stops satisfying the criterion** — repurposed, or its volume becoming instruction-driven. **v1 does not detect that, and accepts it:** all five current members are structurally journaling- or codebase-driven, and none is plausibly about to become instruction-driven. |
| Why 1%, and why a percentage rather than an absolute line count? | **Calibrated against measured shares** (denominator 9,091, the counted corpus): `learnings.md` would be **6.71%** — unambiguously material; the largest retained file, `library-survey.md`, is **0.605%**; then `dependency-versions.md` 0.550%, `panic-index.md` 0.154%, `unsafe-index.md` 0.077%. 1% ≈ 91 lines sits between the two groups: **1.65× above** the largest retained file and **6.71× below** `learnings.md`. That placement is deliberately asymmetric — nearer the retained files than the material one — because the two errors are not symmetric: excluding a file slightly too early costs at most ~1% of a number that is itself only a proxy, whereas retaining a distorting file silently misreports a trend, which is the exact failure this amendment exists to fix. Bias toward early exclusion. **A percentage, not an absolute count**, because an absolute guard **tightens silently as the denominator rises** — a fixed 91-line limit is 1.00% at a 9,091-line corpus but 0.57% at 16k and 0.40% at 23k, so it would eventually exclude files that are proportionally trivial. A percentage self-adjusts, and the harm being guarded against (distortion of a trend) is inherently proportional. |
| The derived list is a **measurement with a date** | It carries the same discipline as every other figure in this spec: **re-derive it whenever the pinned command is next re-run; never transcribe it forward.** The 2026-07-31 membership above is evidence of the criterion's current application, not a standing fact. |
| Why not the AXIOM set, which is immune to this by construction? | **Rejected on a disqualifying ground independent of the journaling question:** it excludes on-demand `ai-docs/` pages, so an `/improve` extraction out of `AGENTS.md` into `ai-docs/` would read as a **pure reduction** — the field would blind itself to precisely the event it exists to measure. |
| Why not just document the distortion, like the other `AC13b` gaps? | **Rejected for consistency with the `findings_first_seen` coupling clause.** The `AC13b` entries are predominantly *coverage gaps* — things the log does not see. This is a field that **actively misreports**: it would move for reasons unrelated to its name. A number that looks meaningful and is not is worse than an absent one, which is the same argument that decided the coupling clause; deciding it the other way here would be incoherent. |

## Technical constraints

Verified against the live tree on 2026-07-31.

1. **Progress-file ignore status:** `git check-ignore -v ai-docs/plans/foo.progress.md`
   → `.gitignore:11:/ai-docs/plans/**/*.progress.md`. The record must be a
   *derived* artefact in a non-ignored path; the source can never enter the commit
   (Step 12 sub-step 2 already asserts this).
2. **Progress-file shape** (`ai-docs/templates/progress-format.md` +
   `.claude/agents/self-review.md` § *Findings format*):
   - `self-review` appends `## Self-Review (Round N)` sections, never replacing
     earlier ones; each carries a `**Verdict:**` line and a
     `| # | File:line | Severity | Finding | Status |` table.
   - Status tokens written by `/task` Step 11: `⬜ Open`, `✅ Fixed`,
     `✅ Fixed (design amended)`, `✅ Fixed (spec amended)`,
     `⚠️ Objected: <reason>`. Note two of these carry a **parenthesised suffix**
     and one a **trailing free-text reason** — the parser must match on substring,
     not on whole-cell equality, and the same applies to the new
     `⬜ Open 🔁 Re-opened`.
   - `## Files touched`, `## AC Status`, `## Decisions log` are canonical sections.
3. **`instruction_corpus_lines` — pinned command.** The obvious git pathspec is
   **wrong**: git's default wildmatch lets `*` cross `/`, so
   `git ls-files -- 'ai-docs/*.md'` reaches far below the intended depth — it
   matches every `.md` under `ai-docs/plans/`, `ai-docs/plans/done/` and
   `ai-docs/templates/`, i.e. the entire archive of completed specs and designs,
   which has nothing to do with the instruction corpus. The `:(glob)` magic
   prefix restores pathname semantics. Stated as a **property rather than a line
   count**, because any count here decays with the archive and with the exclusion
   set: the two forms are separated by running
   `git ls-files -- '<form>' | grep -vc '^ai-docs/[^/]*\.md$'`, which returns a
   large positive number for the bare form and **0** for the `:(glob)` form. That
   check is re-runnable at any commit and needs no baseline to interpret. The pinned command is:

   ```bash
   git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
     ':(exclude)ai-docs/learnings.md' \
     | xargs -0 cat | wc -l
   ```

   The trailing `:(exclude)` implements the § *Key decisions* criterion at its
   **current derived membership** — it is not the definition, and refreshing it
   as the other four files grow does not reopen the decision. Spelling verified by
   running it: `:(exclude)`, `:(exclude,glob)`, and the short `:!` form all yield
   the identical 59-file set, and `grep -x 'ai-docs/learnings.md'` against the
   output returns **0** (the surviving `learnings` match is
   `.claude/agents/learnings-escalation-audit.md`, a genuine instruction file that
   correctly stays in).

   **Baseline: 9,091 lines over 59 files** (measured 2026-07-31 with the command
   above). **This supersedes the previously recorded 9,403 / 59 figure, and the
   two are NOT comparable** — the earlier number predates this exclusion and also
   predates this task's own additions; the unexcluded Broad set measures **9,701 /
   60** at the same commit, of which `learnings.md` is **610**. Any series must
   start from the post-exclusion baseline. Three properties verified: the
   `:(glob)` file set is byte-identical to the `find`-based set (so no untracked
   or ignored `.md` distorts it), the depth-1-only `ai-docs/*.md` behaviour still
   holds with the exclusion in place — re-probed with the depth check above, which
   returns **0** files below depth 1 for the pinned form and a large positive
   count for the bare one — and
   `:(glob).claude/**/*.md` matches at depth 1 as well as deeper (probe-tested),
   so a future `.claude/foo.md` is not silently dropped. The `xargs -0 cat | wc -l`
   form is used deliberately over `xargs wc -l | tail -1` — the latter emits
   multiple `total` lines if `xargs` splits the argument list.
4. **Script precedent:** `.claude/skills/pr-merged/scripts/cleanup-progress.sh`
   (documented header, `set -uo pipefail`, fail-soft) and
   `.claude/skills/ai-audit/scripts/check-citations.sh` +
   `test-check-citations.sh`. Both are the in-tree model for a harness-side script
   with a fixture test.
5. **Propagation Rule.** Editing `.claude/skills/task/SKILL.md` and
   `.claude/agents/self-review.md` triggers `AGENTS.md` § *Propagation Rule*: the
   Review sync group check (see *Propagation scope* above), the step-1 `grep -rni`
   sweep, and `ai-docs/claude-tools-hierarchy.md` if a Skill/Subagent contract
   changes — which it does, via the `🔁 Re-opened` marker.
6. **Instruction-file size cap.** The Step-12 addition and the schema doc must
   keep every loaded instruction file under the 35,000-char early warning; the
   schema lives in `ai-docs/` precisely so `SKILL.md` grows by a link, not a table.
   **Settled reading of AC16 (recorded so the implementer does not re-derive it):**
   the criterion is **delta-wise** — no file crosses 35,000 *as a result of this
   change*. `AGENTS.md` measures **38,874** chars today (verified `wc -c`), is not
   modified by this task, and is out of scope for AC16; a pre-existing overage is
   not this task's to fix.
7. **`git diff --shortstat` parsing — the naive form is wrong.** Verified on
   purpose-built commits: the output is ` N files changed, M insertions(+), K deletions(-)`,
   but a **deletions-only** diff omits the insertions clause entirely
   (` 1 file changed, 3 deletions(-)`), counts are **singularised** at 1
   (`1 file changed`, `1 insertion(+)`), and a pure rename yields
   ` 1 file changed, 0 insertions(+), 0 deletions(-)`. A positional parse breaks on
   the omitted clause. Extract each number **by its keyword**, defaulting a missing
   clause to `0`; an empty shortstat (no changes at all) yields all three as `0`.
8. **Line-shift key drift — the identity key's dominant failure mode.** The spec
   has no `## Risks` section, so the risk is recorded here; the design's risk
   table has no row for it and needs one. Mechanism, end to end: Step 11 applies
   fixes between rounds → any fix changing a file's line count shifts every line
   below it → `self-review` re-derives locations each round against the current
   tree (`.claude/agents/self-review.md:169`) → a carried-forward finding's
   `File:line` key changes → it re-counts as first-seen. Frequency is **common,
   not marginal**, and is *positively coupled* to the runs where the field
   matters (multi-finding files). Verified state of the already-committed schema
   page: `ai-docs/task-run-schema.md:85–86` states the key correctly
   ("the `File:line` cell, verbatim") and the following bullet gives the bias
   *direction* and an "upper bound" reading — but `grep -niE 'common
   case|almost always|frequen|expected in|edge case|rare'` over that file returns
   **nothing**, confirming no frequency language anywhere. It therefore needs an
   **addition, not a reversal**: the key statement stands; the frequency
   statement is missing.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `ai-docs/metrics/task-runs.jsonl` exists, is tracked (not gitignored), and every line parses as one JSON object (`jq -c . < file` succeeds on the whole file); the file ends with a newline. |
| AC2 | `.claude/skills/task/SKILL.md` Step 12 contains a sub-step that appends exactly one record for the current run, ordered **before** the "stage all changed files" sub-step, and that sub-step's file list names `ai-docs/metrics/task-runs.jsonl` so the append reaches the PR diff. |
| AC3 | *(amends issue AC3 — see § Carve-out.)* Every record field is derived from `ai-docs/plans/<spec-base>.progress.md` or from ambient facts available at Step 12. No Subagent gains a new file to write. The **only** permitted Subagent-contract change is the `🔁 Re-opened` status marker in `.claude/agents/self-review.md`; `git diff --stat` over `.claude/agents/` shows `self-review.md` as the sole changed agent file. |
| AC4 | `.claude/agents/self-review.md` defines `🔁 Re-opened` as an **additive** annotation producing `⬜ Open 🔁 Re-opened` — in § *Findings format* (vocabulary), § 7 (l.128–130), and the round>1 rules (l.175–176, the re-open **write** site). **Stated as a property, not a census:** in `git diff <base>..HEAD` over `.claude/`, no pre-existing `⬜ Open` *consumer* site is modified — the only file with `⬜ Open` changes is `self-review.md` itself. (The 11-consumer / 3-template enumeration in § *The marker is ADDITIVE* is the evidence for the additive design; it is deliberately **not** the acceptance test, so a later commit adding a consumer elsewhere cannot break this AC.) |
| AC5 | The Review sync-group check is performed and its outcome recorded in the PR body: `.claude/skills/project-review/SKILL.md` and `.claude/agents/review-findings.md` were read, and either need no change or were changed. Property to satisfy: neither file is modified to accommodate the marker, and `review-findings.md`'s creation-time template row **remains free of the marker** (a freshly created table cannot contain a re-opened finding). A silent skip fails this AC. |
| AC6 | `.claude/skills/task/scripts/append-task-run.sh` exists with a companion fixture test; the test covers, at minimum: a well-formed 3-round progress file, an absent file, a present-file-with-no-`## Self-Review`-sections, and a truncated/garbled findings table. |
| AC7 | Degradation: for the absent, no-sections, and garbled fixtures the script appends one valid JSON line carrying the fields it could fill plus `"incomplete": true`, and **exits 0**. |
| AC8 | Exit-code contract: the script exits non-zero **only** when it cannot append at all; the fixture test asserts exit 0 on every degraded-parse path and non-zero on at least one cannot-append path (e.g. unwritable target). |
| AC9 | Extraction correctness against a synthetic 3-round fixture (REJECT / REJECT / APPROVE, known per-severity counts, ≥1 `⚠️ Objected` row, ≥1 `⬜ Open 🔁 Re-opened` row): `rounds`, `hit_round_cap`, `verdicts`, `findings`, `findings_first_seen`, `objections`, and `objections_reopened` all match hand-counted expected values exactly. The fixture **must include a finding that persists across two consecutive rounds**, and the test must assert that finding is counted **once** in `findings_first_seen` while appearing **twice** in `findings` — the carry-forward case that motivated the field split. |
| AC9a | **The fixture must also reach the key-drift clause, not merely the stable-key one.** The AC9 carry-forward row proves de-duplication only under a *byte-identical* `File:line`; it never exercises the failure the identity key actually has. Add a second carried-forward finding that appears in round N+1 in the **same file with the same `Finding` text at a shifted line number**, and assert it **is** counted again in `findings_first_seen` (and so contributes twice). The assertion **records measured behaviour — it is not a bug to be fixed**, and a later reader must not "repair" the test by changing the key. This applies `.claude/agents/self-review.md` § *Patterns* 2 (a test must reach the clause it names) to our own fixture. **Required, not suggested:** the fixture/test carries an **inline comment** at that assertion stating the over-count is *expected under the current `File:line` key and is not a defect*, and pointing at the schema page. Without it, a reader six months out sees a wrong-looking expected value, "fixes" the parser, and silently changes what the corpus means **mid-series** — the failure this AC exists to prevent. An assertion without the comment fails this AC. |
| AC10 | Two-path drift constraint: the schema doc classifies every field `fallback-required` or `fallback-optional` (the `fallback-required` set includes `files_changed`, `insertions`, `deletions`), and carries a worked example fallback line whose key set is a **strict subset** of the script's key set and a **superset** of the `fallback-required` set. Both containments are asserted mechanically (a `jq` check in the fixture test), not by prose. |
| AC11 | `.claude/skills/pr-merged/scripts/cleanup-progress.sh` is unchanged by this PR (`git diff --stat <base>..HEAD` lists no change to it). |
| AC11a | Diff-size fields: `files_changed` / `insertions` / `deletions` are parsed **by keyword** from `git diff --shortstat`, with a missing clause defaulting to `0`. The fixture test covers the deletions-only shape (insertions clause absent), the singular shape (`1 file changed, 1 insertion(+)`), and the empty shortstat, per § *Technical constraints* 7. |
| AC12 | The schema — every field, its type, its source in the progress file, its fallback class, and the `incomplete` semantics — is documented on one `ai-docs/` page; `.claude/skills/task/SKILL.md` references it by relative link and does not restate the field table. The link resolves (`realpath`). |
| AC13 | The schema page states that the log is append-only with a single writer (`/task` Step 12), that hand-edits are forbidden, and why (one malformed line breaks any `jq` read of the whole file) — mirroring the `_inbox.jsonl` rationale. It also documents the 10-findings-per-round cap as a known truncation, and the last-line-wins rule for Step-12 re-entry duplicates. |
| AC13b | The schema page carries a compact section titled **"What this log does NOT measure"**, written as a list of **open questions with their current status — not as caveats**. The framing is the requirement, not a style preference: a caveat reads as a closed topic (acknowledged, therefore handled, therefore nobody's problem) and decays into skimmed boilerplate, whereas each entry ending in an **undecided** question keeps the uncovered axis a live agenda item. Every entry must end in a question that is explicitly open. At minimum ten: **(i)** *is the second axis worth measuring at all?* — process and handoff failures are orthogonal to every field here (a run where durable state was not maintained, or a handoff protocol was skipped, is **indistinguishable from a clean run**, since no field encodes handoff state); **undecided** whether this warrants instrumentation, a separate log, or nothing. **(ii)** *should the `## Decisions log` be parsed?* — unparsed in v1, so spec churn during implementation and reopened subtasks are invisible; note explicitly that this one is **not near-free**, because the event exists only as prose and would need a parser or new instrumentation; **undecided**. **(iii)** *should post-Step-12 rounds be captured?* — reviewer and CI-fix rounds land after the record is written and are outside it entirely (§ *Deferred* row 1); **undecided** (would need a second writer at `/pr-merged`). **(iv)** *should the 10-findings-per-round truncation be corrected or merely flagged?* — the cap silently truncates `findings` on high-finding rounds; **undecided**. **(v)** *does the harness need a durable surface that is **not** the repository?* — **(v), (vii) and (viii) are three faces of one property and the page must present them as such, opening with the consequence rather than the mechanisms: _a run that took three attempts at the fixture and a run that took one produce identical records._** That sentence tells an analyst which question the log cannot resolve; the mechanisms below explain only how the gap arose. **The property:** the harness **destroys working state at the boundary of the step that produced it, and versions only the product.** State plainly that this is **reasonable hygiene for human review** — it keeps diffs clean and stops transient state polluting history — a real trade with a real benefit, **not an oversight**; the cost falls entirely on the harness's ability to measure its own behaviour, a use case the design predates. **First face:** this log's **source**, `ai-docs/plans/<spec-base>.progress.md`, is gitignored (`.gitignore:11`) and therefore has **no history**, so any question about *when* a field was written or *by whom* (delegate vs. orchestrator backfill) is unanswerable once the session ends; the only reconstruction available is file mtime, which does not survive a copy, checkout, archive, or clone. The task persists **derived** telemetry from a source that is itself unauditable. **Undecided** whether that matters enough to change. State the trade honestly: the repo records a **classification, not a justification** — `.gitignore:10` reads `# Harness local-only state (see AGENTS.md § Agent Docs)`, and `AGENTS.md:269` likewise only labels the file "local-only (gitignored)". The pattern arrived in a single bulk harness-import commit (`9077bfb`, 2026-07-12, "adapted from quartzite"), so **no per-line rationale exists in this repo's history**, and a `(diff\|commit).{0,25}(noise\|churn)` sweep over `.gitignore`, `AGENTS.md`, `ai-docs/*.md`, and the task skill/agent files returns **nothing**. The plausible diff-noise argument (the file is rewritten at every step boundary, so versioning it would put that churn in every PR) is therefore a **conjecture, not the recorded reason** — the page must not present it as one. **The question is therefore not whether to version transient state in-repo** — that would sacrifice the very benefit that motivates destroying it — **but whether a separate append-only event journal _outside_ the repository is warranted.** **Undecided**, and out of scope here; do **not** propose implementing one. Recorded so whoever picks this up begins from that question rather than re-litigating the ignore rule. **(vi)** *should the record count **planning** rounds, not just review rounds?* — `rounds` counts `/task` Step 10 self-review rounds **only**; `/interview` spec rounds, `design` rounds, and `design-review` rounds are not represented anywhere in the record. Two runs with identical `rounds` may therefore have reached implementation at wildly different cost, and **a flat `rounds` trend after an `/improve` escalation is compatible with that escalation having doubled the design phase**. This is the sharpest of them, because it is **the log's own motivating question**: issue #186 exists to ask whether process density pays for itself, and a record that omits most of the process cannot answer it — a run can spend the large majority of its effort in spec and design and still emit a record whose only effort signal is a self-review count that never moved. **Undecided** whether to add per-phase round counts, a coarse planning-effort proxy, or nothing. (Per the section's own rule, state this **structurally** — do not pin a specific run's round counts or artefact line counts to the page; those belong in a PR body, where they are allowed to go stale.) **(vii)** *is the harness's working state systematically unrecoverable?* — **second face of the property stated in (v)** — subordinate to it, not a separate issue. Two artefacts recording how a run actually proceeded are destroyed by design: `.state.md` (interview round state) is *"Created at the start of round 1; deleted on terminal exit"* (`.claude/skills/interview/SKILL.md:56`), and `<spec-base>.progress.md` is gitignored and deleted after merge. Both hold exactly the process history this log tries to summarise, and neither survives the step that produced it — so spec round counts and handoff authorship are **unverifiable after the fact, not merely unrecorded**. Note the two reach that outcome by **different mechanisms** — the progress file is matched by an ignore rule (`.gitignore:11`), while `.state.md` is matched by **none** (`git check-ignore` exits 1) and is simply deleted outright, never tracked. That they converge without sharing a mechanism is what supports reading this as a **property of the harness** rather than one rule applied twice. This log is a partial, derived compensation for one of the two. **Undecided** whether the pattern warrants a general fix (persisting working state), a per-artefact one, or nothing. On rationale, the same finding as (v) holds: **no recorded rationale exists in this repo** for either deletion — both files arrived in the single bulk harness import (`9077bfb`), and the nearest statement, `interview/SKILL.md:94`, gives `.state.md`'s *scope* (it exists so an in-flight interview survives compaction and cold re-spawns) without arguing for destroying it at terminal exit. A scope note is not a justification. **(viii)** *is uncommitted work invisible to the record?* — **third face of the property stated in (v)** — likewise subordinate to it. This log is derived at Step 12 from committed history plus the progress file, so work that exists but has **not yet been committed** — a drafted fixture, a half-finished edit, an implementation later discarded and redone — appears nowhere: not in `git log`, not necessarily in the progress file, and not in the record. Where (v) and (vii) concern state **destroyed** after the fact, this concerns state that **never entered version control in the first place**; the shared consequence is that **the harness's true working state is recoverable only while the session is live**, and only by inspecting the working tree directly (`git status --porcelain`, `ls`) rather than by reading any durable surface or trusting any report. A Step-12-derived record therefore describes what *landed*, and is structurally blind to how much work existed, was discarded, or was redone before it landed — so it cannot be read as a measure of effort. **Undecided** whether effort that did not survive to a commit is worth representing at all, or whether committed history is simply the right unit. **(ix)** *do these fields report the sign backwards?* — **this entry is different in kind from (i)–(viii) and the page must present it as such: those are coverage gaps (the log does not see X); this one is a field the log DOES measure and reports inverted.** It is nearer in kind to the reason `findings` was split from `findings_first_seen` than to anything else in this list. The inversion: a **rigorous** self-review that finds real defects across three rounds emits `rounds: 3`, high `findings`, non-zero `objections`; a **perfunctory** one that APPROVEs on round 1 emits `rounds: 1`, `findings: {}`. On **every** field, the thorough run reads worse than the careless one. **The inversion is conditional, and the condition is the usable part:** it holds while the **number of defects in the diff is roughly fixed and review thoroughness varies**. When it is instead **input code quality** that varies, the sign is normal and the fields read correctly — better code genuinely produces fewer findings and fewer rounds. State the discriminator plainly: *these fields are readable when comparing runs of **comparable review thoroughness**, and unreadable when thoroughness itself differs between the runs being compared.* Without it an analyst either trusts the fields everywhere or distrusts them everywhere, and both are wrong. **Prohibition, stated directly because the misuse is the obvious next step: do NOT optimise against these fields.** Once `rounds` becomes a target, the cheapest way to improve it is to review less thoroughly — Goodhart's law, and more dangerous here than usual because the goal reads as virtuous: *"reducing review-cycle cost"* is exactly what a well-meaning reader would adopt this log for. **A downward `rounds` trend is not self-evidently an improvement and must never be adopted as an objective.** **Undecided** what, if anything, to do about it — a thoroughness covariate, a norm against target-setting, or nothing. **(x)** *what will cause this log to be read?* — **different in kind from both preceding groups, and the page must say so: (i)–(viii) concern what the record cannot say, (ix) concerns a field whose sign is inverted, and this concerns whether the record is consulted at all — which is upstream of every other question here.** A record that accumulates correctly and is never read costs the writing and returns nothing. The shape is **already instantiated in this repository**: `ai-docs/learnings.md` has a write trigger that is an AXIOM (`AGENTS.md:278` compels an entry on any instruction violation) and a read trigger that fires **only on explicit human invocation** (`/improve`). `task-runs.jsonl` is being given the same architecture — mandatory mechanised writes at `/task` Step 12, and no consumer at all, since § *Out of scope* defers every one of them. **State this qualitatively on the page** — *a comparable in-repo log has accumulated an order of magnitude past its escalation threshold without ever being read* — and **do not put the counts on the page**: they are command outputs that decay in one commit, and this section is the last place that should carry a rotting figure. The precise measurements and their reproducing command live in § *Key decisions* instead. **Undecided** whether this log needs a read trigger, a consumer, or a threshold of its own — but the failure mode is **observed in-repo, not hypothetical**. **The section must also preserve the standing warning in some form:** **a clean `rounds` trend is not evidence the surrounding process was sound, because the log cannot report otherwise.** **No fields are added for any of this** — `spec_amended_during_impl`, `subtasks_reopened`, and any handoff-compliance flag are explicitly **out of scope for v1**; the criterion is satisfied by prose on the schema page alone. |
| AC13a | **Counting units are stated explicitly on the schema page, per field, in words** — not left to be inferred from the extractor: `findings` and `objections` / `objections_reopened` are **summed across all rounds** and therefore inflate with `rounds` because rows carry forward (`self-review.md:177`); `findings_first_seen` counts **only rows absent from the immediately preceding round's table**. The page names the **`File:line` cell** as the cross-round identity key and states the moved-location failure mode with its **frequency, not only its direction**: line-shifting fixes make key drift the *expected* case whenever multiple findings share a file, so `findings_first_seen` collapses toward `findings` on exactly the runs the split exists to illuminate. The page must state plainly that a **low `findings_first_seen`/`findings` ratio is ambiguous** — it may mean "few repeat findings" **or** "many line-shifting fixes" — and that **the record cannot distinguish the two**. Direction-only wording ("upper bound", "biased upward") is insufficient on its own: it invites an analyst to treat the series as merely conservative when it is, on the informative runs, close to uninformative. **The page must also carry the degeneracy signature** (§ *Key decisions*, "Degeneracy signature") in substance: on a run with `rounds > 1`, `findings_first_seen == findings` means **no row matched between rounds**; the log **does not distinguish the cause** — either genuinely no repeat findings, or a `File:line` key drifted by a Step-11 line shift; a ratio below 1 is evidence the key held for at least one row. The signature must be stated at that strength and **no stronger** — claiming a ratio of 1 "almost certainly" indicates drift is false, since `self-review.md:173` permits a legitimate zero-carry-forward run. **Verification adds a fifth independent grep** to the four the design already defines for `AC13a` (`design.md:589`, split precisely so a missing clause cannot hide behind a sibling in an alternation): a standalone `grep` for the signature sentence against `ai-docs/task-run-schema.md`. A miss fails `AC13a` on its own. **Coupling clause (binds future edits, not just this PR):** `findings_first_seen` and its degeneracy signature **ship together or not at all**. If this signature requirement is ever unsatisfied — sentence absent from the schema page, or its grep failing — the correct response is to **remove `findings_first_seen` from the schema**, never to keep the field and drop the sentence. The field's presence in the record is conditional on a reader being able to identify the runs where it degenerated. |
| AC14 | `instruction_corpus_lines` is reproducible **and correctly scoped**: the schema page carries the pinned command verbatim **including its `:(exclude)` term**, notes why the non-`:(glob)` pathspec is wrong, and re-running it at the implementing commit yields the number recorded in that commit's own record. The page states the **exclusion criterion as a property** — files whose length reflects instruction content are counted; files whose volume is driven by the codebase or by journaling are excluded — and presents the file list as **derived from that criterion, not as its definition**, naming the four sub-threshold files that fall under it but are retained in v1 as negligible. The page must also record that the pre-exclusion figure is **not comparable** with post-exclusion values. A page naming `learnings.md` as the rule rather than as current derived membership fails this AC. The page must also carry the **re-check threshold and its mechanical promotion rule** as specified in § *Key decisions* (crossing it means the file **is** excluded at the next pinning, not that someone should consider it; and that crossings are **one-way** — dropping back below the threshold does not restore a file, because the criterion excludes it, not its size) — the threshold figure is owned by the schema page and appears **there** and in § *Key decisions* only; this criterion references it and does not restate it. |
| AC15 | `AGENTS.md` § *Propagation Rule* step-1 `grep -rni` sweep is run for every keyword introduced or changed (`Re-opened`, the metrics path, the script name), every hit is updated in the same PR, and `ai-docs/claude-tools-hierarchy.md` reflects the changed `self-review` / `/task` contracts. |
| AC16 | No loaded instruction file crosses 35,000 chars after the change (`wc -c` over the AXIOM's enumerated set). |

## Open questions

- **Step-12 re-entry duplicates.** Default is append-and-last-wins (Key
  decisions). The alternative — the script rewrites an existing line with the same
  (`spec_base`, `branch`) in place — gives a cleaner one-line-per-run invariant at
  the cost of a non-append write path. Design may override if it prefers the
  stricter invariant; AC1 and AC13 would need the wording adjusted with it.
- **Should the record carry the PR number?** Not known until Step 12 sub-step 10
  (`gh pr create`), which runs *after* the commit that must contain the record.
  Default: omit; `branch` + `issue` recover the PR.
- **Should Design-Amendment and nested-`/bugfix` rounds be marked?** Both are
  visible in the progress file's `## Decisions log`, but extracting them is
  parsing beyond the issue's field list. Default: not in v1; the `## Decisions
  log` is not parsed.
- **Retention / rotation.** One short line per merged task is negligible for the
  foreseeable term. Default: no rotation.
