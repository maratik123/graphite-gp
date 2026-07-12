# `/dependabot-pr` reference

Per-cell bodies + delegation prompt + verdict-translation table + message templates extracted from `SKILL.md`. Loaded on demand by Step 4 once the matrix routing in Step 3 has picked a cell.

Layout:

- **Per-cell bodies** — one section per `(diff-scope × CI-state)` cell. Step 4 executes the named cell's body.
- **`/pr-ci-failed` delegation-prompt template** — the verbatim prompt the parent uses to spawn `/pr-ci-failed` from any `× red` cell, with the EXIT-between-Step-3-and-Step-4 carve-out directive.
- **Verdict-translation table** — the `(class, reproducer outcome)` → terminal-action mapping the parent applies to the child's exit state.
- **Message templates** — bail-with-issue body, bail-with-comment body, confirm-merge message.

The matrix table in `SKILL.md` § Step 3 cross-links each cell to its body anchor below.

## Cell — lockfile-only × all-green

**Terminal action:** print-merge-command-and-pause (KD-4).

Body:

1. **Local lockfile sanity check.** Run `cargo build` from the workspace root with the Dependabot branch checked out. The bot's `Cargo.lock` update must resolve cleanly locally before we suggest a merge. On non-zero exit: record the failure in the progress file, escalate to the user (treat as if the cell were `lockfile-only × red`-shaped pending re-classification on the next CI run). On zero exit: proceed.
2. **Re-fetch checks** (`gh pr checks <N> --json name,state`) to confirm the CI state has not flipped to `red` while step 1 ran. If a check has gone red: bail this round, re-route via the matrix on next invocation.
3. **Print** the [confirm-merge message template](#confirm-merge-message-template) verbatim to the user. Pause. The user runs `gh pr merge --merge <N>` manually (KD-4).

Side effects this cell may produce: none (the merge is a USER action). Step 5 AXIOM-2 re-read is therefore a no-op for this cell; the next invocation's Step 0 snapshot covers it.

## Cell — lockfile-only × red

**Terminal action:** delegate to `/pr-ci-failed` (KD-3 + KD-14) with the EXIT-between-Step-3-and-Step-4 carve-out; on return, apply the [verdict-translation table](#verdict-translation-table) on `(class, reproducer outcome)`; post the resulting `@dependabot <command>` OR open a bail-with-issue per the table.

Body:

1. **Spawn `/pr-ci-failed`** via the Skill Tool, passing the [delegation-prompt template](#pr-ci-failed-delegation-prompt-template) verbatim as the spawn arguments. The child writes into its own fallback progress file `ai-docs/ci-fixes/pr-<N>.progress.md` (KD-14: the parent does not pass a path override).
2. **Wait for the child to return.** The child EXITs between Step 3 and Step 4 per the prompt directive; no fix has been applied to the workspace, no commit, no push (KD-8 satisfied).
3. **Read the child's progress file** `ai-docs/ci-fixes/pr-<N>.progress.md`. Extract two inputs:
   - **`**Class:**`** field (set by the child at Step 2). One of `fmt` / `clippy` / `test` / `doc` / `actionlint` / `build` / `other`.
   - **`Step 3:` decisions-log bullet.** Records the reproducer outcome: "reproduced" or "NO REPRODUCE, surfaced to user". Treat `Step 3 — reproduced` lines as REPRODUCES; `Step 3 — NO REPRODUCE, surfaced to user` lines as NO-REPRODUCE.

   > **Exit-step caveat.** When `**Class:**` is `other`, the child pauses at Step 2 and never reaches Step 3. In that case the verdict-translation table maps `other` × (any reproducer outcome) → pause-for-user — so routing is robust to "Step 3 never ran". The parent reads the child's `**Class:**` field as the sole input for the `other` class.
4. **Look up the verdict** in the [verdict-translation table](#verdict-translation-table) using `(class, reproducer outcome)`. Three possible terminal actions:
   - **`@dependabot rebase`** comment — for transient CI failures (NO-REPRODUCE on inline-fix / `build` / `test` classes).
   - **bail-with-issue** — for genuine regressions (REPRODUCE on `fmt` / `clippy` / `doc` / `actionlint` / `build` / `test`).
   - **pause-for-user** — for `other` × (any).
5. **Execute** the chosen action via [bail-with-issue body template](#bail-with-issue-body-template), [bail-with-comment body template](#bail-with-comment-body-template), or the printed pause message.
6. **Record** the parent's chosen action into `ai-docs/dependabot/pr-<N>.progress.md` (a `### Decisions log (round M)` bullet, prefixed `Step 4 (verdict-translation):`).

Both progress files survive the round (gitignored). `/pr-merged` cleans both on PR merge.

## Cell — lockfile-only × pending

**Terminal action:** pause-for-user (KD-13).

Body:

1. Print a message instructing re-invocation once CI completes:

   ```
   PR #<N> CI is still running (<n> pending checks).
   /dependabot-pr does not poll — re-invoke once the checks finish.
   Pending checks: <list>
   ```
2. Stop the round. No side effect on the PR.

Step 5 AXIOM-2 is a no-op for this cell (no side-effecting action).

## Cell — scope-drift × all-green

**Terminal action:** bail-with-comment (KD-7).

Body:

1. The diff touches files beyond `Cargo.lock` despite CI being green. This is unusual for Dependabot cargo PRs — Dependabot may have updated `Cargo.toml` in addition to `Cargo.lock`, or a maintainer may have pushed an extra commit to the branch via `--allow-edits-from-maintainers`.
2. Post the [bail-with-comment body template](#bail-with-comment-body-template) on the PR (`gh pr comment <N> --body @body.md`). Do NOT close the PR — user owns close decisions (KD-7).
3. Stop the round.

Step 5 AXIOM-2 re-read fires after the comment post.

## Cell — scope-drift × red

**Terminal action:** bail-with-comment.

Body: same as `scope-drift × all-green` — CI state is moot once scope-drift is detected. The comment body MAY mention the red CI state as additional context but does not change the routing decision.

Step 5 AXIOM-2 re-read fires after the comment post.

## Cell — scope-drift × pending

**Terminal action:** bail-with-comment.

Body: same as `scope-drift × all-green` — CI state is moot. The comment body MAY note "CI still pending; not waiting" as additional context.

Step 5 AXIOM-2 re-read fires after the comment post.

## `/pr-ci-failed` delegation-prompt template

The parent passes this prompt VERBATIM when spawning `/pr-ci-failed` from any `× red` cell. The directive is **generic** (a stop-point cutoff) — it does NOT name Dependabot inside the child's executed instructions, per KD-14.

### Precondition table for the child's exit-step

| Class assigned at child Step 2 | Where the child stops |
|---|---|
| `fmt` / `clippy` / `test` / `doc` / `actionlint` / `build` | Step 3 (reproducer runs; outcome recorded; EXIT before Step 4) |
| `other` | Step 2 (pauses and surfaces logs; never enters Step 3) |

The parent's verdict-translation table accommodates both exit shapes — the `other` row ignores the reproducer-outcome column.

### Prompt (verbatim)

```
You are running under a parent skill on a branch the parent has flagged
as read-only for downstream-side fixes. EXIT between Step 3 and Step 4
— i.e., after recording your Step-3 reproducer outcome (PASS or
NO-REPRODUCE) into the progress file's `### Decisions log (round M)`
bullet (line prefixed `Step 3:`). Do NOT enter Step 4. Do NOT apply a
fix. Do NOT delegate to /bugfix. Do NOT commit or push. Your job ends
with the Step-3 decisions-log line; the parent skill consumes that line
and translates it into the user-facing action.

If your Step 2 classification is `other`, you pause at Step 2 and never
reach Step 3. The parent's translation table handles that shape — your
`**Class:**` field at Step 2 is sufficient input for the `other` row.

Mark this round's `**Self-review:**` field as `n/a (early-exit path)`.

Write progress per the standard Step-2 / Step-3 boundary contracts
already documented in your SKILL.md.
```

## Verdict-translation table

The parent applies this table to `(class, reproducer outcome)` after the child returns.

| Class | Reproducer outcome | Inferred verdict | Parent's terminal action |
|---|---|---|---|
| `fmt` / `clippy` / `doc` / `actionlint` | REPRODUCES locally | Bumped crate exposed a new lint / removed an item — workspace fix needed but Dependabot can only regenerate the bump | **bail-with-issue** (user owns whether to reject the bump, pin, or fix the workspace separately) |
| `fmt` / `clippy` / `doc` / `actionlint` | does NOT reproduce | Transient (CI environment difference) | **`@dependabot rebase`** comment; close round |
| `build` | REPRODUCES locally | Dep API change broke our code; Dependabot can't help | **bail-with-issue** (user owns: pin / patch / drop the bump) |
| `build` | does NOT reproduce | Transient | **`@dependabot rebase`** comment |
| `test` | REPRODUCES locally | Real regression — semantic change in dep | **bail-with-issue** (KD-5: never silently fork the bump) |
| `test` | does NOT reproduce | Transient or flaky test | **`@dependabot rebase`** comment |
| `other` | (any — child may have exited at Step 2) | Insufficient signal for automation | **pause-for-user**; print child's surfaced log + parent's diagnostic context |

**`@dependabot recreate` defaults.** Round 1 of the design considered `recreate` as the default for inline-fix REPRODUCE — Dependabot's `recreate` semantics regenerate the entire bump rather than apply a fix, so it does NOT solve the underlying problem (workspace lint exposed by the bump). REPRODUCE rows default to bail-with-issue; the user decides whether `recreate` is appropriate after reading the tracked issue.

## Bail-with-issue body template

Used when the verdict-translation table routes to bail-with-issue (REPRODUCE on `fmt` / `clippy` / `doc` / `actionlint` / `build` / `test`).

Two side effects:

1. `gh issue create` with the body below.
2. `gh pr comment <N>` cross-linking the new issue, so the Dependabot PR thread has the audit trail.

### Issue body

```markdown
# CI failure on Dependabot PR #<N> — <class> regression

**PR:** #<N> (`<pr-title>`)
**Failing run:** https://github.com/<O>/<R>/actions/runs/<run-id>
**Failing check:** <check-name>
**Class:** <fmt|clippy|test|doc|actionlint|build>
**Reproducer outcome:** REPRODUCES locally
**Local reproducer:** `<command>`

## Symptom

<one-paragraph summary of the failing log + reproducer evidence>

## Why /dependabot-pr bailed

This is a **real regression** introduced by the bumped crate(s) in
PR #<N>. Dependabot can only regenerate the bump (via `@dependabot
recreate`) or re-run CI (via `@dependabot rebase`); it cannot fix
workspace code or pin the offending crate. KD-5 forbids
/dependabot-pr from silently cherry-picking a fix onto a new branch.

## User options

1. **Pin** the crate at the pre-bump version in `Cargo.toml`; close PR
   #<N>.
2. **Patch** the workspace to be compatible with the new crate version;
   merge PR #<N> after the patch lands.
3. **Drop** the bump from the Dependabot group; close PR #<N>.

Each option is a manual decision — open a follow-up `/task` if the
patch path is chosen.

## Child progress file

`ai-docs/ci-fixes/pr-<N>.progress.md` — captures the child
`/pr-ci-failed`'s Step-0 through Step-3 records.

## Parent progress file

`ai-docs/dependabot/pr-<N>.progress.md` — captures `/dependabot-pr`'s
matrix routing decision and the verdict-translation lookup.
```

### Cross-link comment (posted to PR #<N>)

```markdown
`/dependabot-pr` detected a real regression in this bump (class:
`<class>`, reproducer outcome: REPRODUCES). Tracked as #<X>. Not
posting `@dependabot recreate` — the regression is in our code's
compatibility with the bumped crate, not in the bump itself. See #<X>
for user options (pin / patch / drop).
```

## Bail-with-comment body template

Used for scope-drift and ecosystem-bail.

### Scope-drift comment

```markdown
`/dependabot-pr` detected **scope-drift** on this PR: the diff touches
files beyond `Cargo.lock`. Expected diff shape for a Dependabot cargo
PR: `Cargo.lock` only. Observed:

<bulleted list of changed files>

The skill bailed without taking action. Not closing this PR — that's a
user decision (KD-7). Possible explanations:

- Dependabot updated `Cargo.toml` in addition to `Cargo.lock` (common
  for major-version bumps configured in `.github/dependabot.yml`).
- A maintainer pushed an extra commit via
  `--allow-edits-from-maintainers`.
- The bump is grouped and one sub-bump touched a non-lockfile path.

User decides whether to merge, close, or convert to a manual `/task`.
```

### Ecosystem-bail comment

Not posted on the PR — `/dependabot-pr` v1 bails at **preconditions** for non-cargo Dependabot branches, before the PR is touched. The bail message goes to the user only:

```
PR #<N> is a Dependabot `<ecosystem>` PR (branch: `<headRefName>`).
/dependabot-pr v1 supports cargo only (KD-2). Handle manually or wait
for v2 ecosystem support.
```

## Confirm-merge message template

Printed on `lockfile-only × all-green`. Hard-codes `gh pr merge --merge <N>` — NEVER `--squash` or `--rebase` (AGENTS.md `## Workflow` merge-strategy rule).

```
PR #<N> Dependabot cargo bump — READY TO MERGE.

Diff scope: lockfile-only
CI state: all-green (<n> required checks PASS)
Local validation: cargo build clean (lockfile resolves)

To merge, run:

    gh pr merge --merge <N>

(/dependabot-pr does not auto-merge — KD-4 confirm-first.)

Re-invoke /dependabot-pr after merging to handle the next open
Dependabot PR.
```
