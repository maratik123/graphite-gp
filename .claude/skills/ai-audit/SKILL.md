---
name: ai-audit
description: "Two-phase instruction audit. Phase 1 (subagent) verifies every learnings.md `Escalated?` claim points to a rule that actually exists AND that every `Superseded by:` reference resolves to a real later entry or merged PR; fixes drift in either field. Phase 2 (main session) reads all instruction files (AGENTS.md, ai-docs/*, .claude/skills/**, .claude/agents/**, hooks in settings.json) against official Claude Code docs and proposes fixes for inconsistencies, dead references, format violations, and refactor candidates."
disable-model-invocation: true
argument-hint: "[scope: 'phase1' | 'phase2' | omit for both]"
allowed-tools: Bash(rg *) Bash(grep *) Bash(find *) Bash(realpath *) Bash(jq *) Bash(awk *) Bash(shellcheck *) Bash(wc *) Bash(basename *) Bash(git branch *) Bash(git status *) Bash(git checkout *) Bash(git rev-parse *) Bash(git diff *) Bash(git add *) Bash(git commit *) Bash(comm *) Bash(.claude/skills/ai-audit/scripts/check-citations.sh)
---

# AI Audit

Compliance + structural audit of the project's instruction surface. Complements `/improve` (which detects new patterns from `learnings.md`) by checking that **already-recorded** rules are coherent, reachable, and live where they claim to live.

Steps execute **strictly in sequence**. Stop on any unrecoverable mismatch and surface to user.

## Scope argument

- `phase1` — only the escalation audit subagent
- `phase2` — only the main-session instruction audit
- omitted (default) — run Phase 1 then Phase 2

Argument received: `$ARGUMENTS`

---

## Pre-flight: branch check

Run `git branch --show-current`. If `main` and any change is anticipated:

```bash
git checkout -b chore/YYYY-MM-DD-ai-audit
```

Switch *before* any edit — uncommitted edits accumulating on main is the failure mode this guards.

---

## Phase 1 — Escalation audit (subagent)

Skip if `$ARGUMENTS` is `phase2`.

Spawn the subagent in a clean context. The subagent reads `.claude/agents/learnings-escalation-audit.md` for full instructions.

```
Agent(subagent_type="general-purpose", prompt="
  Read .claude/agents/learnings-escalation-audit.md and follow it exactly.
  Working directory: <CLAUDE_PROJECT_DIR>
  Report back: (a) entries audited, (b) mismatches found, (c) fixes applied, (d) entries that need user judgment.
")
```

After the subagent reports back:

1. Surface its summary to the user.
2. If the subagent left any entry as **needs user judgment** — present each one and ask how to resolve before continuing to Phase 2.
3. If the subagent applied edits to `ai-docs/learnings.md` or other files, do **not** auto-commit yet — Phase 2 may add more changes; bundle into one commit at the end.
4. If the subagent emitted any `🌱 Stale-validation` flags, surface them to the user as a **signal for `/improve`** — NOT an auto-fix. Each flag identifies a `Kind: validation` entry that is > 30 days old AND `Escalated? no` AND has ≥1 instruction-file commit since its validation date. The audit does not promote the pattern (promotion is a `self-improve` Carrot-pass Step 2b decision); it only surfaces the candidate so the user can decide whether to invoke `/improve` in a follow-up turn. Per AGENTS.md § Learning Log threshold line, a single `🌱` flag (alongside the ≥3-correction / ≥2-validation thresholds) is sufficient to justify an `/improve` invocation.

---

## Phase 2 — Instruction audit (main session)

Skip if `$ARGUMENTS` is `phase1`.

### Step 2.1: Pull canonical Claude Code docs

Spawn the embedded `claude-code-guide` Subagent via the `Agent` Tool. One spawn returns the verbatim shape contracts for the three primary references; inline its output into orchestrator context and re-use the returned text for the rest of the run (no temp-file persistence).

```
Agent(subagent_type="claude-code-guide", prompt="
  Return the verbatim canonical shape contracts from these three Claude Code documentation pages:

  - https://code.claude.com/docs/en/skills.md         — skill frontmatter schema (which fields are required vs optional; allowed-tools shape; argument-hint; disable-model-invocation)
  - https://code.claude.com/docs/en/sub-agents.md     — Subagent file structure (frontmatter `name` / `description` / `tools` line; body conventions; when Subagents fire)
  - https://code.claude.com/docs/en/hooks-guide.md    — Hook event names, matchers, JSON I/O contract, exit-code semantics

  For each page, extract the verbatim schema text — not a paraphrase. Cite the URL alongside each block so the orchestrator can re-quote it during the audit.
")
```

If a referenced behavior in the codebase is unclear, ask `claude-code-guide` for additional pages from `code.claude.com` (settings reference, MCP, slash commands) on demand — do not pre-fetch everything.

### Step 2.2: Inventory the instruction surface

