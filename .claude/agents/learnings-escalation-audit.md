---
name: learnings-escalation-audit
description: "Verifies that every entry in ai-docs/learnings.md has accurate `Escalated?` and `Superseded by:` fields — `Escalated?` targets contain the rule; `Superseded by:` references resolve to a real later entry or merged PR. Fixes drift in-place (edits only the `Escalated?` and `Superseded by:` lines of affected entries; never touches date, category, description, what-happened, or rule text). Authorised by AGENTS.md § Learning Log Boundary rule 1 Exception. Invoked by /ai-audit Phase 1. Does not write project code."
model: opus
---

# Learnings Escalation Audit

Reactive audit subagent. Walks every entry in `ai-docs/learnings.md` and checks whether the **`Escalated?`** field still tells the truth — the named destination must contain a rule that addresses the recorded mistake.

**Do NOT write project code.** Only read instruction files; only edit `ai-docs/learnings.md` and the instruction file the entry points at, and only when the fix is mechanical.

## Inputs

Read up front:

1. `ai-docs/learnings.md` — full learning log (entries are append-only; do not delete).
2. `AGENTS.md` — current project rules.
3. `.claude/settings.json` — hooks + permissions.
4. Every `.claude/skills/*/SKILL.md` and `.claude/agents/*.md` — the targets that entries may point at.

## What `Escalated?` can say

Per AGENTS.md "Learning Log":

| Value | Means | Verification |
|---|---|---|
| `no` | Not yet acted on (no project-level rule). May also have been saved to user-local persistence (`~/.claude/.../MEMORY.md`, `settings.local.json`) — those are private to one developer and DO NOT count as project-level escalation. | Nothing to verify — but flag if the same mistake repeats ≥2 times unescalated. |
| `AGENTS.md` | Rule lives in `AGENTS.md`. | Find a section/sentence in `AGENTS.md` that addresses the mistake. |
| `skill:[name]` | Rule lives in `.claude/skills/<name>/SKILL.md`. | File exists; rule is there. |
| `agent:[name]` | Rule lives in `.claude/agents/<name>.md`. | File exists; rule is there. |
| `rules:[name]` | Rule lives in `.claude/rules/<name>.md`. | File exists; rule is there. |
| `hook` | Rule is a hook in `.claude/settings.json`. | A hook with a matcher + command that addresses the mistake exists. |
| `settings` | Rule is a non-hook setting (permission allow/deny, env). | Listed in `.claude/settings.json` `permissions.*` or `env`. |
| `doc-convention` | Rule lives in `ai-docs/doc-convention.md`. | File exists; rule is there. Use only for documentation-style rules that genuinely belong in the workspace doc-convention reference. |
| `code-style` | Rule lives in `ai-docs/code-style.md`. | File exists; rule is there. Use only for code-style rules that genuinely belong in the workspace code-style reference. |

Multiple values are comma-separated (`AGENTS.md, hook`). Each must independently verify.

## Workflow

### Step 1: Parse entries

Each entry in `ai-docs/learnings.md` follows:

```
### YYYY-MM-DD — [category] — [short description]
**What happened:** ...
**Rule:** ...
**Escalated?** ...
```

Extract `(date, category, description, rule, escalated)` for each entry.

### Step 2: Verify each escalation target

For each entry where `Escalated?` is **not** `no`:

- For each target in the comma-separated list:
  - **`AGENTS.md`** — `rg -n "<keyword from rule>" AGENTS.md`. The rule keyword should be a distinctive phrase from the `Rule:` field, not generic (avoid "test", "commit"). If no match → mismatch.
  - **`skill:<name>`** — verify `.claude/skills/<name>/SKILL.md` exists, then grep for keyword. If file missing → blocker (file deleted or renamed). If file exists but no keyword → mismatch.
  - **`agent:<name>`** — same as `skill:` against `.claude/agents/<name>.md`.
  - **`rules:<name>`** — same as `skill:` against `.claude/rules/<name>.md`. If file missing → blocker (file deleted or renamed). If file exists but no keyword → mismatch.
  - **`hook`** — read `.claude/settings.json`, scan `hooks.*[].hooks[].command` for the keyword. If no hook references the relevant tool/behavior → mismatch.
  - **`settings`** — scan `.claude/settings.json` `permissions.allow`, `permissions.deny`, `env`. Mismatch if absent.
  - **`doc-convention`** — verify `ai-docs/doc-convention.md` exists, then grep for keyword. If file missing → blocker. If keyword absent → mismatch.
  - **`code-style`** — verify `ai-docs/code-style.md` exists, then grep for keyword. If file missing → blocker. If keyword absent → mismatch.

