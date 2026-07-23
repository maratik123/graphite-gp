# Design: Self-reflection retrospective + clean-context eval methodology

**Issue:** #80 (Part A) + #114 (Part B) — combined spec/PR
**Date:** 2026-07-23

## Approach

A single harness/process change closing two related refinements of the
self-learnings loop. The whole diff is prose in `.claude/**` + `ai-docs/**`;
**no game code** (`crates/**` untouched). All edits are authored **in-thread**
by the orchestrator — see § Handoff plan for why a background subagent is
structurally wrong here.

### Part A (#80) — parent-facing clean-context eval ordering

`improve/SKILL.md` item 6 is today a one-liner: *"Run a targeted eval to verify
the fix works. Reports PASS/FAIL."* [measured: `Read .claude/skills/improve/SKILL.md:18` → that line]. Part A expands **only that item** to
state the parent-thread procedure explicitly:

1. apply the approved proposals to the **working tree**, then
2. dispatch the clean-context reproducers in fresh contexts (the parent holds
   `Agent` and owns the dispatch — the subagent only *assembles* the reproducer
   blocks), then
3. let the eval **gate the commit**: PASS proposals commit; a FAIL **reverts the
   failing proposal from the working tree before it lands** and loops back to
   Step 3 to strengthen the rule and re-run the eval.

**Reference, do not duplicate (AC2).** Item 6 points at `self-improve.md § Step
6` for the reproducer template, the clean-context requirement, the forbidden
degraded paths (`Bash`-shelled invocation, `TaskCreate`-poll, in-memory
close-read), and the parent-dispatch contract — it must **not** restate any of
them. The cross-reference uses the existing **prose `§` style** the file already
uses for sibling references (`self-improve.md § Step 1c`, `§ Step 2c`, `§
Anti-patterns`) — **not** a `[](#anchor)` markdown link, so the doc-gate
intra-doc-link check does not apply and cannot break [measured: `Read
.claude/skills/improve/SKILL.md:24,26,52` → all sibling refs are bare `§` prose].
The referent section exists: `self-improve.md` heading `### Step 6: Eval
(REQUIRED after Step 5)` [measured: `Read .claude/agents/self-improve.md:261` →
that heading].

**Rejected alternative:** inlining the reproducer template into `improve/SKILL.md`
— rejected because it duplicates the subagent-side contract (AC2 violation) and
grows a file that item 6 exists to keep thin. `self-improve.md § Step 6` is
already complete on the subagent side; Part A is purely the parent-facing wording.

### Part B (#114) — self-reflection retrospective

A new **`/reflect` skill + `self-reflect` subagent** pair, mirroring the
`/improve` + `self-improve` split verbatim in shape:

- **`self-reflect` subagent** (`.claude/agents/self-reflect.md`, `model: opus`)
  reads the work transcript and emits a **structured good/bad list** — empty
  permitted — where each finding names a **concrete moment**, carries exactly one
  route `{learnings | ticket | none}` chosen by the cost/value rubric below, and a
  one-line justification. It **assembles/proposes only** and **yields to the
  parent**; it issues **no** `AskUserQuestion` and performs **no** project-side
  write, exactly as `self-improve` yields its candidates (`self-improve.md § Step
  2c`, § Step 6 pause-and-surface).
- **`/reflect` skill** (`.claude/skills/reflect/SKILL.md`, parent thread) launches
  the subagent, then surfaces **per-finding routing consent** via parent-side
  `AskUserQuestion` before any project-side write — the same
  structured-output-plus-parent-surfacing pattern `interview/SKILL.md` and
  `improve/SKILL.md § Auto-memory consent gate` use.

**Trigger predicate — standalone `/reflect` only (AC4).** Reflection fires exactly
when the user invokes `/reflect`. It does **NOT** auto-fire from `/task`,
`/improve`, or `/bugfix` — **none of those files is modified** by this task. The
`/reflect` frontmatter carries `disable-model-invocation: true` (mirroring
`improve/SKILL.md:4`) so the model cannot self-invoke it; only an explicit user
`/reflect` runs it. Fully opt-in.

**New files, not inlining (char-band).** AGENTS.md is 35,008 chars and
`self-improve.md` 34,752 — both already inside the 35k–40k "minor" warning band
[measured: `wc -c AGENTS.md .claude/agents/self-improve.md` → 35008 / 34752]. The
reflection machinery therefore lands in **two new files**, each of which must stay
< 40,000 (target < 35,000). Neither `AGENTS.md` nor `self-improve.md` receives any
Part-B *body* text.

