# Code-writing subagent with frontmatter-pinned effort

**Source:** user description (free-text)
**Date:** 2026-07-16
**Tracked in:** none — internal harness task; user opted out of a tracking issue

## Scope

1. **Create `.claude/agents/code-writer.md`** — a file-based implementor subagent whose frontmatter pins the tier that `/task` code groups currently only *claim* to run at: `model: sonnet` + `effort: medium`, `tools` omitted (inherit-all). Its body defines the code-writing role and works in the two spawn contexts below (see Technical constraints → two-mode contract).
2. **Route the `/task` code-group spawn** — `.claude/skills/context-reset/SKILL.md` § Handoff protocol step 3, the `sonnet` branch (currently line 41) — to `subagent_type="code-writer"` and **drop** the redundant inline `model="sonnet"`. Effort becomes frontmatter-pinned instead of an unenforceable inline claim. The `opus`/harness branch (line 42) is **unchanged** — stays `Agent(subagent_type="general-purpose", model="opus")` with inherited effort.
3. **Update `.claude/skills/task/SKILL.md`** Step-8 every-group-handoff prose (line ~163) so the code-group description references `code-writer` + frontmatter-pinned effort, not an inline `model=sonnet` override carrying an (unenforceable) "effort medium pinned".
4. **Update the `design` ↔ `design-review` sync group** where the code→`sonnet`/`medium`-pinned marker is written and checked (`.claude/agents/design.md` group-marking prose incl. lines 67, 73, 77, 113, 114; `.claude/agents/design-review.md` marker-check prose lines 34–35) so "code group → sonnet/medium-pinned" describes routing through `code-writer`.
5. **Convert `/bugfix` Step 5 (Fix)** to delegate the fix's code-writing to `subagent_type="code-writer"` instead of the orchestrator opening `Edit` directly. Steps 1–4 (Reproduce/Trace, Root Cause, Failing Test, Plan) and Step 6 (self-review) stay with the orchestrator.
6. **Convert `/main-ci-failed` Step 4** — substantive code-fix writing delegates to `code-writer`; classification, reproduction, Step-5 self-review, and the new-feature-branch / commit / push / PR orchestration stay with the orchestrator.
7. **Convert `/pr-ci-failed` Step 4** — same delegation contract as `/main-ci-failed`.
8. **Convert `/pr-commented` Step 4 (Fix)** — the `fix`-classified code-writing delegates to `code-writer`; comment classification (Steps 2–3), the architectural-bail routing, Step-5 self-review, and single-commit/push orchestration stay with the orchestrator.
9. **Update `ai-docs/claude-tools-hierarchy.md`** — add `code-writer` to the Subagents table (with all spawners) and reflect the routed spawn contract in the PART-5 note (same PR, per the Tool/Subagent-contract propagation rule).

## Out of scope

- A second frontmatter subagent for the `opus`/harness path (Q3 = keep inline; nothing to pin there, so no file).
- Any `*.rs` / Rust source change. This task edits only `.claude/**` + `ai-docs/**`.
- Changing the orchestrator model or the Opus quality-gate subagents (`design` / `design-review` / `self-review` / `spec-writer`) — they keep their Opus contract regardless of any group marker.
- Moving the trace/root-cause analysis, classification, self-review, or commit/push orchestration into `code-writer` — those stay with each orchestrating skill. Only the fix's code-writing delegates.
- Curating a restricted `tools` allowlist for `code-writer` (defaulted to inherit-all — see Open questions).

## Deferred
- (none) — scope B is fully in-scope this PR; the Q3 opus-path decision resolves to "keep inline" with no follow-up needed.