Record each entry's status:

- ✅ **OK** — every target verified.
- ⚠️ **Mismatch** — claimed target exists but rule absent (rule was removed or never landed there).
- ❌ **Broken** — claimed target file/Skill/Subagent does not exist at all (renamed/deleted).
- ❓ **Ambiguous** — keyword too generic to verify mechanically; needs human read of the rule.
- 🌱 **Stale-validation** — `Kind: validation` entry whose `Escalated?` is `no`, whose entry-date is **> 30 days** old, AND whose targeted surface has had **≥1 instruction-file commit since the validation date** (see Step 2b). Signal for `/improve` (the validation has aged without project-level promotion despite ongoing instruction-file churn near its surface), not an auto-fix.

For each entry that has a `**Superseded by:**` line, ALSO verify the reference resolves:

- **`YYYY-MM-DD` ref** — at least one OTHER entry in `ai-docs/learnings.md` shares that date AND (when a disambiguation slug is present) contains the slug text in its description. If no match → ⚠️ Mismatch on `Superseded by:`.
- **`PR #N` ref** — `gh pr view <N> --json state --jq '.state'` returns `MERGED`. If not merged or not found → ⚠️ Mismatch on `Superseded by:`.
- **Both date and PR comma-separated** — both must resolve.

Entries WITHOUT a `Superseded by:` line are not flagged here — absence is the default; only presence-with-broken-ref is drift.

### Step 2b: Stale-validation sweep

For every entry whose `**Kind:**` line says `validation`, evaluate three conjuncts:

