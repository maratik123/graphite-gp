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
RUSTFLAGS="-C target-cpu=native" cargo bench -p gp-core --bench supercover   # local-only, NOT a gate
```

> **Benchmarks are not a required gate** — same status as the Miri job. CI never runs `cargo bench`; `clippy --all-targets` only compiles the bench so it cannot bit-rot. Run it when you are changing the code it covers or evaluating a replacement, not on every task.

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

> **A zero exit status is evidence about the LAST pipeline stage, not about your question.** Never pipe a gate whose exit code is load-bearing — `cargo test … | tail -6` reports `tail`'s status (always 0), so a RED gate records as green, and `tail -N` can truncate away the `test result:` line you needed. Capture to a file and grep the saved log: `cargo test --workspace > gate.log 2>&1 && echo GATE-GREEN || echo GATE-RED`, then `grep -E "test result:|^error" gate.log`. (`set -o pipefail` also works.) A `PreToolUse` hook blocks the `cargo … | tail/head` form; the principle is broader than what the hook matches — the same silent-success shape covers a `jq` filter printing `null` from an error body, and a mutating flag (`rg -r`) rewriting output while exiting 0 ([#160](https://github.com/maratik123/graphite-gp/issues/160)).

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
> **Five categories, each with its own recipe — the command must reach the CATEGORY, or its exit 0 is about a different question than yours:** a crate version (crates.io API, `User-Agent` **required** — UA-less returns an error body, so `jq` prints a literal `null` and **exits 0**; `null` means *the query failed*, never *no stable release*); whether `X` is a dep here (`grep --include='Cargo.toml'` **AND** `cargo tree --invert X` for transitive reach); an external tool's flag (`<tool> --help` or run it — **never** from memory); a file's **tracked / ignored / on-disk** status (category-matched `git` command — `git status` is **blind to ignored files**, so empty output is never proof of absence); an upstream issue's state (`gh issue view <N> --json state,comments` — the body is frozen, the **closing comment** carries the resolution; fetch the `#fragment` the user cited).
>
> **Full per-category recipes: [`ai-docs/dependency-versions.md`](ai-docs/dependency-versions.md).**
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
- **Delegation has FOUR phases, and failures land in the middle two.** **(1) Fit** — check the delegate's charter AND environment, not the step text or a `tools:` grant: `code-writer` is a *code* implementor (a prose-only diff has nothing to delegate — author in-thread), and a *background* Subagent cannot answer a self-modification prompt, so protected-file edits **fail closed** regardless of allow-lists. **(2) Hand-off** — leave the index **CLEAN**; `git commit` captures the whole index, so anything you pre-staged lands in the delegate's first commit. **(3) While it runs** — a delegate waiting on a long job is **waiting, not stuck**; never start a parallel investigation of the same question. **(4) Return** — a **RETURN SUMMARY is a claim, not a record**: verify every gate / PASS / "I did X" against the durable artifact (`.progress.md`, commit body, `git log`/`git diff`, the file), and trust the durable record when they disagree. **Mechanics + the incidents behind each phase: [`ai-docs/delegation-rules.md`](ai-docs/delegation-rules.md) — read before any spawn that commits, edits protected files, or runs long.**
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
> Per-skill instances: `/task` Step 10, `/pr-commented` Step 5, `/pr-ci-failed` Step 5, `/main-ci-failed` Step 5, `/bugfix` Step 6. This AXIOM names them as one workspace rule so the next surface without its own step still falls under it — the enumeration is a list of *named* instances, **never** a list of the only covered surfaces. An unnamed surface is covered when it **either** ships executable code (a hook body, a script) **or** changes an instruction-file rule that other surfaces must obey. `/improve` is the standing example — its diff is frequently pure prose, so it is covered by the **second** criterion, not the first; "no `.rs` diff" is never the test. Full enforcement matrix: [`.claude/agents/self-review.md` § When self-review applies](.claude/agents/self-review.md).
>
> **Carve-out — `/reflect` is exempt.** Not on cost grounds but **structural** ones: `/reflect`'s product is `learnings.md` entries, and every consumer that **escalates or otherwise acts on** an entry is already contractually obliged to re-verify its claims — `.claude/agents/self-improve.md` § Step 3's CANDIDATE-truth AXIOM plus its mandatory `Claims re-verified` line on the `/improve` path, and `.claude/agents/design.md`'s `[measured:]` requirement on the filed-issue → `/task` path. The quantifier is deliberately narrow: `learnings-escalation-audit` also reads the log, but re-verifies only `Escalated?` / `Superseded by:` targets — never an entry's `Rule:` or `What happened:` claims — so it is **not** part of this guarantee. Verify a reflection entry's claims **inline while writing it** (`.claude/agents/self-reflect.md` § Per-route contracts carries the operative requirement — that inline check, not the downstream consumers, is the control this carve-out actually rests on). Generalises: **verify at the point of CONSUMPTION, not the point of RECORDING** — before adding any gate, ask where the claim is already checked downstream, and confirm that consumer really checks *the claim you care about* rather than some adjacent field.
>
> APPROVE = push. REJECT = fix on the same branch and re-run; after 3 REJECTs in a row, surface and stop without pushing.

