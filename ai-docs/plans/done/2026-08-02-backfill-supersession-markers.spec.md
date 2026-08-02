# Backfill supersession markers in `ai-docs/learnings.md`

**Source:** user description (free-text)
**Date:** 2026-08-02
**Tracked in:** none — free-text entry; the source request explicitly places opening or closing issues out of scope
**Amended:** 2026-08-02 — A1 changes AC6. See [§ *Amendments*](#amendments).

## Problem

`ai-docs/learnings.md` is append-only (AGENTS.md § *Learning Log* → Boundary rule 1). Later entries
routinely reverse, narrow, or correct earlier ones, but `**Superseded by:**` — the only
machine-readable link between a superseded entry and its corrector — is present on **2 of 123**
entries (measured 2026-08-02, per-record `awk` parse; see § *Measured baseline*). A reader landing on
an unmarked superseded entry acts on a claim a later entry refutes. One such entry is a
high-salience `/improve` escalation candidate, so the gap is not merely cosmetic: escalating a
refuted rule would move it off a self-correcting surface onto one that is not.

## Measured baseline

All figures measured in the verification pass on 2026-08-02 against the working tree; re-derive
before writing any of them into a durable artefact (AGENTS.md § *Communication* — a recorded result
is a claim).

| Figure | Value | Command |
|---|---|---|
| Entries in the log | 123 | `awk '/^### /{n++} END{print n}' ai-docs/learnings.md` |
| Entries carrying `**Superseded by:**` | 2 | per-record `awk` (accumulate at each `^### ` boundary, **not** a per-line `grep` on the field line) |

> **MEASUREMENT constraint.** Every count written into the log, the PR body, or the report MUST be
> re-derived in the implementing run by a **per-record parse**, never a per-line field match. A
> per-line match on the `**Escalated?**` line is the documented origin of a wrong count already in
> this log (entry `2026-07-31 — documentation — CORRECTION: the entry below contains two wrong
> numbers`, verified :655). Never carry a count from this spec.

## Scope

1. **Derive the field's format and placement from the two live instances** (`:300`, `:466` at time of
   writing — resolve by content, not by line number) plus the five sites that define the field
   (Technical constraint 6 — read-only). Do not invent a format, and do not extend it (KD-6).
2. **Verify every candidate relationship** in § *Verified candidate relationships* still holds against
   the tree at implementation time, and classify each as total reversal / partial withdrawal /
   refinement.
3. **Sweep all 123 entries** for supersession relationships not on the candidate list. The candidate
   list is a **floor, not the scope**. At least one off-list relationship is already known (§
   *Off-list relationships found during verification*).
4. **Write the markers** on the superseded (prior) entries.
5. **Append one new entry** repairing the stale supporting fact in the final entry (§ *E2*).
6. **Report the count of off-list relationships found** — the number that measures whether the
   candidate list was a floor or the whole scope (AC4).

## Out of scope

- **No escalation.** Do not run `/improve`, do not queue escalations, do not add entries beyond the
  E2 one.
- **No file outside `ai-docs/learnings.md` is touched** (plan files aside), **except** the closed
  enumerated `/task` Step-9.5/12 path list added by **§ *Amendments* A1 (2026-08-02)** — read A1 before
  relying on this bullet. In particular, still no `AGENTS.md`, `.claude/rules/**`, `.claude/skills/**`,
  `.claude/agents/**`, `ai-docs/code-style.md`, `ai-docs/doc-convention.md`, `ai-docs/corrections-log.md`,
  or `ai-docs/templates/learnings-entry.md`. The field's grammar is **not** extended (KD-6), so nothing
  needs documenting anywhere.
- Do not open or close GitHub issues.
- Do not touch any line of an existing entry other than its `**Superseded by:**` line (Boundary rule 1).
- Repairing the **placement contradiction** found between `ai-docs/corrections-log.md:39` and
  `.claude/agents/self-improve.md:264` (see § *Deferred*) — record it, do not fix it here.

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| `ai-docs/corrections-log.md:39` (bi-directional `Kind:` supersession convention) says a NEW `correction` entry disconfirming a `validation` carries `Superseded by:` **pointing back**; `.claude/agents/self-improve.md:264` says *"Write to the PRIOR entry's `Superseded by:`, **never** to the new entry"*, and both live instances follow the prior-entry form. The two definition documents contradict each other. | Fixing it is an instruction-file edit in a task whose stated scope is one data file. **Where it fires is a criterion, not a count** — *every prior entry whose `**Kind:**` is `validation` and which a later entry supersedes*. The count is an output of a sweep that has not run (`ai-docs/learnings.md:667`); the run reports it. This spec resolves all such cases by taking the majority convention (Key Decision KD-3). | **Yes** — file after this PR merges. |
| The log is neither date-ordered nor append-ordered in its tail (e.g. `2026-07-31` entries at `:612` and `:715` sit below `2026-08-01` entries at `:576`–`:606`; several correcting entries were placed **above** the entry they correct and refer to it as *"the entry below"*). | Out of scope to reorder (Boundary rule 1 forbids it), but it invalidates position-as-chronology and is a live trap for future automated readers. | Optional — note in the report. |

