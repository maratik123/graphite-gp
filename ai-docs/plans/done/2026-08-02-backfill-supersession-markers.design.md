# Design: Backfill supersession markers in `ai-docs/learnings.md`

**Issue:** none — free-text request; the spec places opening/closing issues out of scope
**Spec:** `ai-docs/plans/2026-08-02-backfill-supersession-markers.spec.md`
**Date:** 2026-08-02
**Branch:** `chore/2026-08-02-backfill-supersession-markers` `[measured: git branch --show-current → chore/2026-08-02-backfill-supersession-markers]`

> **Transcription rule for this document (round-2, narrowed in round 3).** Every **runnable gate
> body** lives in a fenced code block. Table cells carry only figure → value, ID → pass-condition,
> and **deliberately-quoted broken forms, each marked as broken**. A table cell cannot hold a raw
> `|`, so a piped or ERE-alternated command transcribed into one silently acquires escapes that
> change its meaning — under ERE, `\|` is a **literal pipe**, not alternation. Round 1 shipped
> exactly that defect at two load-bearing sites (§ *Corrections log*).
>
> **Do not paste a command out of a table cell.** Table rows still quote command text in **24**
> places (the rejected alternatives, the round-2/3 defect descriptions, and the gate
> pass-conditions), of which **3** re-quote the *broken* round-2 `'STANDS?\|stands'` form,
> retained as evidence and marked as broken in situ. The authoritative form of every gate is its
> fenced block: all of **G1–G12** have a fenced definition, so no gate body exists only in a cell
> `[measured: M33 below]`.

## Approach

### Shape of the work

This is **verification plus a mechanical prose edit to exactly one data file**. There is no
Rust code, no new abstraction, and no design question left open — the only genuine one
(PARTIAL VS TOTAL) is closed, and closed *by measurement*, not by a ruling (§ *KD-6 is a refuted
premise*). The design's whole job is therefore to make three things **executable** rather than
aspirational:

1. **Verification strictly precedes edit (AC1)** — enforced by read-only subtask gates and by
   commit topology, not by an instruction to be careful.
2. **The sweep is bounded by the property, not by the eight candidates (Scope 3, AC4)** —
   enforced by a per-record pass over every entry, a coverage gate over the ledger (**G12**), and
   a case-insensitive idiom cross-check whose disagreement with the read pass is itself a defect
   signal.
3. **Every number is re-derived by a per-record parse after the run's last edit (AC4, AC9)** —
   enforced by putting the measurement pass *after* the E2 append as its own subtask, and by
   naming the exact `awk` program each count comes from.

Item 3's standard binds this document's own gate bodies, which is why every one of them was
executed before being written down.

### KD-6 is a refuted premise, not a scope decision

Recorded in the shape the spec mandates. The source request rested on the assumption that
`**Superseded by:**` is a bare pointer with no room for partiality — so that a marker on a
*partial* withdrawal would tell a reader to drop a rule that still holds. **That assumption is
false and was refuted by measurement**, not traded away as scope:

- the `[one-line reason]` slot is documented as **freeform** — `ai-docs/corrections-log.md`
  § *Entry format — field glossary*: *"The `[one-line reason]` is freeform — a short note
  explaining the nature of the supersession (reversed / refined / generalized / subsumed /
  withdrawn)"* `[measured: M10 below]`;
- both live instances already spell out partiality in prose — the two field-carrying records are
  the headings at lines **295** and **461** `[measured: M2 below]`; `:300` reads *"**ONLY** this
  entry's closing clause … is withdrawn … The entry's primary rule … STANDS unchanged"* and
  `:466` reads *"**ONLY** this entry's *rationale* is withdrawn, not its rule … STANDS"*
  `[measured: M11 below]`.

Because the premise is refuted, **nothing is extended and nothing needs documenting anywhere** —
which is what makes AC6 unconditional **on the definition-document half** rather than a scoping
choice — no `AGENTS.md`, `.claude/**`, `ai-docs/corrections-log.md`, or
`ai-docs/templates/learnings-entry.md` edit is authorised, and A1 does not touch that half. For
the Step-9.5/12 carve-out that A1 *did* add, see § *Amendment A1*. A scope decision would invite
re-litigation; a refuted premise does not. Do not re-open it.

### KD-2 — author in-thread; `code-writer` is foreclosed

The diff is prose in one `ai-docs/**` file plus judgement about what rules mean.
`code-writer`'s charter is *"File-based **code-writing** implementor"* `[measured: M12 below]`,
so a prose-only diff has nothing to delegate to it (AGENTS.md § *Workflow*, delegation phase (1)
*Fit*). Verified precedent: the entry *"delegated ~30 instruction-file prose edits to
`code-writer`, whose charter is code only"* `[measured: M13 below]`.

Consequence for § *Handoff plan*: **every group is an instructions/harness group** (`ai-docs/**`
— the change-type is named verbatim in the grouping rule), routed to
`subagent_type="general-purpose"` with inline `model="opus"` and inherited effort. No group is
ever marked `sonnet` / `code-writer` in this design.

### Why the usual `cargo` gates are vacuous here, and what replaces them

**Zero `.rs`, `.toml`, `.yml`, or `.github/**` files change.** `cargo build`, `cargo test`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
`RUSTDOCFLAGS=… cargo doc`, `actionlint`, Miri, and `cargo bench` therefore **cannot be evidence
for any AC in this task** — a green run of any of them says only that the tree it was already
green on is unchanged. Listing them as verification would be exactly the "recorded result is a
claim" defect this task exists to repair. There is also no repo-side commit hook that would
inspect the file `[measured: M14 below]`.

`/context-reset`'s Handoff-protocol step 1 (`cargo build`) still runs — it is the skill's ritual
tree-integrity check `[measured: M15 below]` — but it MUST NOT be written to the progress file's
`**last_passed_gate:**` for any subtask of this task, because it discharges nothing about this
diff. `last_passed_gate` records a gate from § *Test Design* instead.

The real gate suite is **G1–G12** in § *Test Design*. Every one is a concrete command over `git`
or `awk`/`grep` on `ai-docs/learnings.md`.

### Rejected alternatives