> **AXIOM — `ai-docs/deferred/_inbox.jsonl` is written ONLY by `/task` Step 12 and `/triage`.**
> Hand-edits defeat the propagation contract — they hide rows from the parser and conflict with future Step-12 appends; the JSONL line-per-object format is hand-edit-hostile (one malformed line breaks the whole `jq` read). Row shape for the two writers: [`ai-docs/templates/inbox-row.md`](ai-docs/templates/inbox-row.md).
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
> | `.claude/skills/reflect/SKILL.md` | `.claude/agents/self-reflect.md` (Reflect group) |
> | `.claude/agents/self-reflect.md` | `.claude/skills/reflect/SKILL.md` (Reflect group) |
> | `.claude/skills/triage/SKILL.md` | `.claude/agents/triage-runner.md` AND `.claude/skills/next/SKILL.md` (Triage group) |
> | `.claude/agents/triage-runner.md` | `.claude/skills/triage/SKILL.md` AND `.claude/skills/next/SKILL.md` (Triage group) |
> | `.claude/skills/next/SKILL.md` | `.claude/skills/triage/SKILL.md` AND `.claude/agents/triage-runner.md` (Triage group) |
> | `.claude/skills/task/SKILL.md` (Steps 6–8 design phase) | `.claude/agents/design.md` AND `.claude/agents/design-review.md` AND `.claude/skills/context-reset/SKILL.md` (Task/Design group) |
> | `.claude/agents/design.md` OR `.claude/agents/design-review.md` OR `.claude/skills/context-reset/SKILL.md` | See *Task/Design group* anchor row above. |
> | `.claude/skills/task/SKILL.md` Spec/Design Amendment recipe | `.claude/skills/pr-commented/SKILL.md` AND `.claude/skills/pr-ci-failed/SKILL.md` AND `.claude/skills/main-ci-failed/SKILL.md` AND `.claude/agents/self-review.md` (Spec-Amendment group) |
> | `AGENTS.md` "Learning Log" section (Boundary rules 1/2, entry format incl. `Kind:`, `Escalated?` semantics, 🌱 verdict) | `.claude/agents/self-improve.md` AND `.claude/agents/learnings-escalation-audit.md` (Learning-Log group) |
> | `AGENTS.md` (rule add / exemption) | Run `grep -rni "<changed-keyword>" .claude/ AGENTS.md ai-docs/` and apply the same change to every match (new pre-resolved rules also add a Rule-5 substring-blacklist entry in `.claude/agents/spec-writer.md`). |
> | Any edit that changes a Tool/Subagent/Skill/Hook contract | Update `ai-docs/claude-tools-hierarchy.md` in the same PR. |
> | Any other instruction file | Run the same grep — the Procedure below catches lingering references. |