## Key decisions

| Question | Decision |
|---|---|
| **KD-1 — Authorisation.** `ai-docs/corrections-log.md:17` states manual edits to `Superseded by:` are **not** authorised by the Boundary-rule-1 Exception; `learnings-escalation-audit` is explicitly forbidden from **adding** a `Superseded by:` line, and `/improve` is out of scope by the user's own instruction. | Proceed. The same sentence at `:17` names the second authorising path verbatim: *"invoke `/ai-audit` or **explicitly request the change**."* This task **is** the explicit request. Record this authorisation basis in the PR body. |
| **KD-2 — Delegation.** Delegate the edit to `code-writer`, or author in-thread? | **Author in-thread.** The diff is prose in one `ai-docs/**` file plus judgement calls about what a rule means — `code-writer`'s charter is *"File-based **code-writing** implementor"*, so a prose-only diff has nothing to delegate (AGENTS.md § *Workflow*, delegation phase (1) *Fit*). Verified precedent: `ai-docs/learnings.md:193` — *"delegated ~30 instruction-file prose edits to `code-writer`, whose charter is code only"*, `Escalated? AGENTS.md, agent:code-writer`. Pre-resolved; do not re-litigate. |
| **KD-3 — Marker direction.** Forward-on-prior (`Superseded by:` on the earlier entry, naming the later one) or backward-on-new? | **Forward-on-prior**, unconditionally — including every `Kind: validation` → `Kind: correction` case, however many the sweep finds. Backed by 2/2 live instances, `.claude/agents/self-improve.md:264`, `ai-docs/templates/learnings-entry.md:19`, and `ai-docs/corrections-log.md:49`. The single dissenting sentence (`corrections-log.md:39`) is deferred, not followed — and forward-on-prior is the **only** direction that serves the problem statement, since the unmarked-prior-entry reader is who the field protects. |
| **KD-4 — Ref disambiguation.** `[ref]` is a `YYYY-MM-DD` date; most candidates share `2026-07-31` or `2026-08-01`. | A bare date is ambiguous for every candidate here. **Every** marker written MUST use the glossary's documented disambiguation form — `YYYY-MM-DD ("quoted slug from the other entry's description")` (`ai-docs/corrections-log.md:49`) — so `learnings-escalation-audit`'s resolver (`.claude/agents/learnings-escalation-audit.md:81`, which matches date **AND** slug) can resolve it. |
| **KD-5 — Does `:751` take a marker?** *(re-derived round 3 — the earlier ground was refuted; see § KD-5 derivation)* | **Conditional, and the condition is a test, not a ruling.** The field's object is the entry's **`Rule:` text**, per all three definition sites. Apply the **field-scope test** to `:751`: does commit `656ea79` invalidate `:751`'s `Rule:`? Measured answer: **no** — the stale material (*"No such gate exists"* + the hard-coded-banner quote) sits entirely in `**What happened:**`, and the `Rule:`'s one historical clause (*"both 'firings' were manual derivation"*) stays **true**, since the gate landing afterwards does not retroactively automate two catches already made by hand. So no marker. **If the implementing run's re-check finds otherwise, the conclusion flips and `:751` takes a marker like any other** — AC5 is the output of this test, never its premise. |
| **KD-6 — "PARTIAL VS TOTAL".** Does distinguishing total / partial / refinement require extending the field's defined grammar? | **No — and this is CLOSED BY VERIFICATION, not by a scope ruling.** The premise the source request rested on — that `Superseded by:` is a bare pointer with no room for partiality, so *"a bare `Superseded by:` on a partial withdrawal tells a reader to drop a rule that still holds"* — was an **unverified assumption**, and the round-1 check refuted it. Measured: the `[one-line reason]` slot is already *freeform*; `ai-docs/corrections-log.md:49` already names five supersession verbs (*reversed / refined / generalized / subsumed / withdrawn*); and **both** live instances already spell out partiality in the exemplar shape — `:300`: *"**ONLY** this entry's closing clause … is withdrawn, on COST grounds … The entry's primary rule … **STANDS** unchanged"*; `:466`: *"**ONLY** this entry's *rationale* is withdrawn, not its rule. The ruling it records … **STANDS**"*. Nothing is extended, so nothing needs documenting. **The design doc MUST record this as a refuted premise, not as a decision the owner made** — the distinction matters because a scope decision invites re-litigation and a refuted premise does not. |