Read every file in these locations (use Read, not grep — content matters, not just keywords):

- `AGENTS.md` and `CLAUDE.md`
- `ai-docs/context.md`, `ai-docs/doc-convention.md`, `ai-docs/learnings.md`
- `ai-docs/plans/INDEX.md` and any active `*.spec.md` / `*.design.md` / `*.progress.md` (skip `done/` and `deferred/` content; just note their existence)
- Every `.claude/skills/*/SKILL.md`
- Every `.claude/agents/*.md`
- `.claude/settings.json` (hooks + permissions)

Do **not** read `.claude/settings.local.json` content unless the user explicitly authorizes it — it is user-local state.

### Step 2.3: Run the checklist

For each item below, when a violation is found record: file path, line number (where applicable), the rule it conflicts with, the proposed fix. Each row links to the matching detail body in [`reference.md`](reference.md); load the detail only when a row surfaces a finding.

| Letter | One-line purpose | Detail body |
|---|---|---|
| A | Cross-reference integrity — every relative link + named Skill/Subagent resolves | [`reference.md` § Checklist A — Cross-reference integrity](reference.md#checklist-a--cross-reference-integrity) |
| B | Conflicting / duplicated rules — no contradictions across files; verbatim duplicates consolidated | [`reference.md` § Checklist B — Conflicting / duplicated rules](reference.md#checklist-b--conflicting--duplicated-rules) |
| C | Dead references — every sync-group member exists; `ai-docs/plans/done/` references still resolve | [`reference.md` § Checklist C — Dead references](reference.md#checklist-c--dead-references) |
| D | Frontmatter conformance (skills) — `name` / `description` / `allowed-tools` shape per official docs | [`reference.md` § Checklist D — Frontmatter conformance (skills)](reference.md#checklist-d--frontmatter-conformance-skills) |
| E | Frontmatter conformance (agents) — YAML block present; `name` == basename; `description` is one line | [`reference.md` § Checklist E — Frontmatter conformance (agents)](reference.md#checklist-e--frontmatter-conformance-agents) |
| F | Hooks (`.claude/settings.json`) — event names, matchers, exit codes, env-var quoting | [`reference.md` § Checklist F — Hooks (`.claude/settings.json`)](reference.md#checklist-f--hooks-claudesettingsjson) |
| G | AGENTS.md "Propagation Rule" coherence — sync groups intact; exemptions replicated everywhere | [`reference.md` § Checklist G — AGENTS.md "Propagation Rule" coherence](reference.md#checklist-g--agentsmd-propagation-rule-coherence) |
| H | Documentation conformance pointers — `doc-convention.md` references resolve; section order matches | [`reference.md` § Checklist H — Documentation conformance pointers](reference.md#checklist-h--documentation-conformance-pointers) |
| I | File-size & structure (instruction files) — no `SKILL.md` / Subagent file > ~500 lines without sectioning | [`reference.md` § Checklist I — File-size & structure (instruction files)](reference.md#checklist-i--file-size--structure-instruction-files) |
| J | Allow-list / permission consistency — `allowed-tools` covered by `permissions.allow`; no dead entries | [`reference.md` § Checklist J — Allow-list / permission consistency](reference.md#checklist-j--allow-list--permission-consistency) |
| K | Skill-directory layout — oversized SKILL.md, multi-consumer supporting files, inline-script extraction candidates | [`reference.md` § Checklist K — Skill-directory layout (SKILL.md + supporting files + scripts/)](reference.md#checklist-k--skill-directory-layout-skillmd--supporting-files--scripts) |
| L | Learning-Log field coherence — every Entry-format field covered in all four mandatory locations | [`reference.md` § Checklist L — Learning-Log field coherence](reference.md#checklist-l--learning-log-field-coherence) |
| M | `agent-writing-style.md` conformance — 11 sub-checks (Patterns 1–7 + Anti-patterns + Sub-checks 9/10 + Cross-shape verbs) over the audited corpus | [`reference.md` § Checklist M — `agent-writing-style.md` conformance](reference.md#checklist-m--agent-writing-stylemd-conformance) |
| N | Bidirectional `## Patterns` ↔ `Kind: validation` coherence — every promoted carrot round-trips both ways | [`reference.md` § Checklist N — Bidirectional `## Patterns` ↔ `Kind: validation` coherence](reference.md#checklist-n--bidirectional--patterns--kind-validation-coherence) |
| O | Embedded-name clash scan — project-defined Tool / Subagent / Skill / Hook names MUST NOT clash with embedded names in `claude-tools-hierarchy.md` §§1a/1b/2a/3a/3b | [`reference.md` § Checklist O — Embedded-name clash scan](reference.md#checklist-o--embedded-name-clash-scan) |
| P | Cross-repo citation resolvability — every cited `#N` (bare or `PR #N`) / `learnings.md` date / memory file resolves for its reader; run `scripts/check-citations.sh` | [`reference.md` § Checklist P — Cross-repo citation resolvability](reference.md#checklist-p--cross-repo-citation-resolvability) |

The audited corpus for Checklist M is enumerated in `reference.md § Checklist M — audited corpus`.

### Step 2.4: Categorise findings

Group by severity:

- `blocker` — broken link, dead reference, frontmatter missing → instruction is unusable as written.
- `major` — contradicting rules, drifted exemption, hook with shell injection.
- `minor` — duplication, inconsistent style, unhelpful description.
- `nit` — wording, ordering, formatting.

Cap the report at 25 findings (same convention as `review-findings`); if more exist, list the 25 most severe and note the truncation.

### Step 2.5: Present + apply

Show the user a numbered list of findings with proposed fixes (concrete diffs for non-trivial edits). For each finding:

- `blocker` / `major`: ask user to confirm before applying.
- `minor` / `nit`: may apply autonomously if the fix is mechanical and obvious; otherwise ask.

Apply approved fixes via `Edit` / `Write`. Update `ai-docs/learnings.md` with a new entry per AGENTS.md "Learning Log" format (copyable skeleton: [`ai-docs/templates/learnings-entry.md`](../../../ai-docs/templates/learnings-entry.md)) **only if** the audit revealed a *new* class of mistake worth tracking — do not log routine cleanup.

### Step 2.6: Verify

After edits:

1. Re-run any `find`/`grep` checks from Step 2.3 that detected violations — confirm zero remaining.
2. If hooks were edited, eyeball the `.claude/settings.json` JSON validity: `jq . .claude/settings.json`.
3. If Subagent or Skill files were edited, confirm their frontmatter still parses by reading the file back.
4. **Cross-reference re-verification (anchor-aware)** — see [`reference.md` § Step 2.6 sub-step 4 — Cross-reference re-verification (anchor-aware)](reference.md#step-26-sub-step-4--cross-reference-re-verification-anchor-aware) for the verbatim bash recipe. Use it rather than naive `realpath -m` (which mistakes `#anchor` for part of the path) over every relative link the audit touched in any `.claude/agents/*.md` or `.claude/skills/**/SKILL.md`.

5. **Script verification (when checklist K extracted any `scripts/*.sh`).** For every script the audit added or modified:
   - **`shellcheck`** the script if `shellcheck` is on `$PATH`; flag warnings/errors as `minor` post-extraction findings.
   - Confirm executable bit (`-rwx------` or broader). If `chmod +x` was forgotten, the script will fail at invocation time with `Permission denied`.
   - **Smoke-test the documented no-op path** when feasible: most extracted scripts have a documented exit-0 case (missing input, no-op match). Invoke against that input and verify expected exit code + status message (e.g., `cleanup-progress.sh nonexistent-branch-xyz` should print `pr-merged: no merged PR found for nonexistent-branch-xyz` and exit 0).

---

## Step 3: Commit (if changes were made)

Stage explicitly:

```bash
git add .claude/skills/ai-audit/SKILL.md \
        .claude/agents/learnings-escalation-audit.md \
        <any other files actually edited>
git commit -m "$(cat <<'EOF'
chore(instructions): audit + fix [phase1|phase2|both]

[brief summary of fixes by category — A/B/C/... from the checklist]
EOF
)"
```

If `ai-docs/learnings.md` was modified by Phase 1 or Phase 2, stage it together — per AGENTS.md "Workflow", learnings entries are part of the deliverable and must be visible in the diff.

Per AGENTS.md, **never** `git add -A` / `git add .`.

---

## Gate checklist

| Before | Check |
|---|---|
| Phase 1 spawn | not on main OR no edits planned? |
| Phase 1 done | subagent reported every `needs user judgment` item? |
| Phase 2 fetch | all three docs successfully fetched? if a fetch failed, surface and ask whether to proceed without it |
| Phase 2 apply | `major` / `blocker` user-approved? |
| Script extraction (K) | `shellcheck` clean, `+x` bit set, no-op smoke-test passes? |
| Commit | `jq .` passes on settings.json? frontmatter still valid on edited skills/agents? cross-references re-verified anchor-aware? |

## Anti-patterns

- Do **not** rewrite `learnings.md` history — it is append-only. Phase 1 may only correct the `Escalated?` field of an existing entry or add a *new* corrective entry; never delete or rephrase past entries.
- Do **not** invent rules. The audit finds compliance gaps in *existing* rules; new rules go through `/improve`.
- Do **not** skip the `claude-code-guide` spawn in Phase 2. The official docs are the source of truth for Hook/Skill/Subagent shapes — relying on memory is the failure mode this Skill exists to prevent.
- Do **not** auto-resolve a blocker without surfacing it. Severity is a signal that human judgment is needed.
- Do **not** edit `.claude/settings.local.json` from this skill — it is user-local.