**Procedure:**
1. Before closing the edit, `grep -rni "<changed-keyword>" .claude/agents/ .claude/skills/ .claude/rules/ AGENTS.md ai-docs/` for any file referencing the same rule/terminology. **`-i` is not optional** — a sweep over prose is case-insensitive or it under-reports. Corollary: **a file you have already edited is not thereby done** — re-grep it whole, after the edit; one file holding both the fix and the surviving falsehood is the likeliest shape, not the least.
2. Apply the same change (or the corresponding enforcement adjustment) in every match.
3. AGENTS.md rule exemptions must propagate to subagent checklists that enforce the rule (`self-review.md`, `review-findings.md`).
4. When the change propagates a **factual / policy claim** (a version, a CI-gate status, a "the repo does X" statement) rather than a rule keyword, the step-1 grep set is necessary but not sufficient — also sweep repo-root user-facing docs (`README.md`, `docs/**`) for the same claim. Completeness test: every LIVE doc must agree; history surfaces (`ai-docs/learnings.md`, `ai-docs/plans/done/**`, bugfix traces) are left untouched.

Do not refer to a skill as an "agent" or vice versa — the distinction matters for spawning. (`project-review` is a skill; `review-findings` and `self-review` are agents spawned by it.)

## Communication

Interpret user phrasing literally and conservatively. When uncertain — ask, don't guess.