## Key decisions
| Question | Decision |
|---|---|
| Q1 — Scope | **B (expansive)** — create `code-writer` AND convert the four inline-editing skills (`/bugfix`, `/main-ci-failed`, `/pr-ci-failed`, `/pr-commented`) to delegate code-writing to it, in addition to routing the `/task` code-group spawn. |
| Q2 — Subagent name | **`code-writer`** (`.claude/agents/code-writer.md` + `subagent_type="code-writer"`) — non-clashing with existing agents. |
| Q3 — Opus/harness path | **Keep inline** — `Agent(subagent_type="general-purpose", model="opus")`, inherited effort. No second frontmatter subagent. |
| `code-writer` frontmatter | `model: sonnet`, `effort: medium`; `tools` omitted → inherit-all (preserves the current `general-purpose` `*` surface). |
| Effort-pin mechanism | Frontmatter `effort: medium` — verified sole lever (no per-invocation effort param exists; see Technical constraints). |
| Inline `model="sonnet"` on the sonnet spawn | **Dropped** — frontmatter `model: sonnet` governs when the orchestrator passes no per-invocation `model` override. |
| Delegation boundary | Orchestrator keeps trace/root-cause, classification, planning, self-review, and commit/push; `code-writer` writes only the fix/subtask code. |

## Technical constraints

- **Verified frontmatter contract** (`https://code.claude.com/docs/en/sub-agents.md`, fetched 2026-07-16):
  - `effort` frontmatter field — "Overrides the session effort level. Default: inherits from session. Options: `low`, `medium`, `high`, `xhigh`, `max`; available levels depend on the model."
  - `model` frontmatter field — `sonnet` / `opus` / `haiku` / `fable` / full model ID / `inherit`; defaults to `inherit`.
  - Model resolution order: env var `CLAUDE_CODE_SUBAGENT_MODEL` → per-invocation `model` parameter → frontmatter. **There is NO per-invocation `effort` parameter on the Agent/Task tool** — frontmatter (or the session level) is the only place effort can be set. This is the entire rationale: the current inline `model="sonnet"` spawn cannot pin effort, so "effort medium (pinned)" is today unenforceable.
  - `tools` omitted → subagent inherits all tools (equivalent to the current `general-purpose` `*` surface).
- **Two-mode contract for `code-writer`.** It is spawned in two distinct contexts, and its body/prompt contract must accommodate both:
  - **(a) `/task` group-implementor mode** — read the group's `.progress.md`, complete the group's subtasks sequentially in-context, run `cargo` gates + `git commit` after each subtask, return (the current `general-purpose` implementor contract at `/context-reset` § Handoff protocol step 3).
  - **(b) single-fix delegate mode** (`/bugfix`, `/main-ci-failed`, `/pr-ci-failed`, `/pr-commented`) — apply the orchestrator-specified fix edit(s), run `cargo` gates, return; the **orchestrator** owns self-review and the commit/push (so the fix can pass self-review before it is committed). `code-writer` does NOT commit in this mode.
  The exact prompt shape and how the body expresses both modes is a design-phase decision.
