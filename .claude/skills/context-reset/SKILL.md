---
name: context-reset
description: "Handoff protocol for large tasks (>=5 subtasks) AND compaction-recovery re-entry. Prevents context degradation and compaction-related quality loss."
when_to_use: "Activate at the start of every design-defined group per the design's ## Handoff plan, when a summary/compaction block appears at the top of context, or when noticing yourself rushing, simplifying, or skipping steps."
allowed-tools: Bash(cargo build) Bash(cargo test *) Bash(cargo clippy *) Bash(cargo fmt *) Bash(cargo doc *) Bash(git status) Bash(git rev-parse *) Bash(git branch *) Bash(git diff *) Bash(git add *) Bash(git commit *)
---

> **⚡ Compaction recovery check — read FIRST on every invocation.**
> If you are re-entering `/context-reset` after auto-compaction (a
> summary/compaction block appears at the top of context, or workflow
> context feels thin), STOP before any tool call and:
>
> 1. Identify the **parent workflow** (`/task`, `/project-review`, or
>    `/pr-commented`) whose handoff `/context-reset` is performing. The
>    parent's identity is recorded as the active progress file's
>    `parent_skill:` field (or `current_step:` mentions a `/task` /
>    `/project-review` / `/pr-commented` step name).
> 2. Run the parent skill's own compaction-recovery callout against its
>    durable-state file (see `/task`, `/project-review`, or `/pr-commented`
>    SKILL.md for the parent's variant). `/context-reset`'s body is the
>    shared handoff + re-prime action, not a separate durable surface.
>    (The parent's Variant-A callout ends with a "See `/context-reset`
>    § Compaction recovery (re-entry)" cross-link — that is a *reading*
>    link to the canonical rationale, **not** an instruction to re-run
>    this Variant-C callout. The chain terminates at the parent's active
>    subtask.)
> 3. After the parent's callout routes you to the active subtask, follow
>    `/context-reset`'s **Handoff protocol** below for the actual handoff
>    or re-prime action.
>
> This skill carries the canonical rationale below (§ **Compaction
> recovery (re-entry)**) for the other skills to cross-link to.

## Handoff protocol

When triggered (every design-defined group OR compaction detected):

1. `cargo build` — ensure code compiles
2. Update `ai-docs/plans/YYYY-MM-DD-name.progress.md` (format below)
3. Launch ONE `Agent` Tool call per **group** (per the design's `## Handoff plan`). **Read the group's marked implementor model + effort** from its Handoff-plan entry — the marker, not a fixed default, governs which subagent is spawned:
   - **code** group (marked `sonnet`) → `Agent(subagent_type="code-writer", …)`. Model (`sonnet`) and effort (`medium`) are **frontmatter-pinned** in `.claude/agents/code-writer.md`; pass NO inline `model=`/effort override — there is no per-invocation `effort` parameter, so the frontmatter pin is the only lever that enforces the medium tier. 1M-token window.
   - **instructions/harness** group (marked `opus`) → `Agent(subagent_type="general-purpose", model="opus", …)` with effort **NOT set** (inherited from the orchestrator, typically xHigh), 1M-token window.
   The Subagent owns all subtasks in its group and runs them sequentially in-context, committing after each: `prompt="Read ai-docs/plans/YYYY-MM-DD-name.progress.md and complete Group <X>'s subtasks <N>–<M>, then return"` (`code-writer` Mode A for a code group). Only the **instructions/harness** spawn takes an inline `model=` override (`model="opus"`); the **code** group's model + effort come from `code-writer`'s frontmatter, never an inline override. The quality-gate subagents (`design` / `design-review` / `self-review` / `spec-writer`) stay on Opus.
4. Do NOT spawn one Agent per subtask. The group is the unit of fan-out; the subtask is the unit of commit.
5. Do NOT continue in current context

The per-group subagent inherits the canonical schema at [`ai-docs/templates/progress-format.md`](../../../ai-docs/templates/progress-format.md) verbatim and writes `current_step` / `last_passed_gate` / `Decisions log` entries at the same subtask boundaries the orchestrator writes them at today. No new per-group section is introduced.

## Compaction recovery (re-entry)

Canonical rationale for the compaction-recovery callout that every code-side orchestrator SKILL.md (`/task`, `/project-review`, `/pr-commented`, `/bugfix`, `/interview`, `/context-reset`) places at the top of its body. Every Variant-A and Variant-B callout cross-links to this section by the exact h2 heading `## Compaction recovery (re-entry)`; this is the singular destination.

### Why a callout at all

Claude Code's per-skill truncation after auto-compaction keeps the **start** of `SKILL.md` and drops the rest. The compaction-recovery callout therefore lives at the very top of each code-side orchestrator SKILL.md so it survives even when the rest of the body is dropped at the per-skill 5,000-token cap. The user explicitly chose **heuristic self-detection over a `SessionStart|compact` hook** — the callout is the only mechanism; correctness rests on its wording.

### The Full-read-on-re-entry invariant

Every re-entry path (compaction recovery or otherwise) MUST re-read the durable-state file end-to-end before any tool calls, then re-enter the skill from the **top of its body** (preambles included). The recorded `current_step` is a hint for the human reader and a cross-check, NEVER an instruction to skip the read or jump straight to that step.

The invariant matters because:

- `/task` Step 1 is the interview phase (not the active-state probe — that lives in the `⚡ First` preamble above Step 1).
- `/project-review` Step 1 is "Determine branch" (not the RESUME probe).
- `/pr-commented` Step 1 is "Open / extend progress file" (preconditions run *above* the numbered steps).
- `/bugfix` Step 1 is "Reproduce and Trace" — re-running it on a confirmed trace re-asks the user to confirm what's already confirmed (Variant B carries an explicit skip rule for this case).
- `/interview` Step 1 is "Detect entry mode" — round counter lives in `.state.md`, not in Step 1.

Re-entering literally "from Step 1" would skip the active-state probes in three of six skills and re-trace in `/bugfix`. The wording "**re-enter the skill from the top of its body**" covers all six skills uniformly.

### Variant taxonomy

Three callout variants address the three structurally-different probe shapes:

- **Variant A** — `/task`, `/project-review`, `/pr-commented`. Probe lives in a `⚡ First` preamble that globs / greps for the active artefact. The callout routes via the probe (the preamble glob) then re-enters from the top of the body.
- **Variant B** — `/bugfix`, `/interview`. Probe is a fixed-glob (`ai-docs/bugfix/trace-*.md` or `<spec_path>.state.md`); when a single in-flight artefact exists the callout reads it and applies a per-skill resume rule (skip-Step-1 for `/bugfix` on a confirmed trace; resume from the recorded `round:` for `/interview`).
- **Variant C** — `/context-reset` itself. No own durable surface; routes to whichever parent skill (`/task` / `/project-review` / `/pr-commented`) is active. The callout above this section is Variant C.

The variant-distinguishing phrases that the char-cap / variant-identity audit greps for are kept in one canonical place, so the audit's expected hit-count of 1 per skill is preserved.

### Why the callout cross-links here

Concentrating the rationale in one place keeps the per-skill callouts short (each is ~25 lines). The cross-link `See .claude/skills/context-reset/SKILL.md § Compaction recovery (re-entry)` resolves to this exact anchor in every Variant-A and Variant-B callout. Variant C punts to the parent's callout instead of cross-linking back to this section (the chain terminates at the parent's active subtask).

## Checkpoint handoff: 1 Agent = 1 group

- Do NOT ask "continue?" between subtasks within a group — just proceed
- Each `Agent` Tool call = 1 design-defined group; the Subagent commits after each subtask inside the group
- Update progress.md after each subtask (current_step, last_passed_gate, Decisions log). **A Decisions-log bullet is a durable claim, not a note** — it is gitignored but is read by every future context-reset, this skill included. Before writing "verified" / "confirmed" / "observed", ask: did **I**, in **this** invocation, run **that exact** command against **this** code? If the true support is "a prior agent measured something adjacent" or "the passing suite is consistent with this", write **that** — it is weaker, and that is the point. Mirrors `.claude/agents/code-writer.md` § Mode A (same rule, same boundary).
- The next group's Agent is spawned by the orchestrator only after the current group's Agent returns

## `.progress.md` format (canonical)

The full format spec lives in the shared-templates directory: **[`ai-docs/templates/progress-format.md`](../../../ai-docs/templates/progress-format.md)** (template, required vs optional fields, lifecycle by field, lifecycle by process, exemptions). Used by `/task`, `/project-review`, `/pr-commented`, `/bugfix`, and the `review-findings` / `self-review` agents. Required fields now include `**current_step:**`, `**last_passed_gate:**`, and a `## Decisions log` section in addition to the original `**Branch:**` / `**base_commit:**` / `**Last build:**`.

## Rules

1. Progress file = `ai-docs/plans/*.progress.md`. Updated at each checkpoint.
2. On context reset: pass file path in Agent prompt: `"Read ai-docs/plans/YYYY-MM-DD-name.progress.md and continue"`
3. `cargo build` BEFORE handoff — don't pass broken code
4. Default maximum 4 design-defined groups per task; needing more than 4 is surfaced to the user for approval (not an automatic issue-split).

## Patterns

### 1. Trust the compaction-recovery callout

*Default to* following the compaction-recovery callout at the top of every code-side orchestrator SKILL.md exactly — locate the durable-state file via the parent skill's active-state probe, read it end-to-end before any tool call, re-enter the skill from the top of its body. *Prefer* the protocol over shortcut paths even when context feels thin or the recorded `current_step` looks like a clear instruction to jump.

**Why.** Claude Code's per-skill 5,000-token truncation after auto-compaction keeps the start of `SKILL.md` and drops the rest — the callout therefore lives at the very top of the body so it survives compaction. The full-read-on-re-entry invariant (see § *The Full-read-on-re-entry invariant* above) is what preserves workflow correctness across compression events; jumping to a recorded step would skip the parent skill's active-state probe (three of six skills) or re-trace already-confirmed state (`/bugfix`).

Validated in the sibling **quartzite** project (`maratik123/quartzite`, whence this harness was adapted) — [its `ai-docs/learnings.md`](https://github.com/maratik123/quartzite/blob/main/ai-docs/learnings.md) 2026-05-19 *compaction-recovery protocol in skill files works — follow it exactly*: 4 rounds of `/pr-commented` on `maratik123/quartzite#490` plus multiple compressions, user explicitly confirmed workflow state preserved end-to-end.
