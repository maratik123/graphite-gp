# Rust Agent Rules

**CRITICALLY**
1) English for all output. Other language only on explicit user request.

## Project

**graphite-gp** — a grid-based vector-racing game (the classic "Racetrack" pencil game: integer position + velocity, accelerate ±1 per axis per turn) with procedurally generated closed tracks and self-taught AI opponents. Rust workspace.

> Read [`ai-docs/context.md`](ai-docs/context.md) for purpose, architecture, and design decisions — on demand. The canonical spec is [`docs/design.md`](docs/design.md); the review record is [`docs/design-review.md`](docs/design-review.md).

## Permissions

Machine-enforced rules live in `.claude/settings.json` (allow/deny entries) and on `origin` (branch protection, when configured). Read those files for the authoritative list — duplicating them here lets the two sources drift.

Honor-system rules (no machine check; still binding):

- **DENY:** `git push --force` to feature branches — prefer `--force-with-lease`, and only after explicit user approval. Never force-push `main`.
- **DENY:** files outside project root.
- **ASK:** any tool not allow-listed in `settings.json`; if denied — suggest an alternative.

On session start: read `.gitignore`, treat matched paths as a read blacklist.

## Build & Test

```bash
cargo build                                             # whole workspace
cargo test                                              # all tests
cargo test <name>                                       # filter by substring
cargo test -- --nocapture                               # show stdout
cargo clippy --workspace --all-targets -- -D warnings   # strict lint (covers tests, benches, examples)
cargo fmt                                                # fix formatting
cargo fmt --check                                       # check only
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace   # doc gate
actionlint .github/workflows/<file>.yml                 # required gate for any new/modified workflow file
cargo run -p gp-game                                    # run the graphite-gp binary
```

> **AXIOM — `actionlint` MUST pass before `git add` on any modified `.github/workflows/*.yml`.**
> Required gate, **same status as `cargo build` and `cargo clippy --workspace --all-targets -- -D warnings`.** Never bypass.
>
> | If you see... | Action |
> |---|---|
> | `M .github/workflows/<name>.yml` in `git status` | Run `actionlint <file>` (pass every changed workflow file in one invocation) **before** `git add` |
> | `actionlint` reports any error | Fix it. **NEVER** bypass. |
>
> What `actionlint` catches that `cargo` cannot: runner-version mismatches, deprecated action versions, expression-syntax errors, shell-quoting issues.

> **AXIOM — Every project instruction file Claude loads per invocation MUST stay below 40,000 chars.**
> Harness-enforced soft cap; crossing it imposes measurable per-invocation cost on every subagent spawn, `/task`, and review pass. Project-side **35,000-char early warning** gives one full `/task` cycle of headroom. Applies to `AGENTS.md`, `CLAUDE.md`, every `.claude/skills/**/*.md`, every `.claude/agents/**.md`, every `.claude/rules/*.md`, and `ai-docs/{code-style,doc-convention,context,agent-writing-style,corrections-log}.md`.
>
> | If `wc -c <file>` reports... | Action |
> |---|---|
> | ≥ 40,000 chars | **`major`** — plan extraction/dedup for the next `/ai-audit` pass (extract verbose subsections into `ai-docs/<topic>.md` reference pages with anchored links). |
> | 35,000–39,999 chars | **`minor`** — proactive extraction pass; don't let the next `/task` push it over 40k. |
> | < 35,000 chars | OK. |

Search: `ast-index` first (see [`.claude/rules/ast-index.md`](.claude/rules/ast-index.md)); fall back to `rg <pattern> --type rust [-l | -C 3]` when `ast-index` returns empty.

## API Stability

> **AXIOM — No API-stability contract. Clean breaks, always. No compat shims.**
> graphite-gp is a **game application**, not a library — it is **never** published to crates.io and has **no** downstream clients. There is no external API-stability contract to preserve, now or in the future. Public API may be freely renamed, removed, or restructured at any time without backward-compat shims, deprecation layers, or `#[deprecated]` wrappers.
>
> | If you're tempted to... | Do this instead |
> |---|---|
> | Add `pub use OldName as NewName;` "for compat" | **REMOVE** the alias — make the clean rename |
> | Wrap removed fn with `#[deprecated] pub fn old() -> _ { new() }` | **DELETE** the wrapper — call sites update directly |
> | Keep both old and new APIs side-by-side temporarily | Pick one — old is gone |

