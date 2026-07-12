---
name: interview
description: "Requirements interview with the product owner. Output: spec saved to ai-docs/plans/, cross-linked with a tracking GitHub issue. Invoked by `/task` for Steps 1–5, or run standalone for spec-only work that defers implementation."
argument-hint: "[issue-number | task description]"
allowed-tools: Bash(gh issue view *) Bash(gh issue list *) Bash(gh issue create *) Bash(gh issue comment *)
---

Orchestrator for the spec-drafting interview. Drives the round loop, surfaces the subagent's questions to the user, and applies the user's answers — but does **not** draft the spec itself. Spec drafting and question generation live in `.claude/agents/spec-writer.md` (subagent on `model: opus`).

> **MUST run before:** code investigation, `design` Subagent, or writing code.
> Run standalone when you want a spec without committing to implementation (defer it to `ai-docs/plans/deferred/` afterward).
> For the full task workflow use `/task` — it delegates Steps 1–5 to this skill, then continues with design → implementation → PR.

> **⚡ Compaction recovery check — read FIRST on every invocation.**
> If you are re-entering this skill after auto-compaction (a
> summary/compaction block appears at the top of context, or workflow
> context feels thin), STOP before any tool call and:
>
> 1. **Locate the durable-state file** — list `ls ai-docs/plans/*.spec.md.state.md 2>/dev/null` (then read both the matched `.state.md` AND its sibling `<spec_path>`).
>    If exactly one in-flight artefact exists, that's the durable state.
>    If none exists, this is a fresh invocation. (Multiple matches:
>    surface to the user before continuing.)
> 2. Read it **top-to-bottom in one pass** — every line, including older
>    sections. Do not skim. The recorded `round` (from the `.state.md` YAML block) is a
>    cross-check, never an instruction to skip the read.
> 3. **Then re-enter this skill from the top of its body.** The body's
>    re-entry logic uses `round` (after the full read) to skip
>    user-confirmed checkpoints that need not be redone — resume from the round recorded in `.state.md`'s `round:` field; do NOT restart at round 1, and do NOT re-create the state file.
>
> If `ls ai-docs/plans/*.spec.md.state.md 2>/dev/null` returns no matches, this is a fresh invocation —
> proceed normally.
>
> See `.claude/skills/context-reset/SKILL.md` § **Compaction recovery
> (re-entry)** for the canonical handoff rationale.

## Architecture

Two pieces:

1. **This file** (orchestrator) — plumbing only. Detects entry mode, manages state, runs the round loop, parses the subagent's YAML status block, surfaces questions via `AskUserQuestion`, executes action handlers on `unresolvable`, posts the cross-link comment on `ready`.
2. **`.claude/agents/spec-writer.md`** (subagent, `model: opus`) — owns scope extraction, question drafting, AGENTS.md preflight, the Rule-5 substring blacklist, the optimization-target enforcement, and the spec write itself.

## Round / question caps

| Constant | Value | Where |
|---|---|---|
| `round_cap` | 4 | Hard-coded in this skill; passed to subagent every invocation |
| `questions_per_round_cap` | 3 | Hard-coded in this skill; passed to subagent every invocation |