#### Cost/value rubric (resolves Open question 1)

For **each** finding, evaluate in order and take the first match:

1. **`none`** — the finding names **no concrete moment**, OR the insight is
   **already covered** by an existing rule (`AGENTS.md` / a Skill / a Subagent) or
   an existing `ai-docs/learnings.md` entry. Acknowledge and **record the decision
   in the emitted list** (do not filter it out); no write.
2. **`ticket`** (costly) — fixing/adopting the finding needs a **substantial
   process/harness change**: it spans **multiple instruction files**, introduces
   or reshapes a **skill / subagent / hook**, or is otherwise **cross-cutting**
   enough to warrant the full spec→design→implement→review workflow. File a
   `process`-labelled gh issue at **pattern altitude**.
3. **`learnings`** (cheap — the **default** for most findings) — the insight is a
   single, well-scoped, **append-only `learnings.md`-sized** observation: a
   one-turn wording nuance, a keep-doing validation, or a one-file behaviour
   correction. Append one well-formed entry.

The three routes are fixed by #114; the rubric is the concrete decision tree that
assigns them. It is a **defensible default**, not a hard gate — the subagent
proposes, the user confirms per finding.

#### Per-route write contracts

- **`learnings`** — a **well-formed** `ai-docs/learnings.md` entry: `Kind:
  validation` for a good/keep-doing finding, `Kind: correction` for a
  bad/stop-doing one; `Escalated? no`; appended at end. Both files **reference,
  never restate**, `AGENTS.md § Learning Log` Boundary rule 1 (append-only) and
  Boundary rule 2 (no same-turn instruction-file edit). **Boundary rule 2 is
  satisfied by construction**: `/reflect`'s only writes are learnings appends, gh
  issues, and none-records — it **never edits an instruction file**, so it can
  never trip the same-turn rule, and the in-flow `/task` Steps 8–12 carve-out is
  irrelevant to it (referenced, not restated).
