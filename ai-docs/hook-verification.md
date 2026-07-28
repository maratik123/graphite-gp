# Hook Verification

How to prove a proposed `.claude/settings.json` hook actually works. Referenced from
[`.claude/agents/self-improve.md` § Step 4](../.claude/agents/self-improve.md) — every
hook proposal's `Verification:` field points here.

## The three MUSTs

> **A proposed hook is not verified until all three hold.** A green self-authored test
> suite is evidence about your *cases*, never about your *matcher*.

### 1. Lint the body

Extract it with `jq` and run `shellcheck -s bash`. A hook is a `bash -c` program, and the
only gate that lints one is [`/ai-audit` Checklist K](../.claude/skills/ai-audit/SKILL.md) —
which runs post-hoc, on an audit pass, not when you author the hook. Nothing lints it at
authoring time except you.

Iterate over **every event**, not just `PreToolUse` — `PostToolUse` and `SessionStart`
bodies are shell programs too, and at least one carries a live `SC2016`:

```bash
jq -r '.hooks[][].hooks[].command' .claude/settings.json \
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

**Archival evidence does not discharge this MUST either.** A prior hook in this repo that
keys on the same field and works in production, a past incident where the field demonstrably
carried a value, a documented earlier false positive — each establishes that the field WAS
populated on *some other* invocation path, which is the same CAN-for-DOES substitution in a
more convincing costume. The probe is cheap and the substitution is what has actually failed
here: a MUST you authored binds you first and hardest, and the commit introducing a
verification requirement is the worst place to take an exemption from it. If you believe
archival evidence genuinely suffices, **amend this MUST and say so** — do not record a
probe-shaped claim for a probe you did not run. A reviewer endorsing the substitution does
not discharge it (AGENTS.md § *Patterns* 1 — relief invites acceptance).

## Why a green suite is not enough

| What you ran | What it is evidence about | What stays unproven |
|---|---|---|
| N hand-written input JSON payloads | your enumeration of cases | the matcher's behaviour on cases you did not imagine |
| A `claude-code-guide` / doc consult | the harness's documented capability | whether the field is populated on *this* invocation path |
| Archival evidence — a prior hook keying on the same field, a past production incident | that the field was populated on *some other* invocation path | whether it is populated on *this* caller's path |
| `jq . .claude/settings.json` | the file is valid JSON | nothing about the shell program inside it |
