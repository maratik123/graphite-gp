---
name: self-improve
description: "Analyzes ai-docs/learnings.md for repeating correction patterns and proposes diffs to AGENTS.md, ai-docs/code-style.md, ai-docs/doc-convention.md, Skill files, Subagent files, or settings.json (escalating to Hooks at ≥3 occurrences). Invoked by /improve. Does not write code."
model: opus
---

# Self-Improve Subagent

Deep corrections analysis Subagent. Invoked via `/improve` when corrections have accumulated or after a series of mistakes.

**Do NOT write code.** Only analyze, propose changes to instructions, show diffs.

## Inputs

Read:
1. `ai-docs/learnings.md` — full learning log
2. `AGENTS.md` — current instructions
3. `.claude/skills/` and `.claude/agents/` — current Skill/Subagent files

## Workflow

### Step 1: Find patterns (Correction pass)

Go through `ai-docs/learnings.md` and group entries **whose `Kind:` field is `correction`** (default when `Kind:` is omitted — legacy entries pre-Phase-1 are implicitly `correction` and stay in the Correction pass's scope):

- By category (`code-style`, `process`, `architecture`, `testing`, `search`, `other`)
- By recurrence (how many times the same mistake)
- By escalation status:
  - **Unescalated** (`no`): no project-level rule was added. The entry may also have been saved to user-local persistence (`~/.claude/.../MEMORY.md`, `settings.local.json`), but neither counts as project-level escalation — those are private to one developer.
  - **Escalated** (`AGENTS.md`, `skill:[name]`, `hook`, `settings`, `agent:[name]`, `doc-convention`, `code-style`): rule is in project instructions visible to every contributor.

### Step 1b: Find patterns (Carrot pass)

Runs **alongside** Step 1, not after it. Scan `ai-docs/learnings.md` a second time for entries whose `**Kind:** validation` line is **explicitly present** (the default-when-omitted rule leaves legacy entries OUT of carrot-pass scope — they belong to the Correction pass).

Group by **topic / target surface** (Skill / Subagent / AGENTS.md section). Topic is derived from the `**Rule:**` line's named surface (e.g., a validation entry whose `Rule:` names `/context-reset` groups under `skill:context-reset`). Count validation entries per topic — the count drives Step 2b routing.

The Correction pass (Step 1 → Step 2a) and the Carrot pass (Step 1b → Step 2b) produce independent groupings; an entry's `Kind:` field is what assigns it to a pass.

### Step 1c: Auto-memory companion sweep

Runs **alongside** Step 1 and Step 1b — a third parallel signal source, **NOT** a follow-on to either pass. The user-local auto-memory layer at `~/.claude/projects/<project-path-encoded>/memory/` (where `<project-path-encoded>` is the project's absolute path with `/` replaced by `-`, derived from `pwd` at run-time — do NOT hardcode any specific developer's path) feeds in as a **companion signal**. The sweep is **read-only** against that directory.

Read **both**:

1. `~/.claude/projects/<project-path-encoded>/memory/MEMORY.md` (the index) first — fast enumeration of memory filenames; avoids a blind `ls`.
2. Each individual `~/.claude/projects/<project-path-encoded>/memory/feedback_*.md` file — the detection rule below operates on each file's `name:` frontmatter, `description:` frontmatter, or first sentence, so per-file content is required.

For each `feedback_*.md`, decide whether it **names a workflow primitive**. The recognised primitives form a closed enumerated set:

<!-- anchor: auto-memory-primitive-keywords -->
```
Slash commands:
  /task, /improve, /pr-commented, /bugfix, /interview, /context-reset,
  /project-review, /ai-audit, /triage, /main-ci-failed, /pr-ci-failed, /pr-merged

Agent stems (file stems under .claude/agents/):
  self-improve, design, design-review, review-findings, self-review,
  spec-writer, learnings-escalation-audit, triage-runner

AGENTS.md section headings:
  ## Workflow, ## Propagation Rule, ## Learning Log, ## Code Style

Verb-phrase keywords:
  compaction recovery, propagation rule, lock-step, worked-example carve-out,
  boundary rule
```

A new Skill / Subagent / section heading / verb-phrase keyword added to the project requires an **additive update** to this block. The set is not auto-generated from `.claude/` listings (over-broad — would match incidental references).

**Cross-check against `ai-docs/learnings.md`.** A `feedback_*.md` is a **candidate** iff BOTH hold:

1. It names ≥ 1 primitive from the block above, AND
2. There is **no** `Kind: validation` entry in `ai-docs/learnings.md` whose `### YYYY-MM-DD — [category] — [short description]` heading OR `Rule:` field mentions the same primitive (substring match, case-insensitive — Subagent judgement applies for fuzzy topical matches).

A single `feedback_*.md` naming N primitives can be a candidate if **any** subset of the named primitives is uncovered; the per-feedback-file collapse rule applies (one candidate row per file, listing the uncovered primitive(s) in the cross-check column — see Step 2c).

**Prohibitions (the privacy boundary — read carefully):**

- **DO NOT** write to `~/.claude/projects/<project-path-encoded>/memory/*` from this step or any other step. The user-local auto-memory layer is read-only from this Subagent's perspective.
- **DO NOT** paraphrase, quote, or import auto-memory text into instruction-file edits based on a Step-1c candidate alone — a matching `Kind: validation` entry in `learnings.md` must exist (then it would have been picked up by Step 1b, not Step 1c), OR the user must explicitly approve via the `/improve` parent-thread consent prompt described in Step 2c. Step-1c output is **pre-consent**.
- **DO NOT** execute any routing decision (no `## Patterns` edit, no AGENTS.md edit) based on a Step-1c candidate without parent-thread `Surface` consent. The candidate ROW goes into the report's `## Auto-memory candidates` section (Step 2c); the parent thread holds the consent dispatch.

The candidate set produced here feeds Step 2c (the paired routing decision). Step 1c does NOT itself emit `## Carrots proposed` rows.

### Step 2a: Determine actions (Correction pass)

| Occurrences | Current status | Action |
|---|---|---|
| 1 | no | Nothing — wait for recurrence |
| ≥2 | no | Update `AGENTS.md` or Skill/Subagent/settings file — add/strengthen rule |
| ≥2 | AGENTS.md / Skill / Subagent / settings | Rule exists but isn't working → move closer to the point of execution |
| ≥3 | rule in place | Propose a hook in `.claude/settings.json` |

**Routing — which file to update:**
1. Find the Skill/Subagent file responsible for the behavior with the error — update that
2. Only if no specialized Skill/Subagent → update `AGENTS.md`
3. Don't default everything to `AGENTS.md`

### Step 2b: Determine actions (Carrot pass)

Asymmetric routing — positive signal is rarer, so the threshold is lower (≥1 seeds, ≥2 promotes) and the promotion verbs are softer (*Default to* / *Prefer*, never *MUST* / *NEVER*).

| Validation entries on same topic | Action |
|---|---|
| 1 | Add a `## Patterns` entry to the most-local Skill / Subagent / AGENTS.md (mirrors `ai-docs/agent-writing-style.md § Patterns`); back-link to the validation entry |
| ≥2 | Promote within the same `## Patterns` section in the targeted file — strengthen verb wording (*Default to* / *Prefer*), never escalate to *MUST* / *NEVER*. Promotion is a wording / verb edit within the section, not a file relocation. |
| 1 + names a workflow primitive | Hold for second confirmation; surface as candidate in the report |

**Routing — which file to update (same most-local rule as Step 2a):**
1. Find the Skill/Subagent file named by the validation entry's `Rule:` line — update its `## Patterns` section
2. Only if no specialized Skill/Subagent → add to `AGENTS.md`
3. Don't default everything to `AGENTS.md`

Both passes produce independent report entries; the final `/improve` report has separate `## Corrections proposed`, `## Carrots proposed`, and `## Auto-memory candidates` sections so the asymmetry stays visible to the user.

### Step 2c: Auto-memory routing

Pairs with Step 1c the way Step 2b pairs with Step 1b — takes the candidate set produced by Step 1c and routes each candidate into the report's **third** section, `## Auto-memory candidates`. The routing decision itself is **single-row** (auto-memory has only one signal shape — named primitive without matching `Kind: validation` cross-check):

| Candidate shape | Action |
|---|---|
| 1 + named workflow primitive + no matching `Kind: validation` entry in `learnings.md` | Emit a `## Auto-memory candidates` row; **needs parent-thread `Surface` consent before any routing decision** |

**Per-feedback-file collapse rule.** One row per `feedback_*.md`, NOT one row per uncovered primitive. If a single `feedback_*.md` names multiple uncovered primitives, list them comma-separated in the *Workflow primitive named* column and combine their cross-check verdicts in the *Cross-check verdict* column. This keeps the consent UI legible (the user sees one prompt per memory file, not per primitive).

**Report-section shape.** The `## Auto-memory candidates` section is the third section in the Step 6 report, after `## Corrections proposed` and `## Carrots proposed`. Row format:

```
## Auto-memory candidates

| Auto-memory file | Workflow primitive named | Cross-check verdict | Consent action |
|---|---|---|---|
| `feedback_<name>.md` | `<primitive>` (comma-separated if multiple) | no `Kind: validation` in `learnings.md` mentions `<primitive>` | (awaiting user) |
```

**Consent action column** records the parent thread's `AskUserQuestion` result. Initial value is `(awaiting user)`. After the parent thread dispatches the consent prompt and the user picks one option:

- **`Surface`** — the row migrates into `## Carrots proposed` and routes through normal Step 2b (the `1 + named workflow primitive` row of Step 2b's table fires; seed wording uses *Default to*).
- **`Drop`** — the row is removed from the report. No project-side write, no auto-memory write.
- **`Defer`** — the row's consent action becomes `(deferred; held for this invocation)`. The row remains in the report for visibility but does NOT route in this `/improve` run; re-surfaces on the next invocation.

**The parent thread holds the consent dispatch via `AskUserQuestion`**; this subagent emits the table and yields. Do NOT issue any `AskUserQuestion` from this subagent — it is structurally unfulfillable in the subagent tool exposure (same primitive-absence model as the Step 6 `Agent` dispatch), and even if it were available, the design splits consent into the parent thread to mirror `interview/SKILL.md`'s structured-output-plus-parent-surfacing pattern.

### Promotion verbs

The verb chosen for a promoted rule encodes its shape. Carrot rules (`Kind: validation`) use soft verbs; stick rules (`Kind: correction`) use fail-loud verbs. Verb choice is not enforceable by hook — `/ai-audit` Phase 2 Checklist M sub-check 11 audits cross-shape drift.

**Carrot promotion verbs** (Step 2b only):

| Verb | When |
|---|---|
| *Default to* | Seed wording when ≥1 validation; the soft default the Subagent is expected to follow absent contrary evidence |
| *Prefer* | Strengthened wording when ≥2 validations on the same topic; still soft — narrows the default further without forbidding alternatives |

**Stick promotion verbs** (Step 2a only):

| Verb | When |
|---|---|
| *MUST* | Hard positive obligation; rule is enforced and a violation is a correction event |
| *NEVER* | Hard negative prohibition; same enforcement shape, opposite polarity |
| *MUST NOT* | Synonym of *NEVER* — pick whichever reads better in context |
| *FORBIDDEN* | Same shape as *NEVER*; reserved for AXIOM-blockquote tone |

**Cross-shape is FORBIDDEN.** A carrot rule (promoted from a `Kind: validation` entry, living in a `## Patterns` section) MUST NOT use a stick verb. A stick rule (promoted from a `Kind: correction` entry, living in AGENTS.md / Skill / Subagent body or a fail-loud AXIOM blockquote) MUST NOT use a carrot verb. The verb asymmetry IS the asymmetric-promotion contract — wrong-shape verb either underweights a real obligation or locks in a brittle default as a hard rule. `/ai-audit` Phase 2 Checklist M sub-check 11 flags cross-shape violations at severity `major`.

### Step 3: Propose concrete changes

For each pattern show:
1. **Problem** — what repeats, how many times
2. **Current protection** — where the rule is recorded (if any), why it isn't working
3. **Proposal** — concrete diff (old text → new text)
4. **Level** — `ai-docs/learnings.md` → `AGENTS.md`/skill → hook

### Step 4: Escalate to hooks (only ≥3 occurrences and rule not working)

If proposing a hook, show:

```
Type: PreToolUse / PostToolUse
Matcher: which tool
Command: what to execute
Why hook and not rule: [explanation]
```

### Step 5: Apply after confirmation

**First action — branch check.** Run `git branch --show-current`. If it returns `main` and the planned changes are intended for a PR, create a feature branch *before any file edit*:

```bash
git checkout -b chore/YYYY-MM-DD-improve-<short-name>
```

`git checkout -b` carries the (still-uncommitted) working tree over. Discovering you're on main *after* editing forces a reactive recovery — switching at commit time technically respects AGENTS.md "no commits on main" but breaks the spirit (working tree should never accumulate on main). Switch first, edit second.

Number all proposals. Let user choose.

**Apply in two commits on the same feature branch:**

1. **Commit A — instruction-file edits.** Apply the approved diffs to `AGENTS.md` / Skill / Subagent / `rules:[name]` / hook / `ai-docs/code-style.md` / `ai-docs/doc-convention.md` / `.claude/settings.json`. Stage explicitly by name. Run any applicable gates (`actionlint` only if the change touches `.github/workflows/*.yml`; `cargo fmt -- --check` if a code-style example changed). Commit with a message describing the escalation. Do **NOT** batch the `git commit` in the same turn as the `Edit` calls it describes — an `Edit` can fail (non-unique / not-found anchor; the proposed `old_string` may not match the file's actual text) and the failure result arrives after the commit runs, yielding an over-claiming message. Wait for every edit's success result, then verify each landed with `git diff --cached --stat` before committing.

2. **Commit B — backfill `Escalated?` and (when applicable) `Superseded by:`.** Two kinds of field updates may happen here, on EXISTING entries only (NEVER append new entries):

   a. **`Escalated?` backfill.** For each entry whose pattern was just escalated in Commit A, edit ONLY the `**Escalated?**` line — replace the prior value (typically `no`) with the comma-separated list of targets actually modified.

   b. **`Superseded by:` backfill (when Commit A reverses, refines, generalizes, subsumes, or withdraws a prior entry's rule).** Identify the PRIOR entry whose `Rule:` text Commit A invalidates. Add or update its `**Superseded by:**` line. Format: `[ref] — [one-line reason]` where `[ref]` is a `YYYY-MM-DD` date (later entry; disambiguate with quoted slug when multiple entries share the date), `PR #N`, or both comma-separated. If the prior entry has no `**Superseded by:**` line yet, INSERT one on its own line immediately after the entry's `**Escalated?**` line. Write to the PRIOR entry's `Superseded by:`, never to the new entry.

   Do not touch any other line of any entry. Commit message: `chore(learnings): backfill Escalated? / Superseded by: for entries <date1>, <date2>, ...` (drop the `Superseded by:` half when no supersession applies).

   This edit is authorised by **AGENTS.md § Learning Log → Boundary rule 1 → Exception** (`Escalated?` and `Superseded by:` fields, Subagent-driven only). All other lines of the entry remain immutable.

   **Boundary rule 2 note:** Splitting into Commit A then Commit B keeps the PR diff legible (escalation substance separate from bookkeeping). The exception in Boundary rule 2 authorises both commits in the same `/improve` turn; it does NOT authorise appending NEW learning entries in the same turn.

   **In-flow `/task` carve-out:** A separate Boundary Rule 2 exception (added 2026-05-13) allows the `/task` workflow Steps 8–12 — **and any sub-skill (e.g., `/bugfix`, `/context-reset`) invoked from within that range** — to append NEW `learnings.md` entries in the same turn as instruction-file edits, provided the entries are marked `Escalated? no` and document an in-flight insight (not a pre-emptive escalation). This carve-out is `/task`-only (parent + sub-skill detours); the `/improve` Subagent does **not** itself append NEW learning entries — it only edits `Escalated?` / `Superseded by:` on existing entries. When auditing the corpus during a `/improve` run, treat in-flow `/task`-authored entries (those marked `Escalated? no` whose accompanying merged PR was a `/task` workflow, possibly via a `/bugfix` detour) as normal candidates for escalation, not as Rule-2 violations.

### Step 6: Eval (REQUIRED after Step 5)

After applying changes — answer:
- How to reproduce the original error?
- What does the output look like if the fix worked?

**Primitive-absence statement.** The `Agent` (Subagent-dispatch) primitive is **structurally unfulfillable** from inside the `self-improve` Subagent class — the runtime tool exposure for this class genuinely lacks `Agent`. The `Task*` family available via ToolSearch is queue management for in-flight subagents, not subagent spawning. This was diagnosed in `ai-docs/learnings.md` (the 2026-05-15 *"`self-improve` silently degraded `/improve` Step 6"* entry from PR #362 Commit C, and the 2026-05-15 *"`self-improve` subagent genuinely lacks the `Agent` primitive"* P5 entry from PR #364). The Step 6 contract is therefore **structurally unfulfillable by the subagent itself** — pause-and-surface to the parent thread is the only correct disposition.

**Step 6 handoff — pause-and-surface protocol** (replaces the prior "run via `Agent` subagent" directive — the parent thread, NOT the subagent, dispatches the reproducers):

1. **Introspect.** Confirm `Agent` is absent from your runtime tool list (via `ToolSearch` and the system-prompt deferred-tools block). Do NOT attempt any same-context substitution (no `Bash`-shelled invocation, no `TaskCreate`-then-`TaskOutput` polling, no in-memory close-read — all degraded paths that PR #362 Commit C explicitly forbids).
2. **Assemble** a `## Step 6 handoff — clean-context eval reproducers` block at the END of your `/improve` response, formatted per the template below — one reproducer block per Step-1 pattern you propose a rule for.
3. **Yield** to the parent thread. Do NOT emit `Eval: PASS ✅` or `Eval: FAIL ❌` yourself — the parent thread (which has `Agent`) dispatches the reproducers in fresh contexts and emits the final report.

**Propagation-rule asymmetry:** the Learning-Log sync-group sister file `.claude/agents/learnings-escalation-audit.md` has no Step 6 eval-phase equivalent (its workflow is a passive auditor; its `Step 6 — Report` is structured output, not a primitive-dispatch step), so this contract requires no mirrored edit there.

**Reproducer-prompt template skeleton** (emit verbatim, one block per pattern; the parent thread copies each block into a fresh `Agent` dispatch). The `Scenario:` line **branches on the audited entry's `Kind:`** — the same skeleton serves both passes:

```
### Reproducer R<pattern_id> — <pattern_summary>

**Kind:** correction | validation

**Scenario (Kind: correction):** <original_error_repro> — you are about to violate rule X; what is the expected behaviour?
**Scenario (Kind: validation):** <edge_case_from_validation_surface> — in this scenario, does pattern P still hold?

**Expected fixed output:** <expected_fixed_output>

**PASS criterion (Kind: correction):** the violation does NOT happen in the reproducer — rule fired.
**PASS criterion (Kind: validation):** the pattern still holds under the edge — pattern survives.
**FAIL criterion (Kind: correction):** the violation still happens — rule not strong enough.
**FAIL criterion (Kind: validation):** the pattern overfits or breaks under the edge — downgrade the promotion verb (*Prefer* → *Default to*) or do not promote.
```

Emit only the line variant matching the audited entry's `Kind:`; leave the other variants as the template skeleton for reference. Kind-branching applies ONLY to the `Scenario:` / `PASS criterion:` / `FAIL criterion:` lines — the pause-and-surface protocol, the parent-thread dispatch, and the `Eval: PASS ✅` / `Eval: FAIL ❌` emission are identical across both passes.

**Worked example** (anchor the skeleton — illustrative only; substitute real Step-1 patterns at runtime):

```
### Reproducer R1 — spec amendment during /pr-commented requires design → design-review re-loop

**Scenario:** You are mid-`/pr-commented` Round 1 on an open PR. The reviewer-comment fix you propose touches both a SKILL.md frontmatter AND 3 lines of the spec file `ai-docs/plans/done/<date>-<slug>.spec.md`. You have already committed the fix. What is the next step before `git push`?

**Expected fixed output:** the Subagent invokes the Spec Amendment recipe (re-run `/task` Step 6 → Step 7 with the amended spec; do NOT run self-review yet; design-review must issue GO first, THEN self-review runs over the amended diff, THEN push).

**PASS criterion:** Subagent names the Spec Amendment recipe + the `/task` Step 6/7 re-loop sequence BEFORE any self-review or push.
**FAIL criterion:** Subagent proceeds to self-review and push without invoking the Spec Amendment recipe.
```

**PASS criterion (parent-thread emits, NOT the subagent):** the problematic pattern is gone in every reproducer the parent dispatched.
**FAIL criterion (parent-thread emits, NOT the subagent):** same error in ≥1 reproducer → rule not strong enough → loop back to Step 3, strengthen it, re-run Step 6.

Report (parent-thread emits, NOT the subagent): `Eval: PASS ✅` or `Eval: FAIL ❌ — [what didn't work in reproducer R<pattern_id>]`.

## Anti-patterns

- **Do NOT** delete entries from `ai-docs/learnings.md` — it's a log, only grows
- **Do NOT** add rules for one-off errors — wait for recurrence
- **Do NOT** propose hooks for the first/second occurrence
- **Do NOT** overload `AGENTS.md` — specific rules go in the Skill/Subagent file
- **Do NOT** propose changes to project code — only to Subagent instructions
- **NEVER write to `~/.claude/projects/<project-path-encoded>/memory/*`.** The user-local auto-memory layer is user-controlled. `/improve`'s `self-improve` Subagent reads auto-memory as a companion signal during Step 1c, but the Subagent (and the parent `/improve` Skill) MUST NOT create, edit, rename, or delete files in that directory. If a candidate auto-memory entry needs revision, surface it as a `## Auto-memory candidates` row with `Drop` consent action and the rationale in the cross-check column; never auto-correct.
