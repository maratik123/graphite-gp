# Checklist M — `agent-writing-style.md` conformance

> Split out of [`reference.md`](reference.md) — that page was 36,054 chars, over the 35,000 early-warning band, and this section was a third of it. Linked from [`SKILL.md`](SKILL.md) checklist row M.


`ai-docs/agent-writing-style.md` is the canonical style reference for fail-loud rules in instruction files. Checklist M sweeps the audited corpus for drift against the 7 Patterns + Anti-patterns table. **Audited corpus** (named inline; do NOT defer to Step 2.2's inventory which omits some of these): `AGENTS.md` + every `.claude/skills/**/SKILL.md` + every `.claude/agents/**.md` + `ai-docs/code-style.md` + `ai-docs/doc-convention.md` + `ai-docs/agent-writing-style.md` + `ai-docs/corrections-log.md` + `.claude/rules/**/*.md`.

| # | Sub-check | Detection mechanism | Severity |
|---|---|---|---|
| 1 | **Pattern 1 (AXIOM blockquote)** — every `> **AXIOM —`-prefixed block must be followed by an action table within the same blockquote. | For each match of `rg -n '^> \*\*AXIOM —'`, read up to 30 lines following the AXIOM line (the blockquote body is usually `> `-prefixed but the window is a fixed 30-line lookahead — do NOT terminate early on a `>`-only separator line, which is a valid blockquote-internal line). If no `> \|` table row appears within the window, flag the AXIOM line. | `major` |
| 2 | **Pattern 2 (fail-loud verbs)** — at most one bold-uppercase verb per paragraph (`**NEVER**` / `**MUST**` / `**MUST NOT**` / `**FORBIDDEN**` / `**STOP**` / `**REJECT**` / `**REMOVE**` / `**REPLACE**` / `**DELETE**` / `**ALWAYS**`). | **Pre-filter table rows** (lines matching `^\|` or `^> \|` — markdown table rows or blockquoted table rows). Tables are governed by Pattern 3, not Pattern 2; each row is its own action-verb scope. After the pre-filter, `awk` splits the remaining content on blank lines into paragraph chunks. For each chunk, count matches of `\*\*(NEVER\|MUST\|MUST NOT\|FORBIDDEN\|STOP\|REJECT\|REMOVE\|REPLACE\|DELETE\|ALWAYS)\*\*`. If count > 1, flag the paragraph (file:start-line). | `minor` |
| 3 | **Pattern 3 (action tables)** — the right column of every `\| If you see... \| Action \|` (or analogous) table must start with an action verb (imperative form), NOT prose narrative. | For each `\| If you see` table row, extract the right-column cell. Heuristic: starts with one of `Run`, `Apply`, `Stop`, `Add`, `Remove`, `Edit`, `Confirm`, `Bail`, `STOP`, `**NEVER**`, `**MUST**`, etc. OR a backtick-quoted command. If the cell starts with prose narrative (e.g., `"This is..."`, `"Usually..."`), flag the row. | `minor` |
| 4 | **Pattern 4 (explicit file lists, never globs)** — fail-loud lists that enumerate files must spell out each path; no glob-as-the-entire-list. | For each fail-loud block (paragraph containing a Pattern 2 verb), scan immediate `- ` or `* ` bullet list. If the entire list reduces to one or two globs (`.claude/**`, `**/*.rs`) with no specific paths, flag. (Per-bullet parenthetical globs like `.claude/skills/** (any file under this directory)` are acceptable.) | `major` |
| 5 | **Pattern 5 (numbered enumeration of triggers)** — OR/AND connector placement must be consistent across items. | For each numbered list (`^1\.`, `^2\.`) inside a fail-loud block, check that EITHER every non-last item ends in `, OR` (or `, AND`), OR no items carry the connector. Mixed placement (some items connector-suffixed, some not) → flag. | `nit` |
| 6 | **Pattern 6 (do/not examples for non-trivial rules)** — paragraphs that articulate a contrast between two shapes must demonstrate both shapes. | **Tightened heuristic** (per design-review note 4 on the `maratik123/quartzite#369` PR — `not`/`NOT` alone are too noisy, firing on every "do not" / "must not" / "is not" paragraph): trigger iff the paragraph contains **BOTH** (a) a Pattern 2 fail-loud verb AND (b) one of the stronger contrast markers `instead` / `wrong` / `correct` / `forbidden`. Then check if a fenced code block OR a two-column `\| Do this \| NOT this \|` table follows within 8 lines. If both triggers fire AND no example follows, flag the paragraph. (Words `not` / `NOT` / `right` / `bad` / `good` are NOT in the trigger list — they produce false positives at unacceptable scale.) | `nit` |
| 7 | **Pattern 7 (compaction recovery callout)** — every callout-carrying skill must carry exactly one of the three locked variant-distinguishing phrases. | **Drive off the live grep, NOT the style guide table.** For each `.claude/skills/*/SKILL.md` whose body contains the literal string `Compaction recovery check`, run `rg -F` against the three variant-distinguishing phrases (verbatim, as carried by the code-side skill files that use each variant): Variant A = `"Locate the durable-state file via this skill's active-state probe"`; Variant B = `"If exactly one in-flight artefact exists"`; Variant C = `"Identify the **parent workflow**"`. If a callout-carrying skill contains zero or > 1 of the phrases → flag (likely invented 4th variant OR Variant-A/B/C drift). Also flag any callout-carrying skill not enumerated in the style guide Pattern 7 table at `ai-docs/agent-writing-style.md` lines 119–121 (style guide drift; the table should grow when a new skill onboards the callout). | `major` |
| 8 | **Anti-patterns table audit** — no row of the Anti-patterns table (`ai-docs/agent-writing-style.md § Anti-patterns, lines 157–167`) should appear verbatim as a positive rule anywhere in the audited corpus. | For each anti-pattern row's left-column text (e.g., `"Every paragraph in caps"`, `"AXIOM blockquote without action table"`), grep the audited corpus for matches NOT inside the style guide itself. Flag matches. | `major` |
| 9 | **Pattern 8 (file-size AXIOM conformance)** — every covered instruction file must stay below the 40,000-char hard cap; the 35,000-char band is an early warning. Rule-of-truth: `ai-docs/agent-writing-style.md § 8. 40k char-cap on instruction files`; source AXIOM: `AGENTS.md § Build & Test`. | Run the verbatim `wc -c` invocation below against the covered file set, apply the three-band severity table. See § *Sub-check 9 — file-size AXIOM conformance* below for the recipe + severity bands. | see body |
| 10 | **Style-guide audit coverage map** — every `## ` (level-2) heading in `ai-docs/agent-writing-style.md` must map to either an existing Checklist M sub-check or to the explicit exclusion list of non-rule-bearing meta-sections. Unmapped headings produce `nit` "audit coverage gap" findings. | Parse ATX `## ` headings from the **live** `ai-docs/agent-writing-style.md` (re-grep at audit time; do NOT use a baked-in snapshot). Apply the inline coverage map below. See § *Sub-check 10 — style-guide audit coverage map* below for the parser recipe + map + finding format. | `nit` |
| 11 | **Cross-shape verbs** — carrot-shaped rules (entries in a `## Patterns` section) MUST NOT use stick verbs; stick-shaped rules (AGENTS.md AXIOM blockquotes or fail-loud bodies) MUST NOT use carrot verbs. The verb asymmetry IS the asymmetric-promotion contract — a wrong-shape verb either underweights a real obligation or locks in a brittle default as a hard rule. | (a) **Carrot block with stick verb:** for each `### N. <Name>` entry under a `## Patterns` section in the audited corpus, grep the entry body for `**MUST**` / `**NEVER**` / `**MUST NOT**` / `**FORBIDDEN**` — any match flags the entry. **Named exempt source.** `ai-docs/agent-writing-style.md` is the template-source style guide and is exempt from the (a) sweep — Patterns inside it legitimately *describe* stick verbs (Pattern 2 verb-table self-description) and *quote* stick-rule examples (Pattern 4 quoting Boundary rule 2). This mirrors Checklist N's *Forward (named exempt source)* row. (b) **Stick block with carrot verb:** for each `> **AXIOM —` blockquote (and its action-table body) outside `## Patterns` sections, grep for `Default to` / `Prefer` — any match flags the blockquote. Both directions flagged at the same severity. The detection cross-checks the Kind shape (Patterns block ↔ Kind: validation entry; AXIOM block ↔ Kind: correction entry) against the verb pattern. | `major` |

After running Checklist M, surface findings using the same severity-driven apply-or-ask pattern as Checklists A–L (Step 2.5). Pattern 6 noise-management fallback: if AC5's demonstrator run shows > 50% false-positive rate on Pattern 6 findings, record the rate and tighten the heuristic in a follow-up `/improve` cycle (the heuristic itself is encoded here, not in a separate config file — design choice to keep the audit self-contained).

### Sub-check 9 — file-size AXIOM conformance

Detection mechanism. Run this verbatim invocation:

```bash
wc -c AGENTS.md CLAUDE.md .claude/skills/**/*.md .claude/agents/*.md \
      .claude/rules/*.md \
      ai-docs/code-style.md ai-docs/doc-convention.md ai-docs/context.md \
      ai-docs/agent-writing-style.md ai-docs/corrections-log.md
```

Apply the three-band severity table to every reported size:

| Reported size (chars) | Finding | Severity |
|---|---|---|
| `< 35,000` | none | — |
| `35,000–39,999` | `<path>: <count> chars — early warning (≥ 35,000)` | `minor` |
| `≥ 40,000` | `<path>: <count> chars — AXIOM violation (≥ 40,000)` | `major` |

The covered file set is enumerated verbatim from `AGENTS.md § Build & Test` (the source-of-truth AXIOM) and restated in `ai-docs/agent-writing-style.md § 8. 40k char-cap on instruction files`. A future change to the covered file set MUST update Sub-check 9 in the same PR per the Propagation Rule.

Note: the shell-glob form (`.claude/skills/**/*.md`, `.claude/agents/*.md`, `.claude/rules/*.md`) is acceptable here because Pattern 4's explicit-path requirement applies to the *fail-loud bullet list* in Pattern 8 (so static readers see the covered set), not to the shell command that consumes the set.

Sub-check 9 is the sole enforcement surface for the size-cap rule — there is no separate mechanical gate. The audit fires per-`/ai-audit`-run.

### Sub-check 10 — style-guide audit coverage map

Detection mechanism. The audit reads `ai-docs/agent-writing-style.md` at audit time and parses every ATX `## ` heading.

Parser strictness rules:

1. Match **ATX-style level-2 headings only** — exactly two `#` characters followed by exactly one space, then heading text.
2. **Skip lines inside fenced code blocks.** Track ` ``` ` and `~~~` fence state; a `## ` line inside an open fence is NOT a heading.
3. **Case-sensitive match.** `## Patterns` ≠ `## patterns`.
4. **Trim** leading/trailing whitespace from heading text before lookup.

Inline coverage map (live as of this commit; re-validate at audit time by re-running `grep -n '^## ' ai-docs/agent-writing-style.md` and reconciling against this map):

| `## ` heading | Maps to | Outcome |
|---|---|---|
| `## Patterns` | sub-checks 1–7 (audits the shape of every entry under this heading, including the new Pattern 8 via Patterns 1–4 self-conformance) | no finding |
| `## Anti-patterns` | sub-check 8 | no finding |
| `## Writing checklist` | excluded — meta-section (reader checklist, not a rule shape) | no finding |
| `## Citation in PRs` | excluded — meta-section (PR-author convention, not a rule shape) | no finding |
| `## Enforcement` | excluded — meta-section (cross-references the audit itself) | no finding |
| `## Propagation rule for new patterns` | excluded — meta-section (fan-out procedure, not a rule shape) | no finding |
| `## Out of scope` | excluded — meta-section (negative-space scoping, not a rule shape) | no finding |

Unmatched-heading rule. For every parsed `## ` heading NOT in the coverage map above, emit:

- **Finding text:** `audit coverage gap: § <heading>`
- **Proposed action:** `add sub-check N+1 to /ai-audit Checklist M` (where N is the current max sub-check number)
- **Severity:** `nit`

When a future PR adds a new `## ` heading to `agent-writing-style.md`, Sub-check 10 fires at the next `/ai-audit` run with the gap; the operator either adds a corresponding sub-check or extends the exclusion list in the same follow-up.

### Checklist M — audited corpus

The audited corpus enumeration is identical to the Checklist M intro paragraph above:

- `AGENTS.md`
- every `.claude/skills/**/SKILL.md`
- every `.claude/agents/**.md`
- `ai-docs/code-style.md`
- `ai-docs/doc-convention.md`
- `ai-docs/agent-writing-style.md`
- `ai-docs/corrections-log.md`

