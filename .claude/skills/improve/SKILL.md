---
name: improve
description: "Analyze corrections log (ai-docs/learnings.md), find repeating patterns, propose instruction updates and escalation to hooks."
disable-model-invocation: true
argument-hint: "[optional context]"
---

Launch the `self-improve` subagent.

The subagent reads `.claude/agents/self-improve.md` for full instructions.

The subagent will:
1. Read `ai-docs/learnings.md` for all correction records
2. Find repeating patterns (same mistake ≥2 times)
3. Propose concrete diffs to `AGENTS.md` or Skill/Subagent files — **re-verifying every factual claim it carries out of an entry** (`self-improve.md § Step 3` AXIOM: recurrence licenses an entry's **pattern**; the *facts* wrapped around it — a precedent list, a `file:line`, a lint group, a count — are candidate-truth **wherever they sit, `Rule:` line included**, not just in the narrative. `learnings.md` is append-only, so a false claim is only ever fixable in the copy escalation is about to make). Each proposal carries a **Claims re-verified** line; a proposal citing a precedent or a count without one is incomplete.
4. For mistakes that repeat ≥3 times despite existing rules — propose escalation to hooks in `.claude/settings.json`
5. Apply changes after user confirmation — **the parent thread re-runs each proposal's `Claims re-verified` commands against the tree before applying that proposal**, and drops or corrects any claim that no longer holds. Without a consumer that re-runs them the line is ceremony; this is the same contract `self-review.md` §2 already enforces for design's `AC<N> verified by: <command>` lines ("Confirmed during drafting" is not sufficient).
6. Run a targeted eval to verify the fix works. Reports PASS/FAIL.

Run when **≥3 unescalated correction entries**, **≥2 unescalated validation entries**, or a `🌱 Stale-validation` flag from `/ai-audit` accumulates. Mirror of the threshold line in `AGENTS.md § Learning Log` — keep both in sync per the Propagation Rule.

## Auto-memory consent gate

When the `self-improve` subagent returns a `## Auto-memory candidates` report section (per its Step 1c sweep + Step 2c routing), this `/improve` skill — the **parent thread** — MUST dispatch one `AskUserQuestion` per candidate row **before** any project-side write derived from that candidate. The subagent surfaces candidates as structured rows only; it does NOT execute routing. Consent dispatch lives here, in the parent thread, exactly as `interview/SKILL.md` surfaces spec-writer questions via parent-side `AskUserQuestion`.

**Privacy boundary.** Project-side `/improve` writes NEVER originate from auto-memory alone; the consent prompt is the ONLY surfacing path. The Subagent reads `~/.claude/projects/<project-path-encoded>/memory/MEMORY.md` + each `*.md` memory beside it read-only — selecting the **feedback-type** memories by their `metadata.type: feedback` frontmatter, NOT by a `feedback_*` filename glob (the layer's naming convention changed; see `self-improve.md § Step 1c`) — and writes nothing back. See `self-improve.md § Anti-patterns` (`NEVER write to ~/.claude/projects/<project-path-encoded>/memory/*`).

**Per-candidate prompt shape** (literal `AskUserQuestion` payload — one question per candidate):

```yaml
question: "Auto-memory entry `<topic-slug>.md` (`metadata.type: feedback`) names workflow primitive `<primitive>` with no matching `Kind: validation` entry in `ai-docs/learnings.md`. Surface as a /improve candidate (would seed a `## Patterns` entry in `<target-skill-or-subagent>` after the next user-approved Carrot-pass step)?"
header: "auto-memory"
options:
  - label: "Surface"
    description: "Add to this run's Carrot-pass candidate list; routes through normal Step 2b table — which may still hold for second confirmation if no matching `Kind: validation` entry is created in this turn."
  - label: "Drop"
    description: "Drop this candidate; do not surface again in this `/improve` invocation. The auto-memory entry stays user-local (no write-back)."
  - label: "Defer"
    description: "Skip surfacing for now; re-surface on next `/improve` invocation. State holds only for this invocation (no persistent flag file)."
```

`header` is 11 chars (≤ 12-char cap); 3 options sit inside the 2..=4-option range.

**Consent routing.**

- **Surface** → move the row from `## Auto-memory candidates` into `## Carrots proposed` and process via the normal Step 2b table for routing into a `## Patterns` block (still subject to second-confirmation rules if no matching `Kind: validation` entry lands in this turn).
- **Drop** → drop the row from this `/improve` invocation. No project-side write, no auto-memory write-back. The candidate may re-surface on a future invocation if the underlying auto-memory entry persists and remains uncovered.
- **Defer** → drop the row from this invocation only; re-surfaces on the next `/improve` invocation. State holds in parent-thread working memory for the duration of this invocation only — no persistent flag file is written to the project layer or the user-local layer.

**Multi-candidate dispatch.** `AskUserQuestion` accepts up to 4 questions per call. When the subagent returns ≤ 4 candidate rows, dispatch them as a **single** `AskUserQuestion` call (one question per row, all four headers = `auto-memory`). When > 4 rows, dispatch sequentially — one `AskUserQuestion` call per row, each with exactly one question — until the candidate list is exhausted. If user-local memory holds N feedback-type memories, the worst-case dispatch is N prompts (the user can answer `Drop` to each).

**Write guard.** This `AskUserQuestion` is the project-side gate for auto-memory-derived edits. No project-side instruction-file edit may originate solely from a Step-1c candidate — the gate fires first, the user chooses, and only on `Surface` does the candidate flow into normal Step 2b routing. Convention-enforced at three sites: (1) this section; (2) `self-improve.md § Anti-patterns` (NEVER write-back rule); (3) `/ai-audit` Checklist N (passive post-hoc audit — a `## Patterns` block without a `Kind: validation` back-link flags `major`).

See also: `/triage` (`.claude/skills/triage/SKILL.md`) — same batched-approval and ≥3-unhandled threshold patterns; diverges in mutation scope (mutates `ai-docs/deferred/**` + `gh issue create/edit` rather than instruction files + `learnings.md`).

Context from user (if any): $ARGUMENTS
