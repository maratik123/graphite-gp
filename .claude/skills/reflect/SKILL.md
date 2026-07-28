---
name: reflect
description: "Self-reflection retrospective over the just-completed unit of work. Launches the self-reflect subagent, surfaces its good/bad findings for per-finding routing consent, and applies each to learnings (cheap) / a process-labelled gh issue (costly) / no-op (recorded). Standalone, explicit invocation only."
disable-model-invocation: true
argument-hint: "[optional context — what unit of work to reflect on]"
---

Launch the `self-reflect` subagent.

The subagent reads `.claude/agents/self-reflect.md` for full instructions.

## Trigger predicate — standalone `/reflect` only

Reflection fires **exactly** when the user invokes `/reflect` explicitly, like `/improve`. It does **NOT** auto-fire at the end of `/task`, `/improve`, `/bugfix`, or any other workflow — **none of those files is modified to trigger it**. The `disable-model-invocation: true` frontmatter (mirroring `improve/SKILL.md`) means the model cannot self-invoke this skill; only an explicit user `/reflect` runs it. Fully opt-in.

> A future move to auto-fire reflection (e.g. from a `Stop`/`SubagentStop` hook) is **deferred** — it would make reflection non-optional and needs its own enforcement design. It is out of scope for this skill as written.

## What the subagent does

The `self-reflect` subagent assembles a **structured good/bad list** over the just-completed unit of work and **yields** it — it issues no `AskUserQuestion` and performs no project-side write. Each finding names a **concrete moment**, carries exactly one route `{learnings | ticket | none}` chosen by the cost/value rubric in `self-reflect.md § Step 2`, and a one-line justification. The list **MAY be empty** (a valid outcome — not a failure).

## Consent gate — parent thread applies, per finding

When the subagent returns its `## Self-reflection` report, this `/reflect` skill — the **parent thread** — MUST dispatch one `AskUserQuestion` per **actionable** finding (any `learnings` or `ticket` route) **before** any project-side write derived from it. `none` findings need no prompt — they are already recorded in the report — but are shown in the summary so "no action" is visible, never silent. The subagent surfaces findings as structured rows only; it does NOT execute routing. Consent dispatch lives here, in the parent thread, exactly as `interview/SKILL.md` surfaces spec-writer questions and `improve/SKILL.md § Auto-memory consent gate` surfaces auto-memory candidates.

**Per-finding prompt shape** (one `AskUserQuestion` question per actionable finding):

```yaml
question: "Reflection finding: <concrete moment> — proposed route `<learnings|ticket>` (<one-line justification>). Apply?"
header: "reflect"
options:
  - label: "Apply"
    description: "learnings → append the proposed entry to ai-docs/learnings.md; ticket → file the process-labelled gh issue."
  - label: "Downgrade"
    description: "Record as `none` instead (acknowledged, no write) — keeps the finding on record without acting."
  - label: "Drop"
    description: "Discard this finding entirely; not recorded."
```

`header` is 7 chars (≤ 12-char cap); 3 options sit inside the 2..=4-option range.

**Multi-finding dispatch.** `AskUserQuestion` accepts up to 4 questions per call. When ≤ 4 actionable findings, dispatch them as a **single** `AskUserQuestion` call (one question per finding, all headers = `reflect`). When > 4, dispatch sequentially — one `AskUserQuestion` call per finding — until the list is exhausted. Mirrors `improve/SKILL.md § Auto-memory consent gate` batched-≤4 / sequential->4 pattern.

## Per-route apply (parent thread, only after `Apply`)

- **`learnings`** → append the subagent's proposed entry **verbatim** to the end of `ai-docs/learnings.md`: `Kind: validation` (good/keep-doing) or `Kind: correction` (bad/stop-doing), `Escalated? no`, honoring `AGENTS.md § Learning Log` Boundary rule 1 (append-only) and Boundary rule 2 (no same-turn instruction-file edit). **This skill never edits an instruction file**, so Boundary rule 2 holds by construction — reference it, do not restate it. **Verbatim means no gate runs between the subagent and the log** — the factual claims must already have been verified inline by `self-reflect` when it drafted the entry (`.claude/agents/self-reflect.md` § Per-route contracts). That inline check is the compensating control for the `self-review` carve-out below; if an entry carries an unverified `file:line`, count, or command result, send it back rather than appending it.
- **`ticket`** → `gh issue create` with the `process` label at pattern altitude. If the body text contains the substring `git commit`, use `--body-file <path>`, **not** inline `--body` (AGENTS.md § Workflow — the commit-block hook matches `git[[:space:]]+commit`).
- **`none`** → no write. Already recorded in the ephemeral run report shown to the user (owner-fixed: ephemeral only, not persisted to any file).

## Threshold interaction

Reflection-sourced `learnings` entries **feed** the existing `/improve` run threshold (≥ 3 unescalated corrections / ≥ 2 unescalated validations — `AGENTS.md § Learning Log`) — they do **not** bypass it, and `/reflect` **never** itself escalates a rule into an instruction file. Run `/improve` separately when the threshold accumulates.

## Write guard

The `AskUserQuestion` consent prompt is the **only** path to a project-side write in this skill. No `learnings.md` append and no `gh issue create` may originate without an `Apply` answer for that specific finding. `none` and `Downgrade`/`Drop` answers write nothing.

## No `self-review` on this skill's output

Do **NOT** spawn `self-review` over a `/reflect` run — `AGENTS.md § Workflow`'s self-review AXIOM carries an explicit `/reflect` carve-out. The reason is **structural**, not a cost judgement: this skill's product is `learnings.md` entries, and every downstream consumer of that surface is already contractually obliged to re-verify the claims in them. Verify each entry's factual claims **inline as you write it** — that is where the obligation actually lives.

See also: `/improve` (`.claude/skills/improve/SKILL.md`) — the consumer of the `learnings.md` queue that reflection feeds; same batched-consent pattern, different mutation scope.

Context from user (if any): $ARGUMENTS