- **`ticket`** — a `process`-labelled gh issue, filed **by the parent thread**
  after consent (mirroring `/improve`'s parent-side apply). If the issue body text
  contains the substring `git commit`, use `gh issue create --body-file <path>`,
  **not** inline `--body` (AGENTS.md § Workflow — the commit-block hook matches
  `git[[:space:]]+commit` inside a `--body` argument).
- **`none`** — the acknowledge-and-drop decision is recorded in the **ephemeral
  `/reflect` run report** (the emitted good/bad list, retained with its
  justification and surfaced to the user in the consent pass — **not** silently
  dropped). **Owner-fixed: ephemeral only** — "on record" means the run's
  output/report surfaced to the user; the `none` route is **NOT** persisted to any
  file (YAGNI). No durable audit-trail file is written.

**Threshold interaction (AC9).** Reflection-sourced `learnings` entries **feed**
the existing `/improve` run threshold (≥3 unescalated corrections / ≥2 unescalated
validations) — they do **not** bypass it, and `/reflect` **never** itself
escalates into instruction files. This is a reference to the existing threshold
line (`AGENTS.md § Learning Log`, `improve/SKILL.md:20`), not a re-tuning.

**Anti-theater guard (AC5/AC8).** The good/bad list MAY be empty (a valid,
expected outcome — not a failure). Every **non-empty** finding MUST name a
concrete moment, carry exactly one route, and a one-line justification; `none`
findings are recorded, never silently dropped.

#### Open question 2 — no AGENTS.md sync-group row (resolved: **do not add one**)

Add **no** dedicated `/reflect` ↔ `self-reflect` row to AGENTS.md § Propagation
Rule. Rationale, load-bearing fact first:

- The pair it would "mirror" — `/improve` ↔ `self-improve` — has **no dedicated
  sync-group row of its own** [measured: `grep -n "self-improve\|Tool/Subagent/Skill/Hook contract" AGENTS.md` → the only
  `self-improve` mention in the Propagation Rule is the *Learning-Log group* row
  (about the AGENTS.md § Learning Log **section**, not the skill↔subagent pair);
  the pair is coupled solely by the catch-all row *"Any edit that changes a
  Tool/Subagent/Skill/Hook contract → update `ai-docs/claude-tools-hierarchy.md`"*
  (AGENTS.md:184) plus the general grep-based Procedure]. A dedicated row for
  `/reflect` ↔ `self-reflect` would be **asymmetric with its own template**.
- AGENTS.md is at 35,008 chars — a row would push it further into the warning band
  for **zero coverage gain** (the catch-all row already fires).

So the coupling is carried by the **existing** catch-all row (→ subtask 5,
`claude-tools-hierarchy.md`) + subtask 4's primitive-block update + the grep
Procedure — identical to how `/improve` ↔ `self-improve` is coupled today.
**AGENTS.md receives zero edits in this task**, which also removes the only
AGENTS.md char-band risk. The `claude-tools-hierarchy.md` update (AC10) is required
either way and is subtask 5.

**Counter-consideration (recorded so it is not re-litigated).** A dedicated row IS
the **dominant** table pattern, not just an available one: `interview` ↔
`spec-writer`, `triage` ↔ `triage-runner` ↔ `next`, `project-review` ↔
`review-findings` ↔ `self-review`, and `task` ↔ `design`/`design-review`/`context-reset`
all carry explicit Propagation-Rule sync-group rows [measured: `Read AGENTS.md`
§ Propagation Rule (≈ lines 171–180) → those four groups each have a dedicated
row]; `/improve` ↔ `self-improve` is the **minority** no-row case. This design
**deliberately** follows the minority pattern: it trades the dominant pattern's
explicitness for **AGENTS.md char-band headroom** (35,008 chars, already in the
warning band), given that the coupling is already carried by the catch-all
Tool/Subagent/Skill/Hook-contract row + subtask 4 (primitive block) + subtask 5
(inventory) + the grep Procedure. The decision stands; a future reviewer weighing
"but most pairs have a row" should read this paragraph rather than reopen it.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Part A.** Expand `improve/SKILL.md` item 6 to the working-tree-apply → clean-context-eval → commit-gated-on-PASS parent-thread procedure (PASS commits; FAIL reverts before landing + loops to Step 3); cross-reference `self-improve.md § Step 6` in prose `§` style with **no duplication** of the reproducer template / clean-context rule / forbidden-degraded-path list. | `.claude/skills/improve/SKILL.md` | — |
| 2 | **Part B — subagent.** Create `self-reflect.md`: `model: opus`; do-not-write-code + assemble-and-yield contract (no `AskUserQuestion`, no project-side write — mirror `self-improve`); structured good/bad list (empty permitted; concrete-moment + route + one-line justification per finding); the cost/value rubric (§ Approach); per-route write contracts (learnings `Kind`/`Escalated? no` + **reference** Boundary rules 1&2; ticket `process`-label + pattern altitude + `--body-file` guard; none = recorded-not-dropped); anti-theater guard; threshold-feed note; report shape yielded to parent. | `.claude/agents/self-reflect.md` (new) | — |
| 3 | **Part B — skill.** Create `reflect/SKILL.md`: frontmatter `disable-model-invocation: true` + `argument-hint`; trigger predicate = explicit `/reflect` only, NO auto-fire in `/task`/`/improve`/`/bugfix` (those unmodified); launch `self-reflect`; parent-side per-finding `AskUserQuestion` consent (batched ≤4 / sequential >4, mirroring `improve/SKILL.md § Auto-memory consent gate`) before any write; write guard; threshold-feed note; `$ARGUMENTS`. | `.claude/skills/reflect/SKILL.md` (new) | 2 |
| 4 | **Propagation — primitive block.** Additively add `/reflect` to the *Slash commands* list and `self-reflect` to the *Agent stems* list of the `auto-memory-primitive-keywords` block in `self-improve.md § Step 1c` (its own rule: *"A new Skill / Subagent … requires an additive update to this block"*). Re-measure `self-improve.md` < 35,000. | `.claude/agents/self-improve.md` | 2, 3 |
| 5 | **Propagation — inventory (AC10).** Add a `self-reflect` row to the Subagents table and `/reflect` to the Skills list of `claude-tools-hierarchy.md` (the catch-all Tool/Subagent/Skill/Hook-contract Propagation row). | `ai-docs/claude-tools-hierarchy.md` | 2, 3 |
| 6 | **Plan row (AC10).** Add the `2026-07-23 · self-reflection-eval-methodology` row to `ai-docs/plans/INDEX.md`. | `ai-docs/plans/INDEX.md` | — |

Scope: 6 subtasks (well under the 15-task split threshold). No AGENTS.md edit
(Open question 2). All change-type **instructions/harness** (`.md` + `.claude/**`
+ `ai-docs/**`); **zero `.rs`**.

## Handoff plan

**M = 6.** All six subtasks are the **same change-type — instructions/harness**
(`*.md`, `.claude/**`, `ai-docs/**`; no `*.rs`), so group-minimization packs them
into the **fewest groups possible = one** (≤ 10 subtasks, homogeneous, no
change-type switch, dependency order preserved: 1 · 2 · 3 · 4 · 5 · 6, with 3→{4,5}
and 2→{3,4,5} respected within the group).

- **Group A** — **instructions/harness** change-type → implementor **`opus`**,
  effort **inherited from the orchestrator (typically xHigh) — NOT pinned**,
  1M-token window — subtasks **1, 2, 3, 4, 5, 6**. Terminal group (6 subtasks;
  within the `1..=10` range). Single group ⇒ default max-4-groups not approached.
- **Entry handoff:** at the start of Group A, spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). One
  group ⇒ no inter-group handoff; Group A completes `/task` Step 8 in its own
  `/context-reset` subagent.

