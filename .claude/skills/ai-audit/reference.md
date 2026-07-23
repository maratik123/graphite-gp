# `/ai-audit` reference

Static reference content extracted from `SKILL.md`. Loaded on demand when `/ai-audit` Phase 2 Step 2.3 hits a specific checklist letter, or when Step 2.6 sub-step 4 needs to re-verify cross-references anchor-aware.

The `SKILL.md` body keeps a collapsed table of every checklist letter + one-line purpose + the matching anchor below. Read the SKILL.md table first; jump here only for the detail body of a letter that surfaced a finding.

## Checklist A — Cross-reference integrity

- Every relative link (`../`, `./`, file references in prose) resolves to an existing file. Verify with `realpath` from the link's source directory or with `find`.
- Every `[text](file.md)` and bare `file.md` mentioned in instructions points to a file that exists.
- Every Skill or Subagent named in another Skill/Subagent (e.g., `project-review` references `review-findings`, `self-review`) actually has a matching file.

## Checklist B — Conflicting / duplicated rules

- The same topic must not have contradictory guidance in two places (e.g., commit policy in AGENTS.md vs. in a skill).
- Verbatim-duplicated rules across files → consolidate to one canonical home + reference.
- Rule says "see `<other-file>`" — confirm the target file actually contains that rule.

## Checklist C — Dead references

- Skills/agents named in `AGENTS.md` "Sync groups" must exist. (E.g., the AGENTS.md note about `task` ↔ `task-issue` collapse — verify no stale references remain.)
- Agent names referenced in skills must match a file under `.claude/agents/`.
- `ai-docs/plans/done/` references in Subagent checklists must still resolve.

## Checklist D — Frontmatter conformance (skills)

Per the official docs:

- Every `SKILL.md` has YAML frontmatter with at minimum `name` and `description`.
- `description` should make trigger conditions clear (when to invoke).
- `disable-model-invocation: true` ↔ skill is user-only — verify intent matches.
- `argument-hint` style is consistent across skills.
- `allowed-tools` syntax matches `ToolName(pattern)` form documented at `code.claude.com/docs/en/skills`.

## Checklist E — Frontmatter conformance (agents)

- Every `.claude/agents/*.md` starts with YAML frontmatter (`---`-delimited block at top of file). A file in `.claude/agents/` without frontmatter is not a Subagent — it's a stray document. Enumerate the directory at audit time rather than baking in a count.
- `name` field equals the file basename.
- `description` is one line and tells the orchestrator when to spawn this Subagent.

## Checklist F — Hooks (`.claude/settings.json`)

- Each hook event name is one of the documented set (`SessionStart`, `PreToolUse`, `PostToolUse`, etc. — confirm against `hooks-guide`).
- Matchers are valid tool name patterns.
- Hook commands fail closed (`exit 2` for blocking) where intended; non-blocking informational hooks use stderr without `exit 2`.
- Timeouts are reasonable (≤30s default, longer only when the work demands it).
- Commands quote `$CLAUDE_PROJECT_DIR` and other env vars correctly — no shell injection footguns.

## Checklist G — AGENTS.md "Propagation Rule" coherence

- Every "sync group" listed in AGENTS.md still has all listed members present and cross-referenced.
- Behaviors described in AGENTS.md and replicated in Subagent checklists agree (e.g., file-size hard/soft limits in `review-findings.md` match AGENTS.md).
- Exemptions in AGENTS.md (e.g., `examples/` and `benches/` no-test exemptions, trait-impl doc-convention exemption) appear in every enforcement file.

## Checklist H — Documentation conformance pointers

- `ai-docs/doc-convention.md` is referenced by `review-findings.md` and `self-review.md`. Confirm the relative paths resolve.
- The canonical rustdoc section order is `ai-docs/doc-convention.md § DOC-2 — Section order (strict)` (its sole authoritative home — AGENTS.md carries no duplicate order list). Confirm any file that restates the order agrees with DOC-2 exactly.

## Checklist I — File-size & structure (instruction files)

- No `SKILL.md` or Subagent file exceeds ~500 lines without clear sectioning. Long files should split into a thin `SKILL.md` + reference file (the `improve` / `project-review` / `task` Skills use this pattern).
- Each skill directory contains exactly one `SKILL.md` (no extra markdown unless intentional reference material).

## Checklist J — Allow-list / permission consistency

