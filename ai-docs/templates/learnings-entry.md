# `learnings.md` — entry template

> Canonical copyable skeleton for an `ai-docs/learnings.md` entry. **Consult this template to inspect the format — do NOT open the live `ai-docs/learnings.md` to reverse-engineer it** (it is append-only and long). Full boundary rules + when-to-write: [`AGENTS.md` § Learning Log](../../AGENTS.md#learning-log). Per-field semantics: [`corrections-log.md` → Entry format — field glossary](../corrections-log.md#entry-format--field-glossary).

## Skeleton — copy, fill, append at the END of the log

```
### YYYY-MM-DD — [category] — [short description]
**What happened:** [quote or paraphrase]
**Rule:** [what to do instead, or what to keep doing]
**Kind:** correction | validation    (optional; defaults to `correction` when omitted)
**Escalated?** no | AGENTS.md | skill:[name] | hook | settings | agent:[name] | rules:[name] | doc-convention | code-style (comma-separate multiple)
**Superseded by:** [ref] — [one-line reason]    (optional; omitted when not applicable)
```

- **Categories:** `code-style` | `process` | `architecture` | `testing` | `documentation` | `tooling` | `search` | `other`
- **`Kind:`** defaults to `correction` when omitted. `validation` = a working protocol/pattern to keep doing (carrot signal); `correction` (or omit) = a violation to stop doing (stick signal).
- **`Escalated?`** records **project-level** persistence only — user-local auto-memory and `settings.local.json` do **not** count → stay `no`.
- **`Superseded by:`** omit unless a later entry/PR reverses, refines, generalizes, subsumes, or withdraws this entry's rule.

## Filled example

```
### 2026-07-16 — process — treating a reviewer's retractions and suggestions as skeptically as its findings
**What happened:** Accepted a reviewer's withdrawal of a finding without re-running the command that would confirm it; the withdrawal was wrong.
**Rule:** Verify a reviewer's retractions / salvage suggestions / "harmless" calls with the same command you would run against its original finding.
**Kind:** validation
**Escalated?** AGENTS.md
```

## Before you write — two boundary rules (full text in AGENTS.md § Learning Log)

- **Append-only** (Boundary rule 1): NEVER edit, rewrite, reorder, summarise, or delete an existing entry. A newer correction/supersession is a **new** entry. (Exception: the `Escalated?` / `Superseded by:` fields, updated in-place only by the `self-improve` / `learnings-escalation-audit` subagents.)
- **No same-turn escalation** (Boundary rule 2): writing a new entry triggers **no** instruction-file edits in the same turn (set `Escalated? no` and stop). Exceptions: `/improve` + `/ai-audit` existing-entry field updates, and the in-flow capture carve-out during `/task` Steps 8–12.