- **"Submit / push to PR"** = `git push` the branch to remote so commits appear in the open PR. **NOT** `gh pr merge`. Only merge when the user explicitly says "merge".
- **"wtf?" / "what?" / "huh?"** (or similar surprise/frustration) = the previous action was the opposite of what the user wanted. **Stop immediately**, do not retry, ask what was wrong before doing anything else.
- **IDE files** (`.idea/`, `*.iml`, `.vscode/`, `*.swp`, etc.) — never add, remove, modify, stage, or `.gitignore` them unless the user explicitly asks. "add ide files" most likely means **commit and track them** — confirm before acting.
- **A verbal acknowledgement is not a fix.** When the user corrects a fact — especially one you have already written into a file — the correction is a **work item**, not a conversational beat. Reply **and**, in the same turn, `grep` the artifact for the wrong claim and edit it. Tell: any reply containing *"fair"*, *"good point"*, *"you're right"*, *"consistent with"*, or *"that closes it"* that is **not accompanied by an `Edit`** to whatever asserts the now-refuted thing.
- **A recorded result is a claim, not a completion.** A sentence asserting your *own* work-state — "verified", "confirmed", "backfilled", "gate PASSed", "done" — written to any durable surface (a PR body, a `.progress.md` decisions log, a trace's `last_passed_gate`, an instruction-file diff) is a **timestamped claim**, not a standing fact. Re-run the underlying check *after the LAST edit of the turn*, immediately before recording — never record-then-edit. After fixing a claim-class defect, re-scan the **whole section**, not just the fixed line. Tell: any "verified"/"done"/"PASS" you did not (re-)produce with a command in *this* turn. (Generalises the `.progress.md`-scoped rule in `.claude/agents/code-writer.md` § Mode A and `.claude/skills/context-reset/SKILL.md` to PR bodies and trace fields.)
- **A citation offered as authority is itself a claim — open it.** Before invoking an in-repo rule, table row, learnings entry, date, or `file:line` as the reason for an action *or an inaction*, resolve it and confirm it says what you are citing it for. Three failure shapes, all observed: a **premise** attached to a correct rule (which propagates further than the rule, because a rule filed under a weak or false rationale propagates the rationale); a **banded rule** where you quote a neighbouring row's action instead of the row your own measurement falls in — and the direction of that error is predictably self-serving, because the row you half-remember is the one that lets you skip work; and a **date or `file:line`** attached to supporting evidence, where a thematically *adjacent* entry is what makes the misattribution feel checked. Special force when the cited rule lives in the file you are editing — re-measure and re-read the row first. And a reviewer's reading of a rule is an argument, not the rule (§ *Patterns* 1): verify a **permissive** reading harder than a restrictive one.
- **Deviating from user-approved scope requires an ask, not a notification.** *"I also did X — say the word if you'd rather I revert"* puts the burden of catching scope drift on the user. Ask **before** widening scope, even when the argument is compelling and the mirror cost looks low — and especially when the argument comes from a reviewer whose premise you have not run. A reviewer's finding is an argument, not a fact; run its premise before acting on it.

## Patterns

### 1. Verify an intermediary's claims — findings, retractions, premises, wave-throughs — as skeptically as each other

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

**Not only reviewers — any intermediary.** The same posture applies to a
*delegate's* claims. *Default to* verifying a delegate's design-blocking STOP
("this primitive can't satisfy its AC") with a command — compile a reduced
repro, read the cited code — before triggering a Design Amendment; it is a real
finding, but a finding, not a fact. And *Prefer* checking a delegate's "this AC
clause is untestable on the fixture I used, so I generalized/skipped it": build
the missing coverage (a purpose-built fixture) rather than accepting a PARTIAL.

Validated by [`ai-docs/learnings.md`](ai-docs/learnings.md) 2026-07-16 (reviewer
retractions/suggestions) and 2026-07-23 (verifying a delegate's design-blocking
premise before amending; /task Step-9 per-AC sweep catching a delegate's
"untestable" wave-through) — one posture across reviewer and delegate.

## Agent Docs

| Path | Purpose |
|------|---------|
| `ai-docs/context.md` | Project context (orientation) — read on demand |
| `ai-docs/context-status.md` | Per-issue implementation status log — read on demand |
| `ai-docs/code-style.md` | Workspace code-style reference — read on demand |
| `ai-docs/doc-convention.md` | rustdoc conventions — read on demand |
| `ai-docs/corrections-log.md` | Learning-Log carve-outs + field glossary |
| `ai-docs/key-decisions.md` | Key design-decision detail bodies |
| `ai-docs/api-naming.md` | `_unchecked` AXIOM + naming rules |
| `ai-docs/dependency-versions.md` | Live-lookup recipes for all five AXIOM categories: crate / Action versions, dep-graph membership, external-tool flags, **VCS tracked-ignored-committed status**, upstream issue state |
| `ai-docs/miri-gate.md` | Miri-gate mechanics + the two mechanical gp-render gate triggers — read on demand |
| `ai-docs/rust-test-conventions.md` | `proptest` oracle-cost budget + `assert_matches!` `Debug` bounds — read on demand |
| `ai-docs/delegation-rules.md` | The four-phase delegation lifecycle — read before any committing/long-running spawn |
| `ai-docs/improve-eval-contract.md` | Why `/improve` Step 6's eval dispatch is the parent's, + forbidden degraded paths |
| `ai-docs/hook-verification.md` | The three MUSTs for proving a `settings.json` hook fires — read on demand |
| `ai-docs/agent-writing-style.md` | Binary-rule writing style for dual-model readability |
| `ai-docs/agent-docs-index.md` | Verbose bodies of `§ Agent Docs` rows — read on demand |
| `ai-docs/instruction-file-validation.md` | Dual-model instruction-file-clarity test methodology |
| `ai-docs/claude-tools-hierarchy.md` | Project Tool/Subagent/Skill/Hook inventory |
| `ai-docs/templates/progress-format.md` | Canonical `.progress.md` format spec |
| `ai-docs/templates/improve-eval-reproducer.md` | `/improve` Step 6 eval reproducer template — read on demand |
| `ai-docs/templates/learnings-entry.md` | Canonical `learnings.md` entry skeleton + example — consult instead of the live log |
| `ai-docs/templates/inbox-row.md` | Canonical `_inbox.jsonl` row shape + example — consult instead of the live file |
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
> **Two exceptions, both narrow.** (a) `/improve` + `/ai-audit`: `self-improve` and `learnings-escalation-audit` MAY update `Escalated?` / `Superseded by:` on **existing** entries alongside instruction-file edits — existing-entry updates only, never a NEW entry. (b) **In-flow capture during `/task` Steps 8–12** (incl. sub-skills `/bugfix`, `/context-reset`): a NEW entry MAY be appended in the same turn as an instruction-file edit when it records an in-task insight (not a pre-emptive escalation) and is marked `Escalated? no`. Full conditions and rationale: [`ai-docs/corrections-log.md` → Boundary rule 2 Exception](ai-docs/corrections-log.md#boundary-rule-2-exception).

### Entry format

**Copyable skeleton + a filled example: [`ai-docs/templates/learnings-entry.md`](ai-docs/templates/learnings-entry.md) — consult that template to inspect the format, NOT the live log.** For orientation, an entry is a `### YYYY-MM-DD — [category] — [short description]` heading followed by `**What happened:**`, `**Rule:**`, optional `**Kind:**`, `**Escalated?**`, and optional `**Superseded by:**`.

`Kind:` defaults to `correction` when omitted. Write `Kind: validation` for entries that document a working protocol/pattern to keep doing (carrot signal); `Kind: correction` (or omit) for a violation to stop doing (stick signal). `Escalated?` records **project-level** persistence only — user-local auto-memory and `settings.local.json` do **not** count → stay `no`.

See [`ai-docs/corrections-log.md` → Entry format — field glossary](ai-docs/corrections-log.md#entry-format--field-glossary) for the semantics of each field.

Categories: `code-style` | `process` | `architecture` | `testing` | `documentation` | `tooling` | `search` | `other`

Run `/improve` when **≥3 unescalated correction entries**, **≥2 unescalated validation entries**, or a `🌱 Stale-validation` flag from `/ai-audit` accumulates.

## Rust Test Conventions

- **Miri gate — a red Miri is a REGRESSION TO FIX, but does not block merge** ([#76](https://github.com/maratik123/graphite-gp/issues/76), resolved 2026-07-18). `Miri` is deliberately **not** among `main-protection`'s required contexts (verified 2026-07-25) — held out while its wall-clock is long, pending [#134](https://github.com/maratik123/graphite-gp/issues/134). That does **not** relax the discipline: green it, or gate the offending test, before merge. **Gate every test that aborts Miri — or is a zero-production-UB-signal *cost* test — with `#[cfg_attr(miri, ignore = "<why>")]` in the SAME commit**; per-test, **never** a crate-level `--exclude` (two sanctioned exceptions, both **cost** carve-outs, never correctness ones: `gp-gen` under [#134](https://github.com/maratik123/graphite-gp/issues/134) and `gp-game` under [#184](https://github.com/maratik123/graphite-gp/issues/184)). A new test in an **excluded** crate needs no `#[cfg_attr(miri, ignore)]` — that crate never runs under CI Miri — but an existing one is kept, not stripped. Every **other** crate stays per-test. The trigger is *"aborts (or costs) under Miri"*, **NOT** *"is FFI"*, and the reason must name **that test's own** cause, never a sibling's. Local Miri is **exception-only**, on cost grounds — never routine Step-8 verification. **Read [`ai-docs/miri-gate.md`](ai-docs/miri-gate.md) before gating a test** — it carries the workspace repro command, the `gp-gen` carve-out detail, the aggregator/branch-protection mechanics, and the two mechanical `gp-render` triggers. Check Miri after any PR adding a new dependency class.
- Unit tests in the same file under a `#[cfg(test)]` module. Integration tests in `tests/`.
- `rstest` for parameterized tests; `mockall` for mocking traits; `pretty_assertions` for diffs. **`proptest` is for DIFFERENTIAL properties** — pinning a rewrite to the implementation it replaced — and you **bound the input space by what the ORACLE costs, not by the type's range**.
- Assert with `assert_eq!` / `assert_matches!`. **`assert_matches!` needs `Debug` on the scrutinee** (it formats with `{:?}` on mismatch); `assert!(matches!(...))` does not. If the scrutinee is non-`Debug`, leave `assert!(matches!(...))` alone — **never** add a production `#[derive(Debug)]` for a test-only assertion. A search miss on a construct that SHOULD exist is a **search-method failure first** ([`.claude/rules/ast-index.md` → Negative results are NOT evidence](.claude/rules/ast-index.md#negative-results-are-not-evidence)).
- Detail for both bullets above — the oracle-cost budget with its in-tree precedent, the exact `Debug` bounds, and the `rg -U` counting trap: [`ai-docs/rust-test-conventions.md`](ai-docs/rust-test-conventions.md).
- Test names as `snake_case` describing behaviour: `returns_empty_when_not_found`.
- No `unwrap()` in production code without a justifying comment; `expect("reason")` preferred.
- No `#[allow(clippy::...)]` / `#[allow(dead_code)]` unless unavoidable (with justification).
- Test behaviour, transitions, errors, edge cases. The `gp-core` physics is deterministic — assert exact states. The `supercover` predicate (`docs/design.md` §3 C4) ships with its full case table as unit tests.