| Alternative | Why rejected |
|---|---|
| Split verify and edit into **two** design-defined groups, using the group boundary as AC1's enforcement | Group-minimization ((f)) bounds groups by size cap / dependency / change-type only; a verify→edit ordering is an intra-group dependency, so two groups here would be an avoidable non-minimized count = `major`. AC1 is instead enforced by read-only subtask gates (G11) + commit topology, which is *stronger*: it names a command that fails, not a context boundary that might be crossed. |
| Extend `[ref]` to a comma-separated **two-date** form for the double-corrector case (C4+C5) | The documented `[ref]` grammar admits a date, a `PR #N`, or **both** comma-separated — a date+date pair is not in it `[measured: M10 below]`. Using it would extend the grammar, contradicting KD-6 and risking `learnings-escalation-audit`'s resolver. **Chosen instead:** one date+slug in `[ref]`, the second corrector cited *inside the freeform reason slot* in the same `YYYY-MM-DD ("slug")` shape. Extends nothing; both citations stay greppable and both get run through G6. |
| Write a helper shell script for the AC8 resolver | A new file in the repo breaks AC6 — a script path is **not** in A1's closed enumeration, so G1's union limb reds it whether it is tracked or untracked. (Pre-A1 this row read *"`git status --porcelain` must be clean of strays"*; post-A1 the test is membership in the allow-list, **not** emptiness — see § *Amendment A1*.) All checks must be **inline** one-shot `Bash` invocations. |
| Use `grep -c '^\*\*Superseded by:\*\*'` for the AC9 population count | Per-line field match — the documented origin of a wrong count already in this log (the entry *"CORRECTION: the entry below contains two wrong numbers, and it is the entry about wrong numbers"*). It would *currently* agree with the per-record parse only because G5 happens to hold; agreement guarded by another gate is not a measurement. Per-record `awk` only (**G7**). |
| Widen G1's allow-list to `ai-docs/plans/**` | Proposed in round-1 review to accommodate an `ai-docs/plans/archive/**` path. That path **does not exist** on this branch — no commits, no index entry, not on disk `[measured: M16, M17 below]` — so the accommodation was unnecessary. It also rested on reading spec AC6's *"the spec/design plan files"* as covering an archive subtree. **Still rejected, and still as a glob.** Amendment A1 later extended G1's allow-list, but by a **closed literal enumeration** of `/task`'s own mandated Step-9.5/12 artefacts — never a glob, and explicitly **not** `ai-docs/plans/**`. A1 leaves this rejection standing: the archive path reds under the amended gate too (negative control in G1's block). |

### Corrections log (round 2 and round 3 — what changed, and why)

| # | Defect | Fix |
|---|---|---|
| 1 | **G2 could not see a removed blank line.** `git diff -U0` renders a deleted empty line as a bare `-`, so `grep -c '^-[^-]'` returns **0** while a deletion really occurred. G2 was the only mechanical enforcement of AC7's "zero removals", and this edit shape is exactly the one that can drop a separator blank — the anchor `**Escalated?** no` is non-unique, so an `Edit` anchor tends to span into the following blank line. G3 is additions-only and does not cover it. | G2's body is now `git diff --numstat main -- ai-docs/learnings.md` with the **deletions column required to be 0**, and the raw `--numstat` line pasted verbatim into the report. Demonstrated end-to-end at **M18**. G3 is unchanged. |
| 2a | **G10's `grep -oE 'STANDS?\|stands'`** matched `STANDS?` + a literal pipe + `stands`; it exits 1 on real marker text, so AC3's "names the withdrawn clause AND states the remainder stands" passed **every** partial-withdrawal marker vacuously. | `grep -oiE 'stands?'`. Verified against real marker text at **M19**: broken form exit 1 / no output; fixed form prints `STANDS` on both `:300` and `:466`. |
| 2b | **The case-insensitive idiom command**, transcribed into a table cell with `\|` under `-icE`, returns **0**, not 62 — and it was the only written form of subtask 3's cross-check pass. An implementor pasting it gets zero hits and concludes the read pass missed nothing: the "sweep silently bounded" failure this design exists to prevent, re-introduced by the gate meant to prevent it. | Correct ERE alternation, in a fenced block (**M7b**). Both forms re-run there: escaped → `0`, correct → `62`. The transcription rule at the top now governs **runnable gate bodies**; quoted broken forms deliberately remain in cells, marked as broken (narrowed in round 3 — the round-2 claim of a document-wide fix was false, M33). |
| 3 | **G1's allow-list** — see § *Rejected alternatives*, last row. | Allow-list unchanged; branch-state rows refreshed to the verified current state (**M16**, **M17**). |
| 4a | **The records-with-field `awk` transcript** claimed output `300: …, 466: …`. That program prints the **heading** `NR`; actual output is `295: …, 461: …` (300/466 are the *field*-line numbers, carried over from the spec). | Re-run at **M2**; actual output pasted. Both the underlying fact (2 records) and the exemplar identities were independently correct. |
| 4b | **F-A cited `sed -n '751p'`** for the `~l.583` text. Line 751 is that entry's heading; the text is on line **752**. | Re-run at **M21**; F-A and G9 now say `:752`. |
| A | Subtask 4's entry precondition ("a `swept` row for all `N` records") was prose checked against a gitignored file — the one place the sweep's structural coverage was asserted rather than measured. | New gate **G12**: `grep -c` of `swept` ledger rows compared against `N`, run at subtask 4 entry **and** again at subtask 6. |
| B | **G8 runs after the E2 append and returns `N+1`**, while the sweep denominator is `N`; nothing said so, inviting the implementor to reach back to subtask 1's baseline for it. | G8's pass condition now states the sweep denominator is reported as **`G8 − 1`** (the E2 entry), derived in the same pass — never carried from subtask 1 and never from the spec. |
| **R3-1** | **An unexecuted number survived round 2, and the round-2 summary claimed it had been caught.** § *Marker construction* was corrected to **46** but § *Risks* R3 still read **39**, citing M24 — the very transcript that contradicts it. R3's premise for first-class-risk status *is* that count. The round-2 report asserted the catch; the artefact refuted it. | `39` → **46** at R3, and this row added so the catch is auditable in the document rather than only in a summary. Whole-document figure sweep re-run (**M34**). |
| **R3-2** | **G12 measured the wrong thing and could FALSE-PASS.** `grep -c 'swept'` counted the ledger's *header* row too, so complete coverage returned `N+1` (spurious FAIL, inviting a fudge) and a sweep covering `N−1` returned exactly `N` (**silent pass**). G12 is the sole mechanical control for R6 — the log's own documented failure mode — and it replaced a prose precondition precisely to measure coverage. | Ledger restructured to **one row per record** (`E1`…`EN`); G12 now counts **row IDs**, not the bare token, and adds a **gap detector** that prints the missing index. Demonstrated against a throwaway ledger in all three states — complete, short, interior gap (**M32**). |
| **R3-3** | **The transcription rule overstated itself** ("every command … never inside a table cell"; "fixed document-wide"). Table cells still carry command text in **24** places, **3** of them the *broken* `'STANDS?\|stands'` form. | Rule narrowed to **runnable gate bodies**, with an explicit do-not-paste warning, the counts measured, and the load-bearing claim restated as *every G1–G12 body has a fenced definition* (**M33**). "Fixed document-wide" struck from row 2b. **The first draft of M33 itself claimed 11 with a regex returning 0** — re-derived, and the miss recorded here rather than in a summary. |
| **R3-4** | **M7b's inference did not follow from the figure it cited.** *"62 — 4.1× M7a, so `-i` is mandatory"* attributed to case-folding a gap dominated by **pattern-set** differences (`\[\[` alone matches 22 lines; `the same-day entry` is absent from M7a's list). The conclusion is right; the evidence as stated did not reach it — and this design's own thesis is that a number belongs in an artefact only where it is an INPUT. | Split like-for-like: **`-i` delta = 52 → 62 (1.19×)**; **pattern-set delta = 15 → 52**. The `-i` mandate now cites the **52**. |
| **R3-5** | **F-C cited a measurement of something else.** Its load-bearing safety claim (the resolver is indifferent to ref punctuation) was tagged `[measured: M22 …]`, but M22 measures **F-B**. | New **M31** reads the resolver line itself; F-C retagged. F-C also now states the **AC2 source conflict** and its tie-break explicitly. |
| **A1** | **AC6 AMENDED by owner ruling, mid-Step-9.5 — a CHANGE, not a clarification.** AC6 as written was unconditional and **did** cover `/task`'s own mandated Step-9.5/12 artefacts (`INDEX.md`, the `done/` move, `task-runs.jsonl`, `_inbox.jsonl`), so the workflow could not complete without breaching it. **The framing "AC6 always meant the content deliverable" is explicitly rejected** — a reinterpretation would erase the record of the change, the very failure this task exists to repair. | New § *Amendment A1* records the change, its ground, and the closed literal enumeration. **Only G1 changes**; no other gate, no KD reopened. G1 additionally pairs `git diff --name-only main` with `git status --porcelain --untracked-files=all`, because the former is **blind to untracked files** and both plan documents are untracked — AC6's own command could not evidence its own criterion (**M38**). The round-1 Issue-3 prohibition **stands**: `ai-docs/plans/archive/**` still reds (negative control). The three content docs stay **outside** the carve-out, each cleared by a recorded per-file vacuity check, re-run here rather than accepted from the relay (**M39**). AC6 now reports **"AMENDED, and the amendment is recorded"**, never PASS. |
| **A1-b** | **Stale pre-A1 prose left standing beside the claim that displaces it, with no link between them — this task's OWN defect class, recurring inside the document that specifies the repair.** Four sites stated an AC6 pass condition A1 had already changed: § *Measured baseline*'s callout (*"only becomes meaningful once … committed"*, *"empty entirely post-commit"*, and a bare `git status --porcelain`); § *Approach*'s *"makes AC6 unconditional"*; and — **not named in review, found by sweeping the class** — the AC8-helper-script rejected-alternative row and **R9**, both of which tested *emptiness of the porcelain* rather than membership in the allow-list. Post-A1 the porcelain is **legitimately non-empty** at Step 12, so all four invited a **spurious FAIL** — the shape R3-2 warns *"invites the implementor to fudge it"*. | Each retagged rather than silently rewritten, so the pre-A1 text and its supersession stay linked (the marker discipline this task backfills, applied to itself). The callout keeps what still holds, then states what it said before and why that is now wrong. *"Unconditional"* narrowed to **the definition-document half**, which A1 does not touch, with a pointer to A1 for the carve-out. The two unnamed sites restated as **membership in A1's closed enumeration, not emptiness**. No gate body changed; the fenced G1 body was already correct and executes correctly. Also recorded: G1's new union limb keeps a rename's destination and drops its source, so residual risk is **nil** here only because Step 12's `done/` move has both sides enumerated. |
| **R4-1** | **A dangling measurement citation, in the row written to prevent exactly that.** R3-1 cited `(**M34**)` as the evidence its catch was "auditable in the document", but no `M34` transcript existed — the row did the thing it was created to prevent. | **M34** added: the whole-document figure sweep (bolded figures + `# →` results), **plus a companion dangling-citation check** that walks every `M<n>` reference and fails on any with no body. That check is what would have caught this, so it now lives in the document rather than in a review. |
| **R4-2** | **G10 was still vacuous sub-word.** `grep -oiE 'stands?'` matches inside `standard` / `understand` / `outstanding`, so a marker with **no** stands-clause could pass. Weaker than the round-2 `'STANDS?\|stands'` defect (which failed on *all* real text) but the same family. | `grep -oiE '\bstands?\b'`. Verified at **M35**: rejects `standard` and `understand`/`outstanding` (exit 1), still matches both live exemplars — no true positives lost. |
| **R4-3** | **G12 proved row *existence*, not record *examination*.** It counted `E<n>` ids and gap-checked them, never inspecting cell content — so an `E1..EN` skeleton generated mechanically from the file's own headings (the natural way to guarantee contiguity) passed with nothing read. R6 re-entered one level down. | **Limb 2** added: every E-row's `kind` cell ∈ {`correction`, `validation`}, never `—`/blank. Determinable only by opening the record, since `Kind:` **defaults** to `correction` when absent. Demonstrated pass + fail at **M36**. Subtask 1 is now explicitly forbidden from pre-generating the skeleton. |
| **R4-4** | **Sequencing ambiguity over who materialises the ledger rows** — subtask 1 "opened" the ledger, subtask 3 filled `E1..EN`, and subtask 2 wrote verdicts in between onto rows of unstated origin. Interacts with R4-3: if subtask 1 built the skeleton, G12 at subtask-3 exit would measure nothing new. | Named explicitly: **subtask 1** creates heading + header row, **zero** data rows; **subtask 2** writes to a separate `## Candidate verdicts (C1–C8)` list keyed by heading text; **subtask 3** materialises each `E<n>` row as it reads the record and merges subtask 2's verdicts in. G12 at subtask-3 exit therefore checks **completion**; at subtask-4 entry it is a **drift re-check**. |
| **R4-5** | **An untagged claim about the spec** — *"not the 3 the spec once estimated"*. The spec never estimates the `Kind: validation` class size; its only estimate is "~8 **markers**", a different quantity. (Originated in the relayed addendum, not in review.) | Restated as *"not the 3 validation-kind **priors** the spec's tables happen to enumerate"*, with both halves measured at **M37** — the 3 priors (C7, C8, off-list `:606`) and the "~8 markers" line — and re-pointed as evidence for R6 that the candidate list is a floor. |
| **R3-6** | **§ *Scenarios* pre-approved both outcomes for C8, so the set could not discriminate** — the happy path listed C8 as receiving a marker with no reading of `:582`'s `Rule:`, while the FAIL bullet pre-approved the opposite. The FAIL bullet also legitimised FAIL unconditionally, ignoring that a `Kind: validation` prior entry loses its *evidentiary* status without its `Rule:` text becoming false. | C8 **struck from the happy-path enumeration** (its placement is the run's verdict to make). FAIL bullet **split by `Kind:`** — `correction` → settled; `validation` → **Technical-constraint-4 REPORT case**, recording the FAIL, the quoted `Rule:` fragment, **and** the open question about the test's second limb. Class size measured: **18** records (M5). |

### Amendment A1 — 2026-08-02 — AC6 gains a closed, enumerated Step-9.5/12 carve-out

**Owner ruling, taken mid-Step-9.5, after Step 8 completed. This is a CHANGE to AC6, not a
clarification of it.** The authoritative record is the spec's `## Amendments` § A1; this section
records what the *design* must do differently. **Only G1 changes. No other gate, and no Key
Decision (KD-2…KD-6), is reopened.**

> **Explicitly rejected framing.** Do **not** write, here or anywhere, that "AC6 always meant the
> content deliverable", or any variant. AC6 as previously written was **unconditional** and **did**
> cover these paths. Recasting the change as a reinterpretation would erase the record of the
> change — which is the same failure this whole task exists to repair: *a later claim quietly
> displacing an earlier one with no durable link between them.*

**State at the time of the amendment** — re-derived here, not carried from the ruling
`[measured: M38 below]`: 2 commits (`45080a7` markers, `a048890` E2), `git diff --numstat main`
→ `16	0 ai-docs/learnings.md` (**zero deletions**), **10** markers written, **1** new entry.