## API Naming

See [`ai-docs/api-naming.md` → The _unchecked AXIOM](ai-docs/api-naming.md#the-_unchecked-axiom) for the `_unchecked` AXIOM + naming rules.

## Code Style

Thin by design — this project grows its own style rules through the learning loop (`/improve`). Start with:

- **Source files:** Rust-only (`.rs`) under `crates/*/src/`; format via `cargo fmt`, never `rustfmt <file>` directly.
- **Linter posture:** strict clippy (`-D warnings`); no blanket `#[allow]` without a justifying comment.
- **Rust idioms:** prefer idiomatic Rust over literal ports; comparison helpers (`.min`/`.max`/`.clamp`/`Option::or`/`Option::filter`) over explicit `if`/`match`. The physics core (`gp-core`) is integer-only and deterministic — no floats in `sim`/`geom` (see `docs/design.md` §3a).
- **Magic numbers:** numeric literals with semantic meaning → module-level `const SCREAMING_SNAKE_CASE`, not inline. Self-evident constants (`0`, `1`, `-1`, `2`) and test fixtures exempt.
- **Documentation:** every public item has at least a one-line `///`; broken intra-doc links are denied. See [`ai-docs/doc-convention.md`](ai-docs/doc-convention.md).
- **Error types:** `thiserror` for new error enum/struct; hand-rolled `Display`/`Error` only where the derive cannot express it.
- **File size:** soft 500/800, hard 1000/1500 (excl./incl. `#[cfg(test)]`) — refactor before merge unless exempt (codegen output, single `match`/state machine, `macro_rules!`); per-fn `clippy::too_many_lines` (>100); counter-rule — don't over-split (one-struct-per-file is not Rust idiom).

See [`ai-docs/code-style.md`](ai-docs/code-style.md) for the canonical (growing) reference.

## Dependency Versions

> **AXIOM — Query live state BEFORE asserting any claim about an external dep, the project's own dep graph, an external tool's flags/behavior, this repo's VCS state, or an upstream issue's status. Memory is stale — and so is any tool blind to the category you are asking about.**
>
> | If you're about to write... | Verify first with |
> |---|---|
> | A specific version of crate `X` | `curl -sS -H "User-Agent: graphite-gp-agent (<contact-email>)" "https://crates.io/api/v1/crates/X" \| jq -r '.crate.max_stable_version'` — the `User-Agent` is **required**: crates.io's data-access policy rejects UA-less requests with a JSON *error body*, so a bare `curl` makes `jq` print the literal `null` and **exit 0**. A `null` means **the query failed** — never "the crate has no stable release". Re-run without the `jq` filter and read the raw body before concluding anything. |
> | A claim that `X` is / isn't / would-be-added-as a dep in this project | `grep -r '<X>' --include='Cargo.toml' .` AND `cargo tree --invert <X>` (catches transitive presence) |
> | A specific flag / subcommand / capability of an external tool (`cargo`, `gh`, `actionlint`, …) — e.g. "`cargo test` supports `--keep-going`" | `cargo <cmd> --help` (or run the command), or read the offline docs at `~/.rustup/toolchains/stable-*/share/doc/`. **Never assert a tool flag from memory.** |
> | A claim that a file is **committed / tracked / ignored** ("the repo commits X", "X is gitignored", "there are no stale Y") | **Match the command to the FILE CATEGORY, and name the category before choosing:** tracked → `git ls-files <path>`; ignored + which rule → `git check-ignore -v <path>`; untracked-but-not-ignored → `git status --porcelain`; ignored included → `git status --porcelain --ignored`; exists on disk at all → `ls` / `find`. `find`/`ls` prove on-disk presence, **never** tracked status. `git status` is **blind to ignored files** — empty output is NEVER proof a path is absent, and is actively misleading for any question about gitignored build/regen output, which is exactly where stale-artifact questions live. |
> | A claim about an **upstream issue/PR's current state** ("bug X is unfixed", "affects 1.98 beta", "no fix released") | `gh issue view <N> --json state,comments` — the issue *body* is frozen at filing time; the **closing comment** carries the resolution. **When the user cites a URL with a `#fragment`, fetch THAT anchor** — the fragment is the citation, the page is merely where it lives; a user linking a specific comment has usually already found the answer. |
>
> If your draft contains substrings like *"would add"*, *"introduce X as a dep"*, *"pull in X"*, *"avoid X as a dep"*, *"X is not currently a dependency"*, *"supports `--flag`"*, *"takes `--flag`"*, *"passing `--flag`"*, *"is committed"*, *"is tracked"*, *"is gitignored"*, *"there are no"*, *"still affects"*, *"is unfixed"* — **STOP**, run the relevant check above (`grep` + `cargo tree --invert` for deps; `--help` for tool flags; the category-matched `git` command for VCS state; `gh issue view --json state,comments` for upstream status), and either rewrite with the verified fact or drop the claim.
>
> See [`ai-docs/dependency-versions.md`](ai-docs/dependency-versions.md) for the lookup recipes. Apply the pinning rule to the **observed** version, never the remembered one.

When adding or editing dependencies in `Cargo.toml`:

- Use `0.x` for `0.x.y` versions — never pin the patch.
- Use `x` for `x.y.z` versions — never pin minor or patch.
- No `~` prefix — Cargo's default `^` semantics are sufficient.
- After changing a **dependency version constraint** (a `[dependencies]`/`[dev-dependencies]` version bump, or a new/removed dep), run `cargo update` then `cargo build` to verify — and **name which constraint changed before running it**. A `Cargo.toml` edit touching only package metadata (`license`, `description`, `authors`, …) has no dep-graph delta: run `cargo build` alone, which will not touch `Cargo.lock`. A bare `cargo update` there pulls unrelated transitive bumps into the lockfile. Confirm the delta is only the intended edges with `git diff --stat Cargo.lock` **before staging**.

## Workflow

> **AXIOM 1 — NEVER edit on local `main` when work is intended for a PR.**
> Create a feature branch (`git checkout -b feat/...` or `chore/...`) **before** any file edit — not before commit, **before edit**.
>
> | If `git branch --show-current` returns... | Action |
> |---|---|
> | `main` AND you're about to make a PR-targeted edit | **STOP**. Run `git checkout -b <prefix>/<descriptive-name>` first. Only then edit. |
> | A feature branch | Proceed with edits |
> | `main` AND you've already made commits (recovery) | `git stash` → `git checkout -b <feature>` → `git checkout main && git reset --soft origin/main && git restore --staged .` → push feature branch → open PR. Pop stash on feature branch if needed. |
>
> The first action of any skill/workflow that produces commits (`/task`, `/improve`, `/ai-audit`, etc.) is `git branch --show-current`; if `main`, switch **before** any `Edit`/`Write`. Before any `git push`, confirm again — if it is `main`, stop and apply recovery.

- Merge PRs via merge commit (`gh pr merge --merge`); never squash/rebase-merge.
- Run `cargo build` before commit so `Cargo.lock` refreshes.
- Stage explicitly; **never** `git add -A` / `git add .`.
- **Before every `git commit` during a PR task**, stage `ai-docs/learnings.md` with related code. **After every push**, give a post-push learning entry its own commit.
- **Never** `git commit --no-verify` (or any hook-skip flag) — fix the hook.
- **`gh … --body` vs the commit-block hook.** The commit-block hook matches `git[[:space:]]+commit`, so a `gh issue create` / `gh pr create` / `gh pr comment` invocation whose `--body` argument *contains* that substring (e.g. a body mentioning `git commit`) is falsely blocked — use `--body-file <path>` instead of inlining the body.
- **NEVER** batch a `git commit` / data-dependent `AskUserQuestion` in the same turn as the `Edit`/subagent call producing its inputs; verify with `git diff --cached --stat` first.
- **Before delegating to a Subagent, verify the delegate can actually do the work — check its charter AND its environment, not the step text or a `tools:` grant.** (a) **Charter fit:** `code-writer` is a *code* implementor; a predominantly-prose diff (`.claude/**` / `ai-docs/**` / `*.md`) has no code to delegate — author it in-thread. (b) **Environment fit:** a *background* Subagent cannot answer an interactive / self-modification permission prompt, so a protected-file edit **fails closed** regardless of `Edit(...)` allow-lists — apply those in-thread. A step saying "delegate" is evidence about CAN, never about fit.
- **CI-fix commits get self-review too.** Spawn `self-review` before pushing any CI-fix commit.
- **No "too simple" step-skip in `/task`.** Steps 6 / 7 / 10 are MANDATORY; user authorisation is the only bypass.
- **NEVER** `git reset --hard` — discards uncommitted work. The same hazard applies to `git checkout -- <file>` and `git restore <file>`: both restore the *whole* working-tree file to HEAD, silently dropping every uncommitted edit to it — safe **only** when you mean to discard the file's entire delta (e.g. reverting an unwanted `cargo update` on `Cargo.lock`), **hazardous** when the file mixes an edit you keep with the one you drop. To undo a test/injection edit on such a file, use a cp-backup (`cp f bak; …; cp bak f`) or a scratch file, then re-verify with `git diff --name-only <base>`. (`git checkout <branch>`/`-b` and `git restore --staged` are unaffected — the hazard is the working-tree-file forms.)
- Plan first. Tests before prod code (TDD). Lint changed files.
- Files with ~50+ lines of substantial logic MUST have a `#[cfg(test)] mod tests` block (exceptions: `examples/`, `benches/` with `harness = false`).
- After generating/moving a markdown file with relative links, trace one link via `realpath` before committing.
- **PR review comment resolution:** Resolve only comments fixed by code; objections stay open for the reviewer.

> **AXIOM 2 — Read the PR body via `gh pr view <N>` after EVERY `git push` to a feature branch with an open PR. Unconditional.**
> The READ is mandatory even for a routine typo/format/nit push. The EDIT is conditional — only when the body contradicts the new commits.
>
> | After... | Required action |
> |---|---|
> | `git push` to a feature branch with an open PR | Run `gh pr view <N> --json title,body` immediately. Read the body. |
> | The body still describes the diff accurately | No `gh pr edit` needed — read complete |
> | The body contradicts the new commits (renames, scope drift, AC flips, cited counts) | Run `gh pr edit` to sync |
> | `gh pr create` immediately preceded the push (first push that opened the PR) | **Skip** the read — the body is what you just authored. The rule fires on the **next** push. |

> **AXIOM — Every code-producing commit on a feature branch with an open PR (or about-to-be-opened PR) must pass `self-review` before `git push`.**
> Per-skill instances: `/task` Step 10, `/pr-commented` Step 5, `/pr-ci-failed` Step 5, `/main-ci-failed` Step 5, `/bugfix` Step 6. This AXIOM names them as one workspace rule so the next surface without its own step still falls under it.
>
> APPROVE = push. REJECT = fix on the same branch and re-run; after 3 REJECTs in a row, surface and stop without pushing.

> **AXIOM — `ai-docs/deferred/_inbox.jsonl` is written ONLY by `/task` Step 12 and `/triage`.**
> Hand-edits defeat the propagation contract — they hide rows from the parser and conflict with future Step-12 appends; the JSONL line-per-object format is hand-edit-hostile (one malformed line breaks the whole `jq` read).
>
> | If you see... | Action |
> |---|---|
> | A row in `_inbox.jsonl` you want to move or drop | Run `/triage`; let it sort/drain the row |
> | A row missing for a freshly-merged spec | Re-run `/task` Step 12 manually |

## Propagation Rule

> **AXIOM — Edits to one instruction file MUST propagate to its sync-group siblings in the SAME PR.**
> The Propagation Rule fires whenever you edit an instruction file. Sister files in the same sync group must receive the corresponding change before the PR is opened.
>
> | If you edit... | You MUST also check / update... |
> |---|---|
> | `.claude/skills/project-review/SKILL.md` | `.claude/agents/review-findings.md` AND `.claude/agents/self-review.md` (Review group) |
> | `.claude/agents/review-findings.md` | `.claude/skills/project-review/SKILL.md` AND `.claude/agents/self-review.md` (Review group) |
> | `.claude/agents/self-review.md` | `.claude/skills/project-review/SKILL.md` AND `.claude/agents/review-findings.md` (Review group) |
> | `.claude/skills/interview/SKILL.md` | `.claude/agents/spec-writer.md` (Interview group) |
> | `.claude/agents/spec-writer.md` | `.claude/skills/interview/SKILL.md` (Interview group) |
> | `.claude/skills/triage/SKILL.md` | `.claude/agents/triage-runner.md` AND `.claude/skills/next/SKILL.md` (Triage group) |
> | `.claude/agents/triage-runner.md` | `.claude/skills/triage/SKILL.md` AND `.claude/skills/next/SKILL.md` (Triage group) |
> | `.claude/skills/next/SKILL.md` | `.claude/skills/triage/SKILL.md` AND `.claude/agents/triage-runner.md` (Triage group) |
> | `.claude/skills/task/SKILL.md` (Steps 6–8 design phase) | `.claude/agents/design.md` AND `.claude/agents/design-review.md` AND `.claude/skills/context-reset/SKILL.md` (Task/Design group) |
> | `.claude/agents/design.md` OR `.claude/agents/design-review.md` OR `.claude/skills/context-reset/SKILL.md` | See *Task/Design group* anchor row above. |
> | `.claude/skills/task/SKILL.md` Spec/Design Amendment recipe | `.claude/skills/pr-commented/SKILL.md` AND `.claude/skills/pr-ci-failed/SKILL.md` AND `.claude/skills/main-ci-failed/SKILL.md` AND `.claude/agents/self-review.md` (Spec-Amendment group) |
> | `AGENTS.md` "Learning Log" section (Boundary rules 1/2, entry format incl. `Kind:`, `Escalated?` semantics, 🌱 verdict) | `.claude/agents/self-improve.md` AND `.claude/agents/learnings-escalation-audit.md` (Learning-Log group) |
> | `AGENTS.md` (rule add / exemption) | Run `grep -rn "<changed-keyword>" .claude/ AGENTS.md ai-docs/` and apply the same change to every match (new pre-resolved rules also add a Rule-5 substring-blacklist entry in `.claude/agents/spec-writer.md`). |
> | Any edit that changes a Tool/Subagent/Skill/Hook contract | Update `ai-docs/claude-tools-hierarchy.md` in the same PR. |
> | Any other instruction file | Run the same grep — the Procedure below catches lingering references. |

**Procedure:**
1. Before closing the edit, `grep -rn "<changed-keyword>" .claude/agents/ .claude/skills/ .claude/rules/ AGENTS.md ai-docs/` for any file referencing the same rule/terminology.
2. Apply the same change (or the corresponding enforcement adjustment) in every match.
3. AGENTS.md rule exemptions must propagate to subagent checklists that enforce the rule (`self-review.md`, `review-findings.md`).

Do not refer to a skill as an "agent" or vice versa — the distinction matters for spawning. (`project-review` is a skill; `review-findings` and `self-review` are agents spawned by it.)

## Communication

Interpret user phrasing literally and conservatively. When uncertain — ask, don't guess.

- **"Submit / push to PR"** = `git push` the branch to remote so commits appear in the open PR. **NOT** `gh pr merge`. Only merge when the user explicitly says "merge".
- **"wtf?" / "what?" / "huh?"** (or similar surprise/frustration) = the previous action was the opposite of what the user wanted. **Stop immediately**, do not retry, ask what was wrong before doing anything else.
- **IDE files** (`.idea/`, `*.iml`, `.vscode/`, `*.swp`, etc.) — never add, remove, modify, stage, or `.gitignore` them unless the user explicitly asks. "add ide files" most likely means **commit and track them** — confirm before acting.
- **A verbal acknowledgement is not a fix.** When the user corrects a fact — especially one you have already written into a file — the correction is a **work item**, not a conversational beat. Reply **and**, in the same turn, `grep` the artifact for the wrong claim and edit it. Tell: any reply containing *"fair"*, *"good point"*, *"you're right"*, *"consistent with"*, or *"that closes it"* that is **not accompanied by an `Edit`** to whatever asserts the now-refuted thing.
- **A recorded result is a claim, not a completion.** A sentence asserting your *own* work-state — "verified", "confirmed", "backfilled", "gate PASSed", "done" — written to any durable surface (a PR body, a `.progress.md` decisions log, a trace's `last_passed_gate`, an instruction-file diff) is a **timestamped claim**, not a standing fact. Re-run the underlying check *after the LAST edit of the turn*, immediately before recording — never record-then-edit. After fixing a claim-class defect, re-scan the **whole section**, not just the fixed line. Tell: any "verified"/"done"/"PASS" you did not (re-)produce with a command in *this* turn. (Generalises the `.progress.md`-scoped rule in `.claude/agents/code-writer.md` § Mode A and `.claude/skills/context-reset/SKILL.md` to PR bodies and trace fields.)
- **Deviating from user-approved scope requires an ask, not a notification.** *"I also did X — say the word if you'd rather I revert"* puts the burden of catching scope drift on the user. Ask **before** widening scope, even when the argument is compelling and the mirror cost looks low — and especially when the argument comes from a reviewer whose premise you have not run. A reviewer's finding is an argument, not a fact; run its premise before acting on it.

## Patterns

### 1. Verify a reviewer's retractions and suggestions as skeptically as its findings

*Default to* verifying a reviewer's *retraction*, *salvage suggestion*, and
*"leave it / harmless / follow-up"* call with the same command you would run
against its original finding. A retraction is an assertion; a proposed fix is a
claim that the fix works; a "harmless" ruling is a claim about harm.

**The asymmetry to resist.** A finding feels like a challenge and invites
checking, while a withdrawal or a wave-through feels like *relief* and invites
acceptance — which is exactly when an unverified claim slips through, because
agreeing costs nothing in the moment. *Prefer* overriding a reviewer only in the
direction of **more** verification: declining a suggested fix because you tested
it and it fails is sound; accepting one because it sounds right is not.

Validated by [`ai-docs/learnings.md`](ai-docs/learnings.md) 2026-07-16 —
*treating a reviewer's retractions and suggestions as skeptically as its
findings*.

## Agent Docs

| Path | Purpose |
|------|---------|
| `ai-docs/context.md` | Project context — read on demand |
| `ai-docs/code-style.md` | Workspace code-style reference — read on demand |
| `ai-docs/doc-convention.md` | rustdoc conventions — read on demand |
| `ai-docs/corrections-log.md` | Learning-Log carve-outs + field glossary |
| `ai-docs/key-decisions.md` | Key design-decision detail bodies |
| `ai-docs/api-naming.md` | `_unchecked` AXIOM + naming rules |
| `ai-docs/dependency-versions.md` | Live Cargo / GitHub Action version lookup recipes |
| `ai-docs/agent-writing-style.md` | Binary-rule writing style for dual-model readability |
| `ai-docs/agent-docs-index.md` | Verbose bodies of `§ Agent Docs` rows — read on demand |
| `ai-docs/instruction-file-validation.md` | Dual-model instruction-file-clarity test methodology |
| `ai-docs/claude-tools-hierarchy.md` | Project Tool/Subagent/Skill/Hook inventory |
| `ai-docs/templates/progress-format.md` | Canonical `.progress.md` format spec |
| `ai-docs/plans/INDEX.md` | Plan index — statuses and dependency order |
| `ai-docs/plans/*.spec.md` | Active task spec + acceptance criteria |
| `ai-docs/plans/*.design.md` | Active task design documents |
| `ai-docs/plans/*.progress.md` | Active task progress / handoff state — local-only (gitignored) |
| `ai-docs/plans/done/` | Completed plans (spec + design, implemented) |
| `ai-docs/plans/deferred/` | Blocked or future plans |
| `ai-docs/deferred/_inbox.jsonl` | Triage queue — rows from completed specs awaiting `/triage` |
| `ai-docs/bugfix/trace-*.md` | Bugfix trace + durable-state surface — deleted on resolution |
| `ai-docs/learnings.md` | Corrections log — feed for `/improve` |

## Learning Log

On **ANY** instruction violation, of any kind, write a new entry to `ai-docs/learnings.md` — there is no "obvious", "minor", "trivial", "already-known", or "duplicate" disposition. The history (including recurrences and superseded entries) is the artefact `/improve` audits to decide escalation fan-out. See [`ai-docs/corrections-log.md` → FORBIDDEN reasoning for skipping a `learnings.md` write](ai-docs/corrections-log.md#forbidden-reasoning-for-skipping-a-learningsmd-write) for the enumerated skip-reasons that are explicitly disallowed. **Read the two boundary rules below before you write.**

### Boundary rule 1 — `ai-docs/learnings.md` is APPEND-ONLY

> **NEVER** edit, rewrite, reorder, summarise, or delete an existing entry in `ai-docs/learnings.md`. Only append new entries at the end. This applies even when:
> - a newer correction supersedes an older one — write a NEW entry that says so, leave the old one intact
> - an entry turns out to be wrong, redundant, or poorly worded — write a NEW entry that corrects it
> - you are tempted to "tidy up" or "consolidate" the file
>
> The history of corrections (including superseded and wrong ones) is itself the artefact `/improve` audits. Editing past entries destroys that history.
>
> **Exception — `Escalated?` and `Superseded by:` fields, subagent-driven only.** Both fields MAY be updated in-place by the `self-improve` subagent (`/improve`) and the `learnings-escalation-audit` subagent (`/ai-audit` Phase 1). See [`ai-docs/corrections-log.md` → Boundary rule 1 Exception](ai-docs/corrections-log.md#boundary-rule-1-exception). All other lines of an entry remain immutable.

### Boundary rule 2 — writing to `learnings.md` triggers NO other rule-file edits in the same turn

> When you write to `ai-docs/learnings.md`, you **MUST NOT** also edit any of these files in the same conversation turn:
>
> - `AGENTS.md`
> - `CLAUDE.md`
> - `.claude/skills/**` (any file)
> - `.claude/agents/**` (any file)
> - `.claude/settings.json`
> - `ai-docs/code-style.md`
> - `ai-docs/doc-convention.md`
>
> Writing a learning entry is **NOT** authorisation to escalate the rule into instruction files. Set `Escalated? no` and stop. Project-level escalation happens only when:
>
> 1. The user runs `/improve` (which spawns the `self-improve` subagent), OR
> 2. The user explicitly asks ("escalate this", "update AGENTS.md", "add to skill X").
>
> **Exception — `/improve` and `/ai-audit` workflows.** `self-improve` + `learnings-escalation-audit` MAY update `Escalated?` / `Superseded by:` on existing entries alongside instruction-file edits. Existing-entry updates ONLY — NEW learning entries still cannot be appended in the same turn as instruction-file edits.
>
> **Exception — in-flow learning capture during `/task` Steps 8–12.** A NEW learning entry MAY be appended in the same turn as an instruction-file edit when ALL hold: (a) running skill is `/task` Steps 8–12 (incl. sub-skills `/bugfix`, `/context-reset`); (b) entry documents an in-task insight (not pre-emptive escalation); (c) marked `Escalated? no`.

### Entry format

```
### YYYY-MM-DD — [category] — [short description]
**What happened:** [quote or paraphrase]
**Rule:** [what to do instead, or what to keep doing]
**Kind:** correction | validation    (optional; defaults to `correction` when omitted)
**Escalated?** no | AGENTS.md | skill:[name] | hook | settings | agent:[name] | rules:[name] | doc-convention | code-style (comma-separate multiple)
**Superseded by:** [ref] — [one-line reason]    (optional; omitted when not applicable)
```

`Kind:` defaults to `correction` when omitted. Write `Kind: validation` for entries that document a working protocol/pattern to keep doing (carrot signal); `Kind: correction` (or omit) for a violation to stop doing (stick signal). `Escalated?` records **project-level** persistence only — user-local auto-memory and `settings.local.json` do **not** count → stay `no`.

See [`ai-docs/corrections-log.md` → Entry format — field glossary](ai-docs/corrections-log.md#entry-format--field-glossary) for the semantics of each field.

Categories: `code-style` | `process` | `architecture` | `testing` | `documentation` | `tooling` | `search` | `other`

Run `/improve` when **≥3 unescalated correction entries**, **≥2 unescalated validation entries**, or a `🌱 Stale-validation` flag from `/ai-audit` accumulates.

## Rust Test Conventions

- **Miri aborts on the FIRST unsupported operation**, and cargo's fail-fast then drops every phase queued behind it — the offending crate's whole test binary plus the doc-test phase. Gate any test that can abort Miri with `#[cfg_attr(miri, ignore = "<why>")]` **in the same commit**. Per-test, **never** a crate-level `--exclude` (that also drops the crate's Miri-clean tests). The trigger is **"aborts under Miri", NOT "is FFI"** — the two in-tree gates have unrelated causes: `golden_guard` drives wgpu and `dlopen`s the Vulkan ICD (FFI), while `tessellation_smoke` has **no FFI** and aborts because drawing text runs `vello_cpu`'s checked `u8`→`u32` pixmap cast, which panics under Miri's 1-byte allocator alignment. Write the reason for **that test's own** abort; do not copy a sibling's — a wrong reason is a false justification for a different failure. Reproduce with the **workspace** command CI runs (`ci.yml:176`+`:188` → `MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace`; locally add `+nightly` to select the toolchain that has `miri`), never a narrower `-p` run. **Whether a red Miri BLOCKS merge is UNRESOLVED — do not assert either way** ([#76](https://github.com/maratik123/graphite-gp/issues/76)): `ci.yml:170-172` says *"Advisory only — no `miri-pass` gate is wired into branch protection"*, while the **active** `main-protection` ruleset lists `Miri / Tree Borrows` among its required status checks (`gh api repos/maratik123/graphite-gp/rulesets/18954622 --jq '.rules[]|select(.type=="required_status_checks")|.parameters.required_status_checks[].context'`). **What IS measured (2026-07-17):** `continue-on-error: true` splits the two conclusions — on `973d99d` the check-run `Miri / Tree Borrows` reports **`failure`** while its workflow run (`29518271322`) rolls up to **`success`**. Consequence for you: **`gh run list` is BLIND to a red Miri** — it reports the roll-up, so an all-green run list is NOT evidence Miri passed; query `gh api repos/maratik123/graphite-gp/commits/<sha>/check-runs` instead. Per #76's own criterion a `failure` check-run makes the required-check entry live rather than inert — but that has never been exercised **at merge time** (973d99d was an intermediate commit; PR #70's merged head was green), so it stays a derivation, not a fact. Either way, treat a red Miri as a **regression to fix**, not a status to interpret. Check it after any PR adding a new dependency class.
- Unit tests in the same file under a `#[cfg(test)]` module. Integration tests in `tests/`.
- Use `rstest` for parameterized tests when useful; `mockall` for mocking traits; `pretty_assertions` encouraged for diffs.
- Assert with `assert_eq!` / `assert_matches!`. **`assert_matches!` formats the scrutinee with `{:?}` on mismatch, so its type MUST impl `Debug`** (`Result` needs both `T` + `E`; `Box<dyn Trait>` needs a `Debug` supertrait) — `assert!(matches!(...))` imposes no such bound. If the scrutinee is non-`Debug`, leave `assert!(matches!(...))` as-is — do NOT add a production `#[derive(Debug)]` to satisfy a test-only assertion. Counting `assert!(matches!)` sites for a migration: the multi-line message form is invisible to single-line `rg 'assert!\(matches!'` — use `rg -U`.
- Test names as `snake_case` describing behaviour: `returns_empty_when_not_found`.
- No `unwrap()` in production code without a justifying comment; `expect("reason")` preferred.
- No `#[allow(clippy::...)]` / `#[allow(dead_code)]` unless unavoidable (with justification).
- Test behaviour, transitions, errors, edge cases. The `gp-core` physics is deterministic — assert exact states. The `supercover` predicate (`docs/design.md` §3 C4) ships with its full case table as unit tests.
