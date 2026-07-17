---
name: project-review
description: "Whole-codebase review on the current branch (or branch given as argument). Reads all source files and done plans, runs fix loop and self-review loop until APPROVE, then commits."
disable-model-invocation: true
argument-hint: "[branch-name]"
allowed-tools: Bash(cargo build) Bash(cargo test *) Bash(cargo clippy *) Bash(cargo fmt *) Bash(cargo doc *) Bash(git diff *) Bash(git rev-parse *) Bash(git checkout *) Bash(git branch *) Bash(git log *) Bash(git add *) Bash(git commit *)
---

Whole-codebase review workflow. Steps execute **strictly in sequence**.

> **⚡ Compaction recovery check — read FIRST on every invocation.**
> If you are re-entering this skill after auto-compaction (a
> summary/compaction block appears at the top of context, or workflow
> context feels thin), STOP before any tool call and:
>
> 1. **Locate the durable-state file via this skill's active-state probe**
>    — run the preamble glob (`ls ai-docs/plans/*.progress.md 2>/dev/null`) and apply the validation it
>    documents (stale-merge, branch-match, or PR-linkage as the preamble
>    prescribes). The probe both finds the path AND decides whether to
>    RESUME, delete, park, or treat the situation as fresh.
> 2. Once the probe identifies the correct durable-state file
>    (the matched `ai-docs/plans/YYYY-MM-DD-project-review.progress.md`), read it **top-to-bottom in one
>    pass** — every line, including older sections and the `## Decisions
>    log` section. Do not skim. The recorded `current_step` is a
>    cross-check, never an instruction to skip the read.
> 3. **Then re-enter this skill from the top of its body** — let the
>    preamble's probe / validation / RESUME sequence route control. Do
>    NOT jump to a numbered Step directly; the preamble owns the routing.
>    The probe will land you at the right next action without re-doing
>    completed work.
>
> If the probe finds no matching durable-state file (or returns a
> validated "no active task" result), this is a fresh invocation —
> proceed normally.
>
> See `.claude/skills/context-reset/SKILL.md` § **Compaction recovery
> (re-entry)** for the canonical handoff rationale.

## ⚡ First: check for active review

```bash
ls ai-docs/plans/*.progress.md 2>/dev/null
```

**If found → RESUME:**
1. Read the `.progress.md` file
2. Jump to `## Next action`
3. Tell user: "Found active review, resuming from [next action]"

---

### Step 1: Determine branch

- If `$ARGUMENTS` is non-empty: confirm the user wants to review that branch, then `git checkout $ARGUMENTS`.
- Otherwise: use current branch (`git branch --show-current`).

Record `base_commit`:
```bash
git rev-parse HEAD
```

### Step 2: Spawn review Subagent

Create the progress file path: `ai-docs/plans/YYYY-MM-DD-project-review.progress.md` (use today's date). The progress file MUST include the canonical schema header fields per [`ai-docs/templates/progress-format.md`](../../../ai-docs/templates/progress-format.md): `**Branch:**`, `**base_commit:**`, `**Last build:**`, `**current_step:**`, `**last_passed_gate:**`, and a `## Decisions log` h2 section. Initialise `**current_step:** Phase 1 — review-findings` before spawning the Subagent.

```
Agent(subagent_type="general-purpose", prompt="
  Read .claude/agents/review-findings.md and follow it exactly.
  Branch: [branch name]
  base_commit: [base_commit]
  Write progress file to: ai-docs/plans/YYYY-MM-DD-project-review.progress.md
")
```

After the Subagent completes: read the progress file and report finding count and severity breakdown to the user.

**Write progress at this phase boundary** before further tool calls: rewrite `**current_step:**` to `Phase 1 — review-findings complete`; append a `## Decisions log` bullet recording the finding count + severity breakdown (one line, prefixed `Phase 1:`).

### Step 3: Fix loop

For each `⬜ Open` finding in the `## AC Status` table (top-to-bottom):

- **Fix it** → implement the change, mark `✅ Fixed` in the progress file.
- **Object to it** (finding is wrong or intentionally out of scope):
  - `nit` / `minor`: may object autonomously — write reason, mark `⚠️ Objected: <reason>`.
  - `major` / `blocker`: **surface to user first** before objecting. User must approve.

After every 3 fixes (or when all findings in a subtask are resolved):
1. `cargo build` — must compile
2. `cargo test` — all green
3. `cargo clippy --workspace --all-targets -- -D warnings` — clean
4. `cargo fmt`
5. `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` — clean (broken intra-doc links denied; add `--all-features` if the workspace later grows feature-gated modules)
6. Update `## Files touched` and mark subtask `[x]` in progress file
7. **Write progress at this phase boundary** before further tool calls: rewrite `**current_step:**` to `Phase 2 — fix loop (after N fixes)`; rewrite `**last_passed_gate:**` to `cargo clippy --workspace --all-targets -- -D warnings | <ISO-8601 UTC timestamp> | <commit SHA from git rev-parse HEAD>`; append a `## Decisions log` bullet for any `⚠️ Objected` finding rationale beyond the inline reason (one line, prefixed `Phase 2:`; omit if no decisions).

**Context handoff rule:** if the finding count is ≥ 10 and more than half remain open, spawn a Subagent per subtask rather than working inline — pass the progress file path so it can resume.

### Step 4: Final verify

> Skill orchestrators must consult the AC-verification-grep re-run gate documented in `review-findings.md` § 0 and `self-review.md` § 2 — every design's "AC<N> verified by: <command>" line MUST be re-run against the shipped artefact before the verdict is finalised (see quartzite's `ai-docs/learnings.md` 2026-05-15 tooling entry on spec-writer `tools:` frontmatter — `maratik123/quartzite#295`).

