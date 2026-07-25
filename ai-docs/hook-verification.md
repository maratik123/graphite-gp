# Hook Verification

How to prove a proposed `.claude/settings.json` hook actually works. Referenced from
[`.claude/agents/self-improve.md` § Step 4](../.claude/agents/self-improve.md) — every
hook proposal's `Verification:` field points here.

## The three MUSTs

> **A proposed hook is not verified until all three hold.** A green self-authored test
> suite is evidence about your *cases*, never about your *matcher*.

### 1. Lint the body

Extract it with `jq` and run `shellcheck -s bash`. A hook is a `bash -c` program and
nothing else in this workspace lints it — [`/ai-audit` Checklist K](../.claude/skills/ai-audit/SKILL.md)
covers `scripts/*.sh` only, never `settings.json`-inlined bodies.

```bash
jq -r '.hooks.PreToolUse[].hooks[].command' .claude/settings.json \
  | while IFS= read -r body; do printf '%s\n' "$body" | shellcheck -s bash - || true; done
```

### 2. Exercise it in the real environment

A corpus you wrote is drawn from the same imagination that wrote the bug. Run the *real*
commands you expect to pass — **including innocent ones that merely CONTAIN the matched
substring** (`grep -rn 'crates.io/api'` is not an HTTP call) — and confirm they are not
blocked.

The `crates-io-ua` incident (`ai-docs/learnings.md` 2026-07-16) is the reference failure:
15 self-invented cases all passed, then the live hook blocked its own author's `grep`, and
four more valid `curl` spellings were later found wrongly blocked.

### 3. Prove the load-bearing INPUT FIELD is populated — passively

For any hook keyed on a harness-supplied field (`agent_type`, `subagent_type`,
`tool_input.*`), deploy a temporary **non-blocking** hook that LOGS the field on a benign
allowed action (`git status`), read the log, then revert the diagnostic.

> **NEVER** verify by instructing a compliant actor to issue the action the guard blocks.
> If the guard is inert, the action really executes — a real push, a real PR, a real spawn.
> And if the actor is compliant it refuses at its charter, *upstream* of the hook, so the
> block stays unobservable either way. Both halves of that dilemma were hit live
> (`ai-docs/learnings.md` 2026-07-21): the first probe told a real `code-writer` to issue
> the banned commands, and it correctly refused at the charter, proving nothing about the
> hook.

A doc consult (`claude-code-guide`, harness documentation) establishes that the harness
**can** populate the field. It is not evidence the field **is** populated for this caller —
that is a CAN claim standing in for a DOES claim.

## Why a green suite is not enough

| What you ran | What it is evidence about | What stays unproven |
|---|---|---|
| N hand-written input JSON payloads | your enumeration of cases | the matcher's behaviour on cases you did not imagine |
| A `claude-code-guide` / doc consult | the harness's documented capability | whether the field is populated on *this* invocation path |
| `jq . .claude/settings.json` | the file is valid JSON | nothing about the shell program inside it |