- Tools used in skills' `allowed-tools` should be present (or coverable by) the `permissions.allow` list in `settings.json` — otherwise the user gets a prompt every time.
- Conversely: any allow-listed pattern that no skill actually uses is dead and should be reviewed.

## Checklist K — Skill-directory layout (SKILL.md + supporting files + scripts/)

Per the [Claude Code skill-directory pattern](https://code.claude.com/docs/en/skills#add-supporting-files), a skill directory may contain SKILL.md plus supporting files (reference docs loaded on demand, scripts the skill executes, examples Claude can read). Audit checks:

1. **Oversized SKILL.md check.** Severity `minor`. Run `wc -l .claude/skills/*/SKILL.md` against the live tree. For each file > 200 lines, emit a `minor` finding: scan for *reference content* sections (format specs, parser rules, lookup tables, long checklists, embedded templates) — material that is referenced once or twice in the workflow but loaded into context on every invocation — and propose extraction to a supporting file (a `reference.md` sibling loaded on demand, as `improve` / `project-review` / `task` already do). graphite-gp keeps no skill-size exemption index; if an oversized SKILL.md is intentional, note the rationale inline in the finding rather than tracking it in a separate file.

2. **Multi-consumer supporting files belong in `ai-docs/templates/`.** When a supporting file is referenced from **>1 Skill or Subagent**, propose moving it from the owning Skill's directory to `ai-docs/templates/<file>.md` (per AGENTS.md *Agent Docs*). Single-consumer supporting files stay inside the owning Skill directory. Cross-references then point at `ai-docs/templates/` directly instead of routing through another Skill's body. Severity `minor`.

3. **Inline-script extraction candidates.** Identify `SKILL.md` sections containing **self-contained `bash` blocks** — a complete, executable recipe with at most one or two `<placeholder>` substitutions, NOT orchestration guidance that Claude reconstructs dynamically per call. For each, propose extraction to `.claude/skills/<skill>/scripts/<descriptive-name>.sh`, invoked via the canonical `${CLAUDE_SKILL_DIR}/scripts/<name>.sh <args>` pattern. After extraction, narrow the skill's `allowed-tools` from per-command patterns to a single `Bash(.claude/skills/<skill>/scripts/<name>.sh *)` entry. Severity `minor`.

   **Counter-rule.** Bash snippets that are *orchestration guidance* — every call has different placeholder values that the Subagent constructs — are NOT script-extraction candidates. Forcing them into scripts requires the Subagent to call a helper for guidance it can express inline. Skip those.

## Checklist L — Learning-Log field coherence

When AGENTS.md § Learning Log's *Entry format* block lists a field maintained by `/improve` and `/ai-audit` (currently `Escalated?` and `Superseded by:`), verify each field is covered in **all four** mandatory locations:

| Location | Required content |
|---|---|
| AGENTS.md *Boundary rule 1 Exception* | Explicit authorization to edit the field in-place |
| AGENTS.md *Boundary rule 2 Exception* | Explicit authorization for the field's edits to coexist with instruction-file edits in the same `/improve` / `/ai-audit` turn |
| `.claude/agents/self-improve.md` Step 5 (Commit B backfill) | Workflow describing when and how `/improve` writes the field |
| `.claude/agents/learnings-escalation-audit.md` Steps 2/3/4 | Verification recipe + Category-1 drift fixes (including typo fixes within the field's value) |

A field added to the entry format without parallel coverage in all four targets → `major` finding (rules diverge across the surface; one of the two gates fails silently). The historical proof point: F1 in this PR — Boundary rule 2 Exception text lagged behind Boundary rule 1 Exception after the `Superseded by:` field landed, leaving Boundary rule 2 under-describing the contract.

**Declared-schema fields (no `/improve`-time mutation).** Some Entry-format fields are *declared* by AGENTS.md and *parsed* by the two agents but are **never** mutated by `/improve` or `/ai-audit` after the entry is written (currently: `Kind:`). For each such field, verify coverage in this analogous 4-location list:

| Location | Required content |
|---|---|
| AGENTS.md `## Learning Log` *Entry format* block | Declaration of the field name, allowed values, and default-when-omitted semantics |
| `.claude/agents/self-improve.md` Step 5 (Commit B backfill site) | Mention of the field where the workflow branches on its value (e.g., Step 5 / Step 6 routing on `Kind:`) |
| `.claude/agents/learnings-escalation-audit.md` Steps 2/3/4 | Parse-site usage of the field (verdict routing, sweep predicates) |
| `ai-docs/corrections-log.md` field glossary | Declaration mirror — same allowed-values list, same default-when-omitted note |

The Exception-body locations used by the `Escalated?` / `Superseded by:` rows do not literally apply since declared-schema fields have no `/improve`-time mutation. A declared-schema field added to the entry format without parallel coverage in all four locations above → `major` finding (the field's allowed values or default semantics drift across the surface; the two agents disagree on how to interpret it).

## Checklist M — `agent-writing-style.md` conformance

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

## Checklist N — Bidirectional `## Patterns` ↔ `Kind: validation` coherence

The carrot-side analog of Checklist C (dead references). Every promoted-from-validation carrot must round-trip in both directions:

**Forward direction.** Every `### N. <Name>` entry under a `## Patterns` section in the audited corpus (`.claude/skills/**/SKILL.md`, `.claude/agents/**.md`, `AGENTS.md`) whose **body uses carrot verbs** (`Default to` / `Prefer`) MUST back-link to at least one `Kind: validation` entry in `ai-docs/learnings.md`. Detection recipe: for each `### N. <Name>` block, grep its body for `Default to` or `Prefer`; if found, also grep for a `learnings.md` back-link (path + date-slug citation) within the same block. Carrot-verb present AND no back-link → flag.

**Forward-sweep carrier-vs-template exemption.** Entries WITHOUT carrot verbs (template scaffolding, non-promoted prose, structural placeholders) are out of scope for the forward sweep — the audit greps for carrot-verb presence within each `### N. <Name>` block **before** requiring a back-link. The named exempt source is `ai-docs/agent-writing-style.md § Patterns` (template source, not a promoted-from-validation carrier). Other `## Patterns` sections that grow in future PRs are subject to the same carrot-verb-presence filter — no further per-file exemptions.

**Reverse direction.** Every `Kind: validation` entry in `ai-docs/learnings.md` whose `Escalated?` ≠ `no` MUST have a corresponding `## Patterns` block in each named target file (Skill / Subagent / AGENTS.md). Detection recipe: parse each `Kind: validation` entry's `Escalated?` line; for each comma-separated target value, confirm the named file contains a `## Patterns` block AND that block contains an entry back-linking to this validation entry. Predicate gate: entries with `Escalated? no` are NOT subject to reverse-direction enforcement — only entries the operator has promoted (`Escalated? ≠ no`) require a paired `## Patterns` block.

Multi-target reverse direction. The `Escalated?` field may name multiple comma-separated targets (e.g., `skill:context-reset, AGENTS.md`). The reverse sweep iterates each value independently — a validation entry escalated to two targets must have a `## Patterns` block in BOTH files; missing in either flags.

| Direction | Trigger | Action |
|---|---|---|
| Forward | `### N. <Name>` block in `## Patterns` uses `Default to` / `Prefer` AND no `learnings.md` back-link in the same block | flag (severity `major`) |
| Forward (exemption) | `### N. <Name>` block has no carrot verb in its body | no flag (carrier-vs-template exemption) |
| Forward (named exempt source) | The audited file is `ai-docs/agent-writing-style.md` | no flag (template source) |
| Reverse | `Kind: validation` entry with `Escalated? ≠ no` AND named target file lacks a `## Patterns` block OR lacks a back-linking entry | flag (severity `major`) |
| Reverse (predicate gate) | `Kind: validation` entry with `Escalated? no` | no flag (not promoted; pattern block not required) |

Severity `major` — dead-reference class. The bidirectional shape mirrors Checklist C: every reference resolves AND every target has a back-reference.

## Checklist O — Embedded-name clash scan

Enforces the [AGENTS.md `## Propagation Rule` clash-rename AXIOM](../../../AGENTS.md#propagation-rule). Project-defined Tool / Subagent / Skill / Hook names MUST NOT clash with embedded (Anthropic-shipped or marketplace-plugin) names enumerated in [`ai-docs/claude-tools-hierarchy.md`](../../../ai-docs/claude-tools-hierarchy.md) §§1a + 1b + 2a + 3a + 3b. Any match → `major` finding with rename recommendation (project side renames; the embedded name is never renamed).

**Recipe.** Enumerate two sorted lists and intersect them; empty intersection passes.

1. **Project names** — collect every name the project DEFINES across the four axes. Hook **matcher** values are NOT project-defined (they reference embedded Tool names) and are excluded:
   - Subagent names: `awk 'FNR==1{f=0} /^---$/{f=!f; next} f && /^name:/{print $2}' .claude/agents/*.md` — the `FNR==1{f=0}` reset is load-bearing: it scopes the in-frontmatter flag to each file. Without it, a file with an odd number of `---` delimiters leaks state into the next file in the glob, silently dropping `name:` matches.
   - Skill names: `awk 'FNR==1{f=0} /^---$/{f=!f; next} f && /^name:/{print $2}' .claude/skills/*/SKILL.md` — same `FNR==1` per-file reset rationale.
   - Hook event names (top-level `hooks.<Event>` keys): `jq -r '.hooks | keys[]' .claude/settings.json`. Event names like `SessionStart` / `PreToolUse` / `PostToolUse` are themselves harness-defined; they appear in §4 of `claude-tools-hierarchy.md` (which is NOT in the embedded-name corpus below). Including them in `project-names.txt` is intentional — if a future PR introduces a project-defined hook event, the scan catches a collision with §4's enumerated event set.
   - Sort + dedupe → `project-names.txt`.
2. **Embedded names** — extract names from the FIRST column (the Tool / Subagent / Skill column) of `ai-docs/claude-tools-hierarchy.md` §§1a + 1b + 2a + 3a + 3b table rows. Restricting to the first column avoids false-positives from parameter columns (which also backtick token names like `file_path`). Namespaced names like `ast-index:initialize-rust` count as ONE token; do NOT split on `:`:
   ```bash
   awk '
     /^### 1a\.|^### 1b\.|^### 2a\.|^### 3a\.|^### 3b\./ { in_embed=1; next }
     /^### / { in_embed=0 }
     /^## /  { in_embed=0 }
     in_embed && /^\| `/ {
       line=$0; sub(/^\| /, "", line)
       first_cell = line; sub(/ \|.*/, "", first_cell)
       n = split(first_cell, parts, "`")
       for (i = 2; i <= n; i += 2)
         if (parts[i] ~ /^[A-Za-z]/) print parts[i]
     }
   ' ai-docs/claude-tools-hierarchy.md | sort -u > embedded-names.txt
   ```
3. **Intersection** — `comm -12 <(sort -u project-names.txt) <(sort -u embedded-names.txt)` MUST return empty.

| Trigger | Action |
|---|---|
| `comm -12` output is empty | no flag — clash-scan baseline holds |
| `comm -12` output is non-empty | `major` finding per name: *"Project-defined `<name>` clashes with embedded `<name>` enumerated at `claude-tools-hierarchy.md` §§<sections>. Rename the project side."* |

Severity `major` — the clash makes ambiguous which name resolves at dispatch time. False positives are exceedingly unlikely (the embedded inventory is a closed set in §§1a/1b/2a/3a/3b); when one is suspected, re-grep the canonical doc to confirm the token still appears.

## Step 2.6 sub-step 4 — Cross-reference re-verification (anchor-aware)

For every relative link the audit touched in any `.claude/agents/*.md` or `.claude/skills/**/SKILL.md`, confirm the target file exists AND the anchor (if present) matches a heading slug. Use this anchor-aware check rather than naive `realpath -m` (which mistakes `#anchor` for part of the path):

```bash
for f in <changed-files>; do
  grep -oE '\(\.\./[./]*[^)#]+(#[^)]*)?\)' "$f" | sort -u | while read ref; do
    path_with_anchor=$(echo "$ref" | tr -d '()')
    path=${path_with_anchor%%#*}
    anchor=${path_with_anchor#*#}; [ "$anchor" = "$path_with_anchor" ] && anchor=""
    src_dir=$(dirname "$f")
    abs=$(realpath -m "$src_dir/$path")
    [ -e "$abs" ] || { echo "FILE MISSING: $f -> $path"; continue; }
    [ -z "$anchor" ] && continue
    # heading-slug match (GitHub algorithm): lowercase, strip to alnum/underscore/hyphen/space, spaces->hyphens; keep '_' and consecutive hyphens (GitHub does NOT drop underscores or collapse '--', so an em-dash heading like `A — B` slugs to `a--b`)
    awk '/^#{1,6}\s/{line=$0; gsub(/^#+\s+/,"",line); line=tolower(line); gsub(/[^a-z0-9 _-]/,"",line); gsub(/ /,"-",line); sub(/^-+/,"",line); sub(/-+$/,"",line); print line}' "$abs" | grep -Fx "$anchor" >/dev/null || echo "ANCHOR MISSING: $f -> $path#$anchor"
  done
done
```

See [`ai-docs/corrections-log.md`](../../../ai-docs/corrections-log.md) for the FORBIDDEN-reasoning anchor target referenced from the broader audit narrative.

## Checklist P — Cross-repo citation resolvability

Enforces one invariant: **every citation resolves for its reader.** A citation whose referent a reader cannot reach is not neutral decoration — but the failure is almost never a *fabricated* reference. It is an **unqualified** one.

**Why.** This harness was adapted from the sibling **quartzite** project (`maratik123/quartzite`) in commit `9077bfb`. `#N` is repo-relative by GitHub convention; `learnings.md <date>` / `feedback_*.md` are project-relative. The imported text never changed, but each referent silently rebound from a real quartzite object to a nonexistent local one — and `ai-docs/learnings.md` was created empty in that same commit, so its date citations pointed into nothing from the start.

> **AXIOM — QUALIFY, never drop.** A repo-scoped probe cannot falsify a cross-repo reference: a bare `gh pr view` resolves against **this** repo and reports *Could not resolve* for a real, MERGED `maratik123/quartzite` PR. That false negative reads as proof of fabrication and has already destroyed accurate provenance once. **Name the namespace, then re-probe.** Drop a citation only after `--repo`-qualified verification shows the referent genuinely does not exist.

**Recipe.** Run the script; it encodes all three probes.

```bash
.claude/skills/ai-audit/scripts/check-citations.sh
```

| Probe | Fires on |
|---|---|
| 1 | An unqualified `#N` — **bare or `PR #N`** — whose `N` exceeds this repo's live high-water mark (`gh pr list --state all --limit 1`). Bare `#N` is the same defect; a `PR #N`-only matcher misses it (that blind spot shipped once, in self-review Round 1). |
| 2 | A `learnings.md` date outside this log's range, on a line carrying a `see`/`entry`/`validated`/`recurrence`/`added` cue. Five cues — `added` catches the `(added <date>)` parenthetical shape. |
| 3 | A `feedback_*.md` cited without the namespace that owns it. |

Any hit → **`major`**. Fix by qualification: `PR #295` → `maratik123/quartzite#295`; `` `ai-docs/learnings.md` 2026-05-19 `` → quartzite's `` `ai-docs/learnings.md` `` 2026-05-19; a bare memory filename → its full `~/.claude/projects/<encoded>/memory/…` path.

**Do NOT replace the script with a substring hook.** It must tell a **citation** from an **example** (an illustrative `YYYY-MM-DD-slug.spec.md` filename asserts nothing) and a **use** from a **mention** (prose naming a quartzite PR in order to retire its claim). A blanket matcher over-fires on both; the script's cue-word, field-name, and filename-shape filters exist for that reason, and it excludes `ai-docs/bugfix/**` (traces discuss citations by nature).

**Self-test before trusting a PASS.** A green run proves nothing until it has failed for the right reason. Append a **bare** ref — no `PR` prefix — numbered far above the high-water mark, confirm RED, then restore. The bare form is deliberate: a `PR`-prefixed self-test would still pass if someone narrowed probe 1 back to `PR #N` — a self-test that inherits the aim of its phrasing cannot detect the aim being wrong.

**A fifth shape exists, currently clean: bare commit SHAs** (`9077bfb` is as namespace-relative as a bare issue ref). No probe is wired — every SHA in scope resolves locally (`git cat-file -t` → `commit`). If one ever fails, add it: `git cat-file -t <sha>` is exact — no substrings, cue words, or skips — and cheaper than probes 1–3.

**This page is the hardest to keep green.** Any text teaching *"do not write unqualified refs"* is under constant pressure to show one, so it is the highest-density false-positive source for its own script — inherent, not carelessness (the guard flagged this page five times during the fix that wrote it, twice while closing the prior finding). The discipline: **describe the bad shape; spell it only alongside its fix.** Exactly one specimen above is spelled — the `Fix by qualification` row — beside its qualified form on one line, which keeps it green. Re-run the script after editing this page; do not assume prose is safe. (Validated as a keep-doing pattern — `learnings.md` 2026-07-17, *a pattern-matching guard's own documentation is the highest-density false-positive source for that guard*; generalises to any lint/guard whose reference page must exhibit the shape the guard forbids.)