**The ground for AC6's unconditional form STANDS UNCHANGED.** AC6 was hardened to refuse exactly
one move: **widening a gate's allow-list to accommodate an artefact that should not exist**
(`design-review` round 1, Issue 3 — the dropped `ai-docs/plans/archive/**` commit). A1 relaxes
none of that. What A1 adds is `/task`'s *own mandated* Step-9.5/12 artefacts — files the workflow
**requires** the run to write, as against files the run merely wishes it could. The negative
control in G1's block shows `ai-docs/plans/archive/x.md` still reds under the amended gate.

**The carve-out is CLOSED and EXHAUSTIVE — these literal paths and no others:**

1. `ai-docs/plans/INDEX.md` — the plan row.
2. This task's spec + design at their `ai-docs/plans/done/` paths (the Step-12 move) — those two
   files only.
3. `ai-docs/metrics/task-runs.jsonl` — the Step-12 telemetry append.
4. `ai-docs/deferred/_inbox.jsonl` — **REQUIRED, not conditional.** Two deferred items ride on it:
   the spec's **Q2** and this design's **F-A follow-up** (§ *Open questions*). Both are deferred
   post-merge, and their carrier documents move to `done/` in the same step — so skipping the
   inbox **loses both**. Written via `/task` Step 12's writer only; per the AGENTS.md AXIOM this
   file is never hand-edited.

> **No glob.** Do not write `ai-docs/plans/**` for items 1–2. **The enumeration IS the control** —
> a glob would re-admit precisely the artefact class the standing prohibition refuses.

**The three Step-9.5 content docs are NOT in the carve-out.** `README.md`, `ai-docs/context.md`,
`ai-docs/context-status.md` are skippable **only** on a measured, recorded, per-file check that
*this doc's own stated content is not made stale by a 10-marker + 1-entry change to
`ai-docs/learnings.md`*. **Skipping them as a class, on the reasoning that the task implements no
crate, is forbidden** — that reasoning is about the *task*; the check is about the *docs*. All
three checks were run and **independently re-run here rather than accepted from the relay**
`[measured: M39 below]`:

| Doc | Measured | Verdict |
|---|---|---|
| `README.md` | **0** matches for `learnings\|superseded\|corrections-log`; **0** entry/marker count claims | not made stale |
| `ai-docs/context.md` | **1** hit (`:38`) — `gp-gen`'s background worker discarding *superseded results*; no `learnings.md` reference, no count claim | not made stale |
| `ai-docs/context-status.md` | **4** hits, all unrelated — `:127` #42's lib target, `:157` the same gen worker, `:236` a bench decision superseded by its own next entry, `:267` a past PR's `learnings.md` commit **structure** (not a count) | not made stale |

**AC6's reporting status changes.** It reports as **"AMENDED, and the amendment is recorded"** —
**never PASS**. This mirrors AC5's inversion path: an AC that *changed* must report that it
changed, so a reader cannot mistake a moved goalpost for a cleared one. Anywhere this design
enumerates AC outcomes, AC6 carries that status.

**A blind spot in G1 that AC6 depended on** (surfaced by `spec-writer`, re-verified here).
`git diff --name-only main` **does not see untracked files**, and both plan documents are
untracked right now — so AC6's *"lists … the spec/design plan files"* half was **unevidenced by
its own command**: the command returns `ai-docs/learnings.md` alone `[measured: M38 below]`. It
would self-resolve at Step 12 once the documents are staged into `done/`, but G1 must not depend
on that. G1 is now paired with `git status --porcelain --untracked-files=all`, so the gate is
sound at **any** point in the run. Category-matched command per AGENTS.md § *Dependency Versions*:
a tool blind to the category cannot answer a question about it.

```bash
# --- M38 --- A1 state + the G1 untracked blind spot
git log --oneline main..HEAD
# → a048890 chore(learnings): repair the final entry's stale supporting fact with a new entry
# → 45080a7 chore(learnings): backfill 10 supersession markers, verdicts from the field-scope test
git diff --numstat main
# → 16	0	ai-docs/learnings.md            (zero deletions -- G2 clean)
git diff main -- ai-docs/learnings.md | grep -c '^+\*\*Superseded by:\*\*'    # → 10
git diff main -- ai-docs/learnings.md | grep -c '^+### '                      # → 1
#
# THE BLIND SPOT, side by side:
git diff --name-only main
# → ai-docs/learnings.md                      <-- the two plan docs are ABSENT
git status --porcelain --untracked-files=all
# → ?? ai-docs/plans/2026-08-02-backfill-supersession-markers.design.md
# → ?? ai-docs/plans/2026-08-02-backfill-supersession-markers.spec.md
git ls-files ai-docs/plans/2026-08-02-backfill-supersession-markers.spec.md \
             ai-docs/plans/2026-08-02-backfill-supersession-markers.design.md
# → (empty)                                   <-- neither is tracked, so --name-only cannot see them

# --- M39 --- the three per-file vacuity checks, re-run here (not accepted from the relay)
for f in README.md ai-docs/context.md ai-docs/context-status.md; do
  echo "=== $f ==="
  grep -cniE 'learnings|superseded|corrections-log' "$f"
  grep -niE  'learnings|superseded|corrections-log' "$f"
  grep -coE  '[0-9]+ (entries|markers|corrections)' "$f"
done
# → README.md              hits 0   count-claims 0
# → ai-docs/context.md     hits 1   count-claims 0   (:38 "superseded results discarded" -- gp-gen worker)
# → ai-docs/context-status.md hits 4 count-claims 0  (:127 #42 lib target, :157 gen worker,
#                                                     :236 bench decision, :267 commit structure)
# None asserts a learnings.md entry or marker count, so none is made stale by this diff.
```

### Measured baseline (re-derive in the run; do NOT carry these forward)

| ID | Figure | Value |
|---|---|---|
| M1 | Entries | **123** |
| M2 | Records carrying `**Superseded by:**` | **2** — headings at lines **295** and **461** |
| M3 | Records lacking an `**Escalated?**` line | **0** — the placement anchor is **total** |
| M4 | Records with **two** `**Escalated?**` lines | **0** — the anchor is also unambiguous |
| M5 | `Kind: validation` entries | **18** |
| M6 | Dates carrying only ONE entry (so KD-4 disambiguation is unavailable) | **1** — `2026-07-13`; **no candidate uses it** |
| M7a | Idiom hits — **the spec's smaller pattern set**, case-sensitive, BRE | **15** |
| M7c | Idiom hits — **M7b's pattern set**, case-**sensitive** (the like-for-like control) | **52** |
| M7b | Idiom hits — **M7b's pattern set**, case-**in**sensitive, correct ERE | **62**. The two deltas are separate: **pattern set 15 → 52**, **`-i` 52 → 62 (1.19×)**. The `-i` mandate rests on the **52 → 62** step; the `\|`-escaped form returns **0**. |
| M8 | File tail | ends `**Escalated?** no\n` — **no trailing blank line**, so the E2 append must begin with one |
| M9 | Progress file is gitignored (so it cannot trip AC6) | yes |
| M16 | Branch commits ahead of `main` | **0** — `git log --oneline main..HEAD` empty; `git diff --name-only main` empty |
| M17 | Working tree | only the two untracked plan files (`.spec.md`, `.design.md`); `ai-docs/plans/archive/` absent from disk **and** index |