## KD-5 derivation — what the field is scoped to, and the gap that leaves

**The refuted ground.** KD-5 previously read *"the rule stands, therefore no marker."* That ground is
**wrong**, and the two exemplars KD-6 relies on are what refute it: `:300` and `:466` are **both**
cases where a clause is withdrawn and the rule STANDS, and both **carry** the field. "The rule stands"
therefore cannot license withholding a marker — it is the normal case for a partial withdrawal. KD-5
and KD-6 were reading the same two instances in opposite directions.

### (a) Is "true when written, false now" the relationship the field encodes?

**The proposed discriminator — a later RULING vs a later COMMIT — is NOT real. It is refuted by the
field's own ref grammar.** `ai-docs/corrections-log.md:49` admits as `[ref]` *"a **`PR #N`** reference
to a merged PR that reversed the rule directly in instruction files"*. A commit or PR supersedes an
entry just as a later entry does. Do not use this axis, and do not let it into the design doc.

**A different discriminator is real, and it is grounded in three sites — the field's object is the
entry's `Rule:`, not its `What happened:`.** Quoted:

| Site | Text |
|---|---|
| `ai-docs/corrections-log.md:49` | *"records that **the rule** recorded above was later reversed, refined, generalized, subsumed, or withdrawn"* |
| `ai-docs/templates/learnings-entry.md:19` | *"omit unless a later entry/PR reverses, refines, generalizes, subsumes, or withdraws **this entry's rule**"* |
| `.claude/agents/self-improve.md:264` | *"Identify the PRIOR entry whose **`Rule:` text** Commit A invalidates"* |

Both exemplars confirm the scope rather than merely being consistent with it — verified by reading
each entry in full, **not** inferred from the marker text:

- **`:300`** — the withdrawn clause *"And run the WORKSPACE Miri command before declaring a
  render/geometry task done"* is the **closing sentence of the `**Rule:**` line**.
