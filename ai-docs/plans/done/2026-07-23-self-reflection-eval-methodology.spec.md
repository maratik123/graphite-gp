# Self-reflection retrospective + clean-context eval methodology

**Source:** issues #80, #114
**Date:** 2026-07-23
**Tracked in:** #80, #114

This is a **combined** spec closing two related refinements of the self-learnings loop (`/improve` + `ai-docs/learnings.md`). Both are harness/process changes touching `.claude/**` + `ai-docs/**` only — **no game code**. Per both issues' notes, implement **in-thread**, never via `code-writer` (the diff is predominantly prose in protected instruction files; a background subagent cannot answer the self-modification permission prompt).

The two parts are independent in substance but share one surface (the eval/routing machinery of the learning loop), so they land in one PR:

- **Part A (#80)** — formalize the *parent-facing* commit-gating ordering of the `/improve` Step 6 clean-context eval.
- **Part B (#114)** — add a *self-reflection retrospective* step that, at the end of a unit of work, produces a structured good/bad assessment and routes each finding to one of {learnings | ticket | none}.

## Scope

### Part A — clean-context eval methodology (#80)

1. Expand `.claude/skills/improve/SKILL.md` item 6 (currently the one-liner *"Run a targeted eval to verify the fix works. Reports PASS/FAIL"*) to state the **parent-thread procedure** explicitly:
   - apply the approved proposals to the **working tree**, then
   - dispatch the clean-context reproducers in fresh contexts (per `self-improve.md` § Step 6's assemble-and-yield contract — the subagent assembles reproducer blocks; the parent, which holds `Agent`, dispatches them), then
   - let the eval **gate the commit**: PASS proposals commit; a FAIL reverts the failing proposal from the working tree **before it lands** and loops back to Step 3 to strengthen the rule and re-run Step 6.
2. **Cross-reference, do not duplicate.** Item 6 points at `self-improve.md` § Step 6 for the reproducer template, the clean-context requirement, the forbidden degraded paths (`Bash`-shelled invocation, `TaskCreate`-poll, in-memory close-read), and the parent-dispatch contract — it must not restate them.
3. Run the Propagation Rule check for `improve/SKILL.md` item 6 ↔ `self-improve.md` § Step 6 so the two sides of the same contract do not drift.

### Part B — self-reflection retrospective (#114)

4. Add a **self-reflection retrospective** step. At its defined trigger it produces a **structured good/bad assessment** and, for EACH finding, chooses exactly one of three consequent actions by an explicit cost/value rubric:
   - **`learnings`** (cheap) — append a well-formed `ai-docs/learnings.md` entry. `Kind: validation` for a good/keep-doing finding, `Kind: correction` for a bad/stop-doing one; `Escalated? no`; honoring the append-only Boundary rule 1 and the same-turn Boundary rule 2 (no instruction-file edit in the same turn — with the existing in-flow `/task` Steps 8–12 carve-out being the only exception). The default route for most findings.
   - **`ticket`** (costly) — file a `process`-labelled gh issue at pattern altitude for a substantial process/harness change warranting the full spec→design→implement→review workflow, rather than a one-line learning.
   - **`none`** — acknowledge and drop, **explicitly recorded** so "no action" is a decision on record (anti reflection-theater).
5. **Trigger predicate — standalone `/reflect` only.** Reflection fires exactly when the user invokes a new `/reflect` skill explicitly (like `/improve`). It does **NOT** auto-fire at the end of `/task`, `/improve`, or `/bugfix` — none of those workflows is modified to trigger it. Fully opt-in. The trigger predicate is documented in the new `/reflect` skill file.
6. **Runner — `/reflect` skill + `self-reflect` subagent**, mirroring the `/improve` + `self-improve` split:
   - the **`self-reflect` subagent** reads the work transcript and produces the structured good/bad list, each finding carrying a `{learnings | ticket | none}` route and a one-line justification (it assembles/proposes only — analogous to `self-improve` yielding to the parent);
   - the **`/reflect` skill** (parent thread) surfaces the routing consent to the user before any project-side write, exactly as `/improve` surfaces `self-improve`'s candidates via parent-side `AskUserQuestion`.
7. Specify the **cost/value rubric** — the concrete cheap-vs-costly-vs-nothing test. (Shape is fixed by the three routes above; the routing decision itself is the deliverable, the good/bad list is its input.) Detailed rubric wording is left to the design phase.
8. **Anti-theater guard.** The good/bad list MAY be empty (an empty list is a valid, expected outcome — not a failure). Every non-empty finding MUST name a **concrete moment** (a specific decision/action in the work unit), carry a `{learnings | ticket | none}` route, and a one-line justification.
9. **Threshold interaction.** Reflection-sourced `learnings` entries **feed** the existing `/improve` run threshold (≥3 unescalated corrections / ≥2 unescalated validations) — they do NOT bypass it. Reflection never itself escalates into instruction files.
10. **Reconcile with existing surfaces** (`/improve`, the `learnings.md` Boundary rules, the skill/agent layout) rather than duplicating them; satisfy all Propagation-Rule / sync-group obligations for whatever files the runner decision touches.

## Out of scope

- Any change to game code (`crates/**`). This is `.claude/**` + `ai-docs/**` only.
- Auto-invocation wiring beyond the chosen trigger predicate (e.g. hooks that force reflection) — the trigger predicate is documented procedure, not a new enforcement hook, unless the design surfaces a concrete need.
- Changing the `/improve` run threshold values themselves (≥3 / ≥2) — Part B feeds that queue, it does not retune it.
- Reworking the auto-memory companion sweep (Step 1c) or the Carrot/Correction pass mechanics of `self-improve.md`.
- Building an eval harness / new tooling for Part A — it reuses the existing `self-improve.md` § Step 6 assemble-and-dispatch machinery verbatim; Part A is purely the parent-facing wording.

## Deferred
- Auto-invoke of reflection from a `Stop`/`SubagentStop` hook | would make reflection non-optional and needs its own enforcement design | separate issue if the trigger predicate proves too easy to skip in practice.

## Key decisions

| Question | Decision |
|---|---|
| Combined spec / PR for #80 + #114? | Yes — user explicitly chose "combined: one spec/PR". Both refine the `/improve` + `learnings.md` loop; #114 references #80. `**Tracked in:**` names both. |
| Part A — where does the edit land? | `.claude/skills/improve/SKILL.md` item 6 only (the parent-facing side). `self-improve.md` § Step 6 is referenced, not edited — its subagent-side contract is already complete (issue #80 § Background confirms `:271`/`:273`/`:275`/`:279`–`:297`/`:313`). |
| Part A — duplication vs reference | Reference `self-improve.md` § Step 6; do NOT restate the reproducer template or clean-context rules. Keeps both files off the 40k cap. |
| Part B — trigger scope (which workflows fire reflection) | **Standalone `/reflect` only** (Q1, round 1). Explicit user invocation; NO auto-fire in `/task` / `/improve` / `/bugfix` — those files are not modified. Fully opt-in. |
| Part B — runner (in-thread vs `self-reflect` subagent vs standalone `/reflect` skill+subagent) | **`/reflect` skill + `self-reflect` subagent** (Q2, round 1), mirroring `/improve` + `self-improve`: subagent produces the good/bad list + per-finding routing; parent skill surfaces routing consent via `AskUserQuestion`. New files → no char-band pressure on existing near-35k files. |
| Part B — anti-theater | Empty good/bad list is valid; each finding names a concrete moment. Fixed by #114; no design latitude. |
| Part B — threshold interaction | Reflection `learnings` entries feed the `/improve` queue; never bypass or auto-escalate. Fixed by #114. |
| Part B — `learnings` route boundary compliance | `Kind` correct per polarity, `Escalated? no`, append-only + same-turn boundary rules honored. Fixed by #114 + AGENTS.md § Learning Log. |
| Implementer | In-thread (prose diff in protected `.claude/**` files); NOT `code-writer`. Fixed by both issues' notes + AGENTS.md delegation rule. |

## Technical constraints

- **Char-band pressure drove the runner decision.** `AGENTS.md` is already **35008 chars** (in the 35,000–39,999 "minor" warning band) and `.claude/agents/self-improve.md` is **34752 chars** (approaching it). The AGENTS.md AXIOM caps every per-invocation instruction file at **40,000 chars**. Part B's reflection machinery therefore lands in **new files** (`.claude/skills/reflect/SKILL.md` + `.claude/agents/self-reflect.md`) — no inlining into `AGENTS.md` / `self-improve.md`. Each new file must itself stay < 40k (target < 35k). Any incidental edit to an existing near-band file (e.g. a Propagation-Rule row in `AGENTS.md`) must be re-measured to confirm it stays < 40k. `improve/SKILL.md` (6516 chars) has ample room for the Part A edit.
- **Part B must not duplicate the Boundary rules.** The `learnings` route obeys — and references, rather than re-states — AGENTS.md § Learning Log Boundary rule 1 (append-only) and Boundary rule 2 (no same-turn instruction-file edit). The in-flow `/task` Steps 8–12 carve-out is the only same-turn exception and already exists.
- **Sync-group / Propagation Rule for the new skill+subagent pair.** Creating `/reflect` + `self-reflect` is an edit that **changes a Skill/Subagent contract**, so AGENTS.md § Propagation Rule requires updating **`ai-docs/claude-tools-hierarchy.md`** in the same PR (the project Tool/Subagent/Skill/Hook inventory). The design must also evaluate whether a new sync-group row belongs in AGENTS.md § Propagation Rule mirroring the `/improve` ↔ `self-improve` relationship — weighed against the AGENTS.md char-band (prefer the lightest sufficient coupling; if a row is added, re-measure AGENTS.md < 40k). `ai-docs/plans/INDEX.md` gets the plan row per the interview flow.
- **`gh … --body` hook interaction.** The `ticket` route files a gh issue; if any body text contains the substring `git commit`, use `--body-file`, not inline `--body` (AGENTS.md § Workflow).
- Both issues instruct writing **at pattern altitude**: survey the then-current `/improve`, `learnings.md` Boundary rules, and skill/agent layout at implementation time rather than trusting any specific `file:line` named here — concrete names in this spec are orientation, not ground-truth.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `.claude/skills/improve/SKILL.md` item 6 makes the **working-tree-apply → clean-context-eval → commit-gated-on-PASS** ordering explicit parent-thread procedure (PASS commits; FAIL reverts before landing + loops back to Step 3). |
| AC2 | Item 6 cross-references `self-improve.md` § Step 6 for the reproducer/dispatch/clean-context contract with **no duplication** of the template or the forbidden-degraded-path list. |
| AC3 | After Part A, both `improve/SKILL.md` and `self-improve.md` remain < 40,000 chars, and their Step-6 descriptions do not contradict each other (Propagation Rule satisfied). |
| AC4 | A new `.claude/skills/reflect/SKILL.md` (the `/reflect` skill) documents the **trigger predicate**: reflection runs on explicit `/reflect` invocation only, with NO auto-fire in `/task` / `/improve` / `/bugfix` (those files are unmodified). |
| AC5 | A new `.claude/agents/self-reflect.md` (the `self-reflect` subagent) emits a **structured good/bad list** (empty permitted); each finding names a concrete moment, carries exactly one of `{learnings \| ticket \| none}`, and a one-line justification. The `/reflect` skill surfaces the per-finding routing consent to the user (parent-thread `AskUserQuestion`) before any project-side write, mirroring `/improve`. |
| AC6 | The `learnings` route produces a **well-formed** `ai-docs/learnings.md` entry: correct `Kind:` for the finding's polarity, `Escalated? no`, honoring append-only (Boundary rule 1) + same-turn (Boundary rule 2) boundary rules. |
| AC7 | The `ticket` route files a **`process`-labelled** gh issue at pattern altitude (spec→design→implement→review-worthy findings only). |
| AC8 | The `none` route **records the decision** — no silent drops; "no action" appears on record. |
| AC9 | Reflection-sourced `learnings` entries **feed** the `/improve` ≥3-correction / ≥2-validation threshold and never bypass it or auto-escalate into instruction files. |
| AC10 | Propagation-Rule / sync-group obligations for the new skill+subagent pair are satisfied in the same PR: **`ai-docs/claude-tools-hierarchy.md`** is updated (new Skill/Subagent contract), `ai-docs/plans/INDEX.md` carries the plan row, and any Propagation-Rule sync-group row added to `AGENTS.md` keeps it < 40,000 chars. Every touched/created instruction file stays < 40,000 chars. |
| AC11 | No game code changed; the diff is confined to `.claude/**` + `ai-docs/**` and was authored in-thread (not via `code-writer`). |

## Open questions

- **Cost/value rubric — exact wording of the cheap-vs-costly-vs-nothing test.** The three routes are fixed (#114); the precise decision tree (e.g. "costly = multi-file / new subagent / cross-cutting; cheap = single-turn wording or behavior fix; nothing = no concrete moment or already covered by an existing rule/entry") is a design-phase concern with a defensible default. Left for the `design` Subagent.
- **Whether a new sync-group row in AGENTS.md § Propagation Rule is warranted** for the `/reflect` ↔ `self-reflect` pair (mirroring the `/improve` ↔ `self-improve` coupling), versus relying on the general grep-based Propagation Procedure. A row makes the coupling explicit but competes with the AGENTS.md char-band (35008 → risk of crossing 40k). Design decides; the `claude-tools-hierarchy.md` update (AC10) is required either way.