```bash
# --- M1 ---------------------------------------------------------------------
awk '/^### /{n++} END{print n}' ai-docs/learnings.md
# → 123

# --- M2 --- per-record parse; prints the HEADING line number, not the field's
awk '/^### /{h=$0;n=NR;has=0} /^\*\*Superseded by:\*\*/{if(!has){has=1;c++;print n": "h}} END{print "records-with-field: "c}' ai-docs/learnings.md
# → 295: ### 2026-07-19 — testing — a new test used exact float equality on a `sqrt`-derived value, passing `cargo test` but reddening the workspace Miri gate
# → 461: ### 2026-07-25 — process — ran `self-review` on `/reflect` output; the owner ruled it unnecessary
# → records-with-field: 2

# --- M3 ---------------------------------------------------------------------
awk '/^### /{if(h!=""&&e==0)print "NO-ESCALATED: "n": "h; h=$0;n=NR;e=0} /^\*\*Escalated\?\*\*/{e++} END{if(h!=""&&e==0)print "NO-ESCALATED: "n": "h; print "done"}' ai-docs/learnings.md
# → done

# --- M4 ---------------------------------------------------------------------
awk '/^### /{h=$0;n=NR;e=0} /^\*\*Escalated\?\*\*/{e++; if(e>1) print "MULTI: "n": "h} END{print "checked"}' ai-docs/learnings.md
# → checked

# --- M5 ---------------------------------------------------------------------
awk '/^### /{h=$0;n=NR} /^\*\*Kind:\*\* *validation/{c++} END{print c}' ai-docs/learnings.md
# → 18

# --- M6 ---------------------------------------------------------------------
grep -o '^### [0-9-]*' ai-docs/learnings.md | sort | uniq -c | sort -rn
# → 22 ### 2026-07-31 / 17 ### 2026-07-17 / 16 ### 2026-07-16 / … / 1 ### 2026-07-13

# --- M7a --- case-sensitive, BRE alternation (\| IS alternation in BRE, so this one is correct)
grep -c 'the entry below\|the entry above\|the preceding entry\|SUPERSEDES\|Supersedes\|CORRECTION\|is WRONG\|is REFUTED\|Retroactive correction\|Extends the\|Corroborates\|Companion to' ai-docs/learnings.md
# → 15

# --- M7c --- LIKE-FOR-LIKE CONTROL: M7b's pattern set, case-SENSITIVE (-cE, no -i)
grep -viE '^\*\*Kind:\*\*' ai-docs/learnings.md | grep -cE 'the entry (below|above)|the preceding entry|the same-day entry|supersedes|correction|is WRONG|is REFUTED|retroactive correction|extends the|corroborates|companion to|\[\['
# → 52
#   So: pattern set 15 → 52, and -i 52 → 62 (1.19x).  The 4.1x claimed in round 2 conflated the
#   two; `\[\[` ALONE matches 22 lines and `the same-day entry` is absent from M7a's list:
#     grep -viE '^\*\*Kind:\*\*' ai-docs/learnings.md | grep -cF '[['   → 22

# --- M7b --- case-INsensitive, ERE alternation.  USE THIS ONE (subtask 3 cross-check).
grep -viE '^\*\*Kind:\*\*' ai-docs/learnings.md | grep -icE 'the entry (below|above)|the preceding entry|the same-day entry|supersedes|correction|is WRONG|is REFUTED|retroactive correction|extends the|corroborates|companion to|\[\['
# → 62        (the -i mandate rests on 52 → 62, NOT on 15 → 62)
#
# The SAME command with \|-escaped alternation under -icE — the round-1 defect:
#   grep -viE '^\*\*Kind:\*\*' ai-docs/learnings.md | grep -icE 'the entry (below\|above)\|the preceding entry\|supersedes\|correction\|is WRONG\|is REFUTED'
# → 0        (\| is a LITERAL PIPE under ERE — a silent zero, not an error)

# --- M8 ---------------------------------------------------------------------
tail -c 24 ai-docs/learnings.md | od -c
# → 0000000   c   t   i   o   n  \n   *   *   E   s   c   a   l   a   t   e
# → 0000020   d   ?   *   *       n   o  \n

# --- M9 ---------------------------------------------------------------------
git check-ignore -v ai-docs/plans/2026-08-02-x.progress.md
# → .gitignore:11:/ai-docs/plans/**/*.progress.md	ai-docs/plans/2026-08-02-x.progress.md

# --- M10 --- the ref grammar + the freeform reason slot (one line of the glossary)
sed -n '49p' ai-docs/corrections-log.md
# → "`Superseded by:` records that the rule recorded above was later reversed, refined,
#    generalized, subsumed, or withdrawn. … `[ref]` is one of: a `YYYY-MM-DD` date matching a
#    later learnings entry (when multiple entries share that date, disambiguate by appending a
#    quoted slug from the other entry's description …); a `PR #N` reference …; or both,
#    comma-separated. The `[one-line reason]` is freeform …"

# --- M11 --- the two exemplar marker texts, read in full
sed -n '300p;466p' ai-docs/learnings.md

# --- M12 --- code-writer's charter
sed -n '1,8p' .claude/agents/code-writer.md
# → description: "File-based code-writing implementor. Pinned model: sonnet, effort: medium. …"

# --- M13 --- the delegation precedent
sed -n '193p' ai-docs/learnings.md
# → ### 2026-07-17 — process — delegated ~30 instruction-file prose edits to `code-writer`, whose charter is code only

# --- M14 --- no repo-side commit hook inspects the file
ls -a .git/hooks/ | grep -v sample
# → .    ..                 (nothing but the two directory entries)

# --- M15 --- /context-reset's step 1
sed -n '38p' .claude/skills/context-reset/SKILL.md
# → 1. `cargo build` — ensure code compiles

# --- M16 --- branch commits ahead of main (see also M17, same block)
# --- M17 --- working-tree state / absence of ai-docs/plans/archive
# branch state, verified at round 2
git log --oneline main..HEAD          # → (empty)
git diff --name-only main             # → (empty)
git status --porcelain
# → ?? ai-docs/plans/2026-08-02-backfill-supersession-markers.design.md
# → ?? ai-docs/plans/2026-08-02-backfill-supersession-markers.spec.md
ls -d ai-docs/plans/archive; git ls-files ai-docs/plans/archive
# → ls: cannot access 'ai-docs/plans/archive': No such file or directory
# → (git ls-files: empty)
```

> **AC6 consequence of M16/M17 — SUPERSEDED IN PART by § *Amendment A1*; read that for the live
> pass condition.** What still holds: `git diff --name-only main` is *blind to untracked files*,
> so its emptiness alone is never evidence of scope compliance, and **G1 therefore pairs it with
> `git status --porcelain --untracked-files=all`**.
>
> What this callout said before A1, and is now **wrong**: that AC6's check *"only becomes
> meaningful once the plan files are committed"*, and that the porcelain must be *"empty entirely
> post-commit"*. A1 makes the gate **sound at any point in the run** by testing the **union** of
> the tracked diff and the untracked set against a closed literal allow-list — so the porcelain is
> *legitimately* non-empty at Step 12 (`INDEX.md`, the `done/` move, `_inbox.jsonl`,
> `task-runs.jsonl`), and an emptiness test there would raise a **spurious FAIL**. That is the
> shape R3-2 warns *"invites the implementor to fudge it"*. The fenced G1 body is authoritative;
> this paragraph is retained only as the pre-A1 record. (The progress file is gitignored, M9, so
> it never enters either half.)

### E2 — both halves re-verified

| Half | Result |
|---|---|
| `656ea79` added exactly the `:751` entry | 1 added heading, and it is the `:751` one `[measured: M20]` |
| The **same commit** added the AC6 derived-count gate the entry says does not exist | `test-append-task-run.sh` `+132`; the file now derives `C` from the design and asserts on it, and the hard-coded banner survives only inside a comment describing the old form `[measured: M20]` |

```bash
# --- M20 --------------------------------------------------------------------
git show 656ea79 -- ai-docs/learnings.md | grep -c '^+### '
# → 1
git show 656ea79 -- ai-docs/learnings.md | grep '^+### '
# → +### 2026-08-01 — process — a `validation` entry certified a gate that does not exist; both "firings" were manual derivation
git show 656ea79 --stat --oneline | grep test-append
# →  .../skills/task/scripts/test-append-task-run.sh    | 132 ++++++++++++++++++++-
grep -n 'cases exercised == design' .claude/skills/task/scripts/test-append-task-run.sh
# → 871:  assert_eq "AC6: cases exercised == design § Cases rows" "$cases_run" "${C:-0}"
grep -n 'PASS: all' .claude/skills/task/scripts/test-append-task-run.sh
# → 839:# `echo "PASS: all 18 cases green."` -- a hand-typed string that agreed with   (a COMMENT about the old form)
# → 879:echo "PASS: all ${cases_run} cases green (count derived from ${design} § Cases)."
```

### Three findings this design adds to the spec's report list

**F-A — the run's own edits invalidate in-log absolute line references, and those lines are
immutable.** Each inserted marker shifts every subsequent line by one. The log contains **two**
self-references by line number: `` `learnings.md:75` `` inside line **92**, and
`(this file, ~l.583)` inside line **752** — *not* 751, which is that entry's heading
`[measured: M21 below]`. Both sit inside `**What happened:**` text that Boundary rule 1 makes
immutable, so there is no in-scope repair. Mitigations that ARE in scope: (i) the run records the
*content* of baseline lines 75 and 583 in the ledger at subtask 1 and reports where that content
lands after the last edit (**G9**); (ii) **no marker and no part of the E2 entry may cite anything
by line number** — every citation uses the KD-4 `YYYY-MM-DD ("slug")` form, so the run adds no new
member to this defect class. Report as a finding; do not attempt a fix.

**F-B — `:751`'s own `Rule:` text will read as a blocker mid-run, and is not one.** It says
*"its `Superseded by:` field is for `self-improve` / `learnings-escalation-audit` to set, not for
this turn"* `[measured: M22 below]`. An implementor reaching C8 will read that as forbidding the
very edit it is assigned. It does not: KD-1's authorising path is the **second** one named in the
same sentence of `ai-docs/corrections-log.md` — *"invoke `/ai-audit` or **explicitly request the
change**"* `[measured: M22 below]` — and this task **is** that explicit request. Proceed; record
the authorisation basis in the PR body per KD-1. Do NOT stop and re-ask.

**F-C — the two live exemplars do NOT use the glossary's parenthesised ref form.** The glossary's
worked example is `` `2026-05-08 ("mutually exclusive markers")` `` — date, then the slug **in
parentheses**. Both live instances instead write `2026-07-25 "slug"` with **no parentheses**
`[measured: M30 below]`. Spec KD-4 is explicit that every marker MUST use *the glossary's*
documented disambiguation form, so **the run follows the glossary (parenthesised) and does not
copy the exemplars' punctuation** — the exemplars are format authority for *placement* and for the
partial-withdrawal *prose shape* (which is what § *KD-6* and § *Marker construction* cite them
for), not for ref punctuation. This is safe for AC8: the resolver rule is *date match* **and**
*slug-text containment*, and nothing in it inspects the punctuation around the slug
`[measured: M31 below — the resolver line itself]`. Report the divergence as an observation;
**do not** edit either exemplar to match (Boundary rule 1, and the spec's C2/C3 rows say "No
edit").

> **AC2's two sources conflict here, and this is the tie-break.** AC2 requires the format to be
> the one derived from *the two live instances* **and** *the five definition sites* — but on ref
> punctuation those two sources **disagree**. Tie-break, applied consistently wherever this design
> cites either source: **the exemplars are authority for placement and for the partial-withdrawal
> prose shape; the glossary is authority for ref punctuation.** KD-4 already points at the
> glossary by name, so this reconciles AC2 with KD-4 rather than overriding either — **no Spec
> Amendment is required**. It is written down here so a future reviewer does not re-derive it,
> and so the run does not read the conflict as a blocker.

```bash
# --- M21 --- F-A, both self-references, with the round-1 off-by-one corrected
sed -n '92p' ai-docs/learnings.md | grep -oE '.{40}learnings\.md:75.{0,10}'
# → aracter fix" is not a gate-free fix]], `learnings.md:75`) attribu
sed -n '751p' ai-docs/learnings.md | grep -c 'this file, ~l\.583'
# → 0            (751 is the HEADING line — the round-1 transcript was off by one)
sed -n '752p' ai-docs/learnings.md | grep -oE '.{0,20}this file, ~l\.583.{0,10}'
# → rived-count gate…` (this file, ~l.583) records

# --- M22 --- F-B, both halves
sed -n '753p' ai-docs/learnings.md | grep -c 'field is for `self-improve` / `learnings-escalation-audit` to set, not for this turn'
# → 1
grep -c 'invoke `/ai-audit` or explicitly request the change' ai-docs/corrections-log.md
# → 1
```

## Decomposition

All six subtasks touch `ai-docs/learnings.md` (read-only for 1–3, write for 4–5) and the
gitignored `ai-docs/plans/2026-08-02-backfill-supersession-markers.progress.md`. No other path is
in scope for **subtasks 1–6**.

> **Scope of the six subtasks vs. Step 9.5/12 (A1).** AC6 was unconditional when these subtasks
> were written and is now **AMENDED** (§ *Amendment A1*): a **closed literal enumeration** of
> `/task`'s own mandated Step-9.5/12 artefacts may additionally enter the diff at those steps —
> `ai-docs/plans/INDEX.md`, the spec/design at their `done/` paths, `ai-docs/metrics/task-runs.jsonl`,
> and `ai-docs/deferred/_inbox.jsonl` (**required**, since the spec's Q2 and this design's F-A
> follow-up would otherwise be lost when their carrier documents move to `done/`). **No subtask
> 1–6 writes any of them**, and nothing outside that enumeration is admissible at any step: it
> reds AC6 and the run **stops and surfaces**. AC6 reports as **"AMENDED, and the amendment is
> recorded"**, never PASS.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Baseline + anchors (READ-ONLY).** Re-derive M1–M9 by the fenced programs above (per-record `awk`, never a per-line field match). Resolve **every** candidate (C1–C8) and the known off-list pair to their *current* line numbers **by heading text**, never by the spec's stale numbers. Record baseline content of lines 75 and 583 verbatim (F-A). Open the progress file with a `## Verification ledger` **heading and header row only — ZERO data rows** (§ *Test Design* → ledger shape). Subtask 1 does **not** materialise the `E<n>` skeleton; **subtask 3 does, one row at a time as each record is read.** Pre-generating it from `grep '^### '` is forbidden — it would satisfy G12 limb 1 with nothing read (R4-3). Exit gate: **G11**. | `ai-docs/learnings.md` (read), `…progress.md` | — |
| 2 | **Re-verify C1–C8 and apply the field-scope test (READ-ONLY).** For each candidate read the prior AND later entry **in full** — not the heading. Classify total reversal / partial withdrawal / refinement. Then apply the field-scope test verbatim from spec § *KD-5 derivation* (a): *does the later event invalidate the prior entry's `Rule:` text — directly, or by removing a premise that `Rule:` explicitly rests on?* Record each verdict in a separate `## Candidate verdicts (C1–C8)` list keyed by candidate id **and** prior-entry heading text — **not** in the ledger, whose rows do not exist yet; subtask 3 merges them into the matching `E<n>` rows by heading text. Record a PASS/FAIL verdict **plus the quoted `Rule:` fragment it turns on** for C1 and C4–C8 (AC11). C2/C3 are already-marked exemplars: verify their existing text is still accurate, record **no edit**. Re-run the E2 field-scope test on `:751` (AC5) and record the verdict with its inversion path. Exit gate: **G11**. | `ai-docs/learnings.md` (read), `…progress.md` | 1 |
| 3 | **Full sweep over all `N` entries (READ-ONLY).** Primary pass: read the file end-to-end in record order and, for **every** record, ask the field-scope question against everything later — **this subtask materialises the ledger's data rows** — one `E<n>` row appended as each record is read, in file order, contiguous `E1..EN`, so coverage is `N`, not 8. Record each prior entry's `**Kind:**` in its row **from the record itself** (absent `Kind:` line ⇒ `correction`); it both selects which FAIL branch applies (§ *Scenarios*) and is G12 limb 2's proof that the record was opened. Merge subtask 2's `## Candidate verdicts` entries into the matching rows by heading text. Cross-check pass: **M7b** (mandatory `-i`, unescaped ERE — the `-i` step is 52 → 62, M7c) plus `rg -U` for any wrapped construct. **A cross-check hit the read pass missed is a defect signal about the read pass** — go back and re-read that region, do not just add the row. Separate supersession (refutes / narrows / withdraws) from mere corroboration (restates / adds a data point) — corroboration gets **no** marker. Exit gates: **G11**, then **G12**. | `ai-docs/learnings.md` (read), `…progress.md` | 1 |
| 4 | **Write the marker set (FIRST EDIT).** Entry precondition, checked and recorded before the first `Edit`: **G12** passes (contiguous `E1..EN` rows, count `== N`, gap detector `-> PASS`), the ledger has a verdict row for C1–C8, and **G11** last returned clean. Insert one `**Superseded by:**` line immediately after the target entry's `**Escalated?**` line, per § *Marker construction*. Any candidate whose field-scope verdict is FAIL is **left unmarked** and carried to the report — never forced. Gates after the edits: **G2, G3, G4, G5, G6, G10**. Commit; the commit body embeds the per-candidate field-scope verdicts from the ledger. | `ai-docs/learnings.md` | 2, 3 |
| 5 | **Append the E2 entry.** Prefix with a blank line (M8: the file has no trailing blank). `Kind: correction`, `Escalated? no`, backward citation to `2026-08-01 ("entry certified a gate that does not exist")` in KD-4 form, the measured fact stated **with its command**, and the new generalisable rule (a claim about a file's contents is timestamped to when it was measured; when the fix lands in the same commit as the entry, the entry ships stale by construction). **No** `Superseded by:` line is added to `:751`. Gates: **G2, G3, G4, G5, G6**. Commit separately from subtask 4. | `ai-docs/learnings.md` | 4 |
| 6 | **Post-edit measurement + AC sweep + report assembly.** Run **G1–G12 in full, in one pass, after the last edit of the run** — every count in the report comes from this pass, none from subtask 1 and none from the spec. Walk AC1–AC11 individually and record the outcome in the progress file's `## AC Status`. **Two ACs do not take a bare PASS/FAIL:** **AC6** reports **"AMENDED, and the amendment is recorded"** (§ *Amendment A1*) — never PASS; and **AC5** reports **inverted-by-measurement** if the `:751` field-scope re-check returns the opposite. Both exist so a reader cannot mistake a moved goalpost for a cleared one. Assemble the report (spec § *Findings to report* items 1–7, **plus F-A, F-B and F-C**) for the PR body and the returned summary. | `…progress.md`, PR body | 5 |

M = 6. Within the ≤ 15 rule.

### Marker construction (binding for subtask 4)

- **Placement.** Own line, immediately after the target entry's `**Escalated?**` line
  (`.claude/agents/self-improve.md` § Commit B(b): *"If the prior entry has no
  `**Superseded by:**` line yet, INSERT one on its own line immediately after the entry's
  `**Escalated?**` line. Write to the PRIOR entry's `Superseded by:`, never to the new entry."*
  `[measured: M23 below]`). Both live instances conform. The anchor is total and unique per record
  (M3, M4).
- **Anchor hazard (round-2 addition, and the reason G2 changed).** `**Escalated?** no` is **not
  unique across the file** — 46 occurrences `[measured: M24 below]` — and an `Edit` anchor widened
  to disambiguate it tends to swallow the following blank separator line. Widen the anchor
  *upward* into the preceding `**Rule:**` / `**Kind:**` text instead of downward into the blank,
  and let **G2** (`--numstat`, deletions = 0) prove nothing was eaten.
- **Shape.** `**Superseded by:** YYYY-MM-DD ("slug") — <reason>`. **Parenthesised**, per the
  glossary and spec KD-4 — *not* the exemplars' unparenthesised `YYYY-MM-DD "slug"` (F-C).
- **Slug rule.** The slug is wrapped in double quotes, so it **MUST be a quote-free verbatim
  substring of the later entry's heading description**. Several later entries contain inner double
  quotes — C1's later entry is *the "drop an unresolvable citation" rule is REFUTED: the citation
  resolved, the probe was mis-aimed*, and C8's later entry contains `"firings"`
  `[measured: M25 below]`. Choose a quote-free run of the description (for C1, e.g. `rule is
  REFUTED: the citation resolved`). **G6** is what proves the choice resolves.
- **Reason slot.** Freeform (M10), but disciplined (AC3): it names the class in the field's
  **existing** vocabulary — *reversed / refined / generalized / subsumed / withdrawn* — and, for a
  partial withdrawal, names **which clause** is withdrawn AND states that the remainder **stands**,
  in the `:300` / `:466` exemplar shape. No new verb (Technical constraint 4); a relationship
  fitting none of the five is a **report item**, not a licence to invent a sixth. **No bare
  `Superseded by:` is written on any marker.**
- **One line per record, maximum.** Where a prior entry has two correctors (the `:739` case — the
  4 → 3 → 2 cascade set by `:570` and `:715`), write **ONE** marker: the terminal corrector in
  `[ref]`, the other cited **inside the reason slot** in the same `YYYY-MM-DD ("slug")` form.
  Never comma-join two dates in `[ref]` (grammar extension — § *Rejected alternatives*).
- **Never cite by line number** anywhere in a marker or in E2 (F-A).
- **Do not rewrite `:300` or `:466`.** If the sweep finds an *additional* corrector for either,
  that is a STOP-and-surface report item, not a silent edit — the spec's C2/C3 rows say "No edit",
  and Boundary rule 1's exception being permissive is not authorisation to exceed approved scope
  (AGENTS.md § *Communication* — verify a permissive reading harder).
- **Worked shape, NOT text to paste** (derived from `:300`; the run writes its own after
  re-verification): `**Superseded by:** <date> ("<quote-free slug>") — <one of the five verbs>:
  ONLY <the named clause> is withdrawn, on <ground>. The entry's <named remainder> STANDS
  unchanged.`

```bash
# --- M23 --- the placement rule
sed -n '264p' .claude/agents/self-improve.md | grep -c 'INSERT one on its own line immediately after'
# → 1

# --- M24 --- the anchor is NOT unique across the file; widen anchors upward, never into the blank
grep -c '^\*\*Escalated?\*\* no$' ai-docs/learnings.md
# → 46

# --- M30 --- F-C: the two exemplars do NOT use the glossary's parenthesised ref form
sed -n '300p;466p' ai-docs/learnings.md | grep -oE '\*\*Superseded by:\*\* [^—]{0,50}'
# → **Superseded by:** 2026-07-25 "ran the workspace Miri gate locally during
# → **Superseded by:** 2026-07-25 "the reason `/reflect` needs no `self-review` is
sed -n '49p' ai-docs/corrections-log.md | grep -oE 'disambiguate by appending a quoted slug[^;]*'
# → disambiguate by appending a quoted slug from the other entry's description — e.g., `2026-05-08 ("mutually exclusive markers")`)

# --- M25 --- later-entry descriptions containing inner double quotes
sed -n '199p;751p' ai-docs/learnings.md
# → ### 2026-07-17 — process — the "drop an unresolvable citation" rule is REFUTED: the citation resolved, the probe was mis-aimed
# → ### 2026-08-01 — process — a `validation` entry certified a gate that does not exist; both "firings" were manual derivation
```

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. **M = 6 ≥ 1, so this section is
mandatory (a).** Every subtask changes `ai-docs/**` and `*.md` only — a single change-type, so
homogeneity (e) forces no boundary; 6 ≤ 10, so the size cap (b) forces none; the dependency chain
is linear and intra-group, so (f) forces none. Minimized count is therefore **1 group**, well
under the default max of 4 (h).

- **Entry handoff into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § *Compaction recovery (re-entry)*. Binding for the
  **first** group too — `/task` Step 8 fans out every group, including the first and including
  M = 1 designs (c).
- **Group A** — model `opus`, effort **inherited from the orchestrator (typically xHigh) — NOT
  pinned**, 1M-token window, via `subagent_type="general-purpose"` with inline `model="opus"` —
  subtasks 1–6 (**instructions/harness** change-type: `ai-docs/**`, `*.md`). **Terminal group**
  (6 subtasks; within the `1..=10` range (d)). No further handoff — Group A completes Step 8 in
  its own `/context-reset` subagent.
- **Explicitly NOT a code group.** No subtask touches `*.rs`; `code-writer` is foreclosed by KD-2
  (§ *Approach*). Do not route any part of this design to `subagent_type="code-writer"`.
- The `design`, `design-review`, `self-review`, and `spec-writer` gates stay on **Opus**
  regardless of the group marker (g).

## Risks

- **R1 — line-number drift invalidates two immutable in-log self-references (F-A).** Unfixable in
  scope; mitigated by recording baseline content and reporting the post-edit location (**G9**),
  and by forbidding line-number citations in every artefact the run writes. `[measured: M21]`
- **R2 — Boundary rule 2 tripwire.** The run appends a NEW entry (E2), so editing `AGENTS.md`,
  `CLAUDE.md`, `.claude/**`, `.claude/settings.json`, `ai-docs/code-style.md`, or
  `ai-docs/doc-convention.md` in the same turn is forbidden. The `/task` Steps 8–12 carve-out
  covers *in-task insights*, **not a task's own deliverable**, so it does not apply. Mitigation:
  **G1 is the mechanical tripwire** — any such file appearing in `git diff --name-only main` is a
  hard STOP-and-surface, not a carve-out to argue for.
  `[derived → G1 at every commit boundary and at subtask 6]`
- **R3 — a removed blank line passes an additions-only check (round-2 root cause of the G2
  change).** The `Edit` anchor hazard (46 non-unique `**Escalated?** no` lines, M24) makes a
  swallowed separator the single most likely Boundary-rule-1 violation this task can commit, and
  `-U0` renders it as a bare `-` that `^-[^-]` cannot see. Mitigated by **G2** on `--numstat`.
  `[measured: M18]`
- **R4 — `:751` reads as a blocker (F-B).** Mitigated by recording the authorisation chain in the
  design and the PR body; the implementor proceeds. `[measured: M22]`
- **R5 — a slug containing a double quote silently breaks the ref.** Mitigated by the
  quote-free-substring rule and proven by **G6**, which runs the actual resolver semantics.
  `[measured: M25 for the hazard; derived → G6 for the proof]`
- **R6 — the sweep gets bounded by the eight candidates.** This is the log's own documented
  failure mode — *"a remediation sweep must be bounded by the PROPERTY, not by the instances that
  surfaced it"* `[measured: M26 below]`. Mitigated by the contiguous per-record `E1..EN` ledger
  rows, made **measurable** by **G12** (count **and** gap detector — round 2's token count could
  false-PASS, R3-2) rather than asserted in prose, plus the mandatory case-insensitive
  cross-check M7b.
- **R7 — a count gets carried from the spec or from subtask 1.** Mitigated by placing the whole
  measurement pass in subtask 6, after the last edit, with the `awk` program named per figure, and
  by G8 stating the sweep denominator as `G8 − 1` rather than a look-back.
  `[derived → G7, G8, G12 at subtask 6]`
- **R8 — over-marking: corroboration mistaken for supersession.** A marker on a corroborating pair
  injects a false claim into an append-only file. Mitigated by making the field-scope test — not
  thematic similarity — the sole admission criterion, and by requiring the quoted `Rule:` fragment
  in every ledger verdict row. `[derived → the per-candidate AC11 report]`
- **R9 — a `--body-file` for `gh pr create` becomes an untracked stray and reds AC6.** Prefer
  inline `--body`; if the body text would trip the commit-block hook (it matches the substring
  `git commit`), write the body file **outside the repo** and re-run **G1** afterwards. Post-A1
  the test is *membership in A1's closed enumeration*, **not** an empty porcelain — a body file
  left inside the repo reds AC6 because its path is not enumerated, not because the porcelain is
  non-empty (which it legitimately is at Step 12). `[measured: M27 below; derived → G1]`
- **R10 — piping a load-bearing exit code.** All gate output is captured into a shell variable in
  the same invocation (shell state does not persist between `Bash` calls in this harness), never
  `cmd | tail`. AGENTS.md § *Build & Test*; a `PreToolUse` hook blocks the `cargo … | tail` form
  specifically, but the principle is broader. `[measured: M27 below]`
- **R11 — self-review is skipped as "just prose".** `/task` Step 10 is MANDATORY (AGENTS.md
  § *Workflow* — *"No 'too simple' step-skip in `/task`. Steps 6 / 7 / 10 are MANDATORY"*), and the
  `/reflect` carve-out does **not** apply — this is a `/task` deliverable, not `/reflect` output.
  `self-review` § *Patterns* 1 (verify every factual claim on a prose diff) is exactly the right
  posture for this diff. `[measured: M28 below]`

```bash
# --- M26 --- the property-not-instances rule this sweep must obey
sed -n '745p' ai-docs/learnings.md
# → ### 2026-08-01 — process — a remediation sweep must be bounded by the PROPERTY, not by the instances that surfaced it

# --- M27 --- the two PreToolUse Bash hooks named in R9 / R10
jq -r '.hooks.PreToolUse[] | select(.matcher=="Bash") | .hooks[].command' .claude/settings.json | grep -c 'git\[\[:space:\]\]+commit'
# → 2      (the commit-block hook and its ast-index companion both match the substring)
jq -r '.hooks.PreToolUse[] | select(.matcher=="Bash") | .hooks[].command' .claude/settings.json | grep -c 'BLOCKED: cargo gate piped through tail/head'
# → 1

# --- M28 --- Step 10 exists and is a loop, not an option
grep -n '^### Step 10' .claude/skills/task/SKILL.md
# → 188:### Step 10: Self-review loop (max 3 rounds)
```

## Test Design

There is **no `#[cfg(test)]` surface and no `tests/` target** for this task — the deliverable is
prose in a data file, so the AGENTS.md "~50+ lines of substantial logic needs a tests module" rule
has no subject. The verification design is the gate suite below. Each gate is one self-contained
`Bash` invocation (shell state does not persist across calls) and captures output into a variable
rather than piping a load-bearing exit code.

### Ledger shape (progress file, `## Verification ledger`)

`ai-docs/plans/…progress.md` is gitignored (M9), so the ledger costs nothing against AC6. The
canonical progress schema mandates required *fields* and does not close the *section* set
`[measured: M29 below]`, so an added section is in-schema. Rows:

**One row per record — exactly `N` data rows, ids `E1`…`EN` in file order.** Candidate labels are a
*column*, not a row type; a relationship is recorded on the **prior** entry's row. This is a
round-3 restructure: the round-2 shape mixed row types and let G12 count the header (R3-2).

| Column | Content |
|---|---|
| `id` | `E<record index>`, `1..N`, contiguous, in file order — **G12** counts and gap-checks these |
| `candidate` | `C1`…`C8`, `OL-n` (off-list), or `—` |
| `prior` / `later` | resolved **by heading text**, with the current line number as a convenience only |
| `kind` | the prior entry's `**Kind:**` — `correction` or `validation`; **drives which FAIL branch applies** (§ *Scenarios*) |
| `class` | total reversal / partial withdrawal / refinement / corroboration-only |
| `field-scope verdict` | PASS / FAIL **+ the quoted `Rule:` fragment it turns on** |
| `action` | marker / no-edit (already marked) / unmarked-report-item / no-marker (corroboration) |

> **Do not name any column with a bare status token** (the round-2 `swept` column). G12 anchors on
> the `E<n>` id at line start, so a header or a stray prose mention cannot be miscounted.

```bash
# --- M29 --- the progress schema constrains FIELDS, not the section set
sed -n '62p' ai-docs/templates/progress-format.md
# → **Required fields** (read by `self-review` at handoff and by the *compaction recovery check*
#   callout in every code-side orchestrator SKILL.md): `**Branch:**`, `**base_commit:**`,
#   `**Last build:**`, `**current_step:**`, `**last_passed_gate:**`, `## Decisions log` section.
```

### Gates

| ID | Discharges | Pass condition |
|---|---|---|
| **G1** | AC6 — **AMENDED by A1** | The **touched set** — tracked diff **∪** untracked files — contains only paths in A1's **closed literal enumeration** (§ *Amendment A1*): `ai-docs/learnings.md`, this task's `.spec.md`/`.design.md` at their `plans/` **and** `plans/done/` paths, `ai-docs/plans/INDEX.md`, `ai-docs/metrics/task-runs.jsonl`, `ai-docs/deferred/_inbox.jsonl`. **Anything else reds AC6 unconditionally — the run stops and surfaces it; it does NOT extend the list.** `AGENTS.md` / `.claude/**` / `ai-docs/{code-style,doc-convention,corrections-log,templates}` remain a hard STOP (R2), as do the three content docs and **`ai-docs/plans/archive/**`**. The allow-list is **not** a glob and is **not** widened to `ai-docs/plans/**`. AC6 reports as **"AMENDED, and the amendment is recorded"**, never PASS. |
| **G2** | AC7 — no removals | The **deletions column of `--numstat` is `0`**. Paste the raw `--numstat` line verbatim into the report. Supersedes round 1's `grep -c '^-[^-]'`, which is blind to a deleted blank line (M18). |
| **G3** | AC7 — addition class | Every `^+` line, excluding `^+++` and excluding `^+\*\*Superseded by:\*\*`, is part of the E2 block and nothing else. Unchanged from round 1. |
| **G4** | AC2 / TC2 — placement | No output. |
| **G5** | one field per record | Only `checked`. |
| **G6** | AC8 — resolver | For **every** `YYYY-MM-DD ("slug")` occurrence on any `**Superseded by:**` line *and* the E2 backward citation — not only the `[ref]` slot — ≥ 1 hit that is a **different** record from the one carrying the marker. Report the per-marker RESOLVED/UNRESOLVED table; AC8 demands a demonstration, not an assertion. |
| **G7** | AC9 — population | Run **after the last edit of the run**. Per-record parse only; the per-line `grep -c` form is forbidden (§ *Rejected alternatives*). |
| **G8** | AC4 — denominator | Run after the last edit. Returns `N + 1` (the E2 entry). **The sweep denominator reported to AC4 is `G8 − 1`, derived in this same pass** — never carried from subtask 1 and never from the spec. |
| **G9** | F-A | Report old → new line numbers for both baseline self-reference targets; confirm the `:92` and `:752` references now point elsewhere and say so in the finding. |
| **G10** | AC3 — vocabulary | Every added marker matches ≥ 1 of the five verbs; every **partial-withdrawal** marker additionally matches the stands-clause **and** names the withdrawn clause. Uses unescaped ERE — round 1's `'STANDS?\|stands'` passed vacuously (M19). Any relationship matching none of the five is escalated to the report, never patched with a sixth verb. |
| **G11** | AC1 — verify-before-edit | **Both commands empty**, at the exit of each of subtasks 1, 2, 3. Non-empty = the verification phase was contaminated → STOP. This is the structural enforcement: subtasks 1–3 produce no commit and no working-tree change, so AC1 is provable from `git`, not from a claim. Reinforced at subtask 4 by the commit body embedding the ledger's per-candidate verdicts. |
| **G12** | AC4 — sweep coverage | **Two limbs, both required.** *Limb 1 (enumeration):* `E<n>`-row count **equals `N`** and the gap detector prints `-> PASS`. *Limb 2 (examination):* every E-row's `kind` cell is `correction` or `validation` — never `—`, never blank — which is determinable only by opening the record, since `Kind:` **defaults** to `correction` when the line is absent. Limb 1 alone is satisfied by a skeleton generated from the file's own headings with nothing read (R4-3). Run at subtask 3 **exit** (completion), subtask 4 **entry** (re-check for drift, before the first `Edit`), and subtask 6 (against `G8 − 1`). Replaces round 1's prose precondition and round 2's `grep -c 'swept'` (which counted the header and passed silently on an under-covered sweep — R3-2). Demonstrated at **M32** (limb 1, three states) and **M36** (limb 2, pass + fail). |

```bash
# --- G1 --- AC6 scope.  AMENDED by A1 (owner ruling, 2026-08-02) -- see § Amendment A1.
# TWO changes from the round-3 body: (a) the allow-list gains A1's closed enumeration, as LITERAL
# paths -- never a glob, never `ai-docs/plans/**`; (b) the touched set is the tracked diff UNION
# the untracked set, because `git diff --name-only main` is BLIND to untracked files (M38).
T=2026-08-02-backfill-supersession-markers
ALLOW="ai-docs/learnings.md
ai-docs/plans/$T.spec.md
ai-docs/plans/$T.design.md
ai-docs/plans/INDEX.md
ai-docs/plans/done/$T.spec.md
ai-docs/plans/done/$T.design.md
ai-docs/metrics/task-runs.jsonl
ai-docs/deferred/_inbox.jsonl"
touched=$({ git diff --name-only main; git status --porcelain --untracked-files=all | sed 's/^...//'; } \
          | sed 's/.* -> //' | sort -u | grep -v '^$')
# RENAME BOUNDARY (noted because this limb is new with A1): `sed 's/.* -> //'` keeps a rename's
# DESTINATION and drops its SOURCE, and `git diff --name-only` collapses renames the same way.
# So a rename OUT OF a forbidden path INTO an allowed one would not red.  Residual risk here is
# NIL: the only rename this run performs is Step 12's `done/` move, where BOTH sides are
# enumerated.  Recorded so a future reader does not have to re-derive it.
printf '%s\n' "$touched"
while IFS= read -r p; do
  printf '%s\n' "$ALLOW" | grep -qxF "$p" || echo "AC6-RED: $p"
done <<< "$touched"
#
# Run at round-4 time -> touched set was exactly:
#   ai-docs/learnings.md
#   ai-docs/plans/2026-08-02-backfill-supersession-markers.design.md
#   ai-docs/plans/2026-08-02-backfill-supersession-markers.spec.md
#   (no AC6-RED lines)
#
# Negative control -- the standing prohibition is INTACT under A1:
#   AGENTS.md                    -> AC6-RED
#   ai-docs/plans/archive/x.md   -> AC6-RED     <-- the round-1 Issue-3 artefact class still reds
#   ai-docs/context.md           -> AC6-RED     <-- a content doc is NOT in the carve-out
#   ai-docs/deferred/_inbox.jsonl-> ok          <-- enumerated by A1

# --- G2 --- AC7 no removals.  Columns are ADDED, DELETED, path.  Require DELETED == 0.
git diff --numstat main -- ai-docs/learnings.md

# --- G3 --- AC7 addition class
d=$(git diff -U0 main -- ai-docs/learnings.md); printf '%s\n' "$d" | grep '^+' | grep -v '^+++' | grep -v '^+\*\*Superseded by:\*\*'

# --- G4 --- placement: every field line's predecessor is an Escalated? line
awk '/^\*\*Superseded by:\*\*/{if(prev !~ /^\*\*Escalated\?\*\*/) print "BAD-PLACEMENT: "NR} {prev=$0}' ai-docs/learnings.md

# --- G5 --- at most one field line per record
awk '/^### /{h=$0;n=NR;s=0} /^\*\*Superseded by:\*\*/{s++; if(s>1) print "MULTI: "n": "h} END{print "checked"}' ai-docs/learnings.md

# --- G6 --- AC8 resolver, per date+slug occurrence (run once per occurrence; DATE/SLUG substituted)
grep -n "^### ${DATE} — " ai-docs/learnings.md | grep -F -- "${SLUG}"
# Mirrors .claude/agents/learnings-escalation-audit.md:81 — "at least one OTHER entry … shares
# that date AND (when a disambiguation slug is present) contains the slug text in its description."

# --- G7 --- AC9 population, per-record parse, AFTER the last edit
awk '/^### /{h=$0;n=NR;has=0} /^\*\*Superseded by:\*\*/{if(!has){has=1;c++}} END{print c}' ai-docs/learnings.md

# --- G8 --- AC4 denominator, AFTER the last edit.  Sweep denominator = this value MINUS 1.
awk '/^### /{n++} END{print n}' ai-docs/learnings.md

# --- G9 --- F-A drift, by verbatim content search (never by remembered line number)
grep -n -F 'learnings.md:75' ai-docs/learnings.md
grep -n -F 'this file, ~l.583' ai-docs/learnings.md

# --- G10 --- AC3 vocabulary.  UNESCAPED ERE.  Run per added marker line (LN substituted).
sed -n "${LN}p" ai-docs/learnings.md | grep -oiE 'revers|refin|generaliz|subsum|withdraw'
sed -n "${LN}p" ai-docs/learnings.md | grep -oiE '\bstands?\b'
# WORD-BOUNDED.  Round 3's `grep -oiE 'stands?'` matched SUB-WORD, so a marker containing
# "standard" / "understand" / "outstanding" passed the stands-clause check with no stands-clause
# (M35).  Third instance of one shape: a gate satisfiable without the property it stands for.

# --- G11 --- AC1 verify-before-edit, at the exit of subtasks 1, 2, 3
git status --porcelain ai-docs/learnings.md; echo ---; git log --oneline main..HEAD -- ai-docs/learnings.md

# --- G12 --- AC4 sweep coverage.  N at subtask-4 entry; G8 − 1 at subtask 6.
# Anchored on the E<n> row id at line start -- NOT on a bare token, which would count the header.
L=ai-docs/plans/2026-08-02-backfill-supersession-markers.progress.md
grep -cE '^\| *E[0-9]+ *\|' "$L"                       # must equal N
grep -nE '^\| *E[0-9]+ *\|' "$L"                       # print the rows, so a mismatch is diagnosable
grep -oE '^\| *E[0-9]+' "$L" | grep -oE '[0-9]+' | sort -n | awk -v n="$N" '
  BEGIN{p=0;bad=0}
  {if($1!=p+1){print "  GAP: E"p+1" missing"; bad=1} p=$1}
  END{if(p!=n){print "  SHORT: last is E"p" of "n; bad=1}
      print (bad ? "  -> FAIL" : "  -> PASS (contiguous E1..E" n ")")}'

# LIMB 2 (round-4 addition) -- proves each record was OPENED, not merely enumerated.
# Limb 1 counts rows and checks contiguity; a skeleton generated mechanically from the file's
# own headings satisfies it without a single record having been read (R4-3).  The `kind` cell
# is determinable ONLY by opening the record, because `Kind:` DEFAULTS to `correction` when the
# line is absent -- so "no Kind: line" and "Kind: correction" are the same cell value and
# different reads.  Every E-row must carry `correction` or `validation`; never `—`, never blank.
awk -F'|' '/^\| *E[0-9]+ *\|/{
    k=$6; gsub(/^[ \t]+|[ \t]+$/,"",k);
    id=$2; gsub(/^[ \t]+|[ \t]+$/,"",id);
    n++; if(k!="correction" && k!="validation"){print "  BAD-KIND: "id" -> ["k"]"; bad=1}}
  END{print (bad ? "  -> FAIL" : "  -> PASS (all "n" kind cells populated)")}' "$L"
```

```bash
# --- M18 --- WHY G2 changed: -U0 renders a deleted BLANK line as a bare '-'
git diff --no-index -U0 <(printf 'a\n\nb\n') <(printf 'a\nb\n') | cat -A
# → diff --git a/dev/fd/63 b/dev/fd/62$
# → --- a/dev/fd/63$
# → +++ b/dev/fd/62$
# → @@ -2 +1,0 @@ a$
# → -$                                    <-- a real deletion, rendered as a bare minus

d=$(git diff --no-index -U0 <(printf 'a\n\nb\n') <(printf 'a\nb\n')); printf '%s\n' "$d" | grep -c '^-[^-]'
# → 0                                     <-- round-1 G2 reports "no removals".  FALSE.

git diff --no-index --numstat <(printf 'a\n\nb\n') <(printf 'a\nb\n')
# → 0	1	/dev/fd/{63 => 62}             <-- deletions column = 1.  Round-2 G2 catches it.

# --- M19 --- WHY G10 changed: '\|' is a LITERAL PIPE under ERE
sed -n '300p' ai-docs/learnings.md | grep -oE 'STANDS?\|stands'; echo "exit=$?"
# → exit=1                                <-- no output; every partial-withdrawal marker passed vacuously
sed -n '300p' ai-docs/learnings.md | grep -oiE 'stands?'; echo "exit=$?"
# → STANDS
# → exit=0
sed -n '466p' ai-docs/learnings.md | grep -oiE 'stands?'
# → STANDS
#
# SUPERSEDED BY M35 / R4-2 -- do NOT paste the form above.  It fixed round 2's literal-pipe bug
# but is still vacuous SUB-WORD ("standard", "understand", "outstanding" all match).  The
# authoritative G10 body is word-bounded: grep -oiE '\bstands?\b'
sed -n '300p' ai-docs/learnings.md | grep -oiE 'revers|refin|generaliz|subsum|withdraw'
# → withdraw

# --- M37 --- validation-kind PRIORS named in the spec's tables (vs the 18-record live class)
S=ai-docs/plans/2026-08-02-backfill-supersession-markers.spec.md
grep -nE '^\|' "$S" | grep -E 'Kind: validation' | grep -vE '^77:'
# → 193: C7  `:630` … (**`Kind: validation`**)
# → 194: C8  `:582` … (**`Kind: validation`**)
# → 203: off-list `:606` … (`Kind: validation`)
#   = 3 priors.  Line 77 is excluded deliberately: it is KD-3's decision row, which mentions the
#   phrase in prose ("every `Kind: validation` -> `Kind: correction` case") and names no entry.
#
#   The spec does NOT estimate this class anywhere.  Its only estimate (§ Findings to report 1)
#   is "~8", and that is a count of MARKERS, a different quantity:
grep -n 'roughly triples that' "$S"
# → 280: … roughly triples that (estimated ~8; the true figure is whatever the sweep produces) …

# --- M31 --- the resolver itself (F-C's safety claim; round 2 mis-tagged this to M22)
sed -n '81p' .claude/agents/learnings-escalation-audit.md
# → - **`YYYY-MM-DD` ref** — at least one OTHER entry in `ai-docs/learnings.md` shares that date
#   AND (when a disambiguation slug is present) contains the slug text in its description.
#   If no match → ⚠️ Mismatch on `Superseded by:`.
#
# Date match + slug-text containment.  Nothing in the rule inspects the punctuation AROUND the
# slug, which is why the glossary's parenthesised form and the exemplars' bare form both resolve.

# --- M32 --- WHY G12 changed: the round-2 body counted the header and false-PASSED.
# Throwaway 3-record ledger built in the scratchpad, in three states.  N=3 throughout.
#
#   ROUND-2 BODY   grep -c 'swept'
#     complete (E1,E2,E3) → 4   == N+1  -> spurious FAIL (invites the implementor to fudge it)
#     short    (E1,E2)    → 3   == N    -> FALSE PASS on an under-covered sweep
#
#   ROUND-3 BODY   grep -cE '^\| *E[0-9]+ *\|'  + gap detector
#     complete            → 3  -> PASS (contiguous E1..E3)
#     short    (E1,E2)    → 2  -> SHORT: last is E2 of 3      -> FAIL
#     interior gap (E1,E3)→ 2  -> GAP: E2 missing             -> FAIL
#
# The interior-gap case also caught a bug in the FIRST draft of the detector: its END branch
# printed "(contiguous)" alongside "GAP", because it only compared the last index to N.  Fixed
# with a `bad` flag; all three states re-run green/red as shown above.

# --- M36 --- G12 LIMB 2 (kind cell), demonstrated pass and fail on a throwaway ledger.
#   good ledger (E1 correction, E2 validation):
#     -> PASS (all 2 kind cells populated)
#   bad ledger (adds E3 with '—' and E4 with an empty cell):
#     BAD-KIND: E3 -> [—]
#     BAD-KIND: E4 -> []
#     -> FAIL
# The FIRST draft of limb 2 printed its pass line unconditionally from END -- the same
# misleading-END shape as the M32 detector's first draft.  Both now gate on a `bad` flag.

# --- M33 --- table cells that still carry command text (the narrowed transcription rule)
D=ai-docs/plans/2026-08-02-backfill-supersession-markers.design.md
grep -E '^\|' "$D" | grep -cE '`[^`]*(grep|awk|sed|git diff|git log|jq)'
# → 24        A deliberately LOOSE superset: it also catches rows whose backticked span is a bare
#             path or identifier, not a runnable command.  Round 3's first draft of this line
#             claimed 11 with a regex that actually returns 0 (it required a trailing space after
#             the command name).  Both the count and the regex are re-derived here.

# Of those, rows quoting the BROKEN 'STANDS?\|stands' form (each marked as broken in situ):
grep -nF 'STANDS?\|stands' "$D" | grep -cE '^[0-9]+:\|'
# → 3         (the round-2 defect row, the R3-3 row, and G10's pass-condition)

# THE LOAD-BEARING CLAIM: no authoritative gate body lives only in a cell.  Every G1..G12 has a
# fenced definition -- this prints nothing if that holds:
for g in G1 G2 G3 G4 G5 G6 G7 G8 G9 G10 G11 G12; do grep -q -- "--- $g " "$D" || echo "UNFENCED: $g"; done
# → (no output)

# --- M34 --- whole-document figure sweep: every bolded figure, and every `# →` result.
# Run after the LAST edit of any round.  Each bolded figure must trace to a fenced transcript.
D=ai-docs/plans/2026-08-02-backfill-supersession-markers.design.md
grep -noE '\*\*[0-9]+[^*]{0,3}\*\*' "$D"      # bolded figures, with line numbers
grep -nE '^# → [0-9]+' "$D"                    # fenced measured results
# Round-4 result: every bolded figure traces to a transcript.  The only line carrying two
# different values for one quantity is the R3-1 corrections row (39 -> 46), where the wrong/right
# pair IS the content.
#
# Companion check -- no citation may name a transcript that does not exist:
for id in $(grep -oE '\bM[0-9]+[a-c]?\b' "$D" | sort -u); do grep -q -- "--- $id " "$D" || echo "DANGLING: $id"; done
# → (no output)   Round 3 shipped `M34` cited once with no body -- caught by exactly this check,
#                 which is why the check now lives in the document instead of only in a summary.
#
# The `[a-c]?\b` matters: a bare `M[0-9]+` pattern splits the lettered ids into a phantom
# suffix-less id and reports it DANGLING.  Round 4's first draft did exactly that -- and also
# flagged M17, which was REAL: M16 and M17 shared one combined label, so M17 had no marker of
# its own (now split).  Both were caught by running the check on its own document first.
#
# Keep this comment free of a literal lettered-id token: with one present, the check flags its
# own documentation.  That is the guard's-own-docs false positive already recorded in
# ai-docs/learnings.md ("a pattern-matching guard's own documentation is the highest-density
# false-positive source for that guard") -- observed here, on this very check, in round 4.

# --- M35 --- WHY G10 gained \b: 'stands?' matched SUB-WORD
printf 'refined: the standard changed.\n' | grep -oiE 'stands?'; echo "exit=$?"
# → stand
# → exit=0                              <-- a marker with NO stands-clause passes
printf 'refined: the standard changed.\n' | grep -oiE '\bstands?\b'; echo "exit=$?"
# → exit=1                              <-- correctly rejected
printf 'I understand the outstanding item.\n' | grep -oiE '\bstands?\b'; echo "exit=$?"
# → exit=1                              <-- "understand" / "outstanding" also rejected
sed -n '300p' ai-docs/learnings.md | grep -oiE '\bstands?\b'    # → STANDS
sed -n '466p' ai-docs/learnings.md | grep -oiE '\bstands?\b'    # → STANDS
#   Both live exemplars still match, so the tightening costs no true positives.
```

### Scenarios the gates must cover

- **Happy path** — the candidates whose field-scope verdict is PASS take markers (C1, C4+C5 as one,
  C6, C7, ≥ 1 off-list), plus E2; G1–G12 all clean; population goes from 2 (M2) to whatever G7
  reports. **C8 is deliberately NOT enumerated here.** Round 2 listed it as receiving a marker
  while the FAIL bullet below pre-approved the opposite outcome — both outcomes pre-approved, so
  the scenario set could not discriminate (R3-6). C8's placement is the run's field-scope verdict
  to make, from a quoted reading of `:582`'s own `Rule:` text, exactly as AC11 requires of every
  other candidate. The spec's **total reversal** classification for C8 is *not* overturned by
  this — classification and licensing are separate steps, and the paragraph below is about
  licensing.
- **A swallowed separator blank line** — the highest-probability Boundary-rule-1 violation here
  (R3). G2 on `--numstat` is the only gate that sees it; G3 does not.
- **Field-scope FAIL — prior entry is `Kind: correction`.** The candidate is left unmarked, G10 has
  nothing to check for it, and AC11's report row carries the FAIL with its quoted `Rule:` fragment.
  A **legitimate settled outcome**, not a failure of the run.

- **Field-scope FAIL — prior entry is `Kind: validation`.** **NOT a settled result — this is a
  REPORT case under Technical constraint 4.** In a validation entry the `Rule:` *is* the practice
  being asserted and the `What happened:` is its **only** support; voiding the support does not
  make the `Rule:` text false. So the field-scope test as stated can return "not invalidated"
  while the entry has lost precisely the evidentiary status validations exist to carry — and a
  false `validation` is the more dangerous direction, because `/improve` escalates a carrot signal
  as confirmed practice. **C8 (`:582`) is exactly this shape.** The class is **18 records**
  `[measured: M5 — awk per-record parse; independently confirmed by grep -c '^\*\*Kind:\*\* validation$' → 18]`,
  — not the **3** validation-kind priors the spec's candidate and off-list tables happen to
  enumerate (C7, C8, and the off-list `:606`) `[measured: M37 below]`, which is itself evidence
  for R6 that the candidate list is a floor. Record **all three** of: the FAIL, the quoted `Rule:`
  fragment, and the **open question** of whether the field-scope test's second limb (*"or by
  removing a premise that `Rule:` explicitly rests on"*) reaches this case. **Do not invent a sixth
  verb and do not force a marker** — surface it and let the owner rule.
- **AC5 inversion** — if the re-run field-scope test on `:751` returns "`Rule:` invalidated", a
  marker **is** written and AC5 is reported as inverted-by-measurement. The design must not
  presuppose either answer.
- **Off-list hit on an already-marked entry (`:300` / `:466`)** — STOP-and-surface; the existing
  marker is not rewritten.
- **Relationship fitting none of the five verbs** — report item (Technical constraint 4); the
  `:751` reachability gap (AC10) is the already-known instance and is reported as **unresolved**,
  with the E2 backward citation named as partial mitigation only, explicitly **not** a fix.
- **Sweep coverage short of `N`** — G12 fails at subtask 4 *entry*, before any `Edit`, so an
  under-covered sweep cannot reach the file.
- **Edge — an entry whose `**Escalated?**` line is absent or duplicated** would break placement.
  Measured impossible today (M3, M4 → 0 of each), but G4 catches it if the sweep's own reading of
  a record was wrong.

## Open questions

None blocking. Carried forward from the spec, both non-blocking and both post-merge:

- **Q2 (spec).** Whether the `ai-docs/corrections-log.md` ↔ `.claude/agents/self-improve.md`
  placement contradiction warrants its own filed issue or folds into the next `/ai-audit`
  Phase 1. KD-3 resolves the direction for *this* task either way; the filing decision is
  post-merge and is **not** an in-scope edit (AC6).
- **F-A follow-up.** Whether the two in-log absolute line references (and the class generally)
  warrant a rule that log entries cite other entries by `YYYY-MM-DD ("slug")` and never by line
  number. That would be an instruction-file change — **out of scope here** (AC6, R2). Report the
  observation; do not propose the rule inside this task.

> **Both of the above must be written to `ai-docs/deferred/_inbox.jsonl` at Step 12** — this is
> why A1 makes that path **required**, not conditional. The carrier documents (this design and the
> spec) move to `ai-docs/plans/done/` in the same step, so an unrecorded deferral is a **lost**
> deferral. Written by `/task` Step 12's writer only; per the AGENTS.md AXIOM the file is never
> hand-edited.