- **`:466`** — the withdrawn *rationale* (*"the question is whether its expected yield beats its cost
  at that surface"*) is likewise **inside the `**Rule:**` line**.

Neither exemplar is a `What happened:`-only repair. `:751` is: its `Rule:` survives `656ea79` intact.

**The field-scope test, stated for the sweep** (this is the operative form — apply it to *every*
candidate, not only `:751`): *does the later event invalidate the prior entry's `Rule:` text —
directly, or by removing a premise that `Rule:` explicitly rests on?* The second limb is not a
loophole; it is required by the candidates already verified. `:739`'s `Rule:` makes its own tally
load-bearing (*"Record frequency instead. If this recurs, the escalation is…"*), so halving the count
4 → 2 reaches the `Rule:`. `:667`'s `Rule:` carries *"all **seven** prior formulations lacked"*
in its own text.

> **Implementation obligation.** Re-apply this test to C1 and C4–C8 during the run. It is the ground
> the whole marker set rests on, and it was derived *after* those candidates were classified. **Any
> candidate that fails it is a report item, not a marker to force** — say so and leave it unmarked.

### (b) Reachability from `:751` — a real gap this spec does NOT close

Whichever way (a) goes, a reader arriving at `:751` in file order gets a stale fact with **no forward
link**. Measured: nothing in the log provides one today. The three available mechanisms all fail:

| Mechanism | Why it fails here |
|---|---|
| `**Superseded by:**` | Scoped to the `Rule:` (above). Using it would tell a reader `:751`'s **rule** was superseded — which is false, and is precisely this task's own defect class (*a marker that tells a reader to drop a rule that still holds*) relocated into the case it was meant to fix. |
| Editing `**What happened:**` | Forbidden — Boundary rule 1; only the `Superseded by:` line of an existing entry is mutable. |
| The E2 entry itself | Append-only puts it at the **end** of the log. It is reachable *from* nowhere; it only points **backward**. |

**This is Technical constraint 4's REPORT case.** The relationship — *a later event invalidated a
`What happened:` fact while the `Rule:` stands* — fits none of the five verbs, all of which are
Rule-directed. Per the constraint: **surface it; do not invent a sixth verb, and do not invent a sixth
field.** The run reports the gap as an unresolved finding (AC10).

What *is* in scope and does partially help, without being mistaken for a fix: the E2 entry MUST carry
an explicit backward citation to `:751` in the KD-4 `YYYY-MM-DD ("slug")` form, so the pair is
greppable and discoverable from the E2 end. **This is not forward-reachability** and the report must
not present it as closing the gap.

## Technical constraints

1. **Boundary rule 1** — only the `**Superseded by:**` line may be added or changed on an existing
   entry. Date, category, description, `**What happened:**`, `**Rule:**`, `**Kind:**`, `**Escalated?**`
   are immutable.
2. **Placement** — when an entry has no `**Superseded by:**` line, INSERT one on its own line
   **immediately after** that entry's `**Escalated?**` line (`.claude/agents/self-improve.md:264`).
   Both live instances conform.
3. **Field shape** — `**Superseded by:** [ref] — [one-line reason]`. `[ref]` per KD-4.
4. **Vocabulary is closed.** Draw every marker's reason wording from the five verbs already in use
   (*reversed / refined / generalized / subsumed / withdrawn*, `ai-docs/corrections-log.md:49`) and
   from the `:300` / `:466` exemplars. **Do not introduce a new verb.** If a verified relationship
   fits none of the five, **report it** in the run's findings and surface it — never invent one
   silently.
5. **Boundary rule 2 is satisfied by construction, and stays that way.** This task appends a NEW entry
   (E2). AGENTS.md § *Learning Log* → Boundary rule 2 forbids editing `AGENTS.md`, `CLAUDE.md`,
   `.claude/skills/**`, `.claude/agents/**`, `.claude/settings.json`, `ai-docs/code-style.md`, or
   `ai-docs/doc-convention.md` in the same turn as a `learnings.md` write. Since KD-6 touches nothing
   outside `learnings.md`, the rule does not bind — but it becomes a **live tripwire** the moment
   anyone reaches for an instruction file mid-run. If that urge arises, stop and surface it; do not
   reach for the `/task` Steps 8–12 carve-out, which covers *in-task insights*, not a task's own
   deliverable.
6. **Definition sites, for reading only.** The field is described in five places — `AGENTS.md:289`/
   `:312`, `ai-docs/corrections-log.md:11`/`:49`, `ai-docs/templates/learnings-entry.md:13`/`:19`,
   `.claude/agents/self-improve.md:264`, `.claude/agents/learnings-escalation-audit.md:81`. Read them
   to derive format and placement (AC2); **write to none of them**.
7. **No pipe on a load-bearing exit code** (AGENTS.md § *Build & Test*) — the sweep's counting
   commands are evidence; capture to a file and grep the saved log.
8. **Branch** — already on `chore/2026-08-02-backfill-supersession-markers` (verified). Do not edit
   on `main`.

## Verified candidate relationships

All eight candidates were **confirmed** against the working tree on 2026-08-02. Line numbers below
are as-observed at spec time and WILL shift once markers are inserted — resolve by heading text.

| # | Prior (superseded) entry | Later (superseding) entry | Class | Note |
|---|---|---|---|---|
| C1 | `:181` `2026-07-17 — process — asserted a remediation as DONE inside the same PR that adds the rule against exactly that` | `:199` `2026-07-17 — process — the "drop an unresolvable citation" rule is REFUTED` | **partial withdrawal** | The later entry opens its `Rule:` with *"SUPERSEDES tell (1) of the … entry"*. Only **tell (1)** ("drop an unresolvable citation") is reversed → *qualify, never drop*. Tell (2) (re-scan the whole section after a claim-class fix) and the entry's primary rule STAND. |
| C2 | `:295` `2026-07-19 — testing — a new test used exact float equality on a sqrt-derived value` | `2026-07-25` *"ran the workspace Miri gate locally…"* | **partial withdrawal** | **ALREADY MARKED** at `:300`. Verified the existing text is accurate and already names the withdrawn clause. **No edit.** Use as a format exemplar. |
| C3 | `:461` `2026-07-25 — process — ran self-review on /reflect output; the owner ruled it unnecessary` | `2026-07-25` *"the reason `/reflect` needs no `self-review` is STRUCTURAL redundancy"* | **partial withdrawal** (rationale only) | **ALREADY MARKED** at `:466`. Verified accurate. **No edit.** Second format exemplar. |
| C4 | `:739` `2026-07-31 — process — the load-bearing-justification rule has a channel gap`, **case (4)** | `:570` `2026-07-31 — search — case (4) of the entry below is WRONG` | **partial withdrawal** | Confirmed: `:570` states *"The tally in the next entry is **three**, not four"* and names cases (1)(2)(3) as surviving. |
| C5 | `:739` same entry, **case (3)** | `:715` `2026-07-31 — documentation — a copied value carries its predicate` | **partial withdrawal** | Confirmed: `:715` states *"its case (3) … is **not** an instance of that failure … The surviving count there is **two**, not three"*. **C4 and C5 target the SAME entry** — `:739` needs ONE marker naming BOTH correctors and the resulting 4 → 3 → 2 cascade. Candidate-list line `:716` was off by one; the heading is at `:715`. |
| C6 | `:667` `2026-07-31 — documentation — a number belongs in an artefact only where it is an INPUT` | `:655` `2026-07-31 — documentation — CORRECTION: the entry below contains two wrong numbers` | **partial withdrawal** (two figures) | Confirmed: **(a)** "26 unescalated corrections" → true count **28**; **(b)** "derived **seven** times" → `2026-07-23` is out of class. The `Rule:` (input-vs-output classification) STANDS — the later entry says so explicitly (*"Nothing new — this is the immediately preceding entry's own rule, violated in the act of writing it"*). Note the heading itself carries the wrong figure ("derived seven times") and is immutable — the marker is the only place a reader learns it is wrong. |
| C7 | `:630` `2026-07-31 — testing — can you name the remedy WITHOUT knowing which input provoked it? Three confirmed predictions` (**`Kind: validation`**) | `:624` `2026-07-31 — testing — DOMAIN of the naming tell` (`Kind: correction`) | **refinement** (domain-narrowing) | Confirmed. The later entry does **not** say the tell is wrong — it bounds its domain (*"it predicts instance-attachment failure only"*), shows `norm()` cleared the tell and broke **five ways**, and calls the three-for-three record a **survivorship artefact**. The heading's *"Three confirmed predictions"* and *"has not yet been wrong"* are the refuted parts. **This is the high-salience `/improve` candidate named in the problem statement — verified.** Marker direction per KD-3 (forward-on-prior), not `corrections-log.md:39`'s backward form. |
| C8 | `:582` `2026-08-01 — testing — a derived-count gate caught a real desync at execution` (**`Kind: validation`**) | `:751` `2026-08-01 — process — a validation entry certified a gate that does not exist` | **total reversal** | Confirmed: `:751`'s `Rule:` states *"**Supersedes** the `AC6` validation entry above; per Boundary rule 1 that entry stays intact and its `Superseded by:` field is for `self-improve` / `learnings-escalation-audit` to set, not for this turn."* — an explicit hand-off of exactly this backfill. **Caveat the marker must carry:** the mechanism `:582` credits (an automated derived-count gate) did not exist *when `:582` was written*, but **now does** — see § *E2*. The marker must not tell a reader the gate is absent today. |

## Off-list relationships found during verification

Verification surfaced at least one relationship absent from the candidate list, confirming the list
is a floor:

| Prior entry | Later entry | Class |
|---|---|---|
| `:606` `2026-08-01 — process — a delegate refused to let n=1 count as a result` (`Kind: validation`) | `:600` `2026-08-01 — process — CORRECTION of attribution: the mtime check was proposed with a misdescribed precedent` | **partial withdrawal** (attribution) — *"That is the wrong carrier. The check was proposed by an **external reviewer**"*. |

This is one confirmed instance, **not** the sweep's result. The sweep (Scope 3) must run over all 123
entries and report its own total (AC4).

> **Sweep heuristics** (starting points, not a closed list — a sweep bounded by the instances that
> surfaced it under-covers; `ai-docs/learnings.md:745`): the in-text idioms `the entry below`, `the
> entry above`, `the preceding entry`, `the same-day entry`, `SUPERSEDES`, `Supersedes`, `CORRECTION`,
> `is WRONG`, `is REFUTED`, `Retroactive correction`, `Extends the`, `Corroborates`, `Companion to`,
> the `[[…]]` wiki-link form, and every `Kind: validation` entry (enumerate them in the run —
> `awk '/^### /{h=$0} /^\*\*Kind:\*\* *validation/{print h}' ai-docs/learnings.md` — do **not** carry a
> count from this spec; each is a candidate for later disconfirmation). Distinguish **supersession**
> (refutes / narrows / withdraws) from mere
> **corroboration** (restates or adds a data point) — the latter gets **no** marker.

## E2 — repair the stale supporting fact in the final entry

**Both halves verified 2026-08-02.**

- **True when observed:** commit `656ea79` (`fix(task): close the escaped-delimiter class by property,
  and implement AC6`) is the commit that **added** the `:751` entry — `git show 656ea79 -- ai-docs/learnings.md`
  shows exactly one added heading, the `:751` one.
- **False by the time it was committed:** the **same** commit added the AC6 check to
  `.claude/skills/task/scripts/test-append-task-run.sh` (+132 lines). The current file derives `C`
  from the design's § *Cases* table (`C=$(awk '/^### Cases/{f=1} …' "$design")`) and asserts
  `assert_eq "AC6: cases exercised == design § Cases rows" "$cases_run" "${C:-0}"`. The hard-coded
  banner the entry cites is gone: the closing line now reads
  `echo "PASS: all ${cases_run} cases green (count derived from ${design} § Cases)."`.

**Why this is not a supersession — the ground, re-derived (KD-5, § *KD-5 derivation*).** Not "the rule
stands" (that is true of `:300` and `:466` too, and both carry the field). The ground is **field
scope**: `Superseded by:` is defined against the entry's `Rule:` text at all three definition sites,
and `656ea79` invalidates only material in `**What happened:**`. The `Rule:` — *"Before writing a
`validation` entry about a gate, **read the gate**"* — is untouched, and its one historical clause
(*"both 'firings' were manual derivation"*) remains true: the gate landing afterwards does not
retroactively automate two catches already made by hand. Append-only then permits exactly one repair:
a new entry. **Re-run this test in the implementing run before relying on it** (AC5, AC11).

**The new entry must:**
- Be `Kind: correction`, `Escalated? no`.
- State the measured fact with its command, not a bare assertion (`ai-docs/learnings.md:667` — a
  number/claim belongs in an artefact only where it is an INPUT; write the command, not just the result).
- Name the mechanism precisely: the gate was absent **at self-review time** and present **at commit
  time**, added by the same commit that recorded the entry — i.e. the entry documents a state its own
  commit had already fixed.
- Carry the generalisable rule (a claim about a file's contents is timestamped to the moment it was
  measured; when the fix lands in the same commit as the entry, the entry ships stale by construction
  — re-measure immediately before the commit that carries the entry). This is a **new** rule, not a
  restatement of `:751`'s.
- Carry an **explicit backward citation** to `:751` in the KD-4 `YYYY-MM-DD ("slug")` form, so the
  pair is greppable from the E2 end. This is partial mitigation of the reachability gap, **not** a fix
  for it (§ *KD-5 derivation* (b), AC10).
- **Not** be an escalation and **not** carry a `Superseded by:` on `:751`.

## Amendments

### A1 — 2026-08-02 — AC6 gains a closed, enumerated Step-9.5/12 carve-out

**Status: owner ruling, taken mid-Step-9.5. This is a CHANGE to AC6, not a clarification of it.**

> **Explicitly rejected framing.** Do **not** write, anywhere, that "AC6 always meant the content
> deliverable" or any variant of it. AC6 as previously written was **unconditional** and **did** cover
> the paths listed below; the owner is **changing** it. A reinterpretation would erase the record of
> the change, which is the same failure this whole task exists to repair — a later claim quietly
> displacing an earlier one with no durable link between them.

**State at the time of the amendment** (verified in this round, not carried from the ruling):

| Fact | Command | Result |
|---|---|---|
| Branch | `git branch --show-current` | `chore/2026-08-02-backfill-supersession-markers` |
| Commits | `git log --oneline main..HEAD` | 2 (`45080a7` markers, `a048890` E2 entry) |
| Diff | `git diff --numstat main` | `16 0 ai-docs/learnings.md` — zero deletions |
| Markers written | `git diff main -- ai-docs/learnings.md \| grep -c '^+\*\*Superseded by:\*\*'` | **10** |
| New entries | `git diff main -- ai-docs/learnings.md \| grep '^+### '` | **1** (the E2 entry) |

**Ground for the amendment — the prohibition AC6 was made unconditional to enforce STANDS UNCHANGED.**
AC6 was hardened to refuse exactly one move: **widening a gate's allow-list to accommodate an artefact
that should not exist** (`design-review` round 1, Issue 3 — the dropped `ai-docs/plans/archive/**`
commit). Nothing in A1 relaxes that. What A1 adds is a carve-out for `/task`'s *own mandated* Step
9.5/12 artefacts — files the workflow requires the run to write, not files the run wishes it could.

**The carve-out is CLOSED and EXHAUSTIVE. These paths, and no others:**

1. `ai-docs/plans/INDEX.md` — the plan row.
2. The spec/design move to `ai-docs/plans/done/` — these two files only, at their `done/` paths.
3. `ai-docs/metrics/task-runs.jsonl` — the Step 12 telemetry append.
4. `ai-docs/deferred/_inbox.jsonl` — **REQUIRED, not conditional.** Two deferred items depend on it:
   the spec's **Q2** (the `ai-docs/corrections-log.md:39` ↔ `.claude/agents/self-improve.md:264`
   placement contradiction) and the design's **F-A** follow-up. Both are explicitly deferred
   post-merge, and the plan documents carrying them move to `done/` in the same step — so skipping the
   inbox **loses both**. Write it via `/task` Step 12's writer; per AGENTS.md this file is written only
   by Step 12 and `/triage`, never by hand.