1. **Age conjunct.** Entry-date age **> 30 days** from today (use the date in the entry's `### YYYY-MM-DD — ...` header).
2. **Escalation conjunct.** `Escalated?` is `no`.
3. **Instruction-file activity conjunct.** ≥1 commit touching the audited instruction-surface corpus since the entry-date.

Compute the activity conjunct with `git log --since=<entry-date> --pretty=oneline -- AGENTS.md ai-docs/ .claude/` — count the commits in the output; non-zero satisfies the conjunct. Constrain the path list to the surface the validation entry's `Rule:` line names when a specific Skill / Subagent is named (e.g., `Rule:` names `/context-reset` → constrain to `.claude/skills/context-reset/`); fall back to the whole corpus path list above when the `Rule:` line is ambiguous.

If **all three** conjuncts hold → emit `🌱 Stale-validation`. If the entry is `Kind: validation` but **fewer than three** conjuncts hold → no flag. Legacy entries (`Kind:` omitted → default `correction`) are out of scope for this sweep.

If the `Rule:` line is ambiguous (no specific Skill/Subagent named AND no AGENTS.md section named) AND the entry would otherwise flag → fall back to `❓ Ambiguous` instead of `🌱 Stale-validation` (the audit cannot mechanically narrow the surface; user judgment needed).

🌱 entries are surfaced to `/improve`; the audit does NOT auto-fix them (the promotion is a Step 2b Carrot-pass decision in `self-improve`, not an `Escalated?` field correction).

### Step 3: Categorise + propose fixes

For each non-OK entry, propose ONE of:

1. **Update `Escalated?` or `Superseded by:` field only.** The rule landed somewhere else (e.g., the entry says `AGENTS.md` but the rule is now in `skill:project-review`). Fix the field to reflect reality. **Also fix obvious typos within the `Escalated?` or `Superseded by:` values** — e.g., `AGENTS,md` → `AGENTS.md`, `skillcode-review` → `skill:project-review`, missing comma between two targets, mistyped date in `Superseded by:` (verifiable against later entries), `PR #N` where N is off-by-one and the correct PR is unambiguously the intended one. Treat typo correction as drift, not as a rewrite. **Never add a `Superseded by:` line that wasn't already there** — adding the field is `/improve`'s job (it has the context to decide a supersession occurred); the audit only fixes drift in existing fields.
2. **Re-add the missing rule.** The rule was lost during a refactor. Add it back to the named target.
3. **Surface to user.** The entry is ambiguous, the fix would be substantive (rewriting a rule, changing a hook), or it might be a `/improve` job rather than an audit fix.

Apply category 1 (field correction) autonomously — it's a documentation truth fix.
Apply category 2 only if the rule and target are obvious; otherwise surface.
Always surface category 3.

### Step 4: Apply approved field corrections

For each category-1 fix, edit `ai-docs/learnings.md` in place — change only the `**Escalated?**` or `**Superseded by:**` line for that entry. Preserve everything else exactly. Do **not** rewrite the date, what-happened, or rule fields. Do **not** add a `**Superseded by:**` line where none was present — that is `/improve`'s job. This edit is authorised by **AGENTS.md § Learning Log → Boundary rule 1 → Exception** (`Escalated?` and `Superseded by:` fields, Subagent-driven only); typo fixes within either value are in scope of the same exception.

### Step 5: Cross-checks

Beyond per-entry verification, also flag:

- **Duplicate entries** — same mistake recorded twice on different dates with different `Escalated?` values. Surface; do not auto-merge.
- **Repeating mistakes despite escalation** — same `category` + `description` keyword recurring after the rule was added. This is a `/improve` signal, not an audit fix; just surface.
- **Stale `skill:` / `agent:` references** in the log — entry names a Skill/Subagent that no longer exists. Surface for user judgment (rename vs. re-add).

### Step 6: Report

Produce a structured report back to the calling skill:

```
## Phase 1 — escalation audit summary

- Entries audited: N
- ✅ OK: N
- ⚠️ Mismatch: N (auto-fixed: M, surfaced: K)
- ❌ Broken: N (auto-fixed: M, surfaced: K)
- ❓ Ambiguous: N (all surfaced)
- 🌱 Stale-validation: N (all surfaced — signals for /improve)

## Auto-applied fixes
- [date] [description] — `Escalated?` was `X`, changed to `Y`. Reason: ...

## Needs user judgment
- [date] [description] — [problem]. Suggested options: A / B / C.

## Stale-validation signals (🌱 — for /improve, not for this skill)
- [date] [description] — `Kind: validation`, age N days, ≥M instruction-file commits since entry-date on surface [skill:foo | agent:bar | AGENTS.md | whole corpus]. Suggested: route through `self-improve` Carrot pass Step 2b.

## Cross-check signals (for /improve, not for this skill)
- ...
```

## Anti-patterns

- **Do NOT delete or reword entries.** The log is append-only history.
- **Do NOT change the `Rule:` text** to match a drifted instruction. If the rule changed, that's a separate `/improve` concern, not an audit fix.
- **Do NOT escalate up severity** — an audit fix is mechanical; if a real rule needs to be re-added, propose it and let the user approve.
- **Do NOT auto-merge duplicate entries.** Surface.
- **Do NOT touch `.claude/settings.local.json`.** User-local.
- **Do NOT commit.** The calling skill bundles Phase 1 + Phase 2 changes into one commit.
- **Do NOT flag in-flow `/task` learning entries as Boundary Rule 2 violations.** AGENTS.md § Learning Log Boundary rule 2 Exception (added 2026-05-13) authorises `/task` Steps 8–12 — **and any sub-skill (e.g., `/bugfix`, `/context-reset`) invoked from within that range** — to append a NEW entry in the same turn as instruction-file edits when the entry is marked `Escalated? no` and documents an in-flight insight. Such entries are normal candidates for `/improve` escalation, not corpus violations — surface them under "Cross-check signals (for /improve, not for this skill)" if they look ripe, never under "Auto-applied fixes" or "Needs user judgment / Boundary Rule 2 violation".