1. `cargo build` — PASS
2. `cargo test` — all green
3. `cargo clippy --workspace --all-targets -- -D warnings` — clean
4. `cargo fmt -- --check` — clean
5. `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` — clean (broken intra-doc links denied; add `--all-features` if the workspace later grows feature-gated modules)
6. **Doc convention conformance.** For every changed `pub` item, verify it conforms to [`ai-docs/doc-convention.md`](../../../ai-docs/doc-convention.md) (summary tense, `# Parameters` on fns with ≥1 non-receiver arg, strict section order, `# Errors` / `# Panics` / `# Safety` where applicable). Methods inside `impl Trait for Type {}` blocks are exempt; the trait *definition* is not. Mechanical heading scan on changed files: `rg '^\s*///\s*#\s*(Parameters|Returns|Type parameters|Lifetimes|Errors|Panics|Safety|Examples|See also)\b' <file>`.
7. Update progress file: `**Last build:** PASS`
8. **Write progress at this phase boundary** before further tool calls: rewrite `**current_step:**` to `Phase 3 — final verify (PASS)`; rewrite `**last_passed_gate:**` to `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace | <ISO-8601 UTC timestamp> | <commit SHA from git rev-parse HEAD>`; append a `## Decisions log` bullet recording any doc-convention finding fixed in this pass (one line, prefixed `Phase 3:`; omit if none).

### Step 5: Self-review loop (max 3 rounds)

```
Agent(subagent_type="general-purpose", prompt="
  Read .claude/agents/self-review.md and follow it.
  Progress: ai-docs/plans/YYYY-MM-DD-project-review.progress.md
  base_commit is recorded in the progress file.
  There is no spec or design doc — this is a review-driven task.
  Treat the findings table in ## AC Status as the acceptance criteria.
")
```

> **CI-2 applicability.** This Step-5 self-review runs **unconditionally** over the whole-branch review — its diff scope (the whole branch, from `base_commit`) is consistent with the `git diff <merge-base>..HEAD` row of the self-review invocation matrix ([`.claude/agents/self-review.md` § When self-review applies](../../agents/self-review.md)). The matrix's "docs-only / instruction-only commit → self-review optional" row does **NOT** exempt project-review: this is a review-driven gate, always run.

**On APPROVE:**
1. **Write progress at this phase boundary** before further tool calls: rewrite `**current_step:**` to `Phase 4 — self-review APPROVE (Round N)`; append a `## Decisions log` bullet recording the round count and any objections accepted (one line, prefixed `Phase 4:`).
2. `cargo fmt` (final pass)
3. Commit all changes (see commit rules below)
4. Delete `ai-docs/plans/YYYY-MM-DD-project-review.progress.md`
5. Done.

**On REJECT:**
- Rewrite `**current_step:**` to `Phase 4 — self-review REJECT (Round N), addressing findings` before re-entering the fix loop.
- Fix each `⬜ Open` finding from the self-review section (same fix/object rules as Step 3)
- Return to Step 5 (loop)

**After round 3 with REJECT:** surface all remaining `⬜ Open` findings to the user and ask how to proceed. Do not delete `.progress.md` until resolved.

### Commit rules

```bash
git add <all changed files — list them explicitly, no -A>
git commit -m "$(cat <<'EOF'
[brief summary of what the review fixed]

Review findings addressed:
- #N: description (severity)
- ...

Deferred:
- #N: description — reason
EOF
)"
```

## Gate checklist

| Before | Check |
|---|---|
| Step 2 | branch confirmed? base_commit recorded? |
| Step 3 | build green after every 3 fixes? |
| Step 4 | all six checks pass (build, test, clippy, fmt, doc, doc convention)? |
| Step 5 | self-review APPROVE before commit? |
| Commit | `major`/`blocker` objections user-approved? progress file deleted? |