> **Do NOT write a glob for the above, and do NOT widen item 1–2 to `ai-docs/plans/**`.** The
> enumeration is the control. A glob would re-admit precisely the artefact class the standing
> prohibition refuses.

**The three Step-9.5 content docs are NOT in the carve-out by default** — `ai-docs/context-status.md`,
`ai-docs/context.md`, `README.md`. Each may be skipped **only** on a **measured, recorded, per-file
vacuity check** of this form:

> *"this doc's own stated content is not made stale by a 10-marker + 1-entry change to
> `ai-docs/learnings.md`"*

**Explicitly forbidden:** skipping the three as a class on the reasoning that the task implements no
crate. That reasoning is about the **task**; the required check is about the **docs**. Record the
check per file, by name, with what was inspected.

**AC6's reporting status changes: it reports as "AMENDED, and the amendment is recorded" — NEVER as
PASS.** This mirrors the posture the spec already gives AC5's inversion path: an AC that changed must
report that it changed, so a reader of the report cannot mistake a moved goalpost for a cleared one.

**Anything outside the enumerated list still reds AC6 unconditionally.** If a Step 9.5/12 action would
touch a path not listed above, the run **stops and surfaces it** — it does not extend the list, and it
does not proceed and report afterwards.

**Verification note on AC6's own check command.** `git diff --name-only main` is blind to **untracked**
files, and both plan documents are currently untracked (`git status --porcelain ai-docs/plans/` → `??`
for the spec and the design; `git ls-files` returns empty for both). So AC6's command cannot by itself
evidence the "spec/design plan files" half of its own criterion. Pair it with a category-matched
command — `git status --porcelain --untracked-files=all` — when discharging AC6, per AGENTS.md
§ *Dependency Versions* (a tool blind to the category cannot answer it).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Every claim in the source request is confirmed or recorded as refuted **before** any edit to `ai-docs/learnings.md`. The two off-by-one/stale refs already found (candidate `:716` → heading at `:715`; `:751`'s "no such gate exists" → gate now present) are reported, not silently dropped. |
| AC2 | The marker format and placement used are the ones derived from the two live instances (`:300`, `:466`) and the five definition sites, quoted in the PR body. The field's grammar is **not** extended (KD-6), so no definition document is edited. |
| AC3 | Every marker written classifies the relationship as **total reversal**, **partial withdrawal**, or **refinement**, expressed as disciplined use of the field's EXISTING freeform reason slot — vocabulary drawn from the five verbs already in use and from the `:300` / `:466` exemplars, with no new verb introduced (Technical constraint 4). For a **partial withdrawal**, the reason slot names *which clause* is withdrawn AND states that the remainder **stands**, in the exemplar shape. No bare `Superseded by:` is written on any marker. |
| AC4 | The sweep covers all 123 entries (count re-derived in the implementing run by a per-record parse). The report states the number of supersession relationships found that were **not** on the eight-item candidate list, with the entry pair for each. |
| AC5 | The E2 entry exists at the end of the log, is `Kind: correction` / `Escalated? no`, and carries an explicit backward citation to the `2026-08-01 — process — a validation entry certified a gate that does not exist` entry in the KD-4 `YYYY-MM-DD ("slug")` form. That entry carries **no** `Superseded by:` line — and the report states the **ground**: the field-scope test (§ *KD-5 derivation* (a)) re-applied in this run, returning "`Rule:` not invalidated". A bare assertion that "the rule stands" does not discharge this AC. If the re-check returns the opposite, the marker is written and this AC is reported as inverted-by-measurement. |
| AC6 | **AMENDED 2026-08-02 (owner ruling, mid-Step-9.5) — see § *Amendments* A1. Reports as "AMENDED, and the amendment is recorded", NEVER as PASS.** `git diff --name-only main` lists `ai-docs/learnings.md`, the spec/design plan files, and **only** the closed enumerated path list in A1. There is still no authorised definition-document edit, and the A1 prohibition (no widening to accommodate an artefact that should not exist) stands unchanged. Any path outside the enumerated list **reds AC6 unconditionally**: the run stops and surfaces it rather than extending the list. The three Step-9.5 content docs are not in the carve-out and are skippable only on a measured, recorded, per-file vacuity check (A1). |
| AC7 | No line of any existing entry other than its `**Superseded by:**` line differs from `main`. Verify by a diff filtered to context, e.g. `git diff -U0 main -- ai-docs/learnings.md` shows only `+**Superseded by:**` additions plus the E2 entry's added block — zero removals. |
| AC8 | Every `[ref]` written resolves under `.claude/agents/learnings-escalation-audit.md:81`'s resolver: the date matches at least one **other** entry AND the disambiguation slug appears in that entry's description. Demonstrate by running the resolution for each marker. |
| AC9 | The report carries the § *Findings to report* items: the post-run population count (measured, per-record parse, stated as a number), the AC4 off-list count, the two stale refs from AC1, and any relationship that fit none of the five verbs. |
| AC10 | The report surfaces the `:751` **reachability gap** (§ *KD-5 derivation* (b)) as an unresolved finding: a later event invalidated a `What happened:` fact while the `Rule:` stands, no forward link exists, and the relationship fits none of the five verbs. **No sixth verb and no sixth field is invented** to close it. The E2 backward citation is reported as partial mitigation, explicitly **not** as a fix. |
| AC11 | The field-scope test (§ *KD-5 derivation* (a)) is re-applied to C1 and C4–C8 during the run, and its per-candidate result is reported. Any candidate that fails it is left unmarked and reported, never forced. |

## Findings to report

These are **observations for the run's report**, not rules and not escalation candidates. Do not
write any of them into an instruction file, and do not add a `learnings.md` entry for them (the E2
entry is the only new entry this task creates).

1. **Population growth.** Before this run the field is carried by **2 of 123** entries. This run
   roughly triples that (estimated ~8; the true figure is whatever the sweep produces). **State the
   count you actually wrote** — re-derived by per-record parse after the last edit of the run, never
   copied from this spec. Whether a freeform prose reason slot holds up at that population is now
   **measurable at the next reading of the file**; this run does not answer it, and no rule is
   proposed from it.
2. **Off-list relationship count** (AC4) — the number measuring whether the candidate list was a
   floor or the whole scope, with the entry pair for each.
3. **The two stale refs already found** (AC1): candidate ref `:716` → the heading is at `:715`; and
   `:751`'s *"No such gate exists"*, true at self-review time and false at commit time.
4. **Any relationship fitting none of the five verbs** (Technical constraint 4), if one arises.
5. **The `:751` reachability gap** (§ *KD-5 derivation* (b), AC10) — reported as unresolved, with the
   three mechanisms that fail and why, and the E2 backward citation named as partial mitigation only.
6. **Per-candidate field-scope test results** for C1 and C4–C8 (AC11), including any candidate that
   fails and is therefore left unmarked.
7. **A refuted axis, recorded so it is not re-proposed:** "later RULING vs later COMMIT" is *not* the
   discriminator — `ai-docs/corrections-log.md:49` admits `PR #N` as a `[ref]`, so a commit supersedes
   an entry just as a later entry does.

## Open questions

**Q1 — CLOSED (round 1).** *"How far may a grammar-extension edit reach?"* — the question dissolved
rather than being ruled on: its premise (the field is a bare pointer) was refuted by measurement. See
KD-6. Recorded here so a later reader does not re-open it as an unresolved scope call.

**Q2 (non-blocking, deferred).** Whether the `corrections-log.md:39` ↔ `self-improve.md:264` placement
contradiction (§ *Deferred*) warrants a filed issue, or is small enough to fold into the next
`/ai-audit` Phase 1. Resolved by KD-3 for this task either way; the filing decision is post-merge.