These constants are **not** configurable via skill arguments in this iteration. Future configurability is a separate concern (deferred — see issue #188 spec).

## State file

Path: `<spec_path>.state.md` — e.g. `ai-docs/plans/2026-05-09-name.spec.md` ↔ `ai-docs/plans/2026-05-09-name.spec.md.state.md`.

Created at the start of round 1; deleted on terminal exit (`ready` / `abort` / `defer_to_deferred`). Format: markdown header + a single fenced YAML block.

```markdown
# Interview state — <task name>

Transient handoff between rounds. Deleted on terminal exit.

```yaml
schema_version: 1
spec_path: ai-docs/plans/YYYY-MM-DD-name.spec.md
issue_ref: "#188"
gh_issue:                          # present only when issue_ref resolves to a real gh issue; omitted for free-text entry mode
  title: "<verbatim from gh issue view>"
  state: open                      # open | closed
  labels: ["enhancement"]          # may be empty
  body: |
    <verbatim issue body block-scalar>
  comments:
    - author: "..."
      body: "..."
  linked_issues: ["#42"]           # extracted from body / comments via #\d+ regex; may be empty
  linked_prs: ["#100"]             # same; may be empty
task_description: |                # present only in free-text entry mode (mutually exclusive with gh_issue:)
  <user's free-text task description>
round_cap: 4
questions_per_round_cap: 3
round: 2
agent_id: <captured from round-1 Agent invocation; null if cold-Agent path>
prior_qa:
  - round: 1
    question: "..."
    answer: "..."
  - round: 1
    question: "..."
    answer: "..."
```
```

The `gh_issue:` / `task_description:` blocks are the durable home for the spec-writer's original inputs. The round prompts (Step 3a) carry the same content inline today, but persistence to `.state.md` survives auto-compaction and cold re-spawns where the round prompt has been dropped — the orchestrator can rebuild the prompt from `.state.md` on re-entry without re-issuing `gh issue view`.

## Workflow

### Step 1: Detect entry mode

Inspect `$ARGUMENTS`:

- **Issue ref** — matches `^#?\d+$`: load `gh issue view <N> --json title,body,state,labels,comments` once. Record `tracking_issue = <N>`. Extract `#\d+` references from the body + comments → `linked_issues` / `linked_prs` (split on whether the referenced number is an issue or a PR; cheap heuristic — running `gh pr view <M>` once per match is acceptable, or treating ambiguous refs as `linked_issues` is acceptable until a downstream consumer needs the precise split).
- **Free text / empty**: use as task description, or ask "What do you want to plan?" if empty. `tracking_issue` is unset until Step 5.

### Step 2: Compute paths and seed state

1. Derive a kebab-case spec slug from the issue title (or task description), ≤ 5 words.
2. `spec_path = ai-docs/plans/<TODAY>-<slug>.spec.md`
3. `state_path = <spec_path>.state.md`
4. Write the initial state file with `round: 1`, `prior_qa: []`, `agent_id: null`, **plus the Step 1 payload**: issue-ref mode emits a `gh_issue:` block populated with `title` / `state` / `labels` / `body` / `comments` / `linked_issues` / `linked_prs`; free-text mode emits a `task_description:` block carrying the user's description verbatim. The two blocks are mutually exclusive — exactly one is present per state file.

### Step 3: Round loop

For each round (1..=`round_cap`):

#### 3a. Invoke the subagent

**Round 1 — cold spawn (primary path):**

```
Agent(
  subagent_type="general-purpose",
  model="opus",
  prompt="""
    Read .claude/agents/spec-writer.md and follow it.

    issue_ref: <#N | "free-text">
    issue_body: |
      <verbatim from gh issue view, OR the user's free-text task description>
    round: 1
    round_cap: 4
    questions_per_round_cap: 3
    prior_qa: []
    spec_path: <spec_path>
  """
)
```

Capture the returned `agentId` into the state file's `agent_id`. If the harness does not return a usable `agentId`, leave it null — rounds 2+ will use the cold-spawn fallback.

**Rounds 2..cap — warm reuse if possible, cold fallback otherwise:**

- If `agent_id` is set in state: `SendMessage(to=agent_id, prompt="""<same fields with updated round + prior_qa>""")`. Capture the response.
- If `agent_id` is null OR the `SendMessage` call fails: cold spawn a fresh `Agent(model="opus", prompt=...)` with the full state in the prompt (the Subagent definition mandates re-derivation from prompt anyway). Update state file's `agent_id` from the new spawn (may again be null).

> The cold-spawn path is the **default contract**; warm reuse is an opportunistic optimization conditional on the harness returning a usable `agentId` and `SendMessage` succeeding.
>
> **Cold-spawn is the contract, not a fallback.** Do **NOT** probe `ToolSearch` for `SendMessage` per round — its absence is stable for the whole session, so re-probing each round is wasted overhead. Do **NOT** emit fallback-framed status lines such as *"SendMessage not available — using cold spawn"* (they read as a regression). If a status line is emitted at all, phrase it neutrally — *"Spawning round N spec-writer."* Treat warm reuse as a silent optimization, never as the headline.

#### 3b. Parse the YAML status block

The subagent's response ends with a fenced YAML block. Extract it; parse `status`, `round`, and `questions` / `reason` as applicable.

**On parse failure** (malformed YAML, missing required fields):

1. **One-shot retry** — `SendMessage(to=agent_id, ...)` (or fresh `Agent` if cold) with prompt: `"Re-emit only the YAML status block, exact schema. Your previous response did not contain a parseable status block at the end."` Parse again.
2. **On second failure** — orchestrator-injects a synthetic `unresolvable` and proceeds to 3d:
   ```yaml
   status: unresolvable
   round: <current>
   reason:
     category: logically_unresolvable
     detail: "Spec-writer subagent emitted unparseable YAML status twice."
     suggested_action: abort
   ```

#### 3c. Branch on status

- **`ready`** → go to **Step 4**.
- **`ask`** → go to **3d** (surface questions).
- **`unresolvable`** → go to **3e** (action chooser).

#### 3d. Surface questions to the user

Validate before forwarding:

- `len(questions) <= questions_per_round_cap` — if exceeded, send the Subagent a one-shot trim instruction (`"Trim to <cap> highest-leverage questions; emit only the YAML status block."`).
- Each `header` ≤ 12 chars; each `options` list has 2..=4 entries (`AskUserQuestion` constraints).
- No question contains a Rule-5 blacklisted substring (final defence — the Subagent should have caught it). On violation: one-shot Subagent re-spawn with explicit instruction to re-read AGENTS.md and the Rule-5 blacklist.

Then call `AskUserQuestion(questions=[...])` with the entire list — the tool supports up to 4 questions per call, so 1..=3 fit cleanly. The user answers all in a single UI exchange.

Append each `(question, answer)` pair to state's `prior_qa` with `round: <current>`. Increment `round`. Loop to 3a.

#### 3e. Action chooser on `unresolvable`

Build an `AskUserQuestion` with:

- The Subagent's `reason.detail` as the question prose.
- Options: the Subagent's `suggested_action` first (recommended), plus the other applicable actions per the table below.

| Category | Actions to offer (recommended first) |
|---|---|
| `cap_reached` | `extend_cap` (recommended), `defer_to_deferred`, `abort` |
| `logically_unresolvable` | `defer_to_deferred` (recommended), `abort`, `request_external_info` |
| `external_dependency` | `request_external_info` (recommended), `defer_to_deferred`, `abort` |
| `empty_scope` | `abort` (recommended), `request_external_info`, `defer_to_deferred` |
| `user_loop` | `defer_to_deferred` (recommended), `abort`, `request_external_info` |

Execute the chosen action:

- **`extend_cap`** — bump `round_cap += 1` in state; loop to 3a with `round: <current> + 1`. The Subagent receives the new `round_cap` and may now `ask` if it has questions.
- **`defer_to_deferred`** — `mv <spec_path> ai-docs/plans/deferred/`; update `INDEX.md` (move row to **Deferred plans**, status `🟡 spec-only`); delete state file; exit. Skip Step 4.
- **`abort`** — delete `<spec_path>` (if exists); delete state file; exit. Skip Step 4.
- **`request_external_info`** — prompt the user via `AskUserQuestion` (single free-form question option) for the additional context; loop to 3a with `extra_context: <user paste>` injected into the next round's prompt.

### Step 4: Cross-link and exit (on `ready`)

1. Show the user the final spec at `<spec_path>` (last 80 lines if long).
2. Confirm — `AskUserQuestion`: "Approve and post cross-link comment?" / { Approve, Tweak first }.
3. On Approve:
   - Resolve the tracking issue if not already pinned (issue-ref mode = already pinned; free-text mode = run the issue-search / propose-new flow):
     ```bash
     gh issue list --state open --search "<keyword>"
     ```
     If a candidate exists, ask user. Otherwise propose a new issue (title from spec name; body from spec scope) and run `gh issue create` after user approval. Capture the number into the spec's `**Tracked in:**` field if it wasn't there.
   - Post the cross-link:
     ```bash
     gh issue comment <N> --body "Spec: \`<spec_path>\`"
     ```
   - Delete the state file.
4. Skill exits. `/task` (the caller) resumes at Step 6 (`design` Subagent).

> **Skip the tracking-issue resolution only if the user explicitly states "no tracking issue".** Note the reason in the spec header (`**Tracked in:** none — <reason>`) and skip the cross-link comment.

## Spec-only run

If the user wants to stop after the interview ("just draft the spec, defer the implementation"):

1. Move the spec to `ai-docs/plans/deferred/`
2. Update `INDEX.md` (move row to **Deferred plans**, status `🟡 spec-only`)
3. Delete the state file
4. Do NOT proceed to Step 6 of `/task`. The spec can be picked up later via `/task`'s deferred-plan-activation preamble.

## Patterns

> **Default to** delegating every question and every spec write to the `spec-writer` Subagent. The orchestrator's role is plumbing — surface the Subagent's questions via `AskUserQuestion` and forward the user's answers as `prior_qa`; never draft a clarifying question yourself, even when the next question feels "obvious" from the user's last answer. Same for the spec body: never edit `*.spec.md` directly — even when the change feels like "just a typo" or "just a tweak the user asked for after `status: ready`". The `spec-writer` Subagent owns ALL writes to `*.spec.md` (mirrors the AXIOM in `.claude/skills/task/SKILL.md` above the Design Amendment header).
>
> _Validated by repeated user correction across multiple rounds: "from now and for future — don't ask by yourself, delegate to subagent" (auto-memory `feedback_interview_delegate_to_subagent.md`). See `ai-docs/learnings.md` 2026-05-24 entries on orchestrator-side direct spec edits._

## Anti-patterns

- Drafting questions yourself in the orchestrator. The subagent owns question authorship; you forward the questions verbatim.
- Mutating the spec yourself. The subagent owns spec writes; the orchestrator only reads it.
- Skipping the YAML status parse and inferring intent from prose. The status block is the contract; treat parse failure as a defect.
- Embedding the Rule-5 substring blacklist in this file. It lives in the Subagent definition; this orchestrator's only Rule-5 role is the validation gate at 3d (defence in depth).
- Forgetting to delete the state file on terminal exit. It's transient handoff; orphaned state files confuse subsequent runs.
- Saving the spec without `**Tracked in:**` (unless user explicitly opted out).
- Skipping the cross-link comment on the tracking issue.
- **Silently switching to implementation mid-interview.** If the Subagent's first round suggests the task is trivially small (< ~20 lines, no design decisions), the Subagent should still emit `ready` with a complete spec; the orchestrator surfaces it normally and the user can choose to spec-only-defer if they want a one-shot edit instead.