- **Delegation-enablement — `allowed-tools` gating.** The four CI/comment skills currently gate tools via `allowed-tools` frontmatter listing only `Bash(...)` + `gh` commands (they edit inline today). Converting them to delegate requires the orchestrator to invoke the `Agent`/`Task` tool. The design must verify each converted skill can spawn `code-writer` and update its `allowed-tools` frontmatter where that frontmatter gates the spawn.
- **Mechanical inline-fix classes.** `/main-ci-failed` and `/pr-ci-failed` distinguish mechanical **Inline-fix classes** (`fmt` / `clippy` / `doc` / `actionlint` — single-command, lint-shaped edits) from substantive code fixes. Whether those mechanical classes also route through `code-writer` or stay inline is a per-skill design decision (default: mechanical single-command lint fixes stay with the orchestrator; a sonnet `code-writer` spawn is for code-writing, not `cargo fmt`). See Open questions.
- **Consistent delegation pattern.** The four skill conversions should share one delegation pattern so the design can decompose them uniformly (one code-writing delegation contract reused across all four `.claude/skills/**` edits; homogeneous instructions/harness change-type for THIS task's own implementation).
- **Sync-group propagation (same PR):**
  - `.claude/skills/context-reset/SKILL.md` ↔ `.claude/skills/task/SKILL.md` — Task/Design sync group.
  - `.claude/agents/design.md` ↔ `.claude/agents/design-review.md` — design ↔ design-review sync group.
  - Any Tool/Subagent/Skill/Hook contract change → update `ai-docs/claude-tools-hierarchy.md` in the same PR.
- **Name-clash AXIOM:** `code-writer` must not collide with any existing `.claude/agents/*` name (`spec-writer`, `design`, `design-review`, `self-review`, `review-findings`, `self-improve`, `learnings-escalation-audit`, `triage-runner`) or a built-in (`general-purpose` / `Explore` / fork). Verified non-clashing.
- **This task's own implementor group is Opus/harness, not sonnet** — it edits only `.claude/**` + `ai-docs/**` (instructions/harness change-type per PART-5), so the design's `## Handoff plan` marks its group(s) `opus` / effort inherited. (Meta-note for the downstream `design` Subagent.)

## Acceptance Criteria
| # | Criterion |
|---|-----------|
| AC1 | `.claude/agents/code-writer.md` exists with valid frontmatter containing exactly `model: sonnet` and `effort: medium`, `tools` omitted (inherit-all), and a body defining the code-writing role covering both spawn modes (group-implementor: read `.progress.md`, do subtasks sequentially, gate + commit per subtask; single-fix delegate: apply the fix, gate, return without committing). |
| AC2 | The `/context-reset` § Handoff-protocol code-group ("sonnet") spawn uses `subagent_type="code-writer"` and no longer passes inline `model="sonnet"`; prose states effort is frontmatter-pinned. The `opus`/harness branch is unchanged (`general-purpose`, `model="opus"`, inherited effort). |
| AC3 | `.claude/skills/task/SKILL.md` Step-8 handoff prose references `code-writer` for the code group; no residual wording implies an unenforceable inline `effort medium pinned` on a `general-purpose` spawn. |
| AC4 | `.claude/agents/design.md` (code-group markers) and `.claude/agents/design-review.md` (marker checks) both describe the code group as routing through `code-writer`; the two files are updated together (design↔design-review sync group). |
| AC5 | `/bugfix` Step 5 (Fix) delegates the fix's code-writing to `subagent_type="code-writer"`; Steps 1–4 and Step 6 (self-review) remain with the orchestrator. |
| AC6 | `/main-ci-failed` Step 4 delegates substantive code-fix writing to `code-writer`; classification, reproduction, Step-5 self-review, and branch/commit/push/PR orchestration remain with the orchestrator. |
| AC7 | `/pr-ci-failed` Step 4 delegates substantive code-fix writing to `code-writer` under the same contract as AC6. |
| AC8 | `/pr-commented` Step 4 (Fix) delegates the `fix`-classified code-writing to `code-writer`; comment classification, architectural-bail routing, Step-5 self-review, and single-commit/push orchestration remain with the orchestrator. |
| AC9 | Each converted skill (AC5–AC8) can spawn `code-writer` — its `allowed-tools` frontmatter permits the `Agent`/`Task` spawn where that frontmatter gates tool access. |
| AC10 | `ai-docs/claude-tools-hierarchy.md` Subagents table lists `code-writer` with all spawners (`/task` via `/context-reset`; `/bugfix`, `/main-ci-failed`, `/pr-ci-failed`, `/pr-commented` fix steps), and the PART-5 note reflects the routed spawn contract. |
| AC11 | Propagation Rule satisfied: `grep -rn "<changed keyword>" .claude/ AGENTS.md ai-docs/` (keywords incl. `general-purpose`, `model="sonnet"`, `effort medium`, `code-writer`) surfaces no stale references to the old inline-`model=sonnet`/unenforceable-`effort medium pinned` contract; both named sync groups updated in the same PR. |
| AC12 | `code-writer` does not clash with any existing `.claude/agents/*` name or built-in subagent name (name-clash AXIOM). |

## Open questions
- **Mechanical inline-fix-class routing** (`/main-ci-failed`, `/pr-ci-failed`): whether the `fmt`/`clippy`/`doc`/`actionlint` single-command lint fixes also delegate to `code-writer` or stay inline. Defaulted: stay inline (sonnet `code-writer` is for code-writing, not `cargo fmt`); design may decide otherwise. Not design-blocking.
- **`tools` frontmatter curation** (defaulted to inherit-all): a curated allowlist could deny the `Agent` tool to stop re-delegation, or scope `Write`/`Edit`. Left inherit-all for behavior-preservation; revisit only if a constraint is wanted. Not design-blocking.