**In-thread authoring override — binding, per AGENTS.md § Workflow delegation
rule (b) Environment fit.** The instructions/harness marker's *default* implementor
route is `subagent_type="general-purpose"` + `model="opus"` (design.md § Rules (g)).
Here that route is **overridden to in-thread orchestrator authoring**: every file
in Group A is a **protected self-modification target** (`.claude/**`,
`ai-docs/**`) whose `Edit`/`Write` triggers a self-modification permission prompt
that a **background** subagent cannot answer — the edit **fails closed** regardless
of any `Edit(...)` allow-list. The `opus`/xHigh implementor profile still applies;
the *executor* is the orchestrator thread, **not** a spawned `general-purpose`
subagent, and **not** `code-writer` (there is no code to delegate). This matches
both issues' explicit "implement in-thread, never via `code-writer`" instruction
and the spec's Implementer key-decision. [derived → design-review confirms the
marker+override wording against design.md § Rules (g) and AGENTS.md § Workflow
delegation rule (b)]

## Risks

- **Duplication of the subagent-side contract (AC2 for Part A; Boundary rules for
  Part B).** Item 6 or the new files could restate the reproducer template /
  forbidden-degraded-path list / Boundary rules instead of referencing them —
  inflating files and forking a contract that then drifts. Mitigation: design
  mandates prose `§` references (Part A → `self-improve.md § Step 6`; Part B
  learnings route → `AGENTS.md § Learning Log` Boundary rules 1&2), and
  design-review/self-review check for restated blocks. — [derived → self-review AC2/AC6 read-through + `wc -c` of both new files < 40k]
- **Char-band regression.** Only `self-improve.md` gains body text (subtask 4, a
  two-token additive edit ≈ +25 chars → ≈ 34,777, still < 35,000); AGENTS.md is
  untouched (Open question 2); the two new files must each land < 40,000 (target <
  35,000 — both are leaner than their `improve`/`self-improve` templates, which
  are 6,516 / 34,752). — [measured: `wc -c` template baselines 6516 / 34752 / 35008] [derived → per-file `wc -c` gate at each subtask's completion]
- **Propagation drift — new Skill/Subagent contract.** Missing either the
  `claude-tools-hierarchy.md` inventory (subtask 5) or the
  `auto-memory-primitive-keywords` block update (subtask 4) leaves the pair
  undetectable to a future `/improve` Step-1c sweep / the inventory audit.
  Mitigation: both are explicit subtasks; close the PR with the grep Procedure
  (`grep -rn "reflect" .claude/ ai-docs/ AGENTS.md`). — [derived → close-of-task Propagation grep]
- **`gh … --body` commit-block hook false-positive on the `ticket` route.** A
  reflection ticket whose body mentions `git commit` is blocked by the
  `git[[:space:]]+commit` matcher. Mitigation: `self-reflect.md` + `reflect/SKILL.md`
  instruct `--body-file`, referencing AGENTS.md § Workflow. — [measured: `Read AGENTS.md` § Workflow → the `--body-file` rule for `git commit` substrings]
- **Same-turn Boundary rule 2 on the `learnings` route.** `/reflect` is a
  standalone skill, **not** `/task` Steps 8–12, so the in-flow carve-out does not
  apply. Mitigation: `/reflect`'s writes are limited to learnings appends / gh
  issues / none-records and it **never edits an instruction file**, so Boundary
  rule 2 holds by construction; both files state this and reference (not restate)
  the rule. — [derived → self-review AC6/AC9 read-through]
- **Name clash for the new primitives.** Mitigation: none observed — [measured: `ls .claude/skills .claude/agents | grep -i reflect` → empty; `grep -rn "reflect\|self-reflect" .claude ai-docs/claude-tools-hierarchy.md` → only the English word "reflect", no primitive].

## Test Design

This task changes **instruction prose only** — there is **no Rust code and no
`#[cfg(test)]` module** to add; the standard AGENTS.md § Build & Test gates
(`cargo build/test/clippy/fmt/doc`, `actionlint`) have **no delta to exercise**
(no `.rs`, no `.github/workflows/*.yml` touched). Verification is therefore the
**per-AC command/read checks** below, run by the implementor and confirmed by
design-review → self-review.

- **Location / entry point:** the six edited files; verification is by
  command + close-read, not a test binary.
- **AC1 / AC2 (Part A):** read `improve/SKILL.md` item 6 — the working-tree-apply
  → clean-context-eval → commit-gated-on-PASS ordering (PASS commits; FAIL reverts
  before landing + loops to Step 3) is explicit; grep item 6 confirms it carries a
  `self-improve.md § Step 6` reference and **no** copy of the reproducer template /
  forbidden-degraded-path list. [derived → self-review read-through]
- **AC3:** `wc -c .claude/skills/improve/SKILL.md .claude/agents/self-improve.md`
  — both < 40,000; the two Step-6 descriptions are read side-by-side for
  non-contradiction (Propagation satisfied). [derived → `wc -c` + read gate]
- **AC4:** read `reflect/SKILL.md` — trigger predicate is explicit-`/reflect`-only
  with `disable-model-invocation: true`; `grep -rn "reflect" .claude/skills/task
  .claude/skills/improve .claude/skills/bugfix` returns **no** auto-fire wiring
  (those files unmodified — confirm via `git diff --name-only`). [derived → grep + `git diff --name-only`]
- **AC5 / AC8:** read `self-reflect.md` — structured good/bad list (empty
  permitted); each finding = concrete moment + one route `{learnings|ticket|none}`
  + one-line justification; `none` findings are recorded-not-dropped **in the
  ephemeral run report only** (owner-fixed; confirm no durable-`none`-file write is
  specified); and `reflect/SKILL.md` surfaces per-finding consent via parent-side
  `AskUserQuestion` before any write. [derived → self-review AC5/AC8 read-through]
- **AC6:** read the `learnings` route — correct `Kind:` per polarity, `Escalated?
  no`, and Boundary rules 1&2 **referenced not restated** (grep the two new files
  for any verbatim copy of the Boundary-rule bodies → none). [derived → grep + read]
- **AC7:** read the `ticket` route — `process` label, pattern altitude,
  `--body-file` guard present. [derived → read]
- **AC9:** read both files — reflection `learnings` entries **feed** the ≥3/≥2
  `/improve` threshold and never bypass/auto-escalate; no instruction-file-edit
  path exists in `/reflect`. [derived → read + the AC6 no-instruction-file-edit check]
- **AC10:** `claude-tools-hierarchy.md` carries the `self-reflect` row + `/reflect`
  skill entry; `ai-docs/plans/INDEX.md` carries the plan row; `git diff --stat
  AGENTS.md` is empty (no row added ⇒ no char-band change); every touched/created
  instruction file `wc -c` < 40,000. [derived → `git diff --stat` + `wc -c` gate]
- **AC11:** `git diff --name-only main...HEAD` lists only `.claude/**` +
  `ai-docs/**`; **zero `crates/**`**; authored in-thread (no `code-writer` spawn in
  the transcript). [derived → `git diff --name-only` + transcript]
- **Fixtures/helpers:** none (prose change).
- **Optional:** a dual-model instruction-file clarity read of the two new files
  (`ai-docs/instruction-file-validation.md`) — recommended but not an AC gate.

## Open questions

- **RESOLVED (owner) — `none`-route persistence is EPHEMERAL.** AC8's "on record"
  is satisfied by the `none` finding being recorded in the **ephemeral `/reflect`
  run report** surfaced to the user (retained, not silently dropped). It is **NOT**
  persisted to any file — no durable audit trail is built (YAGNI). Fixed decision;
  no spec amendment. Reflected in § Approach → Per-route write contracts and the
  AC5/AC8 checklist.

No open questions remain.
