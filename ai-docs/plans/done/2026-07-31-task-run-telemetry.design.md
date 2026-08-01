# Design: Task-run telemetry JSONL log

**Issue:** #186
**Date:** 2026-07-31
**Spec:** [`ai-docs/plans/2026-07-31-task-run-telemetry.spec.md`](2026-07-31-task-run-telemetry.spec.md)
**Branch:** `feat/2026-07-31-task-run-telemetry`

---

## Approach

### Shape of the change

Seven artefacts, one change-type. Nothing in this task is Rust:

| Artefact | Kind | New / edited |
|---|---|---|
| `ai-docs/metrics/task-runs.jsonl` | data (JSONL) | new, tracked, empty at Step 8 |
| `ai-docs/task-run-schema.md` | doc | new |
| `.claude/skills/task/scripts/append-task-run.sh` | shell | new |
| `.claude/skills/task/scripts/test-append-task-run.sh` | shell | new |
| `.claude/agents/self-review.md` | instruction | edited (`🔁 Re-opened`) |
| `.claude/skills/task/SKILL.md` | instruction | edited (Step 12 sub-step 5a, `allowed-tools`, staging list) |
| `ai-docs/claude-tools-hierarchy.md` | doc | edited (contract deltas) |

The extractor is a `jq`-composed record built from an `awk`/`grep` parse of the
progress file, invoked once from `/task` Step 12 as a new sub-step **5a** —
between inbox propagation (5) and `cargo build` (6), therefore before staging
(7). It mirrors the in-tree `check-citations.sh` / `test-check-citations.sh`
pair: documented header block, `set -uo pipefail` (never `-e`), fail-soft, and a
companion fixture test that is run by hand, not by CI.

### Key decision 1 — the whole task is ONE **instructions/harness** group, including the two shell scripts

The spec's shell script looks like a "code" artefact, and the orchestrator asked
for a deliberate call. It is **not** a code group, on two independent grounds:

1. **Path.** `.claude/agents/design.md` § Rules → handoff-grouping **(e)** defines
   the two change-types as *code* = Rust `*.rs`, *instructions/harness* =
   `*.md`, `.claude/**`, `AGENTS.md`, `ai-docs/**`. Both scripts live at
   `.claude/skills/task/scripts/*.sh` — inside `.claude/**`. By the letter of the
   rule they are instructions/harness. [measured: `Read .claude/agents/design.md` → rule (e) enumerates `code` as "Rust `*.rs`" and instructions/harness as "`*.md`, `.claude/**`, `AGENTS.md`, `ai-docs/**`"]
2. **Charter fit.** `code-writer` is a code implementor whose per-subtask gates
   are `cargo build` / `cargo test` / `cargo clippy`.
   **`ai-docs/delegation-rules.md` § *Phase 1* → "Charter fit"** states plainly:
   *"Its cargo gates are also meaningless on a diff with no `.rs`."* A `.sh` diff
   produces no cargo signal at all, so routing it to `code-writer` buys three
   no-op gates and loses the shell-specific ones (`shellcheck`, the fixture test).
   [measured: `sed -n '11,12p' ai-docs/delegation-rules.md` → l.11 carries that sentence verbatim; `sed -n '133p' AGENTS.md | grep -o 'cargo gates are also meaningless[^.]*\.'` → no match, so AGENTS.md § *Workflow* is **not** its home — it carries the condensed rule and links out]
3. **In-tree precedent.** The only comparable pair — `check-citations.sh` +
   `test-check-citations.sh` — lives under `.claude/skills/ai-audit/scripts/` and
   is owned by `/ai-audit`, an instructions/harness surface, not by any code
   flow. [measured: `find .claude -name '*.sh'` → three files, all under `.claude/skills/*/scripts/`]

Consequence: **one homogeneous group**, model `opus`, routed to
`subagent_type="general-purpose"`. This also satisfies rule (f) group
minimisation trivially — a two-group split would be an avoidable extra boundary.

### Key decision 2 — the `.claude/**` self-modification hazard is handled by ORDERING + a pre-planned in-thread takeover, not by a delegation that may fail closed

Measured facts, not assumptions:

- `.claude/settings.json` `permissions.deny` contains **no** `.claude/**` entry;
  its `allow` list carries `Edit(./**)` and `Edit(.claude/**)`.
  [measured: `cat .claude/settings.json` → `deny` = `.idea/**`, `**/.env*`, `**/secrets*`, `**/.secrets*` only]
- The one recorded failure of this class is scoped to **an agent editing its own
  definition file**: `self-improve` was denied on `.claude/agents/self-improve.md`;
  the entry's `Rule:` reads *"any edit that triggers one (self-modification of an
  agent's own definition; anything the harness gates) fails closed regardless of
  `Edit(...)` allow-lists"*, and its remedy is *"Apply protected-file edits
  in-thread, or scope them out of the delegation explicitly."*
  [measured: `sed -n '88,102p' ai-docs/learnings.md` → the 2026-07-16 entry *"spawned a background subagent for edits it structurally could not make"*, `Escalated? AGENTS.md`]

Neither `.claude/agents/self-review.md` nor `.claude/skills/task/SKILL.md` is the
`general-purpose` implementor's own definition, so the recorded incident does not
predict a denial here — **and it does not predict a success either.** Per
`.claude/agents/design.md` § Quality checklist → Claims, a negative
("a `general-purpose` delegate cannot be blocked on these paths") names no
artifact that any gate would run, so this design asserts neither direction.
Instead it removes the cost of being wrong:

1. **Subtask order front-loads the two `ai-docs/**` artefacts that have no
   `.claude/**` dependency** (1 — the log file, 2 — the schema page), so a denial
   at the first `.claude/**` write (subtask 3) leaves *those two* committed and
   intact. Stated precisely, because the weaker claim is the true one: this is
   **not** "every `ai-docs/**` artefact is front-loaded". Subtask 7
   (`claude-tools-hierarchy.md`) sits behind two `.claude/**` writes by
   dependency, and subtask 8 touches whatever the sweep hits. The mitigation
   bounds the blast radius; it does not eliminate it.
2. **The group's spawn prompt carries one explicit contingency**: *if any
   `.claude/**` edit is denied by the permission system, STOP, record the denied
   path in the progress file's `## Decisions log`, and return.* Do **not** retry,
   do **not** rationalise (the learnings entry's second corollary: *"an agent that
   refuses to self-modify on an unverifiable relay is exercising a sound
   default … do not argue it out of the refusal; remove the need for it"*).
3. **On such a return the orchestrator applies the remaining `.claude/**` edits
   in-thread**, per **`ai-docs/delegation-rules.md` § *Phase 1* → "Environment
   fit"** (*"Apply those in-thread"*). This is the documented recovery from the
   same incident, pre-planned rather than improvised, and it costs nothing if the
   delegation succeeds.
   [measured: `sed -n '11,12p' ai-docs/delegation-rules.md` → l.12 carries that sentence verbatim; `sed -n '133p' AGENTS.md | grep -o 'Apply those in-thread'` → no match. AGENTS.md § *Workflow* carries only *"protected-file edits fail closed regardless of allow-lists"* and points here.]

### Key decision 3 — sub-step **5a**, not a renumber

`/task` Step 12 today runs 1 step-skip gate · 2 progress-not-staged · 3 branch
check · 4 INDEX + `done/` move · 5 inbox propagation · 6 `cargo build` ·
7 stage · 8 commit · 9 push · 10 `gh pr create` · 11 post URL · 12 write progress.
[measured: `Read .claude/skills/task/SKILL.md` lines 221–237]

Inserting the append as **5a** rather than a new `6` (with 6→13 renumbered):

- keeps SKILL.md l.230's *"The Step 12 commit (sub-step 7 below) stages
  `_inbox.jsonl`"* correct with **zero** edits;
- avoids a twelve-line renumber whose char delta the file cannot afford (see
  Key decision 6);
- matches in-repo idiom for inserted steps (`/task` Step 9.5, `/bugfix` Step 6.5).
  [measured: `grep -n 'Step 9.5' .claude/skills/task/SKILL.md` → l.176; `grep -n '⬜ Open' .claude/skills/bugfix/SKILL.md` → `Step 6.5` at l.276]

AC2 is order-based ("ordered **before** the stage sub-step"), not
number-based; `5a < 7` satisfies it.

**Rejected alternative:** renumber to `6`. Rejected on char budget + cross-reference
churn, not on aesthetics.

### Key decision 4 — `instruction_corpus_lines` measurement point (the moving-target problem)

The number must satisfy AC14: *re-running the pinned command at the implementing
commit yields the number recorded in that commit's own record*. Three properties
make that achievable:

1. **The record's own file is not in the corpus set.**
   `ai-docs/metrics/task-runs.jsonl` is not `.md`, so appending the record
   cannot change the number it contains. No fixed-point problem exists.
   [derived → discharged by the AC14 re-run in the **Step-12 verification
   block**, after sub-step 5a — *not* at Step 9, where the log is still empty
   and the check has no line to read]
2. **No Step-12 sub-step after 5a touches a corpus-set file.** Sub-step 4 moves
   spec/design into `ai-docs/plans/done/` — `:(glob)ai-docs/*.md` is **depth-1
   only**, so `ai-docs/plans/**` is outside the set; sub-step 5 writes
   `_inbox.jsonl` (not `.md`); 6 is `cargo build`; 7–11 are git/gh.
   [measured: `git ls-files -- ':(glob)ai-docs/*.md' | awk -F/ '{print NF}' | sort -u` → `2` (depth-1 only)]
3. **Tracked-vs-worktree gap is closed by a precondition assertion.**
   `git ls-files` enumerates *index* paths while `cat` reads the *working tree*.
   A corpus-set file created by this task but not yet staged would be silently
   omitted from the count while still landing in the commit. Sub-step 5a's
   **first action** is therefore the assertion below.

   > **Where this block lives:** on `ai-docs/task-run-schema.md`, **not** inline in
   > sub-step 5a — Key decision 6's relief valve is mandatory, and 5a carries a
   > one-line pointer to it. It is reproduced here for this document's
   > readability; inlining it in `SKILL.md` would blow the ≤ 620-char budget.

   ```bash
   git ls-files --others --exclude-standard -- \
     'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md'
   ```

   Non-empty output → `git add` those paths (they are going into this same
   commit at sub-step 7 regardless), then re-run until empty. Only then invoke
   the script. Every *tracked* corpus file already carries its worktree edits, so
   after this assertion the measured set == the committed set.
   [measured: the assertion returns empty on the current tree — `git ls-files --others --exclude-standard -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md'` → no output, exit 0]

**The pinned command gained a `:(exclude)` term in spec round 12.** The corpus
set is now Broad **minus files excluded by the criterion** — *files whose volume
is determined by the codebase or by journaling rather than by instruction
content*. The command this design specifies is therefore:

```bash
git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
  ':(exclude)ai-docs/learnings.md' \
  | xargs -0 cat | wc -l
```

**The baseline is informational — nothing in this design gates on its value.**
AC14 compares the record against a **re-run at the implementing commit**, never
against a number written here.

| | Value | Note |
|---|---|---|
| Post-exclusion (the counted corpus) | **9,091 / 59 files** | the v1 baseline |
| Unexcluded Broad | 9,701 / 60 files | what the old figure measured |
| `ai-docs/learnings.md` | 610 lines | the sole v1 exclusion; `9,701 − 610 = 9,091` ✓ |

**The old 9,403 is superseded and NOT comparable** — it predates both the
exclusion and this task's own additions. Round 7's `9,689` is likewise dead. Any
series starts at the post-exclusion baseline.
[measured: pinned command with `:(exclude)` → `9091`, file count → `59`; without the exclude term → `9701` / `60`; `wc -l < ai-docs/learnings.md` → `610`. The arithmetic closes exactly, and note the exclusion makes the baseline **immune to `learnings.md`'s current uncommitted state** (HEAD has 568 lines, the working tree 610) — the one dirty file in the corpus is the one no longer counted]

**The non-`:(glob)` form is wrong, and the durable statement is a re-runnable
discriminator, not a line count.** `38,538` is gone from the spec and from this
design. What the bare pathspec actually does is reach below depth 1 into
`ai-docs/plans/`, `ai-docs/plans/done/` and `ai-docs/templates/` — the archive of
completed specs, which has nothing to do with the instruction corpus. Separate the
two forms with:

```bash
git ls-files -- '<form>' | grep -vc '^ai-docs/[^/]*\.md$'   # bare → 109 ; :(glob) → 0
```

**Scope matters and is easy to get wrong:** the probe is over the **`ai-docs`
pathspec alone**. Run across the whole corpus set it yields `147` / `38`, because
the 38 non-`ai-docs` members (`AGENTS.md`, `CLAUDE.md`, `.claude/**`) fail the
`^ai-docs/…$` pattern on *both* sides and are counted in both. `147 − 38 = 109`,
the same finding — but `147/38` is not the stated figure and quoting it as such
would misreport the probe.
[measured: `git ls-files -- 'ai-docs/*.md' | grep -vc '^ai-docs/[^/]*\.md$'` → `109`; the `:(glob)` form → `0`; whole-corpus-set variants → `147` and `38`, and `147-38=109`]

The spec's "`:(glob)` matches at depth 1" claim was **re-verified here rather than
carried**, because no depth-1 `.claude/*.md` file currently exists so the live
file list cannot demonstrate it:
[measured: `touch .claude/__depth1_probe.md; git ls-files --others --exclude-standard -- ':(glob).claude/**/*.md'` → `.claude/__depth1_probe.md`; probe removed, `git status --porcelain` clean afterwards except the untracked spec]

### Key decision 4b — which permission layer binds, and the exact invocation spelling

Round 2 raised a concern that sub-step 5a's `git ls-files --others
--exclude-standard …` falls outside `/task`'s frontmatter `allowed-tools`.
Determined from the tree rather than accepted:

- **`settings.json` grants `Bash(git *)`**, which covers every `git` subcommand
  including `ls-files`.
  [measured: `jq -r '.permissions.allow[]' .claude/settings.json` → includes `Bash(git *)`, `Bash(jq *)`, `Bash(grep *)`, `Bash(awk *)`, `Bash(wc *)`, `Bash(shellcheck *)`]
- **`allowed-tools` does not hard-block unlisted commands.** The decisive
  counter-example is `/task`'s own first action: `ls ai-docs/plans/*.progress.md`
  (SKILL.md l.48), run on **every** invocation. `ls` appears in neither
  `/task`'s frontmatter nor `settings.json` — if the frontmatter were a
  narrowing whitelist, the skill's opening probe would prompt every time.
  [measured: `grep -o 'Bash([^)]*)' .claude/skills/task/SKILL.md | grep -E '\((ls|bash) '` → no match; `jq -r '.permissions.allow[]' .claude/settings.json | grep -E 'Bash\((ls|bash) '` → no match]

**Conclusion: no `allowed-tools` entry is needed for `git ls-files`** — adding one
would be redundant with `Bash(git *)`. The reviewer's premise was half right (the
frontmatter indeed lacks it) and its conclusion wrong (that does not make it
ungranted).

**The script invocation is different** — nothing in `settings.json` grants it
(`Bash(bash *)` is absent, and no pattern covers `.claude/skills/**`), so the
frontmatter entry **is** worth adding, and the spelling must match:

| Requirement | Value |
|---|---|
| `allowed-tools` entry | `Bash(.claude/skills/task/scripts/append-task-run.sh *)` — trailing ` *` because the script takes arguments |
| SKILL.md body spelling | `.claude/skills/task/scripts/append-task-run.sh <progress-file>` — **direct execution** |
| Spelling to avoid **for this script** | `bash .claude/skills/task/scripts/append-task-run.sh …` — the command word is `bash`, so it matches neither the new entry nor anything in `settings.json`, which would make the entry we just added **dead**. Not a hard block (see § *Prompt-on-use commands* below); a wasted declaration. |
| File mode | `100755` in the index, matching all three in-tree scripts — a direct-exec form requires it |

In-tree, the closest correct model is `/pr-merged`, whose script also takes an
argument and which declares the ` *` form. Two spelling mismatches already exist
in-tree and are worth not copying: `/ai-audit` declares its two scripts
**without** ` *` while their own usage lines spell a `bash <script>` form, and
`/pr-merged`'s body invokes via `${CLAUDE_SKILL_DIR}/…` (an absolute path) against
a relative declared pattern. Neither is a live failure today — precisely because
`allowed-tools` is a pre-allow list, not a gate — but both would be if it ever
tightened.
[measured: `grep -o 'Bash(\.claude[^)]*)' .claude/skills/ai-audit/SKILL.md` → both entries lack ` *`; `sed -n '1,12p' .claude/skills/pr-merged/SKILL.md` → `Bash(.claude/skills/pr-merged/scripts/cleanup-progress.sh *)` while l.28 invokes `${CLAUDE_SKILL_DIR}/scripts/cleanup-progress.sh <previous-branch>`; `git ls-files -s .claude/skills/*/scripts/*.sh` → all three `100755`]

#### Prompt-on-use commands — accepted, not silently assumed away

The round-2 text said the test script "is run ad hoc, not from `SKILL.md`" while
this design also pins it to Step 9 — which *is* `/task`. Both cannot be true.
The honest position:

Because `allowed-tools` is **additive** (established above), an ungranted command
**prompts**; it does not fail closed. Several commands this design specifies are
granted by neither layer: `bash` (the test-script invocation), `xxd`, `realpath`,
`tail`, `comm`, `mktemp`, `git init` (case 12's sandbox repo — `Bash(git *)` covers
it, unlike the rest).

**Scope, stated accurately.** These are **orchestrator-side at Step 9 and in the
Step-12 verification block, plus exactly one delegate-side invocation**: subtask 4
requires the fixture test to reach all-cases GREEN, and § Test Design spells that
`bash .claude/skills/task/scripts/test-append-task-run.sh` — run by the Group A
**background Subagent**, which cannot answer a prompt. The round-3 wording
("orchestrator-side … run by an interactive session that *can* answer") was wrong
about that one invocation.
[measured: Decomposition subtask 4 requires an all-cases-GREEN run (count derived, not transcribed); § *Test Design* → *Location* spells the invocation as `bash .claude/skills/task/scripts/test-append-task-run.sh`; § *Handoff plan* assigns subtask 4 to Group A]

Consequence, measured as nil: an ungranted Bash command from a background Subagent
here **prompts-or-proceeds, it does not fail closed** — unlike the `.claude/**`
*edit* case of Key decision 2, where a self-modification guard is what fires. And
the failure mode is bounded regardless: R6 already requires the fixture test to be
re-run at **Step 9 by the orchestrator**, so a delegate-side stall surfaces as a
subtask-4 non-completion the orchestrator re-runs, never as a silently skipped gate.

**Decision: accept the prompts; add no further `allowed-tools` entries.** Reasons,
in order of weight: (1) AC16 is delta-wise and `.claude/skills/task/SKILL.md` has
**1,191 chars** of headroom — six more entries would cost ~200 of it against a
budget already tightened below; (2) the one delegate-side invocation is bounded by
R6's Step-9 re-run, and every other use is orchestrator-side where a prompt is
answerable; (3) `/task` already runs `ls` this way on every single invocation, so
prompt-on-use is the skill's existing normal, not a regression this task
introduces.

The **one** entry that is added — for `append-task-run.sh` — is added because that
invocation happens inside a `/task` sub-step on every future run of the skill, not
once during this task's own verification. That is the distinction: recurring
harness use earns a declaration; one-off verification does not.

### Key decision 5 — the `🔁 Re-opened` marker lands on the NEW round's row

`self-review.md` Instruction 8 reads *"**Append** a `## Self-Review (Round N)`
section to the progress file (do not replace existing sections)"*.
[measured: `Read .claude/agents/self-review.md` l.41]

A § 7 re-open therefore cannot rewrite the round-N cell; it emits a row in the
**round N+1** table carrying `⬜ Open 🔁 Re-opened`. This is the only reading
consistent with the append-only rule, and it makes `objections_reopened` (a count
of cells across all sections) well-defined without identity tracking. The design
pins this mechanic in the `self-review.md` edit so the parser and the writer agree.

**No example table row is added.** The spec permits one ("a vocabulary line; an
example row is optional"); adding one would move the mechanically-derived
template subset from 3 rows to 4 and complicate AC4's re-derivation for no gain.
The AC4 contract is therefore sharpened: after the change,

```bash
grep -rn '⬜ Open' .claude/ --include='*.md' | grep -E ':\| [0-9]'
```

must still return **exactly the same 3 rows**.

> **`--include='*.md'` is load-bearing, and round 6 got this wrong.** The
> unscoped form was specified in rounds 1–6 and is **already broken by this
> design's own subtask 3**: the fixture test embeds progress-file heredocs whose
> self-review table rows begin `| 1 |` and carry `⬜ Open` statuses, so they match
> the template pattern. Unscoped, the check returns **11** today and would grow
> again when case 14's drift rows land — a gate that fails for a reason having
> nothing to do with the marker. Restricting to `*.md` keeps the check on
> *instruction* files, which is the population AC4 is about; fixture data under
> `.claude/skills/task/scripts/` is not an instruction template.
> [measured: `grep -rn '⬜ Open' .claude/ | grep -E ':\| [0-9]' | wc -l` → `11` (8 of them in the untracked `test-append-task-run.sh`); the same command with `--include='*.md'` → `3`]

[measured: the scoped command today → `.claude/agents/self-review.md:157`, `:158`, `.claude/agents/review-findings.md:155` — the same three rows the spec's § *Templates (3)* names]

### Key decision 6 — hard char budget on `.claude/skills/task/SKILL.md`

| File | `wc -c` now | Band | Headroom to 35,000 |
|---|---|---|---|
| `.claude/skills/task/SKILL.md` | 33,809 | OK | **+1,191** |
| `.claude/skills/task/reference.md` | 33,254 | OK | +1,746 |
| `.claude/agents/self-review.md` | 24,576 | OK | +10,424 |
| `AGENTS.md` | 38,874 | pre-existing, **unchanged by this task** | n/a — out of scope (R4) |
| `ai-docs/claude-tools-hierarchy.md` | 8,123 | OK (not in the AXIOM set) | n/a |

[measured: `wc -c AGENTS.md CLAUDE.md .claude/skills/*/*.md .claude/agents/*.md .claude/rules/*.md ai-docs/{code-style,doc-convention,context,agent-writing-style,corrections-log}.md | sort -rn`]

**Budget re-derived in round 3.** The original `≤ 850` figure predated the
§ *Step-12 verification block*, which subtask 6 places *inside* 5a. Costed:
the verification fenced block ≈ 380 chars, the precondition-assertion fenced
block ≈ 190, plus invocation + exit-code routing + schema link ≈ 300 → ≈ 870 for
5a alone, and ≈ 987 total delta. That lands at ≈ 34,796 — inside 35,000, but on
**204 chars** of margin, with no room for a rewording during implementation.

**The pressure-relief valve is therefore REQUIRED, not optional.** Both fenced
blocks move onto `ai-docs/task-run-schema.md` (uncapped — not in the AXIOM's
enumerated set); sub-step 5a keeps one-line pointers to them. It must **not** go
into `reference.md`, which is itself only 1,746 chars from the warning band.

Re-derived budget:

| Item | Chars |
|---|---|
| Sub-step 5a body — prose + **three** pointers, no fenced blocks | ≤ 620 |
| `allowed-tools` entry (one, per Key decision 4b) | ≈ 55 |
| Staging-list addition in sub-step 7 | ≈ 62 |
| **Total delta** | **≈ 737** |
| Expected landing (33,809 + 737) | **≈ 34,546** |

Planning aid (**not a gate input** — see § *Figure dependencies*): the edit is expected to land near **34,546** against a measured **33,809**. The gate is `AFTER < 35000` with `BEFORE` re-measured from the base commit;
binding constraint is AC16's delta-wise **< 35,000**. Margin at the expected
landing is ~454 chars.

**Re-checked in round 4 against the grown field count, not carried.** The spec went
from 13 schema fields to **18**, and the `fallback-required` set from 6 to **9** —
which lengthens the *fallback recipe* the orchestrator follows on a non-zero exit
(it must now hand-emit the diff-size trio off `git merge-base main HEAD`). That
growth lands **entirely on the schema page**, which is uncapped: sub-step 5a gains
one more pointer (`… per <schema page> § Fallback recipe`), +20 chars, not a
recipe. This is the relief valve paying for itself — the field count can grow
again without touching `SKILL.md`'s budget, which is precisely why the valve was
made mandatory in round 3 rather than left as a contingency.

**Re-checked in round 6 — confirmed, not assumed.** Spec amendment 2 adds three
clauses to the schema page (subtask 2 🔁), one fixture pair plus one case to the
test script, and one risk row here. **None of it reaches sub-step 5a**, so the
`≤ 620` / `≈ 737` / `≈ 34,546` figures above stand unchanged. The two files that
grow are both outside the AXIOM's enumerated set, which is the property that had
to be checked rather than assumed:
[measured: `grep -n 'ai-docs/{code-style' AGENTS.md` → the AXIOM enumerates `AGENTS.md`, `CLAUDE.md`, `.claude/skills/**/*.md`, `.claude/agents/**.md`, `.claude/rules/*.md`, and `ai-docs/{code-style,doc-convention,context,agent-writing-style,corrections-log}.md` — `ai-docs/task-run-schema.md` is in none of those, and `.claude/skills/task/scripts/*.sh` is not `*.md`]
[measured: `grep -c 'task-run-schema' AGENTS.md` → `0`; `wc -c ai-docs/task-run-schema.md .claude/skills/task/SKILL.md` → `14514` and `33809` — `SKILL.md` is byte-identical to its round-3 baseline, so no re-budget is owed]

### Key decision 7 — AGENTS.md is NOT edited

- No AC names AGENTS.md.
- The spec places the single-writer / hand-edit prohibition **in the schema
  doc**, explicitly: *"The new file is a separate surface with its own single
  writer and its own hand-edit prohibition, stated in the schema doc."*
- AGENTS.md is at **38,874** chars — already past the 35,000 early warning. AC16
  is read delta-wise (R4), and the cleanest way to keep `AGENTS.md` out of this
  task's delta is to not edit it: `git diff --stat -- AGENTS.md` stays empty.
  Two § *Agent Docs* rows would add ~250 chars for no AC.
- No new keyword already appears in AGENTS.md, so the AC15 sweep does not reach
  it. [measured: `grep -rni 'Re-opened\|task-runs\|append-task-run\|ai-docs/metrics' AGENTS.md` → no hits]

Surfaced as an Open question for owner override, not silently dropped.

### Figures that move — read these live, never from this document

Round 6 hard-coded `≥ 6` for `AC13b`'s entry floor beside a prose note saying the
count was moving; the spec closed at **eight**, so the gate would have passed an
implementation that failed the AC. The prose hedge was right and useless — a
number and a warning-about-the-number sitting side by side decay independently,
and the number is what gets executed. Every figure below is therefore either
**derived at run time** or **explicitly marked as a dated baseline with no gate
attached**. Nothing in this design may gate on a transcribed count.

| Figure | Status | How to obtain it |
|---|---|---|
| `AC13b` entry floor | **DERIVED — never transcribed** | § *Deriving `AC13b`'s entry floor*; cross-checks the stated word against the enumerated markers and fails loudly if they disagree |
| `instruction_corpus_lines` baseline | dated baseline, **no gate** | Re-run the pinned command **including its `:(exclude)` term**. `9403`/59 (round 6) → `9689`/60 (round 7) → **`9091`/59** (round 8, post-exclusion). The first two are **superseded and not comparable** — they predate the exclusion criterion. Moves again with subtask 2 🔁. AC14 compares record-vs-re-run, never against a literal |
| `:(glob)` vs bare pathspec | **DERIVED discriminator, no figure** | `git ls-files -- '<form>' \| grep -vc '^ai-docs/[^/]*\.md$'` → `109` bare / `0` glob over the **`ai-docs` pathspec alone**. `38,538` is retired. Whole-corpus-set scoping gives `147`/`38` — same finding, different figure; do not quote it as the stated one |
| Corpus **exclusion membership** (`learnings.md` in v1) | **derived from the criterion, dated** | Never a definition. Re-derive against the 1%-of-counted-corpus threshold at each pinning; crossings are **one-way** |
| `⬜ Open` census (14 occurrences / 6 files; 3 template / 11 consumer) | **pre-implementation baseline, NOT a live invariant** | Mirrors the spec's own § *ADDITIVE* framing. Already `22` / 7 files, because subtask 3's fixture heredocs carry the token. `AC4` is a **property**, so no test depends on it |
| Template-row subset (`3`) | live invariant, **but only when scoped** | `grep -rn '⬜ Open' .claude/ --include='*.md' \| grep -E ':\| [0-9]'`. Unscoped it reads `11` — Key decision 5 |
| `wc -c` file sizes (`SKILL.md` 33,809 · `AGENTS.md` 38,874 · …) | dated baselines | Re-measure at Step 9; AC16 is delta-wise, so the *comparison* is the gate, not the stored number |
| `LOCAL_MAX` PR high-water (`185`) | monotonically increasing | R2 depends only on `42 ≤ LOCAL_MAX`, which holds for every future value. Stated as an invariant, so no re-measure is owed |
| `_inbox.jsonl` size (393 lines / 149,194 B) | dated, illustrative | Scale reference for the no-rotation decision; the argument survives any growth |
| Fixture draft size (449 lines / 18,673 B) | dated | Subtask 3 amends it; re-`wc` before editing |

[measured: every value in this table re-run on the settled tree in **round 8** — corpus post-exclusion `9091`/`59` (pre-exclusion `9701`/`60`, `learnings.md` `610`); discriminator `109` bare / `0` glob over the `ai-docs` pathspec, `147`/`38` whole-set; `grep -rn '⬜ Open' .claude/ | wc -l` → `22`, `grep -rln … | wc -l` → `7`; scoped template subset → `3`, unscoped → `11`; `wc -lc .claude/skills/task/scripts/test-append-task-run.sh` → `449 18673`; `gh pr list --state all --limit 1 --json number --jq '.[0].number'` → `185`]

### Citation audit (round 8) — sorted by whether the reference holds a value

**196 `AC*` references across 20 ACs.** Auditing them by re-resolving each is
wasted effort, because a reference that is *only a pointer* (`per AC13a`,
`fails AC9a`) **cannot drift** — the AC id is stable by construction, since
`AC9a` / `AC11a` / `AC13a` / `AC13b` were added as suffixes precisely to avoid
renumbering. What breaks is a reference carrying **a value or predicate copied
from the spec**. Those were read first; the rest were confirmed as pointers.

| Value-bearing citation | Round-7 value | Round-8 status |
|---|---|---|
| `AC13b` → entry floor | `>= 6`, hard-coded | **MOVED 6 -> 8 (round 8) -> 9 (round 9).** Round 7 replaced the literal with a derivation, and it has now absorbed **two** successive changes with **zero edits** — reading `word=eight num=8 enum=8 floor=8`, then `word=nine num=9 enum=9 floor=9`. This is the audit's load-bearing result: the fix has survived every drift it has met, and each drift would have silently broken a transcribed number |
| `AC14` → corpus baseline | `9,689 / 60` | **MOVED to `9,091 / 59`**, and the basis changed — a `:(exclude)` term now applies. Prior figures are **superseded and not comparable** |
| `AC14` → `:(glob)` justification | `38,538` vs `9,403` | **RETIRED.** Replaced by the re-runnable discriminator `109` / `0`; `38,538` appears nowhere in the spec |
| `AC14` → scope of the criterion | absent | **NEW clauses**: property-not-name, derived membership, 1 % threshold, one-way directionality. Subtask 2 🔁 clause (e) and four AC14 greps added |
| `AC4` → `11 consumer / 3 template` census | dated baseline | **Unchanged as a baseline**; already time-anchored in round 7. The live invariant is the *scoped* template subset (`3`) |
| `AC16` → `33,809` / `38,874` / `24,576` | measured | **Unchanged** — re-measured this round, all three identical, and the spec still records `38,874` |
| `AC10` → 18 fields / 9 `fallback-required` | measured | **Unchanged** |
| `AC13` → "10 findings per round" | spec value | **Unchanged** |
| `AC9` / `AC9a` → fixture counts (`10`/`9`/`12`, `minor 4`) | own derivation | **Not spec-owned** — derived from this design's own fixture and machine-re-verified in round 6; immune to spec drift by construction |

[measured: `wc -c` over the AC16 set → `38874` / `33809` / `24576` / `33254` / `8123`, all identical to round 6; `grep -E '^\| AC13a \|'` and `'^\| AC9a \|'` phrase checks → every clause this design depends on still present; the four `AC13b` gate anchors (`What this log does NOT measure`, `not near-free`, `conjecture`, `cannot report otherwise`) all survive the round-12/13 restructure]

### Key decision 8 — `findings_first_seen` and its degeneracy signature are ONE deliverable (spec amendment 2)

Spec § *Key decisions* → *Row identity across rounds* and `AC13a` now carry the
same **coupling clause**: the field and its degeneracy signature **ship together
or not at all**.
[measured: `grep -c 'ship together or not at all' ai-docs/plans/2026-07-31-task-run-telemetry.spec.md` → `2` — once at the identity decision, once at `AC13a`]

The design reflects it as a **mechanical dependency**, not as a sentence an
implementer could satisfy on one side only:

1. **Subtask 2 owns both.** The field's schema-page entry, the frequency
   statement, and the signature paragraph are written by the *same* subtask into
   the *same* file in the *same* commit. There is no ordering in which the field
   is documented and the signature is not.
2. **The `AC13a` greps are conjunctive, and the signature has its own.** Seven
   independent greps (§ *AC verification commands*), each of which must hit
   standalone; the signature grep failing fails `AC13a` however well the other
   six pass. An alternation would let a missing clause hide behind a sibling —
   the reason the design already refused one at round 4.
3. **The failure branch is prescribed, not left to judgement.** If the signature
   grep ever misses — in this PR or in any later edit — the response is to
   **remove `findings_first_seen`** from the schema page, the record schema, the
   extractor and the fixture, never to keep the field and drop the sentence. R10
   carries this as a standing risk beyond this PR.

**Signature strength is capped, not just floored.** The page must state it at
exactly the spec's strength: on a run with `rounds > 1`, `findings_first_seen ==
findings` means **no row matched between rounds**; the log **does not distinguish
the cause** (genuinely no repeats vs. a drifted key); a ratio **below 1** is
evidence the key held for ≥ 1 row. Stating it *more* strongly — "a ratio of 1
almost certainly means drift" — is **false**, because `self-review.md:173` does
not re-raise `✅ Fixed` items, so a run whose round-1 findings were all fixed and
whose round 2 raised different ones legitimately yields a ratio of exactly 1.
[measured: `awk 'NR==173' .claude/agents/self-review.md` → ``  - `✅ Fixed` items: do not re-raise unless the fix is incorrect or incomplete.``]

**What this design deliberately does NOT do.** It does **not** add a line to
`.claude/agents/self-review.md` requiring a carried-forward finding to keep its
`Finding` cell verbatim. That would fix the key at the root, but it is an
**extension of the AC3 carve-out**, ruled out of scope by the owner and deferred
to a post-merge `learnings.md` entry. No such requirement exists in the file
today and none is added:
[measured: `grep -rniE 'verbatim|unchanged text|same wording|identical text' .claude/agents/self-review.md` → no output, exit 1 — the absence is measured here, not inferred from the spec's assertion of it]

### Rejected alternatives

| Alternative | Why rejected |
|---|---|
| Rust extractor in a workspace crate | The record's source is a gitignored markdown file read once per task by the harness; a crate adds a build-graph edge, a CI lane, and a `#[cfg(test)]` block to solve a `jq` one-liner's problem. |
| In-place rewrite of a duplicate `(spec_base, branch)` line | See *Open questions → resolution 1*: needs read-modify-write of the whole corpus file; a partial write corrupts every line, which is exactly the failure AC13's hand-edit prohibition exists to prevent. |
| `code-writer` for the shell scripts | Key decision 1 — cargo gates produce no signal on a `.sh` diff. |
| Adding a `🔁 Re-opened` example row to § *Findings format* | Key decision 5 — perturbs AC4's mechanical template/consumer split for no informational gain. |
| Fixture test that mutates a tracked file (the `test-check-citations.sh` shape) | Improved on deliberately — see *Test Design § Fixture strategy*. |

---

## Record schema (v1)

Emitted as **one line**, keys in this order (order is cosmetic; `jq keys` sorts).

| Field | Type | Source | Fallback class |
|---|---|---|---|
| `schema_version` | int (`1`) | literal | **fallback-required** |
| `date` | string `YYYY-MM-DD` | `date -u +%F` | **fallback-required** |
| `branch` | string | `git branch --show-current` | **fallback-required** |
| `issue` | int \| null | progress `**Issue:** #N` (script) / ambient (fallback) | **fallback-required** |
| ↳ | | The canonical template specifies `**Issue:** [#number **or URL**]`, so the `#N` form is not guaranteed. The parser extracts `#N` only; a URL (or an absent line) yields `null` **and trips `incomplete`** — see the trigger table. Widening the parser to accept a URL was rejected: `null` is honest and total, whereas a URL regex would silently invent an issue number from any path segment that happens to be digits. [measured: `Read ai-docs/templates/progress-format.md` l.16 → `**Issue:** [#number or URL]`] |
| `spec_base` | string | `basename <progress-path> .progress.md` | **fallback-required** |
| `incomplete` | bool | derived, see table below | **fallback-required** |
| `files_changed` | int | `git diff --shortstat <base>..HEAD`, **by keyword** | **fallback-required** |
| `insertions` | int | same, by keyword | **fallback-required** |
| `deletions` | int | same, by keyword | **fallback-required** |
| `rounds` | int | count of `^## Self-Review \(Round [0-9]+\)$` | fallback-optional |
| `hit_round_cap` | bool | `rounds >= 3 && verdicts[2] == "REJECT"` | fallback-optional |
| `verdicts` | array\<string\> | first `^\*\*Verdict:\*\*` line per section, in order | fallback-optional |
| `findings` | object `{blocker,major,minor,nit}` int | severity cell of every `^\| [0-9]` row inside a Self-Review section, **summed across all rounds** | fallback-optional |
| `findings_first_seen` | object `{blocker,major,minor,nit}` int | same rows, but only those whose **`File:line` cell** is absent from the *immediately preceding* round's table; round 1 contributes all its rows | fallback-optional |
| ↳ | | **The key drifts in the common case** (R9): a Step-11 fix that changes a file's line count shifts every line below it, and `self-review` re-derives locations each round (`self-review.md:169`), so a carried-forward finding arrives under a different key and re-counts as first-seen. Retained with the limit documented, **not** repaired — the key stays `File:line` by owner decision. The field is **coupled** to its degeneracy signature on the schema page (Key decision 8): if the signature is ever absent, the field is removed, never kept unlabelled. |
| `objections` | int | cells containing `⚠️ Objected` (substring), summed across rounds | fallback-optional |
| `objections_reopened` | int | cells containing `🔁 Re-opened` (substring), summed across rounds | fallback-optional |
| `files_touched` | array\<string\> | `` ^- `<path>` `` under `## Files touched` | fallback-optional |
| `instruction_corpus_lines` | int | the pinned command, **`:(exclude)ai-docs/learnings.md` included** (spec round 12) | fallback-optional |

**18 fields, 9 `fallback-required` / 9 `fallback-optional`.** The required set grew
by the diff-size trio; AC10's `jq` containment assertion (case 11) moves with it
and is re-derived from the schema page's own table rather than restated, so the
count cannot drift.

### The diff-size trio: base commit and parsing

**Two-source base rule (spec § *The two-path contract*).** The script uses the
progress file's `**base_commit:**`; the fallback uses `git merge-base main HEAD`,
which needs no progress file. In the normal flow they coincide.
[measured: `git merge-base main HEAD` → `a6d894638e289b27cf090dbb57bdf6f557f644f9`, exit 0, byte-identical to `git rev-parse main`]

**Completion the spec leaves implicit, specified here.** The trio is emitted by the
script *unconditionally* (the script's key set must be a superset of the
fallback's, AC10), yet the script's stated base source — `**base_commit:**` — is
exactly what is unavailable on the F2 *absent-file* path. So the **script** needs
the same last resort: prefer `**base_commit:**`; when the line is absent or
unparseable, fall back to `git merge-base main HEAD` **and set `incomplete: true`**.
This completes the spec's rule rather than contradicting it — the fallback base is
never passed off as the precise one, which is the property the spec's rule exists
to protect.

**Parse by keyword, never positionally.** Re-verified on purpose-built commits
rather than carried from the spec, and **two shapes go beyond what the spec
records**:

| # | Diff | `git diff --shortstat` output |
|---|---|---|
| 1 | deletions only | ` 1 file changed, 3 deletions(-)` — **insertions clause absent** |
| 2 | one insertion only | ` 1 file changed, 1 insertion(+)` — **deletions clause absent too**, and `insertion` is singular |
| 3 | no changes | *empty output*, exit 0 |
| 4 | pure rename | ` 1 file changed, 0 insertions(+), 0 deletions(-)` |
| 5 | both | ` 1 file changed, 2 insertions(+), 1 deletion(-)` — **`deletion` singularises as well** |

[measured: a throwaway `git init` repo in the scratchpad; the five commands and their verbatim outputs are the rows above — shape 1 captured with `cat -A` as ` 1 file changed, 3 deletions(-)$` to prove no trailing insertions clause]

Two corrections to the spec's Technical constraint 7, both from shape 2 and shape
5: the **deletions** clause can be absent as well (the spec names only the
insertions-absent case), and **`deletion` singularises too** (the spec names only
`file changed` and `insertion`). The parser must therefore match
`([0-9]+) files? changed`, `([0-9]+) insertions?\(\+\)`, `([0-9]+) deletions?\(-\)`
independently, each defaulting to `0` when its clause is absent — which also makes
shape 3 (empty output) fall out correctly as `0/0/0` with no special case.

**Substring, never whole-cell equality.** Live status tokens include
`✅ Fixed (design amended)`, `✅ Fixed (spec amended)`, `⚠️ Objected: <reason>`
and now `⬜ Open 🔁 Re-opened` — two carry a parenthesised suffix, one a trailing
free-text reason, one an appended marker.
[measured: `Read .claude/skills/task/SKILL.md` l.211 enumerates the four Step-11 dispositions]

**Section bounding.** All Self-Review parsing is bounded to text between a
`^## Self-Review \(Round [0-9]+\)$` heading and the next `^## ` heading. This
keeps `## AC Status` rows and any future `## Comment cycle round M` table out of
`findings` / `objections`.

### `incomplete: true` triggers (exhaustive)

| Condition | Effect |
|---|---|
| Progress file absent or unreadable | all optional fields omitted; the diff-size trio still emitted, off the `git merge-base main HEAD` base |
| `**Issue:**` line absent, **or present in the `URL` form rather than `#N`** | `issue: null` — the field is `fallback-required`, so it is always emitted; `null` is its defined "present but unparseable" value, not an omission |
| `**base_commit:**` absent or unparseable | trio computed off `git merge-base main HEAD` instead — the looser base is flagged, never silently substituted |
| **Both bases unobtainable** — no `main` ref, detached HEAD, shallow clone, or not a git work tree | trio emitted as `0/0/0`. The table was labelled *(exhaustive)* from round 1 without this row, and the gap is **reachable, not theoretical**: a `git init` sandbox has no `main`, which is exactly case 12's environment. `0/0/0` is indistinguishable from a genuine no-change diff, so `incomplete: true` is what carries the difference — the trio is `fallback-required` and must be emitted, so refusing to emit is not an option |
| Zero `## Self-Review (Round N)` sections | `rounds: 0`, `verdicts: []` |
| A section has no `**Verdict:**` line, or a token that is neither `APPROVE` nor `REJECT` | `"UNKNOWN"` pushed into `verdicts` |
| A `^\| [0-9]` row's severity cell is not one of `blocker`/`major`/`minor`/`nit` | that row contributes to no bucket |
| `## Files touched` section absent | `files_touched` omitted |
| The pinned corpus command fails or yields a non-integer | `instruction_corpus_lines` omitted |

Any trigger → `"incomplete": true`, **exit 0**.

### Exit-code contract

| Code | Meaning | Orchestrator action |
|---|---|---|
| `0` | Appended (full **or** degraded) | continue Step 12 |
| `2` | Usage error (no argument) | write the fallback record by hand, continue |
| `3` | `jq` not on `$PATH` | same |
| `4` | `jq` failed to compose a record | same |
| `5` | Could not append to the target | same |

The script never exits non-zero for a parse problem — only for *cannot append at
all*. `set -uo pipefail`, never `set -e`, matching both in-tree scripts.
[measured: `grep -n 'set -' .claude/skills/*/scripts/*.sh` → `set -uo pipefail` in all three]

### Known truncation (must be stated on the schema page, AC13)

`self-review.md` § Rules: *"Maximum 10 findings per round. If more exist, list the
10 most severe."* `findings` therefore under-counts any round that hit the cap.
[measured: `Read .claude/agents/self-review.md` l.170]

---

## Decomposition

M = 8.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Create the log file, empty and tracked. `git add` it so the path exists for the script's first append; the first record lands at this task's own Step 12. Confirm no gitignore rule matches. | `ai-docs/metrics/task-runs.jsonl` | — |
| 2 🔁 | **REOPENED by spec amendments 2 and 3 — see the `2 🔁 REOPENED` note directly under this table for clauses (a)–(e). The `ca59991` commit STANDS and is AMENDED, never reverted.** Original scope: write the schema page: field table (with the `fallback-required` / `fallback-optional` column), `incomplete` trigger table, exit-code table, append-only + single-writer (`/task` Step 12) + hand-edit prohibition and its rationale, the 10-findings-per-round truncation note, last-line-wins for Step-12 re-entry duplicates, the pinned command **verbatim including its `:(exclude)` term** plus why the bare pathspec is wrong — stated as the re-runnable discriminator (`109` vs `0`), **not** as a line-count pair, and a `### Worked fallback example` heading followed immediately by a ```` ```json ```` fenced single-line example. **Also hosts the three blocks moved off `SKILL.md`** per Key decision 6: the sub-step 5a untracked-corpus precondition assertion, the Step-12 verification block (AC1-newline + AC14), and the **fallback recipe** (the nine `fallback-required` fields incl. the diff-size trio off `git merge-base main HEAD`). **AC13a additions:** per-field counting units in words — `findings` / `objections` / `objections_reopened` summed across rounds and inflating with `rounds` because rows carry forward (`self-review.md:177`), `findings_first_seen` counting only rows absent from the immediately preceding round — plus the **`File:line` identity key** and the **upward** bias direction when a finding's location moves. **AC11a addition:** the five verified `--shortstat` shapes and the keyword-parse rule. **No bare `#186`** anywhere on this page (see § Risks R1). | `ai-docs/task-run-schema.md` | — |
| 3 | Write the fixture test (RED — the script does not exist yet): the **fourteen** cases in § Test Design, all fixtures written into a `mktemp -d` sandbox via heredoc, zero tracked-file mutation. **AC9a (spec amendment 2):** F1 gains the **key-drift pair** — `src/g.rs:70` in R1 and `src/g.rs:73` in R2, same file, **same `Finding` text**, shifted line — plus **case 14** asserting the drifted row **is** re-counted in `findings_first_seen`, with the mandatory inline comment. Case 1's `findings` / `findings_first_seen` and case 2's bounded total move with it; the re-derived arithmetic is in § *Test Design* → *F1 section layout*, per counter. **A complete 13-case draft already exists untracked in the working tree** (`.claude/skills/task/scripts/test-append-task-run.sh`, **449 lines / 18,673 bytes** as of round 7, never committed) — **amend it** to 14 cases rather than rewriting from scratch, and update its closing `PASS: all 13 cases green.` banner. Re-`wc` before editing; the figure is dated (§ *Figures that move*). | `.claude/skills/task/scripts/test-append-task-run.sh` | 2 🔁 |
| 4 | Write the extractor. `awk`/`grep` parse → single `jq -n` compose → `printf '%s\n' >> target`. Documented header block in the `check-citations.sh` house style (what it is for, who runs it, what it deliberately does **not** cover). Parse `--shortstat` **by keyword** with each clause defaulting to `0`; prefer `**base_commit:**` for the diff base and fall back to `git merge-base main HEAD` with `incomplete: true`. **`instruction_corpus_lines` uses the pinned command *including* `':(exclude)ai-docs/learnings.md'`** (spec round 12) — the script must not carry the pre-exclusion form, or every record it writes is on the superseded, non-comparable basis. Run `shellcheck` on both scripts; set mode **100755** on both (`git ls-files -s` must show it — the direct-exec invocation of Key decision 4b depends on it). Test must go **all-cases GREEN** — assert `passed == C` with `C` derived from § *Cases* (§ *Figure dependencies*, rung 1); do **not** transcribe a count here. Case 14 is the AC9a key-drift assertion. | `.claude/skills/task/scripts/append-task-run.sh` | 3 |
| 5 | Add `🔁 Re-opened` to `self-review.md` at **three** sites, additively (`⬜ Open 🔁 Re-opened`), never as a replacement: (a) § *Findings format* — a vocabulary line under the `Severity levels:` line, **no new example table row**; (b) § 7 *Objection quality* (l.126–130) — the three "→ re-open" bullets state the marker once, by pointing at the write site; (c) § *Rules* round>1 (l.175–176) — the **write** site, changed to `re-open as ⬜ Open 🔁 Re-opened`. Also pin Key decision 5's mechanic: the marker goes on the row in the **new** round's table; earlier sections are never edited (Instruction 8). | `.claude/agents/self-review.md` | — |
| 6 | `/task` Step 12: insert sub-step **5a** (**≤ 620 chars, no fenced blocks** — **three** one-line pointers into `ai-docs/task-run-schema.md`: (i) the untracked-corpus precondition assertion, (ii) the Step-12 verification block, (iii) the **§ Fallback recipe** for the nine `fallback-required` fields; plus the script invocation in the **direct-exec** spelling of Key decision 4b and the exit-code → fallback routing. **No** restated field table, **no** inline command blocks, **no** inline fallback recipe — Key decision 6's relief valve is required, not optional. Budget and pointer count must match KD6's costed table exactly); append `ai-docs/metrics/task-runs.jsonl` to sub-step 7's staging list; add **one** frontmatter `allowed-tools` entry — `Bash(.claude/skills/task/scripts/append-task-run.sh *)`. Do **not** add a `git ls-files` entry (Key decision 4b: redundant with `settings.json`'s `Bash(git *)`). Re-measure `wc -c` against `BEFORE` taken from the base commit and assert `AFTER < 35000` (§ *Figure dependencies*, rung 2). The `≤ 620` / `≈ 737` figures are planning aids, not gate inputs. | `.claude/skills/task/SKILL.md` | 2, 4 |
| 7 | Update the inventory: `self-review` row gains the `🔁 Re-opened` additive-marker contract; add a `/task` Step-12 telemetry note to § *Notes* (single writer, append-only, schema page link). No `#186`. | `ai-docs/claude-tools-hierarchy.md` | 5, 6 |
| 8 | Propagation + verification sweep: AC15 keyword greps (§ *AC15 sweep contract*), AC5 Review sync-group check with its outcome recorded for the PR body, AC4 mechanical re-derivation, AC12 `realpath` link check, AC16 `wc -c` re-run, and `bash .claude/skills/ai-audit/scripts/check-citations.sh` (must stay GREEN — see R1). Fix every hit in this same subtask. | any file the sweep hits | 1–7 |

Scope: 8 ≤ 15. No issue split proposed. **M stays 8** — amendment 2 reopens an
existing subtask and extends two others, and amendment 3 (`AC13b`) adds one more
section to the *same* reopened subtask's file. Neither adds a subtask, so the
§ *Handoff plan* group boundaries, the single-group count, and the `opus`
routing are all unchanged.

### Subtask 2 🔁 REOPENED — what must be ADDED to the committed schema page

**The commit stands.** `ai-docs/task-run-schema.md` landed at `ca59991` and is
correct as far as it goes; it simply predates amended `AC13a`. This is an
**addition, not a reversal** — nothing already on the page is retracted, and the
key statement at `:85–86` stays exactly as written.

Measured basis for "addition, not reversal":
[measured: `git show --stat --oneline ca59991` → `ai-docs/task-run-schema.md | 256 +++`, 1 file changed, 256 insertions — the page is wholly new in that commit]
[measured: `awk 'NR>=85 && NR<=93' ai-docs/task-run-schema.md` → `:85–86` states *"Identity key: the `File:line` cell, verbatim … iff their `File:line` cells are byte-identical"*; `:87–93` states *"Known bias, direction fixed: `findings_first_seen` is biased upward … read `findings_first_seen` as an **upper bound**"*]
[measured: `grep -niE 'common case|almost always|frequen|expected in|edge case|rare' ai-docs/task-run-schema.md` → no output, exit 1 — **no frequency language anywhere on the page**]
[measured: `for s in 'no row matched between rounds' 'does not distinguish' 'ratio below 1' 'ship together'; do grep -c "$s" ai-docs/task-run-schema.md; done` → `0 0 0 0` — signature and coupling clause both wholly absent]

Three clauses to add, keeping `:85–93` intact:

| | Clause | Required content |
|---|---|---|
| **(a)** | **Frequency**, not only direction | Key drift is the **expected** case whenever multiple findings share a file. The reason is structural and must be stated as such: the condition that makes the `findings` / `findings_first_seen` split worth anything — several findings in one file, some fixed, some carried — is *exactly* the condition that moves the lines, so `findings_first_seen` collapses toward `findings` on precisely the runs the split exists to illuminate. State plainly that a **low ratio is ambiguous** — "few repeat findings" **or** "many line-shifting fixes" — and that **the record cannot distinguish the two**. Direction-only wording (`upper bound`, `biased upward`, already on the page at `:87–93`) is **insufficient on its own**: it invites an analyst to read the series as merely conservative when it is, on the informative runs, close to uninformative. The existing direction sentence is kept and joined, not replaced. |
| **(b)** | **Degeneracy signature**, at the settled strength and **no stronger** | On a run with `rounds > 1`, `findings_first_seen == findings` means **no row matched between rounds**; the log **does not distinguish the cause** — genuinely no repeat findings, **or** a `File:line` key drifted by a Step-11 line shift; a ratio **below 1** is evidence the key held for **at least one** row. Do **not** write "almost certainly drift" or any equivalent — see Key decision 8 for why that is false (`self-review.md:173`). |
| **(c)** | **Coupling clause**, binding future edits | `findings_first_seen` and its degeneracy signature **ship together or not at all**. If the signature is ever unsatisfied — sentence absent, or its grep failing — the correct response is to **remove `findings_first_seen`** from the schema, never to keep the field and drop the sentence. The field's presence is conditional on a reader being able to identify the runs where it degenerated. |


**Clause (d) — `AC13b`, a THIRD amendment found on disk during this round.** The
round-6 brief described amendment 2 only; the spec also carries `AC13b`, added
after the brief was written. It is covered here rather than deferred, because an
AC with no decomposition owner and no verification command is a design defect.
[measured: `grep -oE '^\| AC[0-9]+[a-z]? \|' …spec.md | tr -d '| '` → `AC1 AC2 AC3 AC4 AC5 AC6 AC7 AC8 AC9 AC9a AC10 AC11 AC11a AC12 AC13 AC13b AC13a AC14 AC15 AC16` — **20** rows, not the 19 the brief states; `AC13b` sits between `AC13` and `AC13a`]
[measured: `grep -n 'does NOT measure' ai-docs/task-run-schema.md` → no output, exit 1 — the section is wholly absent from the committed page, so this too is an addition]

`AC13b` adds one section to the same page, in the same subtask:

- **Title, verbatim: "What this log does NOT measure".**
- **Framing is the requirement, not a style preference.** Each entry is an **open
  question with its current status**, and **must end in a question that is
  explicitly open** — *not* a caveat. The spec's reasoning: a caveat reads as a
  closed topic (acknowledged → handled → nobody's problem) and decays into
  skimmed boilerplate, while an undecided question stays a live agenda item. An
  implementer who writes these as bullet-point caveats has failed the AC even if
  every fact is right.
- **Entry count: derived, not restated here.** The floor lives in `AC13b` and has
  moved every round it was touched; § *Deriving `AC13b`'s entry floor* computes it
  and the gate asserts against it. **This design deliberately records no integer
  for it.** The entries below are the *topics* the spec currently enumerates —
  read `AC13b` itself for the authoritative list and its count.
- **(i)** *is the second axis worth measuring at all?* — process and handoff
  failures are orthogonal to every field; a run where durable state was not
  maintained is **indistinguishable from a clean run**. **(ii)** *should the
  `## Decisions log` be parsed?* — note **explicitly that this one is not
  near-free**, since the event exists only as prose and would need a parser or new
  instrumentation. **(iii)** *should post-Step-12 rounds be captured?* (needs a
  second writer at `/pr-merged`). **(iv)** *should the 10-findings-per-round
  truncation be corrected or merely flagged?* **(vi)** *should the record count
  **planning** rounds, not just review rounds?* — `rounds` counts `/task` Step 10
  self-review rounds **only**, so `/interview`, `design` and `design-review`
  rounds appear nowhere, and **a flat `rounds` trend after an `/improve`
  escalation is compatible with that escalation having doubled the design phase**.
  The spec calls this the log's **own motivating question**: the issue exists to
  ask whether process density pays for itself, and a record that omits most of the
  process cannot answer it.
- **(v), (vii), (viii) are ONE property with three faces, and the page must
  present them as such** — this is the round-7 restructure, not a cosmetic
  regrouping. **Open with the consequence, not the mechanisms:** *a run that took
  three attempts at the fixture and a run that took one produce identical
  records.* That sentence tells an analyst which question the log cannot resolve;
  the mechanisms only explain how the gap arose. **The property beneath it:** the
  harness **destroys working state at the boundary of the step that produced it,
  and versions only the product.**
  - **State it as a real trade, not an oversight.** The page must say plainly that
    this is **reasonable hygiene for human review** — it keeps diffs clean and
    stops transient state polluting history — with a real benefit; the cost falls
    entirely on the harness's ability to measure *itself*, a use case the design
    predates. An implementer who writes this up as a harness bug has mis-stated it.
  - **First face (v):** the log's **source** is gitignored, so *when* and *by whom*
    a field was written is unanswerable once the session ends — derived telemetry
    from an unauditable source. **The question is reframed and the old phrasing is
    now wrong:** it is **not** *"should the progress file be versioned?"* — that
    would sacrifice the very benefit that motivates destroying it — but ***does the
    harness need a durable surface that is NOT the repository?*** The page must
    also **not propose implementing one**; it is out of scope here.
  - **Second face (vii):** the same outcome reached by a **different mechanism** —
    `.state.md` is matched by **no** ignore rule and is simply deleted outright,
    while the progress file is matched by `.gitignore:11`. That two artefacts
    converge **without sharing a mechanism** is what supports reading this as a
    *property of the harness* rather than one rule applied twice.
  - **Third face (viii):** uncommitted work is invisible — where (v) and (vii)
    concern state **destroyed** after the fact, this concerns state that **never
    entered version control**. The shared consequence: the true working state is
    recoverable only while the session is live, and only by inspecting the working
    tree directly. A Step-12-derived record describes what *landed* and **cannot be
    read as a measure of effort**.
- **The whole section obeys a "state it structurally" rule**, called out at the
  spec's § *Scope-boundary disclosure* and again inside (vi). No entry may be
  pinned to a specific run: no round counts, no artefact line counts, no "first
  observed instance" attached to a named task. Those belong in a PR body, where
  they are allowed to go stale; on the schema page they would decay into a wrong
  fact about a run nobody remembers. The structural claim needs no incident to
  support it — a run whose durable state was never maintained emits a
  normal-looking line, and that is true whether or not it has ever happened.
- **Entry (v) has a further trap the spec calls out explicitly.** The repo records a
  **classification, not a justification**, and the page must not manufacture one:
  the plausible diff-noise argument (the progress file is rewritten at every step
  boundary, so versioning it would put that churn in every PR) is a **conjecture,
  not the recorded reason**, and must not be presented as one.
  [measured: `awk 'NR>=10 && NR<=11' .gitignore` → `# Harness local-only state (see AGENTS.md § Agent Docs)` / `/ai-docs/plans/**/*.progress.md` — a label, no rationale; `awk 'NR==269' AGENTS.md` → `| \`ai-docs/plans/*.progress.md\` | Active task progress / handoff state — local-only (gitignored) |` — likewise a label; `git log -1 --format='%h %ad %s' --date=short 9077bfb` → `9077bfb 2026-07-12 Add SDD harness + self-learning loop (adapted from quartzite)` — a single bulk import, so no per-line rationale exists in this repo's history]
- **(ix) is a DIFFERENT KIND of entry, and the page must say so** (spec round 14).
  Entries (i)–(viii) are **coverage gaps** — the log does not see X. Entry (ix) is
  a field the log **does** measure and **reports inverted**; the spec places it
  nearer in kind to the `findings` / `findings_first_seen` split than to anything
  else in the list. Presenting it as one more gap loses the distinction that makes
  it actionable.
  - **The inversion:** a **rigorous** self-review finding real defects across three
    rounds emits `rounds: 3`, high `findings`, non-zero `objections`; a
    **perfunctory** round-1 APPROVE emits `rounds: 1`, `findings: {}`. On **every**
    field the thorough run reads worse than the careless one.
  - **The condition — this is the usable part, not a hedge.** The inversion holds
    while the **number of defects is roughly fixed and review thoroughness
    varies**. When **input code quality** varies instead, the sign is normal and
    the fields read correctly. State the discriminator plainly: *these fields are
    readable when comparing runs of **comparable review thoroughness**, and
    unreadable when thoroughness itself differs.* Without it an analyst trusts the
    fields everywhere or distrusts them everywhere — **both wrong**.
  - **The prohibition — stated directly, because the misuse is the obvious next
    step.** Do **not** optimise against these fields. Once `rounds` is a target the
    cheapest improvement is to review less thoroughly: Goodhart, and more dangerous
    than usual because the framing reads as virtuous — *"reducing review-cycle
    cost"* is exactly what a well-meaning reader would adopt this log for. **A
    downward `rounds` trend is not self-evidently an improvement and must never be
    adopted as an objective.**
- **(x) is upstream of everything else in the section, and the page must say so**
  (spec round 15). Where (i)–(viii) concern what the record **cannot say** and (ix)
  a field whose **sign is inverted**, (x) asks whether the record is **consulted at
  all** — which is prior to every other question here, because a record that
  accumulates correctly and is never read costs the writing and returns nothing.
  - **The precedent is in-repo, not hypothetical.** `ai-docs/learnings.md` has a
    write trigger that is an AXIOM and a read trigger (`/improve`) that fires only
    on explicit human invocation. `task-runs.jsonl` is being given **the same
    architecture** — mandatory mechanised writes at Step 12, and no consumer at
    all, since § *Out of scope* defers every one of them.
  - **Qualitative on the page, counts nowhere near it.** State that *a comparable
    in-repo log has accumulated an order of magnitude past its escalation
    threshold without ever being read*. **Do NOT migrate the figures** — they live
    in the spec's § *Key decisions* provenance row with their reproducing command.
    This is not a stylistic preference: measured live this round, the underlying
    count **is provably in motion**, which is the durable point and needs no value: the per-record parse run against `HEAD` and against the working tree returns **different** results inside this one session, because `learnings.md` is appended to while the design is being written. Re-runnable discriminator, no figure pinned: `[ "$(parse <(git show HEAD:ai-docs/learnings.md))" != "$(parse ai-docs/learnings.md)" ]` → **true**. A number on the page would have been wrong before the PR opened —
    and entry (x) is the last place in the document that should carry one.
  - **Undecided** whether this log needs a read trigger, a consumer, or a
    threshold of its own.
- **The standing warning is preserved in some form:** *a clean `rounds` trend is
  not evidence the surrounding process was sound, because the log cannot report
  otherwise.*
- **No fields are added for any of this.** `spec_amended_during_impl`,
  `subtasks_reopened`, and any handoff-compliance flag are **out of scope for
  v1**; the criterion is satisfied by prose alone. The record stays at **18
  fields** — § *Record schema (v1)* is unchanged by `AC13b`, and the AC10
  containment assertion (case 11) still derives its key sets from the page's
  field table, so a stray new field would fail case 11 rather than pass quietly.

**Clause (e) — the corpus-exclusion criterion (spec round 12, `AC14`).** The
pinned command on the schema page is **stale as committed**: it lacks the
`:(exclude)` term, and the page carries the superseded `9,403` figure. Subtask 2 🔁
also owes:

- **The pinned command re-pinned verbatim at BOTH of its occurrences**, including
  `':(exclude)ai-docs/learnings.md'`. The page carries the command **twice** —
  § *`instruction_corpus_lines` — the pinned command* (`:173`) and § *Step-12
  verification block* (`:218`). **Re-pinning only the first is the failure mode
  `AC14`(a) now gates on**: every Step-9 check goes green while the shipped
  Step-12 block computes `9707` against the script's `9091`, and this design
  routes that mismatch to *stop-and-diagnose*.
- **Delete the retired figures at `:179–180`** — `9,403` and `38,538`. Replace
  with the re-runnable discriminator (`109` bare / `0` glob) and the
  post-exclusion baseline. Gated by `AC14-neg`.
#### Mandated sentences — the single list

**This list is the one source.** The implementer writes the page from it; § *Prose-content
verification* reads the same list. Wrapping, emphasis, nesting and capitalisation
on the page are all fine — `canon` removes them from **both** sides. What must not
drift is the wording.

```text
append-only
single writer
hand-edit
10 findings
last line
summed across all rounds
immediately preceding round
File:line
upward
no row matched between rounds
cannot distinguish
expected case
What this log does NOT measure
not near-free
conjecture
cannot report otherwise
comparable review thoroughness
not self-evidently an improvement
reports inverted
what will cause this log to be read
consulted at all
upstream of every other
order of magnitude past its escalation threshold
driven by the codebase or by journaling
1%
at the next pinning
does not restore
not comparable
```

**The count is whatever this block measures — do not transcribe it.** Verify with
`awk '/^#### Mandated sentences/{f=1} f&&/^```text$/{g=1;next} g&&/^```$/{exit} g' <design> | grep -c .`;
it read **28** at round 12 (`AC13` 5, `AC13a` 7, `AC13b` prose 11, `AC14`(b)–(e) 5), each segment counted from the block rather than asserted. **Round-12 note against this design's own rule:** the caption first said `27` — a transcribed arithmetic done in the same edit that added the block's do-not-transcribe instruction. The `grep -c` above is the reason the error was caught in the same round rather than shipped.
Adding a mandated clause means adding a line here — not writing a new pattern.

---

## Handoff plan

Grouping per `.claude/agents/design.md` § Rules → handoff-grouping (a)–(h).
M = 8 ≥ 1, so this section is mandatory. Change-type is uniformly
**instructions/harness** (`*.md`, `.claude/**`, `ai-docs/**`) — Key decision 1
establishes that the two `.sh` files under `.claude/skills/task/scripts/` are
instructions/harness by rule (e)'s path enumeration and by `code-writer`'s
charter, so no change-type switch occurs and no boundary is forced. Group count
is therefore **1** — the minimum reachable under (f), and within (h)'s default
max of 4, so no user gate is needed.

- **Handoff into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § *Compaction recovery (re-entry)*.
  Binding for the **first** group too, per `/task` SKILL.md Step 8 bullet 5
  (*"every group fans out through `/context-reset`, including the first group"*).
- **Group A** — model `opus`, effort **inherited from the orchestrator**
  (typically xHigh — **not** pinned), 1M-token window, routed to
  `subagent_type="general-purpose"` with inline `model="opus"` per
  `/context-reset` § *Handoff protocol* step 3 — subtasks **1–8**
  (instructions/harness change-type). **Terminal group** (8 subtasks; within the
  `1..=10` range and under the `≤ 10` size cap). No inter-group handoff exists.

**Group A spawn-prompt contingency (Key decision 2).** Beyond the canonical
minimal prompt, this one group's prompt carries exactly one extra line:

> If the permission system denies any edit under `.claude/**`, STOP immediately.
> Record the denied path in the progress file's `## Decisions log` and return.
> Do not retry and do not work around it.

On such a return the orchestrator completes the remaining `.claude/**` subtasks
**in-thread** (`ai-docs/delegation-rules.md` § *Phase 1* → "Environment fit":
*"Apply those in-thread"* — that file, not AGENTS.md, is the sentence's home), then
resumes at whichever subtask the delegate stopped on. Subtask ordering places the
two dependency-free `ai-docs/**` artefacts (1 — log file, 2 — schema page) ahead
of the first `.claude/**` write (3), so a denial preserves those two and costs at
most the uncommitted part of subtask 3. Subtasks 7 and 8 are **not** protected by
the ordering — 7 depends on 5 and 6, and 8 sweeps whatever the keywords hit — so
on a denial the orchestrator owns 3 through 8, not just 3 through 6.

The `design`, `design-review`, `self-review` and `spec-writer` gates stay on
Opus regardless of this marker.

---

## AC15 sweep contract

Enumerated here so Step 9 verifies the sweep rather than re-deriving it.
All baselines measured on the pre-change tree.

**Keywords swept** (`grep -rni "<kw>" .claude/ AGENTS.md CLAUDE.md ai-docs/ docs/ README.md`):

| # | Keyword | Pre-implementation hits (**dated, NOT a gate**) | Files every post-change hit must lie within (**this is the gate**) |
|---|---|---|---|
| 1 | `Re-opened` | **0** | `.claude/agents/self-review.md`, `ai-docs/claude-tools-hierarchy.md`, `ai-docs/task-run-schema.md`, `.claude/skills/task/scripts/*.sh`, plus this task's spec + design (`ai-docs/plans/**`) |
| 2 | `task-runs` | **0** | `.claude/skills/task/SKILL.md`, `ai-docs/task-run-schema.md`, **`ai-docs/claude-tools-hierarchy.md`**, both scripts, spec + design |
| 3 | `append-task-run` | **0** | `.claude/skills/task/SKILL.md` (body + `allowed-tools`), `ai-docs/task-run-schema.md`, both scripts, `ai-docs/claude-tools-hierarchy.md`, spec + design |
| 4 | `ai-docs/metrics` | **0** | `.claude/skills/task/SKILL.md`, `ai-docs/task-run-schema.md`, **`ai-docs/claude-tools-hierarchy.md`**, `append-task-run.sh`, **`test-append-task-run.sh`**, spec + design |
| 5 | `task-run-schema` | **0** | `.claude/skills/task/SKILL.md`, `ai-docs/claude-tools-hierarchy.md`, `test-append-task-run.sh` (AC10 extraction), **`append-task-run.sh`**, spec + design |

[measured: `for kw in "Re-opened" "task-runs" "append-task-run" "metrics/" ; do grep -rni "$kw" .claude/ AGENTS.md CLAUDE.md ai-docs/ docs/ README.md | grep -v '^ai-docs/plans/' | grep -v '^ai-docs/learnings.md'; done` → `Re-opened`, `task-runs`, `append-task-run` all empty; `metrics/` empty]
[measured: `grep -rni 'task-run-schema\|task_run_schema' . --include='*.md' | grep -v '^./ai-docs/plans/'` → empty]

> **Round-9 correction — the `0`s are a PRE-IMPLEMENTATION snapshot and have all
> expired.** They were measured before subtasks 1–3 existed. Every one is now
> non-zero, because the committed schema page and the untracked fixture draft
> legitimately carry these keywords. **An implementer comparing against `0` would
> conclude the tree was broken when it is correct** — the same shape as the
> Step-12 block defect above.
>
> **The gate is therefore the PROPERTY, not the count:** run each sweep and assert
> **every hit lies inside that keyword's expected-files set** (column 4). That is
> falsifiable — a hit in an unexpected file fails it — and it does not decay as
> implementation proceeds, which a count cannot avoid doing.
> [measured: recorded `0` vs actual, this round — `Re-opened` 7, `task-runs` 7, `append-task-run` 6, `ai-docs/metrics` 5, `task-run-schema` 2; all hits confined to `ai-docs/task-run-schema.md` and `.claude/skills/task/scripts/test-append-task-run.sh`, i.e. inside the expected sets]

**Keywords deliberately EXCLUDED, with the measurement that justifies each** — an
unexplained omission reads as a skipped sweep:

| Excluded keyword | Why | Evidence |
|---|---|---|
| `telemetry` | Collides with the unrelated `gp_render::widgets::Telemetry` design-system component. Sweeping it yields **57** pre-existing hits with zero relation to this task. | [measured: `grep -rni 'telemetry' .claude/ AGENTS.md CLAUDE.md ai-docs/ docs/ README.md \| grep -v '^ai-docs/plans/' \| grep -v '^ai-docs/learnings.md' \| wc -l` → `57`; of these `grep -rni 'telemetry' docs/ \| wc -l` → `46` and `grep -rnic 'telemetry' ai-docs/context-status.md` → `5`, all the widget] |
| `schema_version` | Already in use as an unrelated `.state.md` front-matter key. | [measured: same grep → `.claude/skills/interview/SKILL.md:64: schema_version: 1`] |
| `reopened` (unhyphenated) | One pre-existing hit, in prose, inside the AXIOM-protected `_inbox.jsonl`. It is unrelated ("Confirmed, not reopened") and MUST NOT be hand-edited — AGENTS.md § *Workflow* names `/task` Step 12 and `/triage` as its only writers. | [measured: same grep → `ai-docs/deferred/_inbox.jsonl:27`] |
| `degeneracy` (added round 6) | Collides with pre-existing, unrelated **geometry** prose in `docs/design-review.md` (endpoint-on-line degeneracy in the `supercover` review). Sweeping it yields hits with zero relation to this task. | [measured: `grep -rni 'degeneracy' .claude/ AGENTS.md CLAUDE.md ai-docs/ docs/ README.md \| grep -v '^ai-docs/plans/'` → `docs/design-review.md:89` and `:298`, both geometry] |

**Spec amendment 2 introduces no new sweep keyword.** Its three schema-page
clauses and the AC9a fixture pair are prose and test data, not a rule term other
files must track. The one identifier that could have qualified,
`findings_first_seen`, is confined to this task's own artefacts:
[measured: `grep -rni 'findings_first_seen' .claude/ AGENTS.md CLAUDE.md ai-docs/ docs/ README.md | grep -v '^ai-docs/plans/'` → hits only in `ai-docs/task-run-schema.md` (`:44`, `:81`, `:87`, `:93`, `:155`) and the untracked `.claude/skills/task/scripts/test-append-task-run.sh` — both files this task owns; no third-party consumer exists to propagate to]

> **Round-18 fix (Issue 5) — keywords 2 and 4 would have failed on correct work.** Subtask 7 adds a `/task` Step-12 telemetry note to `ai-docs/claude-tools-hierarchy.md` carrying the schema-page link, so a legitimate hit lands in a file both expected-sets omitted; keywords 3 and 5 already listed it. **Note the shape:** these column-4 sets are **enumerated after the artefact's current contents** — this design's own generator, in a gate that had no domain row until round 18. The row now records that failure mode explicitly.

**Non-negotiable:** the sweep is `-i`. AGENTS.md § *Propagation Rule* Procedure
step 1 — *"`-i` is not optional"*.

---

## AC verification commands

**Two verification points, not one.** Most ACs are re-run at **Step 9** against
the shipped tree and quoted PASS/FAIL. But `ai-docs/metrics/task-runs.jsonl` is
**empty until this task's own Step 12 sub-step 5a appends the first record** —
so any clause that needs a *line* to exist cannot be evaluated at Step 9, and a
gate that structurally cannot fire is not a gate. Those clauses are pinned to
**Step 12, immediately after sub-step 5a** and recorded PASS/FAIL in the PR body
(§ *Step-12 verification block* below).

| AC | Command | Point |
|---|---|---|
| AC1 (parse + tracked) | `jq -c . < ai-docs/metrics/task-runs.jsonl > /dev/null && echo OK` (vacuously true on a 0-byte file — that is the correct result for an empty log, and it also proves the path is readable); `git ls-files --error-unmatch ai-docs/metrics/task-runs.jsonl` | Step 9 |
| AC1 (trailing newline) | `[ "$(tail -c1 ai-docs/metrics/task-runs.jsonl \| xxd -p)" = "0a" ]` — **requires ≥ 1 line**; a 0-byte file yields an empty string and fails | **Step 12, post-5a** |
| AC2 | **Compare positions in the BODY, after stripping YAML frontmatter** — `body(){ awk 'NR==1&&/^---$/{fm=1;next} fm&&/^---$/{fm=0;next} !fm' "$1"; }`, then the first `append-task-run` line in `body` must be **less** than the first `Stage all changed files` line, and an absent body invocation is a FAIL. **Second half, mechanism pinned:** the staging sub-step itself must name the log — `body SKILL.md \| awk '/Stage all changed files/{f=1;print;next} f&&/^[0-9]+\. /{exit} f' \| grep -c 'ai-docs/metrics/task-runs.jsonl'` → `≥ 1`. **Round-13 fix — the range form `/a/,/^[0-9]+\. /` spans exactly ONE line**, because awk tests the terminator against the *start* record and `7. Stage all changed files:` itself matches `^[0-9]+\. `. It passes today only because `SKILL.md:232` happens to be a single unwrapped ~300-char line — and **subtask 6 adds ~62 chars to that very line**, so the first person who wraps it reds a correct tree. [measured: on a wrapped two-line staging sub-step, the range form finds the log path `0` times, the property form `1`] The round-8 form grepped the whole file, so a page naming the path **only** in sub-step 5a passed while the record never reached the PR diff — which is the half of `AC2` that clause exists to protect. **Round-9 fix — the previous form compared raw `grep -n` positions and PASSED on broken input.** This design mandates an `allowed-tools` frontmatter entry containing `append-task-run` (Key decision 4b), which sits at line ~2 and is therefore *always* above the staging line; `head -1` selected that entry, so sub-step 5a's real position was never tested and the gate could not fail | Step 9 |
| AC3 | **Frame: the tree that will be committed at sub-step 7, not committed history.** `git diff --name-only <base>..HEAD -- .claude/agents/ \| wc -l` → `1`, and that path is `self-review.md`. **Counting mechanism pinned** — `--stat \| wc -l` reads `2` (it emits a summary line) while `--name-only \| wc -l` reads `1`; the row previously said only "exactly one path". **Co-requisite:** `git status --porcelain -- .claude/agents/` → **empty**, else uncommitted agent edits are invisible to the diff and the AC passes while the property is violated | Step 9 |
| AC4 | **Property, not census** (spec round 4), **and the diff must retain its file headers.** `git diff -U0 <base>..HEAD -- .claude/ \| grep -E '^(\+\+\+ \|[+-].*⬜ Open)'` → the only `+++ b/…` path preceding any `⬜ Open` hunk is `.claude/agents/self-review.md`. **Round-11 fix:** the previous form filtered on `⬜ Open` alone, which **strips every `+++ b/<path>` header**, leaving "hunks in `self-review.md` only" undecidable from the command's own output. **Frame co-requisite:** `git status --porcelain -- .claude/` → empty. Plus a **diff-based discriminator with no baseline** (round-14 ladder, rung 2): `git diff <base>..HEAD -- ':(glob).claude/**/*.md' \| grep -cE '^\+[[:space:]]*\|.*\|.*⬜ Open'` → **`0`** — *this PR adds no new table row carrying the marker*. **Round-20 fix, found by RUNNING the gate against a real tree rather than by analysing it: `-- .claude/ '*.md'` does not scope, it ORs.** `.claude/` matches everything beneath it *and* `'*.md'` matches every `.md` anywhere, so the gate swept the two shell scripts, `ai-docs/learnings.md` and `ai-docs/claude-tools-hierarchy.md`, and read **11** on a correct tree. `':(glob).claude/**/*.md'` is the pathspec form already pinned in the corpus command, so it is taken from the owner rather than invented. [measured, two-sided with the mutation asserted effective first (planted template row in `.claude/agents/review-findings.md`, committed so the `$B..HEAD` range could see it — file modified `1`, literal present `1`): **passes-on-correct** design form `11` / corrected `0`; **fails-on-broken** design form `12` / corrected `1`; after revert, corrected back to `0`] **Round-15 fix — the previous pattern `^\+\| [0-9]+ \|` was named after the three template rows' CURRENT rendering** and is pass-on-broken outside it. [measured: `+| 3 | … ⬜ Open |` → `1`, but `+| N | … ⬜ Open |` → `0` and column-aligned `+|  1 | … ⬜ Open |` → `0`; the generic form matches all three] **Round-14 fix: the previous form asserted `still exactly 3`, a census of the current tree** — a later commit legitimately adding a template row anywhere under `.claude/**/*.md` makes it fail-on-correct, and it needed re-measuring every round. The added-rows form needs no baseline and states the property `AC4` actually cares about. (`--include='*.md'` stays mandatory on any whole-tree variant — unscoped, this design's own fixture heredocs match and it reads 11, Key decision 5.) The 11-site enumeration is **evidence**, not the test | Step 9 |
| AC5 | Read both Review sync-group files; record the outcome in the PR body. Property: `git diff --name-only <base>..HEAD -- .claude/skills/project-review/SKILL.md .claude/agents/review-findings.md` → **empty**, **and `git status --porcelain -- <same two paths>` → empty**. **The worktree half is not theoretical:** subtask 8's sweep says *fix every hit in this same subtask*, and `review-findings.md` is exactly what an `⬜ Open` sweep touches — an uncommitted edit there leaves the diff clean and this AC green while doing the one thing it forbids. Plus `review-findings.md`'s creation-time template row is still marker-free | Step 9 |
| AC11a | Covered by fixture-test case 12; plus the shape table in § *The diff-size trio* is re-derivable via the five commands recorded there | Step 9 |
| AC13a | **All literal spellings in this row are ILLUSTRATIVE — the executable form is `canon` containment over the § *Mandated sentences* list, never these `grep -n` strings.** Round-15 fix: a Step-9 verifier runs what is written, and the raw form **fails on a correct page**. [measured: the exact literal `AC13b`(vii) spells → raw `grep -c` = `0` against a page carrying `**not self-evidently an` / `improvement**` across a wrap with an emphasis marker inside it; `canon` containment = MATCH] **All prose-content clauses in this row are verified by the ONE mechanism** in § *Prose-content verification*, not by per-row patterns: each mandated clause is a pinned sentence in the single list, and both page and sentence pass through the same `canon`. **Seven separate greps, each of which must independently hit** — an alternation (`a\|b\|c\|…`) passes on any one match, so a missing clause would hide behind a sibling: (i) `grep -n 'summed across all rounds' …` (the `findings` / `objections` / `objections_reopened` unit); (ii) `grep -n 'immediately preceding round' …` (the `findings_first_seen` unit); (iii) `grep -n 'File:line' …` (identity key named); (iv) `grep -n 'upward' …` (bias **direction** stated, not merely that a bias exists); **(v) `grep -n 'no row matched between rounds' …`** — the **degeneracy signature**, the fifth grep the spec's amended `AC13a` mandates by name (*"Verification adds a fifth independent grep"*); a miss fails `AC13a` on its own and triggers the Key-decision-8 removal branch, never a quiet drop of the sentence; **(vi) `grep -n 'cannot distinguish' …`** — the ambiguity statement `AC13a` requires (*"the record cannot distinguish the two"*); **(vii)** the **frequency** statement — normalised grep for `expected case` (the page writes `the **expected** case`, so emphasis markers sit inside the literal). All seven against `ai-docs/task-run-schema.md`; any single miss fails AC13a. **(v) is the spec's mandated floor; (vi) and (vii) are design-added**, because `AC13a` requires the ambiguity and frequency clauses in substance and greps (i)–(v) reach neither — (iv) checks only *direction*, which `AC13a` itself calls "insufficient on its own". Adding them widens no shipped scope; it closes a gate on a requirement that already exists | Step 9 |
| AC13b | **All literal spellings in this row are ILLUSTRATIVE — the executable form is `canon` containment over the § *Mandated sentences* list, never these `grep -n` strings.** Round-15 fix: a Step-9 verifier runs what is written, and the raw form **fails on a correct page**. [measured: the exact literal `AC13b`(vii) spells → raw `grep -c` = `0` against a page carrying `**not self-evidently an` / `improvement**` across a wrap with an emphasis marker inside it; `canon` containment = MATCH] **All prose-content clauses in this row are verified by the ONE mechanism** in § *Prose-content verification*, not by per-row patterns: each mandated clause is a pinned sentence in the single list, and both page and sentence pass through the same `canon`. **Independent checks, each of which must hit on its own — no summarising count in this header, since the list grows with the AC.** All against `ai-docs/task-run-schema.md` unless noted. (i) `grep -n 'What this log does NOT measure'` → the section exists under that exact title. (ii) **entry count — DERIVED from the spec, never hard-coded here.** Run § *Deriving `AC13b`'s entry floor*, then count the **mandated roman markers** inside the `^## `-bounded section: `awk '/^## What this log does NOT measure/{f=1;next} f&&/^## /{exit} f' ai-docs/task-run-schema.md \| grep -oE '\*\*\([ivx]+\)\*\*' \| wc -l` → `≥ $floor`. **Counting the markers, not the bullets, is the fix that ends this row's repair cycle.** The marker is the *same token the floor derivation already counts spec-side*, and it is invariant under wrap, nesting, sub-headings and emphasis — the four conventions that broke the three previous attempts (`?$` in round 8; `^#+ ` + wrap in round 9; **nested sub-bullets** in round 10, a nesting *this design itself mandates* via the (v)/(vii)/(viii) grouping). Use `grep -oE … | wc -l`, **not** `grep -coE`, which counts matching *lines* and would undercount if two markers ever shared one. **The marker class is `[ivx]+`, NOT an enumerated alternation.** Round 11 wrote `(i|ii|…|ix)` and it expired one round later: against a correct **ten**-entry page it reads `9` versus floor `10` and fails valid work. The floor derivation carried the same latent defect one numeral further out — its `(i|…|x)` enumeration would have undercounted an eleven-entry AC. **An enumerated set inside a gate pattern is a transcribed count wearing a regex**, and it decays exactly when the thing it counts grows — the failure this design has spent five rounds removing from everywhere else. [measured: on a ten-entry page — enumerated `9`, generic `10`, floor `10`; on a simulated eleven-entry `AC13b` row — enumerated `10`, generic `11`] [measured: on a representative page carrying wrap + emphasis + nested sub-bullets + a `###` sub-heading simultaneously → `9`; with (vii)/(viii)/(ix) removed → `6`] (iii) `grep -n 'not near-free'` → entry (ii)'s required qualifier. (iv) `grep -n 'conjecture'` → entry (v)'s required disclaimer that the diff-noise rationale is **not** the recorded reason. (v) `grep -n 'cannot report otherwise'` → the standing warning. **(vi)–(viii), entry (ix), added spec round 14 — three separate greps, because the entry's three clauses are independently omissible and an alternation would let two hide behind one:** (vi) `grep -n 'comparable review thoroughness'` → the **condition** that makes the fields readable/unreadable; (vii) `grep -n 'not self-evidently an improvement'` → the **prohibition** against target-setting; (viii) `reports inverted` → the statement that (ix) is a **measured inversion, not a coverage gap**. **Round-12 fix — the literal was `different in kind`, and entry (x) now makes that same claim about something else, so it was an ambiguous anchor:** a page carrying only (x)'s different-in-kind sentence satisfied (ix)'s gate. `reports inverted` occurs in (ix) and not in (x); `consulted at all` / `upstream of every other` occur in (x) and not in (ix). [measured: on the settled spec, splitting the `AC13b` row at the `**(x)**` marker — `different in kind` ix=1 x=1 (**not discriminating**); `reports inverted` ix=1 x=0; `consulted at all` ix=0 x=1; `upstream of every other` ix=0 x=1] **(ix)–(xii), entry (x), added spec round 15 — four pinned sentences plus one negative:** `what will cause this log to be read` (the question), `consulted at all` (its different-in-kind content — the record's *consumption*, upstream of every other entry), `upstream of every other` (its positional claim), and `order of magnitude past its escalation threshold` (the in-repo precedent, stated **qualitatively**). **Negative — the provenance counts must NOT reach the page:** `grep -ci 'unescalated' ai-docs/task-run-schema.md` → **`0`**. **Round-13 fix — the numerals are gone from this gate.** It previously pinned `\b28\b|\b9\.3`: figures the spec cites *today*, on a value this design's own round-12 note recorded moving `16 → 29` mid-session, so the gate would stop matching the thing it guards as soon as the count moved — and `\b28\b` can false-fire on any legitimate 28. `unescalated` is a property of the **class** of figure excluded, not of its current value. The precise figures live in the spec's § *Key decisions* provenance row, and entry (x) is the last place that should carry a rotting number — measured live, the underlying count **is provably in motion**, which is the durable point and needs no value: the per-record parse run against `HEAD` and against the working tree returns **different** results inside this one session, because `learnings.md` is appended to while the design is being written. Re-runnable discriminator, no figure pinned: `[ "$(parse <(git show HEAD:ai-docs/learnings.md))" != "$(parse ai-docs/learnings.md)" ]` → **true** — which is why the page carries the qualitative form only. All three are load-bearing: without (vi) an analyst trusts the fields everywhere or nowhere; without (vii) the obvious next step is to optimise against them. **WHITELIST, not a blacklist — the field table's names must EQUAL the mandated field set.** Extract the table's first-cell names with the **pinned, structure-keyed** helper below and assert **set-equality** against case 11's `SCRIPT` key set. **Round-15 fix — this was PROSE ONLY through round 14** ("extract … and assert"), i.e. form (3), so the standing claim that no form-(3) violation remained was false; and the shape it would have been implemented with, `grep -c '^\| `'`, is keyed on the **backtick convention**: a `SKILL.md` restating the field table **without** backticks scores `0` = PASS while carrying `| handoff_protocol_ok | bool |` — the exact row the whitelist exists to reject.

```bash
# column 1 of a markdown table, keyed on STRUCTURE (header row + | delimiters),
# never on rendering. Strip set is ` * ~ but NOT _ : these are IDENTIFIERS.
mdcol1(){ awk -v h="$2" '
  $0 ~ "^\\|[[:space:]]*"h"[[:space:]]*\\|" {f=1;next}
  f && /^\|[[:space:]]*-+/ {next}
  f && /^\|/ {print; next}
  f {exit}' "$1" \
  | awk -F'|' '{print $2}' | sed 's/[`*~]//g; s/^[[:space:]]*//; s/[[:space:]]*$//' | grep -v '^$'; }
```

[measured on three renderings of the same table — backticked, unbackticked, and column-aligned-with-bold: `mdcol1` returns the identical name set for all three; set-equality PASSes the two correct tables and FAILs the unbackticked one carrying `handoff_protocol_ok`]

**`AC12` uses the same helper** rather than `grep -c '^\| `'`, for the same reason — **made true in round 19**, when `AC12`'s executable clause was still the backtick-keyed `grep` this sentence claimed it had replaced. **Note the strip sets differ from `canon`'s deliberately:** `canon` compares *prose* symmetrically so stripping `_` is safe; `mdcol1` extracts *identifiers*, where `_` is significant — two mechanisms, two domains, both now declared. **Round-13 fix — the previous form blacklisted `spec_amended_during_impl\|subtasks_reopened`, and the spec's out-of-scope set carries an unnamed third class: *"and any handoff-compliance flag"*.** A page adding `handoff_protocol_ok` reads `0` → **PASS while doing the forbidden thing**, and case 11's backstop does not fire because it compares the *script's* keys, which a page-only addition never touches. A blacklist can only enumerate the names someone already thought of; a whitelist derived from the artefact rejects **any** unmandated field. The round's structural rule applied: **no enumerated set that tracks a growing artefact**. Paired with case 11's `SCRIPT` cardinality still equalling the derived field count `F` | Step 9 |
| AC9a | Case 14 of the fixture test (see § *Test Design* → *Cases*). Two clauses, both required: the **over-count asserted as expected**, stated as an **equality so that no literal is transcribed at all** — `.findings_first_seen.minor == .findings.minor` (the drifted row receives no de-duplication). **Round-10 fix: this row previously pinned `== 3`, which contradicted § *F1 layout* and case 14 (both `4`) and, by this design's own case-14 note, `3` is precisely the value that signals a *repaired* key.** An implementer reconciling a red test against a hard-coded number fixes **the fixture**, because a number reads as a specification while a fixture row reads as data — the path there deletes `src/g.rs:15` and silently destroys the only drift scenario the field exists to measure. The equality form cannot diverge from the fixture, and the **mandatory inline comment** at that assertion. **The comment's wording is PINNED here so the gate and the artefact cannot disagree** — a grep for a paraphrase is a gate that silently never fires. The comment must contain both literals verbatim: `expected under the current File:line key` (no backticks — it is a shell comment) and `ai-docs/task-run-schema.md`. Gate: **both literals must appear on the SAME line**, since `ai-docs/task-run-schema.md` alone already reads `2` in the untracked draft and therefore discriminates nothing — `grep -c 'expected under the current File:line key.*ai-docs/task-run-schema.md' …/test-append-task-run.sh` → `≥ 1`, or equivalently scope the grep to the case-14 comment block. `AC9a` states that an assertion **without** the comment fails the AC, so the comment carries its own gate rather than a reviewer's eye | Step 9 |
| AC6–AC11a | `bash .claude/skills/task/scripts/test-append-task-run.sh` → **all cases PASS, exit 0**. **Case count DERIVED, not written here** (rung 1): `C=$(awk '/^### Cases/{f=1} f&&/^\|[[:space:]]*[0-9*]/{n++} f&&/^$/&&n{print n;exit}' <design>)`, then assert `passed == C`. **The row matcher is `^\|[[:space:]]*[0-9*]`, NOT the narrow `^\| [0-9*]` this row carried through round 19** — the narrow form requires *exactly one* space after the leading pipe and therefore misses a column-aligned (`\|  19 \|`) or tight (`\|19\|`) leading cell. A gate whose entire job is to catch design/fixture desync must not itself miscount when a row is merely reformatted: `C` under-reads, the assertion reds on correct work, and the next reader goes and "fixes" the fixture instead of the matcher. [measured: on a scratchpad copy of this design with row 19's leading cell column-aligned, narrow → `18` vs tolerant → `19`; with row 18 additionally written tight, narrow → `17` vs tolerant → `19` — the tolerant form is invariant under both reformattings, and `.claude/skills/task/scripts/test-append-task-run.sh` already spells the tolerant form at its `AC6` block] A transcribed `14/14` fails-on-correct the moment a case is added — the shape of the mechanism test's `24 / 24` (covers AC6, AC7, AC8, AC9 incl. carry-forward, **AC9a** via case 14, AC10, AC11a) | Step 9 |
| AC11 | `git diff --name-only <base>..HEAD -- .claude/skills/pr-merged/scripts/cleanup-progress.sh` → empty, **and `git status --porcelain -- <same path>` → empty** (frame co-requisite). **Trimmed in spec round 4** to the in-PR-verifiable clause: the former "a `/pr-merged` run after an append still deletes the progress file" half is post-merge and `self-review` cannot evaluate it at Step 10 | Step 9 |
| AC12 | **`realpath -e`** on the link target from `.claude/skills/task/` — plain `realpath` **exits 0 on a non-existent path**, so the round-8 form could not detect a broken link; `-e` requires existence. Plus the no-restated-field-table check, **run through `mdcol1`** (§ *Prose-content verification*): `mdcol1 .claude/skills/task/SKILL.md Field \| sort \| comm -12 - <mandated-field-set>` must be **empty** — i.e. `SKILL.md`'s `Field`-headed table, if any, names **none** of the mandated fields. **Round-19 fix — the previous form was `grep -c '^\| `' … must be 0`, keyed on the BACKTICK RENDERING**, which is precisely the defect round 15 declared one row below when it built `mdcol1`; `AC12` was listed as a user of that helper while its executable clause still was not. **The gate passed on the artefact it exists to reject.** [measured on fixtures built to `SKILL.md`'s real conventions, mutation effectiveness asserted first (`grep -c '^\| Field \|'` → correct `0`, both broken `1`): **old gate** — correct PASS, backticked-restatement FAIL, **unbackticked-restatement PASS (the miss)**; **new gate** — correct PASS, backticked FAIL, unbackticked FAIL] | Step 9 |
| AC13 | **All literal spellings in this row are ILLUSTRATIVE — the executable form is `canon` containment over the § *Mandated sentences* list, never these `grep -n` strings.** Round-15 fix: a Step-9 verifier runs what is written, and the raw form **fails on a correct page**. [measured: the exact literal `AC13b`(vii) spells → raw `grep -c` = `0` against a page carrying `**not self-evidently an` / `improvement**` across a wrap with an emphasis marker inside it; `canon` containment = MATCH] **All prose-content clauses in this row are verified by the ONE mechanism** in § *Prose-content verification*, not by per-row patterns: each mandated clause is a pinned sentence in the single list, and both page and sentence pass through the same `canon`. **Five INDEPENDENT greps, each of which must hit on its own** — not the alternation this row carried through round 7. (i) `grep -n 'append-only'`; (ii) `grep -n 'single writer'`; (iii) `grep -n 'hand-edit'`; (iv) `grep -n '10 findings'`; (v) `grep -n 'last line'`. All against `ai-docs/task-run-schema.md`; any single miss fails AC13. **Round-8 correction:** the previous form was `grep -n 'a\|b\|c\|d\|e'`, which passes on **any one** match — the exact defect this design diagnosed for AC13a in round 4 and then failed to apply one row below it. Proven, not assumed: a file containing only the string `append-only` scores `1` against the old alternation with four of five clauses absent | Step 9 |
| AC14-neg | **Negative — the RETIRED figures must be gone from the schema page.** `grep -cE '9,?403|38,?538' ai-docs/task-run-schema.md` → must be **`0`**. They are live on the committed page today at `:179–180` (*"**9,403** lines over 59 files with `:(glob)`, **38,538** without it"*), and no gate caught them through round 9. Both are superseded: `9,403` predates the exclusion criterion and is **not comparable** with `9,091`, and `38,538` was replaced by the re-runnable `109`/`0` discriminator. An alternation is correct here — this is a negative, so any match fails | Step 9 |
| AC14 | **Requires the first record to exist.** Re-run the pinned command — **`:(exclude)ai-docs/learnings.md` term included** — and compare to `tail -1 ai-docs/metrics/task-runs.jsonl \| jq -r '.instruction_corpus_lines'`. **(a) Both pinned-command sites re-pinned — EQUALITY, and deliberately RAW (never `canon`).** The page carries the command **twice**; `grep -c ':(exclude)ai-docs/learnings.md'` must **equal** `grep -c 'git ls-files -z'` (the equality is the assertion; no count is pinned — a page that legitimately gained a third pinned-command site still satisfies it). A presence test (`≥ 1`) passes when only one site is re-pinned, leaving the Step-12 block computing `9707` against `9091`. **`canon` must NOT be used here**: it joins the file to one line, so `grep -c` can only return 0/1 and the half-fixed page reads `1==1` **PASS** — verified, and it is why counting gates are excluded from the mechanism (§ *Prose-content verification* → *What the mechanism does NOT cover*). **(b)–(e) are prose clauses and are verified by the ONE mechanism** — pinned sentences `driven by the codebase or by journaling`, `1%`, `at the next pinning`, `does not restore`, `not comparable`. **(f) ORDER assertion**, section-bounded, `canon`-normalised, with the mandated `:(exclude)` literal stripped first, asserting the criterion precedes the first **prose** mention of the file | **(a)–(f) Step 9; the re-run comparison Step 12, post-5a** |
| AC15 | The five greps in § *AC15 sweep contract* | Step 9 |
| AC16 | `wc -c` over the AXIOM's enumerated set, read **delta-wise** (R4): every file that was < 35,000 before the change is still < 35,000 after. Paired with `git diff --stat <base>..HEAD -- AGENTS.md` → empty, which is what puts `AGENTS.md`'s pre-existing 38,874 out of scope. **Sub-target without a stored baseline** (rung 2): capture `BEFORE=$(git show <base>:.claude/skills/task/SKILL.md | wc -c)` at verification time and assert `AFTER < 35000`. The design's `33,809` / `< 34,900` / `≈ 737` are **planning aids, not gate inputs** — `BEFORE` is re-measured from the base commit, so a change to `SKILL.md` between design and implementation cannot make the gate wrong | Step 9 |
| — | `bash .claude/skills/ai-audit/scripts/check-citations.sh` → PASS (R1) | Step 9 |

### Gate self-audit (round 8) — "would this still fail if the thing it names were broken?"

Round 7 found the `AC4` template-row gate was self-defeating: it counted fixture
files this design's own decomposition creates. That was a *does-the-check-reach-
what-it-names* failure committed inside a design document, so every gate here was
swept with the same question. **Four more found, all now fixed.** The sweep is
recorded rather than summarised, because "I checked them all" is precisely the
claim a reader cannot verify.

| Gate | Defect | Fix | Proof it was real |
|---|---|---|---|
| `AC13` | **Alternation** — `grep -n 'a\|b\|c\|d\|e'` passes on **any one** match, so four missing clauses hide behind one present sibling. This design *diagnosed exactly this* for `AC13a` in round 4 and then left it in force one table row below | Split into **five independent greps** | [measured: a file containing only the string `append-only` scores `1` against the old alternation, with four of five required clauses absent] |
| `AC12` | **Unfalsifiable baseline** — "`grep -c '^\| `' … ` unchanged" recorded no value, so nothing could be compared and the check could never fail | Pinned to **`0`**, which is both the measured value and a real discriminator | [measured: `grep -c '^\| `' .claude/skills/task/SKILL.md` → `0`; a restated field table would emit rows beginning `` \| `schema_version` \| ``] |
| `AC13b` negative | **Unscoped** — "on the page's field table" named no mechanism, and a whole-page grep would false-positive on the three out-of-scope field names, which legitimately appear in the section's prose | `awk` the field table out first, then grep it | The names are *required* to appear in prose by clause (d); an unscoped grep therefore fails on a correct page |
| `AC13b` entry count | **Unscoped** — "within the section" named no mechanism | `awk` bounded on the heading to the next `^#+ ` | — |
| `AC9a` comment | **Paraphrase risk** — the gate grepped for wording the design never pinned, so implementer and gate could diverge and the grep would silently never fire | Comment wording **pinned verbatim** in the gate | — |

#### Round-9 sweep — result stated at its true scope

**Four rule-forms swept**, these being the defect shapes this project can
currently *name*: (1) **alternation** used as a positive check; (2) **unscoped**
grep; (3) **prose-scoped** assertion (*"within the section"*, *"on the table"*);
(4) gate asserting a **baseline it never recorded**. Result: **no violations of
those four forms remain.**

**That sentence is deliberately not "the gate column is clean."** A sweep against
a list of known shapes is structurally incapable of finding a shape not yet on the
list — the four were themselves discovered by applying the primitive test, not by
consulting a taxonomy. **The principle is generative; the checklist is not.** The
gate column should be read as *audited against four named forms*, never as
*audited*.

**Three round-9 defects found by re-deriving figures** (all fixed above):

| Gate | Defect | Proof |
|---|---|---|
| Step-12 verification block | Omitted the `:(exclude)` term the script uses, so a **correct** implementation mismatches and the design calls a mismatch *stop-and-diagnose* | [measured: `9707` vs `9091` — a 616-line guaranteed failure; and the gap is not even constant, `learnings.md` grew 610→616 during this session] |
| `AC15` sweep, all five keywords | Recorded pre-change baseline `0` for each; every one is now non-zero because subtasks 1–3 legitimately introduce them. Comparing against `0` reports a correct tree as broken | [measured: actual `7 / 7 / 6 / 5 / 2`; all hits inside the expected-file sets] |
| `AC14` clauses (b)–(e) | **Prose-scoped** — *"grep for the criterion wording"* names no string | Pinned five literals + an order assertion |

#### Round-9 mutation testing — the primitive test applied directly

Five gates that **pass the four-form sweep** were mutated and executed, chosen for
*obviousness* on the reasoning that the gates least likely to have been tested are
the ones whose correctness feels self-evident. **Two failed the primitive test —
neither is an instance of any of the four named forms.**

| # | Gate | Mutation | Result |
|---|---|---|---|
| M1 | `AC13b` entry count (`grep -c '?$'`) | Ran it against a correct page written in the spec's own entry style | **BROKEN — returned `0` on a page with 3 valid entries.** Entries read `*question?* — answer`, so the `?` is mid-line. Fixed to `grep -cE '^[-*] .*\?'`, then re-tested on four inputs: correct → `3`, caveat-shaped → `0`, section absent → `0`, and a bleed test with questions in a *neighbouring* section → `1`, correctly bounded |
| M2 | `AC2` ordering | Placed sub-step 5a **after** staging and ran the gate | **BROKEN — PASSED on broken input.** `head -1` selected the `allowed-tools` frontmatter entry (line ~2), which this design itself mandates, so 5a's real position was never tested. Fixed by stripping frontmatter first; re-tested → correctly rejects the broken file, accepts the correct one, and FAILs when the body invocation is absent |
| M3 | `AC1` trailing newline | Fed a no-newline file and an empty file | Correct — `78` and empty both FAIL, `0a` PASSes |
| M4 | `AC13b` negative | Put a forbidden field in the table; separately left it in prose only | Correct — `1` for the table violation, `0` for prose-only, confirming the round-8 scoping fix by mutation rather than by argument |
| M5 | `AC12` no-restated-table | Restated a field table in a copy of `SKILL.md` — **backticked** | **Reported correct, and was mutation in the wrong axis.** The restatement was backticked, which is the only spelling the then-current gate could see, so an effective-looking mutation confirmed nothing. **Re-run in round 19 with an unbackticked restatement: the old gate PASSED it.** Fixed by moving `AC12` onto `mdcol1`; the re-run now FAILs both spellings |

**The two new shapes, named so the next sweep can look for them:**

- **Formatting-assumption gate** (M1) — the pattern tests a *surface accident* of
  the artefact rather than the property named. Fails on correct input; a
  false-negative that would be "fixed" by weakening the gate.
- **Ambiguous-anchor gate** (M2) — the pattern has **more than one legitimate
  match site** and the wrong one satisfies it. Passes on broken input. **This
  subsumes the round-7 `AC4` defect**, where the extra sites were fixture *files*;
  here they were *lines* in one file. Both are the same shape at different
  granularity, which is why neither was caught by looking for the other.

Even so: **five gates mutated, two broken, three correct** — that is the finding,
not a clean bill of health for the twenty-odd gates not mutated.

#### Round-10 — `design-review` ITERATE, and the method error behind it

**The method error first, because it invalidates how round 9 reported.** The M1
repair was re-tested against a **synthetic** correct page: no hard wrapping, no
`###` sub-headings, no emphasis inside sentences. The committed
`ai-docs/task-run-schema.md` has all three. Against a **representative**
nine-entry section written in the page's real conventions, the round-9 gate reads
**3** — worse than the reviewer's measured 6, because my own (v)/(vii)/(viii)
grouping renders as `###` and `awk … /^#+ /{exit}` truncates the section there.

> **A purpose-built correct input must be representative of the artefact's
> conventions — wrapping, emphasis, sub-headings — not merely satisfy the property
> abstractly. A synthetic cleaner than reality is a positive control that proves
> nothing.** Round 9 reported "passes-on-correct" for a gate that does not.

**The third shape, named by the reviewer** — *emphasis-crossing / source-divergent
pinned literal*: a gate whose literal is copied from source A while the
implementer is instructed from source B, or from a sentence with emphasis markers
inside it. It is M1-family (fails on correct input) and was caught by **neither**
the four forms **nor** the M1/M2 pair, because pattern and artefact are *each
individually correct* — only their pairing is wrong.

**Round 10 found a fourth trigger of the same family that the reviewer did not
name: hard wrapping splits a pinned literal across lines.** Discovered while
testing the `AC14` literals *this design added in round 9*: `driven by the
codebase or by journaling` scores **0** raw on a page that fully satisfies
`AC14`, because the page wraps it mid-phrase.
[measured: on a purpose-built `AC14`-satisfying page — raw `grep -c` for `driven by the codebase or by journaling` → `0`, `does not restore` → `0`, `not comparable` → `0`; all three → `1` after normalisation]

**Round 10 introduced `norm()` to fix this class blanket-wise. It was broken five ways and made five gates worse — two of them previously verified.** All five reproduced independently before acting on the finding:
[measured: (a) called without `$2`, `$0 ~ ""` matches every line so `{f=1;next}` always fires → returns **0 chars**, every literal silently reads 0. (b) it is section-bounded, but `AC13`'s five literals live in **three** sections — `10 findings` only in *Known truncation* (`:153`), `last line` only in *Step-12 re-entry* (`:159`). (c) `{f=1;next}` skips the heading, and the only in-section lowercase `append-only` **is** the heading (`:9`) → `AC13`(i) reads 0 on a satisfying page. (d) stripping `_` → `findings_first_seen` becomes `findingsfirstseen`. (e) `tr '\n' ' '` joins to one line so `grep -c` returns only 0/1 — a 3-occurrence file reads `1`, which makes `AC14`(a)'s equality read `1==1` **PASS** on the half-fixed page the raw form correctly FAILs]

**That is the lesson, not the bug list: a shared helper adopted to fix one class rewrites the semantics of every gate that takes it, and can silently un-fix verified work without touching those rows.** The replacement below is therefore scoped to *containment only*, and the gates it must not touch are named explicitly.

**The five majors, each re-tested on BOTH inputs** (representative-correct and
broken):

| # | Gate | Fix | passes-on-correct | fails-on-broken |
|---|---|---|---|---|
| 1 | `AC13b` entry count | bound `^## `, join bullet continuation lines, count `?` | **9** on the representative page (floor 9) | caveat-shaped → `0`; 6-entry → `6`; section absent → `0`; questions in a neighbouring section → `0` (bounded) |
| 2 | `AC14`(a) `:(exclude)` | **equality** of the two counts, not presence | both sites re-pinned → `2 == 2` PASS | one site only → `1 != 2` FAIL (the old `≥ 1` **passed** this) |
| 3 | `AC14` order | section-bounded + normalised + `:(exclude)` literal stripped | criterion-leads page → PASS | filename-as-rule page → FAIL; criterion absent → FAIL (the old form **failed the correct page**, binding to the AC13 rationale at `:21`) |
| 4 | `AC9a` value | **equality**, no literal transcribed | — | a repaired key makes the two sides differ |
| 5 | pinned literals | one normalisation | 3 of 3 previously-`0` literals → `1` | wording drift still fails |

**Issue 4 generalised, per the owner addendum.** *Where a gate pins a value a
fixture also determines, the fixture is the source of truth: the gate derives from
it or asserts an equality. Where a fixture row is load-bearing for a property, the
row itself carries the prohibition against removing it.* The reasoning is about
**where a person looks under pressure** — a hard-coded number reads as a
specification and a fixture row reads as data, so a red test sends the implementer
to edit the fixture. `AC9a`'s `== 3` would have routed them to delete
`src/g.rs:15`, yielding a green suite with the drift scenario destroyed. See
§ *Two distinct fixture comments*.

**Also fixed this round:** `AC15`-adjacent — no gate caught the retired `9,403` /
`38,538` still live on the committed page at `:179–180`; added as `AC14-neg`.
[measured: `grep -cE '9,?403|38,?538' ai-docs/task-run-schema.md` → `2` today, i.e. the new negative gate fires on the current tree and will only go green once subtask 2 🔁 lands]

#### Round-11 — restructure, and two shapes that patching cannot reach

**The record that ended the patch cycle.** The `AC13b` entry count was repaired
**three times and broken three times** — `?$` (round 8), `^#+ ` + hard wrap
(round 9), **nested sub-bullets** (round 10). The nesting is mandated by this
design's own § *Subtask 2 🔁*. Each repair was validated against an input that did
not reproduce the *next* convention the page legitimately uses. **A fourth
per-pattern patch was the wrong response**; the answer is § *Prose-content
verification* — one list, one transform applied to **both** sides, exact
containment. A pattern can drift from the artefact; a shared transform cannot.

**Shape 4 — shared-helper semantic drift.** A helper adopted to fix one class
**rewrites the semantics of every gate that takes it**, and can silently un-fix
verified work *without touching those rows*. Round 10's `norm()` is the case:
introduced to fix literal-matching, it broke five gates, two of them previously
verified by mutation. The tell is that the damage is invisible at the diff — the
gate rows were not edited. **Mitigation, now in the design:** the replacement is
scoped to containment only, and § *What the mechanism does NOT cover* names every
gate that must stay raw, with the measured reason.

**Shape 5 — reference-frame mismatch.** The gate measures a **different object**
than the property constrains, with a pattern that is *itself correct*. `AC3`,
`AC4`, `AC5` and `AC11` all read `git diff <base>..HEAD` — committed history —
while the property is about the tree that will be committed at Step 12 sub-step 7.
Uncommitted edits to `review-findings.md` or `cleanup-progress.sh` leave every one
of them green. **Reachable, not theoretical:** subtask 8's sweep says *fix every
hit in this same subtask*, and `review-findings.md` is exactly what an `⬜ Open`
sweep touches — precisely what `AC5` forbids. All four now carry a
`git status --porcelain -- <paths>` → empty co-requisite, and **every gate that
picks a frame now states it**.

**Four "checked and sound" gates that were not** — round 10 listed them as sound
without executing them; all four verified broken this round:

| Gate | Defect | Measured |
|---|---|---|
| `AC12` `realpath` | exits **0** on a missing file **when the parent directory exists** — exactly what a broken relative link looks like. (With the *parent* also missing it exits 1, which is why a careless probe reads "fine".) | plain → `exit 0`; `realpath -e` → `exit 1`; existing file → `exit 0` |
| `AC3` "exactly one path" | named no counting mechanism | `--stat \| wc -l` → `2` (summary line); `--name-only \| wc -l` → `1` — the latter pinned |
| `AC4` diff half | filtering on `⬜ Open` **strips the `+++ b/<path>` headers**, making "hunks in `self-review.md` only" undecidable from its own output | a `+++ b/x.md` header does not survive the content filter |
| `AC2` second half | grepped the whole file, so naming the path only in 5a passes while the record misses the PR diff | scoped to the staging sub-step |

**`AC10` case 11** — `comm` requires sorted input; `REQUIRED` is field-table order,
`jq -r 'keys[]'` is sorted. It does not merely misreport: it warns on stderr *and*
emits spurious rows, so the containment assertion reads garbage.
[measured: unsorted `comm -23` → two spurious rows + two warnings; `sort`ed → empty, the correct result]

**Method note carried forward.** Verifying a *negative control* is as necessary as
verifying a positive one. This round's first mutation harness removed nothing —
`re.escape` escapes the space in a multi-word phrase and the follow-up `\s+`
substitution corrupted the pattern — so 19 clauses reported "not caught" against a
gate that was working. **A broken variant that is not actually broken proves as
little as a synthetic correct input.** Mutations are now asserted effective before
their results are believed.

#### Round-12 — the ambiguous anchor caught prospectively, and one found in my own regex

**Entry (x) reproduced the ambiguous-anchor shape, and this time it was caught
before shipping.** (ix) already required a `different in kind` literal; (x) makes
the same claim about something else. A page carrying **only** (x)'s sentence
satisfied (ix)'s gate. Both literals are now discriminating — `reports inverted`
occurs in (ix) and not (x); `consulted at all` and `upstream of every other` in
(x) and not (ix).
[measured: splitting the settled `AC13b` row at the `**(x)**` marker — `different in kind` ix=1 x=1 (not discriminating); `reports inverted` ix=1 x=0; `consulted at all` ix=0 x=1; `upstream of every other` ix=0 x=1]

**Shape 6 — an enumerated set inside a gate pattern is a transcribed count wearing
a regex.** Round 11 ended the `AC13b` repair cycle by counting mandated markers
instead of bullets — correct, but the marker class was written as
`(i|ii|…|ix)`. It **expired one round later**: against a correct ten-entry page it
reads `9` against floor `10` and **fails valid work**. The floor derivation — the
mechanism that has absorbed 6→8→9→10 without an edit and is this design's
standing success story — carried the same defect one numeral further out, with a
latent expiry at `(xi)`. Both are now `[ivx]+`.
[measured: ten-entry page → enumerated `9`, generic `10`, floor `10`; simulated eleven-entry row → enumerated `10`, generic `11`, and the fixed derivation reads `word=eleven num=11 enum=11`]

**The lesson is narrower than "avoid enumerations" and worth stating exactly:** an
enumeration is safe where the set is *closed by definition* and unsafe where it
*tracks a growing artefact*. This one tracked `AC13b`'s entry list, which grows by
design — so it was a decaying figure in every sense the § *Figures that move*
table already forbids, and it survived five rounds of that table because it did not
look like a number.

**Two counts re-derived rather than carried** (spec round 15 moved them):

- The § *Mandated sentences* caption. The list now measures **28** — and the
  caption first said `27`, a transcribed arithmetic performed **in the same edit
  that added the block's do-not-transcribe instruction**. The `grep -c` recipe
  beside it is why that was caught in-round rather than shipped.
- § *Deriving `AC13b`'s entry floor*'s `[measured:]` tag, now `floor=10`. It is
  refreshed every round precisely *because* nothing gates on it: a stale tag
  cannot break a gate, only mislead a reader.

**Method adopted from `spec-writer`: count records with a per-record parse, and
sanity-bound the result before believing it.** Applied here to the `learnings.md`
provenance figures rather than accepting them: my parse pairs each `### ` entry
with its own `**Kind:**` / `**Escalated?**` values and asserts *records parsed ==
`### ` count* before reporting.
[measured: 109 records parsed against 109 `### ` headings (bound holds); working tree → 29 unescalated corrections / 3 validations; `HEAD` → 16 / 0. The spec records 28, measured mid-session; `learnings.md` gained 16 entries while this design was being written, so the difference is drift, not disagreement — and it is the strongest available argument for keeping the figure off the schema page]

#### Round-13 — closing the generator

**The convergence problem, named.** The defect rate had not fallen across five
rounds because every repair fixed an **instance** while the **generator** stayed
live: *a mechanism whose correctness domain is named after the artefact's current
contents rather than after a property of its input.* All four majors this round
were predicted by that one test, and § *Mechanism domains* is the structural
answer — **every mechanism now declares its domain and failure mode, including
the ten that work**, because the floor derivation proved that a mechanism can be
correct and still be undeclared, and an undeclared domain is one nobody checks
the edge of.

| # | Instance | Generator form | Fix |
|---|---|---|---|
| 1 | `AC2` range awk | domain assumed from **today's unwrapped line 232** | property-form awk; measured `0` vs `1` on a wrapped staging sub-step — and **subtask 6 adds ~62 chars to that very line** |
| 2 | `canon` strip set | named after the **five markup forms observed so far** | named by markdown's inline grammar; the old set is correct only over backtick/asterisk forms, the new over every form the grammar admits |
| 3a | field-table negative | blacklist of the **two forbidden names written down** | whitelist: set-equality against the mandated fields; the spec's *"any handoff-compliance flag"* is an unnameable third class |
| 3b | provenance negative | pinned **the figures the spec cites today** | generic `unescalated`; the underlying count is provably in motion (HEAD vs worktree differ mid-session) |
| 4 | `1 %` vs `1%` | list wording diverged from the **owner document** | list reconciled to the spec's spelling; ownership rule stated |
| 5 | test evidence `24/24` | transcribed a count that the list had outgrown | `N` derived from the list; recipe recorded instead of the number |

**Adversarial testing, built fresh rather than reused.** The reviewer's page found
`canon` broken on `_underscore emphasis_`, which mine did not carry — a reused
fixture tests the conventions you already thought of. This round's page applies
**eight** markdown forms to the *same* pinned phrase, so each form is a separate
mutation of one control.
[measured: the page applies `` `code` ``, `*em*`, `**strong**`, `_em_`, `__strong__`, `~~strike~~`, `[text](url)` and a three-line hard wrap to one pinned phrase. **Result recorded as a property, not a ratio:** the round-11 set matches only the backtick/asterisk forms and misses every other; the grammar-based set matches **all** of them. The forms are the durable list — regenerate the page to re-derive, rather than trusting a fraction that changes the moment a form is added]

**Structural change 3 audited against its own rule — the refresh trap.**
*"Re-derived, never transcribed"* produces a **new set of numbers**, and a bare
updated value closes the generator while re-opening it in new clothes. The
precedent is this design's own: **round 9 fixed the Step-12 `:(exclude)` omission
and, one paragraph below its own fix, wrote `38,538` — a figure measured *without*
the exclusion — into a sentence verifying the post-exclusion command.** The repair
minted the defect it was repairing. Every figure touched this round was therefore
put through a ladder rather than refreshed:

| Figure | Rung applied | Result |
|---|---|---|
| Mechanism-test pass / caught counts | **1 — derived** | No integer recorded. `N` derives from the sentence block; the tag records the **assertions** `pass == N`, `fail == 0`, `caught == N`, `harness_failures == 0` and that all four held |
| `1%` spelling | **1 — derived** | Recorded as an **equality between two documents** (`grep -oE '1 ?%' <spec> \| sort -u` vs the list entry). The occurrence tally is deliberately absent |
| `learnings.md` provenance | **2 — discriminator** | No value pinned. The durable claim is that the count **is in motion**, shown by `HEAD` vs worktree parses differing within one session |
| `canon` markup coverage | **2 — property, ratio demoted** | The forms are the durable list (`_em_`, `__strong__`, `~~strike~~`, `[text](url)`); coverage is derivable by comparing the strip set to markdown's grammar with no fixture at all |
| `AC2` wrapped-line probe | **2 — discriminator** | `0` vs `1` from a re-runnable probe, not a stored measurement |
| Floor derivation output | **3 — snapshot with command** | Neither derivable-away nor replaceable: it *is* the derivation's output. Recorded with the block that reproduces it immediately above, and refreshed each round because nothing gates on it |

**The one that would have re-armed it** was the mechanism test: `24 / 24` was
transcribed one edit behind a 28-sentence list, and refreshing it to `28 / 28`
satisfies the letter of change 3 exactly while leaving a figure that is stale at
the next sentence added, with nothing beside it to detect that. Rung 1 was
available, so no count is recorded at all.

**Round-14 — the gate-referenced figure sweep, and a self-correction.**
Round 13's report claimed `16 → 29` was "removed from all three sites"; one
occurrence survives and is **correct to keep** — it is narrative explaining a past
defect, not a live claim. But the claim as phrased did not reproduce, because the
count was taken **before** deciding that narrative does not count as a site. The
fix is procedural and now applied above: **define the membership test, then
count** — never the reverse. § *Figure dependencies* states its test first for
exactly this reason.

The sweep's scope is the **intersection** of {figures in the document} × {figures
a gate or AC references} — not every figure, and not selected by age. Dating a
figure does not stop rot; it only makes rot detectable by someone who re-runs the
command, and nobody re-runs what is marked as checked. **Four members, all reached
rung 1 or 2, so no residual decay site remains inside a gate.**

**Round-15 — the generator moved up a level, and the fix was already written.**
§ *Mechanism domains* carried a rule with teeth (*"a mechanism absent from it is
not approved for use"*) while stating **no membership test** and deriving its rows
from nothing — structurally the round-13 field-name **blacklist** with its polarity
flipped: a list of things already thought of, closure still absent. The correct
form existed forty lines below, in § *Figure dependencies*, which opens with a
membership test. **Round 13's own lesson — define the membership test, then count
— was written about the figure sweep and never swept back over the table that same
round created.** The un-swept-rule shape, at the highest level it has appeared:
not in a gate, but in the section that certifies gates.

The section now states its membership test **before** the table and **derives** the
row set from the gate rows, so a gate without a row is a detectable condition. Four
mechanisms in active use had no row; all four are now declared, and three were
demonstrably broken:

| Instance | Was | Measured |
|---|---|---|
| Raw literal `grep` in `AC13`/`AC13a`/`AC13b` rows | rows said "verified by the ONE mechanism" **and then spelled raw `grep -n`** | the exact `AC13b`(vii) literal → raw `0`, `canon` MATCH — round 10's defect, now also crossing an emphasis marker |
| Field-table whitelist | **prose only**, so form (3) was never actually cleared | `grep -c '^\| `'` on an unbackticked restated table → `0` = PASS while carrying `handoff_protocol_ok` |
| `AC4` added-row discriminator | named after the three rows' **current rendering** | `+\| N \|` → `0`, column-aligned → `0` |
| `AC9a` same-line grep | undeclared | now declared |

**The domain declarations were themselves unsound**, which is the sharper half:
`canon`'s box claimed *"any inline markup markdown's grammar can insert"* on the
evidence of an **eight-form fixture**. Five forms miss — reference links, inline
HTML, backslash escapes, footnote refs, entities — and the committed schema page
**already carries `\*`, `\<`, `\>`, `\|`**. The declaration also asserted `canon`
*"does not strip `_`"* while the implementation has stripped it since round 13.
Runtime impact nil (the transform is symmetric), but a future round told to
consult the declaration would have been misled — **a false declaration is worse
than none, because it is trusted**.

**On the naming tell.** All 16 previously-declared domains cleared it, and issue 5
is a remedy that clears it and is still wrong. That is the tell working as
specified: it filters instance-attachment, it does not certify correctness. Worth
recording so no future round treats a clear tell as a pass.

**Residual basis — one proposition, accepted without proof.** The membership
test's *"whose output a verdict reads"* is decided by a human mapping each
candidate command to a row or dismissing it in writing — no regex decides it. **The reconciliation does NOT only over-approximate**, which round 16 claimed: it **under-approximates whenever a new mechanism shares an already-mapped unit**. Round 18 narrowed the unit from `head` to `(head, arg1)`, which removes the observed instances but not the property — a mechanism sharing both head and first argument is still invisible. So the falsifier fires **outside** the declared mechanism, by human inspection, not by running it. **Its falsifier fired inside this
design:** the corpus-set pathspec, the mechanism with the longest failure history
here, had no row until round 16 (Issue 3) — which is what makes the basis
checkable rather than decorative.

> **Instance of a method-level fact — owner:
> <https://github.com/maratik123/graphite-gp/issues/188#issuecomment-5144792521>.**
> The proposition, its falsifier and the price to discharge it are stated in full
> there; this is a pointer, not a second copy. Recorded here only as an instance,
> because this design moves to `ai-docs/plans/done/` at Step 12 and stops being
> read — a method-level fact held only here is lost by construction, the same
> ephemerality `AC13b` (v)/(vii) describe for `.progress.md` and `.state.md`, on a
> slower clock.

**What a GO means here.** Not "no defects found this pass" — the pass found five.
It means the generator is closed: domains are declared rather than inferred, no
enumerated set tracks a growing artefact, and recorded evidence is re-derived
rather than transcribed. A future defect of this class should now be catchable by
reading § *Mechanism domains* and asking whether the declaration still holds,
without re-running six rounds of mutation testing to rediscover it.

**Checked and sound, no change:** `AC1` (both halves — the parse check is vacuous
on an empty file, which is why the newline half is pinned to Step 12), `AC2` (the
literal `Stage all changed files` exists at `SKILL.md:232`, so the line-number
comparison has something to bind to), `AC3`, `AC5`, `AC11`, `AC15`, `AC16`, and
`AC13a`'s seven greps.
[measured: `grep -n 'Stage all changed files' .claude/skills/task/SKILL.md` → `232:7. Stage all changed files: …` — the AC2 gate's anchor is real, not assumed]

**Note on `AC13a` greps (iii) and (iv).** `File:line` and `upward` already match
the committed page, so they pass *before* subtask 2 🔁 runs. That is correct, not a
false pass: both clauses are genuinely already satisfied (`:85–86`, `:87–93`), and
the greps that discriminate the new work are (v), (vi) and (vii), which fail today.

### Mechanism domains — every mechanism declares where it stops

**The generator this design spent six rounds regenerating, stated once:** *a
mechanism whose correctness domain is named after the **artefact's current
contents** rather than after a **property of its input**.* Every gate defect from
round 8 onward is an instance. `canon`'s strip set was named after the five
markup forms then observed; the marker class after the nine entries then present;
the field-table negative after the two forbidden names then written down. Each was
correct when written and expired when the artefact grew.

**The rule that closes it:** a mechanism is declared with the **property of its
input** it is correct over, and the **failure mode** outside that property.
Verification then checks the *declaration* instead of re-deriving the domain every
round.

**Membership test, stated before the table** — round 15. Through round 14 this
section carried the rule *"a mechanism absent from it is not approved for use"*
with **no membership test and no derivation**, so it held exactly the mechanisms
someone thought to list and an omission was undetectable by any command.
Structurally that is the field-name **blacklist** removed in round 13 with its
polarity flipped: a list of things already thought of, closure still absent. The
correct fix was already written one section below — § *Figure dependencies* opens
with a membership test — and was never swept back over the table the same round
created. **That is the un-swept-rule shape at the highest level it has appeared,
and it is the reason this section, not any individual gate, was the defect.**

> **A mechanism is any command **in this design** whose output a gate or AC's
> verdict reads.**

**No section list, and that is the point.** Round 15 swept § *Figure
dependencies*' membership test back over this table but **added a restriction the
model does not have** — it named three sections. **The sweep-back was performed
with a narrower instrument than the thing it was sweeping from**, which is the
same shape a fourth time. The narrowed form takes the enumeration from **3 objects
to 0**: the universe is *this design* and the quantifier is *some gate or AC's
verdict*, so the enumeration disappears rather than reappearing under a new noun.

**The section list was not merely inelegant — it could not generate this table's
own rows.**
[measured: the three named sections begin at `911`/`963`/`1781`, but `--shortstat` keyword parse is defined at `:558`, the two-source base rule at `:543`, the *Verdict vocabulary* at `:600`, and the `SKILL.md` budget at `:338` — all outside every named section, yet all are declared rows]

**`canon` is available to REVIEWERS, not only to gates.** Any ad-hoc `grep` over this document's or the schema page's prose has the same wrap/emphasis blindness the gates use `canon` to avoid — a fixed-string search for a phrase carrying `**emphasis**` inside it misses on correct text. **Observed three times in this session**, most recently on the membership test itself. Pipe through `canon` before concluding a phrase is absent.

**The quantifier bottoms out in the AC set, which the settled spec closes** — the
same basis form the *Verdict vocabulary* and *Severity vocabulary* rows already
rest on, and § *Figure dependencies* has used the unrestricted form since round 14
without a section list.

**The row set is DERIVED, and the unit is `(head, arg1)` — not the head.**
Round 16 made the derivation reach the whole document; **round 18 fixes its
granularity, which is the axis that was actually binding.** A unit of `$1` alone
means a new mechanism sharing an existing head produces **zero** new candidates:

> ```bash
> # whole document, fenced blocks INCLUDED, no tool whitelist, unit = (head, arg1)
> pair(){ { grep -oE '`[^`]+`' "$1" | sed 's/`//g'
>           awk '/^```/{f=!f;next} f' "$1"; } \
>   | tr '|;' '\n\n' | sed -E 's/^[[:space:]]*//; s/^[$(]+//' \
>   | awk '{h=$1; a=$2; if (h ~ /^[a-z][a-z0-9_.-]*$/) print h, a}' | sort -u; }
> ```

[measured: `(head, arg1)` yields **331** units against **204** heads, and separates `git ls-files` / `git merge-base` / `git diff` / `git status` / `git show` and `grep -rni` / `grep -c` / `grep -cE` / `grep -nE` / `grep -coE` / `grep -ci`, all of which collapse to two candidates under a head-only unit. **Mutation, asserted effective first:** appending a gate running `` `git blame -L 40,60 file | grep -c telemetry` `` produces **0** new head-only candidates and **1** new `(head, arg1)` unit (`git blame`)]

**The live instance this granularity hid:** `AC15`'s recursive keyword sweep
(`grep -rni "<kw>"` over a fixed path set, gated on the per-keyword expected-files
column) is a mechanism with **no row**, invisible because `grep` already mapped to
the *Raw literal `grep`* row — whose domain is *"one word, no markup, no wrap"*, an
entirely different mechanism. It now has its own row.

**Dismissal is mechanical, so "or dismiss it in writing" has an artefact.** Round
16 stated that instruction and produced **no dispositions**, which meant nothing
distinguished *examined and dismissed* from *never looked at*. A unit is retained
iff its head is an executable or a function this design defines; everything else —
English words inside backticks — is dismissed **by rule, not by hand**:

> ```bash
> DEFINED='^(canon|mdcol1|parse|body|recon|pair|sec)$'
> pair <design> | while read -r h a; do
>   if printf '%s' "$h" | grep -qE "$DEFINED" || command -v "$h" >/dev/null 2>&1
>   then echo "MAP  $h $a"     # → must name a row below
>   else echo "DISMISS $h $a  # not an executable or a defined function"
>   fi
> done
> ```

[measured: 331 units → **142** retained, over the heads `awk basename bash body canon cargo case cat chmod comm date echo file find gh git grep head jq ls mdcol1 mktemp printf realpath rg sed shellcheck sort stat tail touch tr wc xargs xxd`; the remainder dismissed by rule. **The four round-18 gaps and the `AC15` sweep all survive the filter** — `realpath -e` (1), `bash .claude/…` (3), `wc -c` (2), `jq -c` (1), `grep -rni` (3) — so they were reachable and simply never mapped. That is an **unfinished run**, not a reach failure]

**Why the round-16 restrictions were dropped — corrected per axis.** The round-16
tag concluded that dropping the section range and fenced-block blindness *"reaches
every one"* of eleven tools. **That does not reproduce, and it was the sole
evidence for a causal story that is false:**
[measured: dropping **only** the tool whitelist, while keeping the section range and backtick-only scan, already surfaces `sort xxd head tr xargs canon mdcol1 awk git` — 9 of the 11. The genuine delta from dropping the section range **and** fenced blocks is **`printf` and `find` only**. The binding restriction was the **whitelist**, axis (i). **Neither axis explains the corpus-set miss** of round 16's Issue 3: the corpus-set pathspec is `git`-headed, so it was hidden by *granularity*, not by reach — which is why round 18 changes the unit and round 16's stated cause is withdrawn]

Run it and reconcile: an unmapped command is either a missing row or a gate that
should be using an existing mechanism. **The four found by round 15's run are now
rows** — raw literal `grep`, markdown-row identification, the `AC4` added-row
discriminator, and `AC9a`'s same-line grep.

**The floor derivation's defect was never that it failed** — it absorbed
6→8→9→10 without an edit. It was that its domain was **undeclared**, so nobody
asked where it ended, and it had a silent expiry at `(xi)`.

| Mechanism | Domain — the property it is correct over | Failure mode outside it |
|---|---|---|
| `canon` | any inline markup **markdown's grammar** can insert inside a phrase; hard wrap; letter case | a **spacing** or **wording** difference — by design |
| Marker class `[ivx]+` | any roman numeral **to `xxxix`** | `xl`+ — unreachable for an AC entry list; declared, not assumed |
| Floor word map | number-words present in the map | an unmapped word yields `-1`; the message now says *extend the map* rather than blaming the spec |
| `--shortstat` keyword parse | **git's output grammar** — each clause matched by keyword, absent ⇒ `0` | a git output-format change; not a content change |
| Section bounding `^## ` | **markdown heading grammar** at one level | a section promoted/demoted to another level |
| Case-11 key sets | derived from the artefacts themselves (`jq keys`, field table) | requires **sorted** input to `comm` |
| Field-table whitelist | **set-equality** against the mandated field set | none by growth — an unmandated field is rejected whatever its name |
| Provenance negative (`unescalated`) | the **class** term, not any figure | a page that discusses the class without the word |
| Two-source base rule | `**base_commit:**`, else `git merge-base`; flagged `incomplete` | neither base obtainable ⇒ `0/0/0` + `incomplete` |
| Reference-frame co-requisites | committed diff **and** worktree status together | neither alone; that pairing is the mechanism |
| `AC14-neg` | retired figures absent | a *new* retired figure not yet listed — re-derive when one is retired |
| Status-token substring match | tokens carry suffixes/reasons, so **substring not equality** | a token that ceases to contain its base form |
| Verdict vocabulary | closed by `self-review`'s contract: `APPROVE`/`REJECT`, else `UNKNOWN` | a contract change — which `AC3` forbids |
| Severity vocabulary | closed by `self-review`'s contract: `blocker`/`major`/`minor`/`nit` | an unknown severity contributes to no bucket, deliberately |
| `AC2` body/staging order | **awk property form**, not a range; frontmatter stripped | a staging sub-step that never names the log |
| Mandated-sentence list | the single wording source; both sides through `canon` | a wording divergence between spec and list — see the ownership rule below |
| **Raw literal `grep`** (round 15) | a literal that is **one word** and carries no markup and no wrap | **fails-on-correct** on any multi-word literal — the page wraps and emphasises. Prose clauses must use `canon`, never this |
| **Markdown-row identification** (round 15) | a table's **structure**: header row, then `\|`-delimited columns | naming a **rendering convention** (backticks, digit-only first cell, single-space padding) fails-on-correct or passes-on-broken as soon as the table is styled differently. Used by `AC12`, the field-table whitelist, case 11's `REQUIRED`, and `F` |
| **`AC4` added-row discriminator** (round 15) | an **added diff line** that is a table row carrying the marker | naming the current three rows' rendering (`\| <digit> \|`) misses `\| N \|` and column-aligned padding |
| **`AC9a` same-line comment grep** (round 15) | two pinned literals **on one line**, whose wording is fixed by this design | a paraphrase, or the literals split across lines |
| **Corpus-set pathspec** (round 16) — `git ls-files -z … ':(glob)…' ':(exclude)ai-docs/learnings.md' \| xargs -0 cat \| wc -l`; what `AC14`'s verdict re-runs | **git's pathspec grammar**: `:(glob)` restores pathname semantics so `ai-docs/*.md` is **depth-1 only**; `:(exclude)` removes a path from the set; `git ls-files` reads the **INDEX**, not the worktree | **Three recorded failures, all previously in prose only.** (a) Bare pathspec — `*` crosses `/`, pulling the whole `ai-docs/plans/**` archive in (**+109 files**). (b) Missing `:(exclude)` — a **616-line mismatch on a correct implementation**, which this design routes to *stop-and-diagnose*. (c) Index-vs-worktree — an unstaged corpus file is counted by `cat` but absent from `git ls-files`, handled by sub-step 5a's precondition assertion |
| **`AC1` trailing-newline probe** (round 16) — `tail -c1 … \| xxd -p` | a **byte-exact** comparison against `0a` | a **0-byte file** yields an empty string, not `0a` — so the check is meaningless until the log has ≥ 1 line, which is why it is pinned to Step 12 rather than Step 9 |
| **`AC4` reference frame** (round 20) — the added-row discriminator reads `<base>..HEAD` | a **committed range**, valid **while the working tree is clean**: on a clean tree `<base>..HEAD` and `<base>` agree | a **dirty tree** — the gate reads *history* while the property is about *what will be committed*, so a working-tree template row is invisible. `AC3` / `AC5` / `AC11` already carry a `git status --porcelain` co-requisite; `AC4` did not. **This is not hypothetical: it has already happened in this task on Step-9 re-entry after edits**, and it produced a false negative during round 20's own mutation test — a planted row could not appear in a committed range, and the resulting `0` looked like a passing gate |
| **`AC15` recursive keyword sweep** (round 18) — `grep -rni "<kw>"` over a fixed path set | a **keyword** swept case-insensitively across a **fixed path list**, gated on the per-keyword expected-files column | the expected-files column is **enumerated after the artefact's current contents** — a legitimate new hit fails the gate on correct work (Issue 5 below). Distinct from *Raw literal `grep`*, whose domain is a single unwrapped word |
| **`realpath -e`** (round 18) — `AC12`'s link check; the verdict reads its **exit status** | resolves a path **and requires it to exist** | plain `realpath` **exits 0 on a missing file whose parent exists** — exactly what a broken relative link looks like — so the `-e` flag is load-bearing, not stylistic |
| **`bash <script>`** (round 18) — the fixture test and `check-citations.sh`; verdict reads **exit status + stdout** | a script that reports **all** failures and accumulates, rather than aborting on first | a first-failure abort would mask later ones; neither script does, which is why the `-D warnings` masking rule does not apply to them |
| **`wc -c` over the AXIOM file set** (round 18) — `AC16` | a **byte** count compared **delta-wise**: every file under the cap before is under it after | comparing against a stored baseline instead of a re-measured `BEFORE` (§ *Figure dependencies*, rung 2) |
| **`jq -c . < file`** (round 18) — `AC1`'s parse half | JSON-Lines validity of **every** line | **vacuously true on a 0-byte file** — correct for an empty log, but it proves only readability, which is why the newline probe is a separate row pinned to Step 12 |

#### Figure dependencies are part of a mechanism's domain

A gate whose correctness rests on a figure has that figure **in its domain**;
declaring the mechanism while leaving the figure undeclared leaves the domain
resting on something nobody checks. The four gate-referenced figures are therefore
declared here alongside the mechanisms.

**Membership test, stated before counting** (the round-13 report got this order
wrong and produced a claim that did not reproduce): a figure is a member iff
**(a)** it appears as a literal in this design **and (b)** some gate or AC's
verdict depends on that value being current — i.e. a legitimate change to the
artefact would make the gate *fail-on-correct* or *pass-on-broken*. **Excluded by
the test:** narrative about a past defect (asserts nothing about the present);
prose rationale no gate reads; and any figure a gate **re-derives at run time**,
which is rung 1 and therefore no longer a literal at all.

| Figure | Gate | Rung applied | Now |
|---|---|---|---|
| Fixture case count (`14/14`) | `AC6`–`AC11a` | **1 — derived** | `C` derived from § *Cases*; assert `passed == C`. No count written |
| Record field count (`18`) | `AC10` case 11, `AC13b` negative | **1 — derived** | `F` derived from § *Record schema (v1)*; assert `\|SCRIPT\| == F` |
| Template-row census (`3`) | `AC4` | **2 — discriminator** | *This PR adds no new template row* — a diff-based probe needing no baseline |
| `SKILL.md` size budget (`33,809` / `< 34,900`) | `AC16`, KD6 | **2 — discriminator** | `BEFORE` re-measured from the base commit; assert `AFTER < 35000`. The design figures are planning aids, explicitly not gate inputs |

**Members where no rung above 3 was available: none.** All four reached rung 1 or
2, so this design currently has **no residual decay site inside a gate**. That is
a stronger claim than round 13's and it is the one the reviewer should test.

**Non-members, and why** — recorded so the boundary is auditable rather than
asserted: `9,091` / 59 files and the `109`/`0` discriminator (no gate compares
against them; `AC14` compares record-vs-re-run and `AC14-neg`'s mention is
rationale prose); `185` (already an invariant, `42 ≤ LOCAL_MAX`); `449` / `18,673`
(subtask 3 re-`wc`s before editing); `0` in `AC13b`-negative / `AC14-neg`
(**absence properties** — a zero asserts *nothing is there*, and unlike a positive
count it cannot go stale as the artefact grows. **`AC12` was listed here until
round 19 and did not belong:** its zero was conditioned on the *backtick
rendering*, so it asserted "nothing is there **in one spelling**" — an absence a
restatement in any other spelling silently satisfies. A rendering-conditioned
absence is not an absence property. `AC12` now runs through `mdcol1` and is not a
literal comparison at all); `1` in `AC3` (part of the AC's
definition — "sole changed agent file" — not a measurement of the tree); the F1
fixture arithmetic (authored to match the fixture, and case 14 already asserts it
as an **equality** rather than a literal).
[measured **document-wide** (round 16 — the round-14 evidence used a section-restricted `awk`, which is the same narrower-instrument error Issue 1 fixed one section above; the members table was always built document-wide, so no member was missed, but the non-member evidence is now gathered on the same universe as its own test): none of `9,091` / `185` / `449` / `18,673` appears as a **comparison target** anywhere in the design — every occurrence is either a `[measured:]` record or narrative. `grep -nE '(→|==|must (be|equal)) *\**`?(9,091|185|449|18,673)'` → no match]

**Ownership rule for wording, so the next divergence resolves by rule.** The
**§ *Mandated sentences* list is the single source for what the page must say**,
and it is **reconciled to the spec's own spelling at pin time**. Where the two
differ, the spec wins and the list is corrected — never the reverse, and never by
widening `canon`.
[**derived, not counted** — the durable statement is an equality between two documents: `spec_spelling=$(grep -oE '1 ?%' <spec> | sort -u)` must equal the § *Mandated sentences* entry, and it does (one distinct spelling, `1%`). The occurrence tally is deliberately not recorded — it decays with every spec edit and the gate does not depend on it. The list previously said `1 %` and would have missed a page written from the spec's own row; widening `canon` to absorb the space was rejected as re-naming the domain after an observed accident, which is the generator]

### Prose-content verification — ONE mechanism, not twenty patterns

**Why this replaces per-gate patterns.** The `AC13b` entry count was repaired
three times and broken three times (`?$` → `^#+ `/wrap → nested sub-bullets), each
repair validated against an input that did not reproduce the *next* convention the
page legitimately uses. A pattern can drift from the artefact; **a shared
transform cannot**. Twenty hand-rolled patterns for `AC13`, `AC13a`,
`AC13b`'s literals and `AC14`(b)–(e) collapse to one mechanism.

**Three parts:**

1. **Subtask 2 🔁 pins each mandated clause as an exact sentence** — § *Mandated
   sentences*, one list, one place. The implementer writes the page from that
   list; the gate reads the same list. Neither transcribes.
2. **One canonical normalisation, applied to BOTH sides.** This is the load-bearing
   property: the page's conventions cannot make the two diverge, because both go
   through the same transform.
3. **Exact containment**, via shell `case` globbing — no regex, so no
   metacharacter hazards in the pinned sentences.

> **DOMAIN — `canon` tolerates exactly the forms its strip set implements:**
> `` `code` ``, `*em*`, `**strong**`, `_em_`, `__strong__`, `~~strike~~`,
> `[text](url)` inline links, plus **hard wrapping** and **letter case**.
>
> **FAILURE MODE — it does NOT tolerate**, and these are not hypothetical:
> **reference links** `[text][ref]`, **inline HTML** `<em>…</em>`, **backslash
> escapes** `\*`, `\<`, `\|`, **footnote refs** `[^1]`, and **HTML entities**
> `&nbsp;`. It also does not tolerate a **spacing** or **wording** difference —
> those two are failures *by design*; the list above is a **boundary**, so a
> mandated sentence must not be written using those forms.
>
> **Round-15 correction to the declaration itself.** Through round 14 this box
> claimed tolerance for *"any inline markup markdown's grammar can insert"* — a
> claim about a **grammar**, supported by a fixture exercising **eight forms**.
> An eight-form page supports a claim about eight forms. The claim is now narrowed
> to the strip set actually implemented, and the excluded forms are enumerated.
> **This matters in practice, not only in principle: the committed schema page
> already carries `\*`, `\<`, `\>` and `\|` at `:42–43`.**
> [measured: `[text][ref]`, `<em>expected</em>`, `\*expected\*`, `expected[^1]`, `&nbsp;` — all five yield `0` under `canon` against the pinned phrase; the eight implemented forms all yield a match. `awk 'NR>=42 && NR<=43' ai-docs/task-run-schema.md | grep -o '\\[<>*|]' | sort -u` → `\*  \<  \>  \|`]

```bash
canon(){ tr '\n' ' ' \
  | sed -E 's/\[([^]]*)\]\([^)]*\)/\1/g' \
  | sed 's/[`*_~]//g' \
  | tr '[:upper:]' '[:lower:]' | tr -s ' ' | sed 's/^ *//; s/ *$//'; }

page=$(canon < ai-docs/task-run-schema.md); rc=0
while IFS= read -r want; do
  [ -z "$want" ] && continue
  w=$(printf '%s' "$want" | canon)
  case "$page" in *"$w"*) ;; *) echo "MISS: $want"; rc=1;; esac
done < <sentence-list>
exit $rc
```

**The strip set is named by markdown's inline grammar, not by the conventions observed so far.** Round 11 stripped `` ` `` and `*` — the five conventions seen at the time — which is the generator this design is closing: a domain named after the artefact's *current contents*. `_` and `__` are emphasis in the same grammar, `~~` is strike, and `[text](url)` hides a phrase behind a link. [**property first, ratio second.** The round-11 set strips `` ` `` and `*`, so it is correct over exactly the forms built from those two characters and **fails on every other form markdown's inline grammar admits** — `_em_`, `__strong__`, `~~strike~~`, `[text](url)`. That is derivable by comparing the strip set against the grammar, with no fixture at all. Snapshot for confirmation, with its command: generate a page applying every such form to one pinned phrase and `canon`-compare — the round-11 set matches only the backtick/asterisk forms, the grammar-based set matches all. Regenerate rather than trusting a ratio]

**Each exclusion in `canon` is also a `norm()` defect it must not repeat:** not
section-bounded (the clauses live in three sections); does not skip headings (the
only lowercase `append-only` is a heading); **does** strip `_` — round 13 added it deliberately when the strip set was renamed after markdown's emphasis grammar, and it is **safe because the transform is symmetric**: both the page and the pinned sentence pass through it, so `findings_first_seen` and `findingsfirstseen` compare equal. (The round-11 text claiming `canon` *"does not strip `_`"* was **false from round 13 onward** — runtime impact nil, but a future round told to consult the declaration would have been misled. Identifier extraction, where `_` **is** significant, uses `mdcol1`, not `canon`.) It folds case (the page writes `Single writer`, the list
`single writer` — a **fifth** convention, found only by running the mechanism, not
named by any of the four triggers).

#### What the mechanism does NOT cover — counting gates stay RAW

`canon` joins the file to a single line, so `grep -c` under it can only return
`0` or `1`. **Every gate that counts occurrences must therefore run on raw
lines**, and these are named so no future round feeds them through it:

| Gate | Why raw |
|---|---|
| `AC14`(a) `:(exclude)` equality | canon reads `1==1` and **passes the half-fixed page** that raw correctly fails |
| `AC13b` entry-marker count | counts markers; canon would collapse them to one line |
| `AC13b` field-table negative | canon would read `0` on a violating table, defeating the round-8 fix |

[measured: a 3-occurrence file reads `3` raw and `1` after canon; `AC14`(a) on a one-site-repinned page → raw `1==2` FAIL, canon `1==1` PASS]

#### Mechanism test — as a restructure, against all conventions AT ONCE

Not per-gate and not sequentially: one representative page carrying **hard wrap +
emphasis + nested sub-bullets + a `###` sub-heading + sentence-initial capitals**
together, then one broken variant per mandated clause.

- **Passes-on-correct: all pinned sentences found.** The count is **derived from the list**, never recorded here — `N=$(awk '/^#### Mandated sentences/{f=1} f&&/^```text$/{g=1;next} g&&/^```$/{exit} g' <design> | grep -c .)`, then assert `pass == N`. **Round-13 fix: this line read `24 / 24` while the list stood at 28** — a transcribed figure one edit behind, sitting immediately below a caption forbidding exactly that.
- **Fails-on-broken: one variant per pinned sentence, all caught** — assert
  `caught == N` with the same derived `N`.
- **Negative-control integrity was verified first**, and this mattered: the initial
  mutation harness silently removed nothing (`re.escape('single writer')` escapes
  the space, and the follow-up `\s+` substitution then corrupted the pattern), so
  19 clauses reported "not caught" when the gate was fine. **A broken-variant that
  is not actually broken proves as little as a synthetic correct input** — the
  round-10 lesson, in its negative form. Every mutation is now asserted to have
  removed the clause before its result is believed.

[measured — recorded as **assertions, not values**: with `N` derived from the § *Mandated sentences* block, the run asserted `pass == N`, `fail == 0`, `caught == N`, `harness_failures == 0`, and **all four held**. **No integer is recorded, by design** — refreshing `24 / 24` to `28 / 28` would satisfy the letter of *re-derived, never transcribed* while re-arming the identical trap. Re-run: derive `N`, assert the four equalities]

### Deriving `AC13b`'s entry floor (used by the `AC13b` gate, clause (ii))

**Why this exists.** Round 6 wrote `≥ 6` into the gate beside a prose hedge saying
the count was moving. The hedge was correct and the gate was not: the spec closed
at **eight**, so an implementation writing six or seven entries would have passed
the gate and failed the AC. A number copied out of the spec decays independently
of it; the fix is to stop copying. **Any spec-side count this design needs is
derived at run time, never transcribed** — see § *Figures that move* for the other
instances of this class.

Two independent sources inside `AC13b`, cross-checked so a spec-internal
disagreement surfaces instead of being silently resolved in our favour:

```bash
SPEC=ai-docs/plans/2026-07-31-task-run-telemetry.spec.md
word=$(grep -oE 'At minimum [a-z]+' "$SPEC" | awk '{print $3}')
num=$(printf '%s\n' "$word" | awk '{w["four"]=4;w["five"]=5;w["six"]=6;w["seven"]=7;
      w["eight"]=8;w["nine"]=9;w["ten"]=10;print (($0) in w)?w[$0]:-1}')
enum=$(grep -E '^\| AC13b \|' "$SPEC" \
       | grep -oE '\*\*\([ivx]+\)\*\*' | sort -u | wc -l)
[ "$num" -eq "$enum" ] && floor=$num || { echo "MISMATCH: stated '$word' ($num) vs $enum enumerated — fix the spec, OR extend the word map below if '$word' is simply unmapped ($num = -1)"; exit 1; }
```

- `num` — the **stated** minimum, as an English word after *"At minimum"*.
- `enum` — the **enumerated** entries actually written, counted from the distinct
  `**(roman)**` markers inside the `AC13b` row.
- **Disagreement is a spec defect, not a tie to break.** `num < enum` means the
  prose lags the list; `num > enum` means the list is unfinished. Either way,
  surface it to the orchestrator rather than picking one — a design that silently
  chose the lower number would reintroduce exactly the round-6 failure.
- `-1` from the mapping means a word outside the table (e.g. "eleven"); extend the
  `awk` map rather than falling back to a literal.

[measured: re-run on the **settled** spec → `word=ten num=10 enum=10`, `AGREE -> floor=10`. **This tag is refreshed every round precisely because nothing gates on it** — the floor derives at run time, so a stale tag cannot break a gate, only mislead a reader. It has now tracked `8 → 9 → 10`; each of those three drifts would have silently broken a transcribed number]

### Step-12 verification block

Run immediately after sub-step 5a returns, before sub-step 7 stages. Both results
go into the PR body under **Test plan**, as PASS/FAIL with the observed values —
they are the only evidence either clause was ever evaluated, since the log is
empty at every earlier point in this task's life.

**These commands live on `ai-docs/task-run-schema.md`, not in `SKILL.md`**
(Key decision 6 — required relief valve); sub-step 5a carries a one-line pointer.
They are reproduced here for the design's own readability:

```bash
tail -c1 ai-docs/metrics/task-runs.jsonl | xxd -p                      # AC1  -> 0a
tail -1  ai-docs/metrics/task-runs.jsonl | jq -r '.instruction_corpus_lines'
git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
  ':(exclude)ai-docs/learnings.md' \
  | xargs -0 cat | wc -l                                               # AC14 -> must equal the above
```

> **Round-9 fix — this block was broken, and would have failed a CORRECT
> implementation.** Through round 8 it omitted the `:(exclude)` term that spec
> round 12 added to the pinned command. The script writes the **post-exclusion**
> number; the block re-ran the **pre-exclusion** one, so the two could never
> match, and the design calls a mismatch a *stop-and-diagnose* — meaning a correct
> run would have halted Step 12 chasing a defect that was in the checker.
> **The two commands must be character-identical**; that is the whole point of the
> comparison. Proven, not inferred:
> [measured: block form without `:(exclude)` → `9707`; script form with it → `9091`. A guaranteed 616-line mismatch on a correct implementation. Note the gap is **not** a constant: `ai-docs/learnings.md` grew from 610 to 616 lines during this design session alone, so the pre-exclusion figure drifts with journaling activity — which is precisely the distortion the exclusion criterion exists to remove, observed live]

A mismatch on the AC14 pair is a **stop-and-diagnose**, not a re-measure-and-record:
it means a corpus-set file changed between the script's measurement and this
check, which is precisely what Key decision 4's precondition assertion and
sub-step ordering exist to prevent. **Diagnose the checker first** — round 9's
defect presented exactly as a "corpus-set file changed" mismatch would.

### AC4 site evidence

**Time-anchored in round 7, matching the spec's own reframing.** The spec now
labels its census *"a pre-implementation baseline, not a live invariant"*, because
the marker renders `⬜ Open 🔁 Re-opened` — which **contains** `⬜ Open` — so the
raw counts move once subtask 5 lands. They have in fact **already** moved, for a
different reason: subtask 3's fixture heredocs carry the token, taking the tree
from 14 occurrences / 6 files to **22 / 7**. Treat every figure in this section as
dated evidence for the additive *design*, never as a quantity to re-verify.
[measured: `grep -rn '⬜ Open' .claude/ | wc -l` → `22`; `grep -rln '⬜ Open' .claude/ | wc -l` → `7`; the 7th file is the untracked `.claude/skills/task/scripts/test-append-task-run.sh` with 8 occurrences]

**Status change in spec round 4: this table is EVIDENCE, not the acceptance test.**
AC4 is now a property — *in `git diff <base>..HEAD` over `.claude/`, no
pre-existing `⬜ Open` consumer site is modified* — and the property is what Step 9
checks. The 11-site census below documents *why* the additive form is the right
design; it is deliberately not the gate, so a later commit that adds a twelfth
consumer somewhere cannot retroactively break this AC. That decoupling is the
point of the restatement.

Retained because it is still the clearest statement of what the additive form
buys, and because one row of it is a genuine trap for anyone who *does* try to
verify site-by-site: line numbers inside `self-review.md` shift, and "match by
text" is **wrong for one site**, since subtask 5(c) deliberately rewrites that very
text.

| Site | Match key |
|---|---|
| 8 of 11 (`task/SKILL.md` ×2, `task/reference.md`, `project-review/SKILL.md` ×3, `bugfix/SKILL.md` ×2) | full line text, **unchanged** — these files are not edited for the marker at all |
| `self-review.md` § *Findings format* l.164 (`For REJECT: at least one … row with ⬜ Open status`) | full line text, **unchanged** — the additive form is exactly what keeps this working: `⬜ Open 🔁 Re-opened` still contains `⬜ Open`, so a round of only re-opened findings can still produce REJECT |
| `self-review.md` round>1 l.177 (`Focus on remaining ⬜ Open items…`) | full line text, **unchanged** |
| `self-review.md` round>1 l.175 — **the write site, text deliberately changed** | the **containing bullet's opening**: `` `major` / `blocker`: valid only if the reason is specific `` (stable — subtask 5(c) edits only the bullet's trailing `→ re-open as …` clause). Matching this site by its full text would fail by construction. |

**The actual AC4 test** is the property:
`git diff -U0 <base>..HEAD -- .claude/ | grep -E '^[+-].*⬜ Open'` → hunks in
`.claude/agents/self-review.md` **only**. Subtask 6 does edit `task/SKILL.md`, but
in Step 12, nowhere near its two `⬜ Open` lines (196, 211) — so the property holds
without any site census being run.

---

## Risks

- **R1 — `check-citations.sh` goes RED on a bare `#186`.** The guard treats any
  bare `#N` above the local PR high-water mark as an unresolvable citation, and
  it scans `.claude/**`, `AGENTS.md`, and `ai-docs/**` (excluding only
  `learnings.md`, `bugfix/`, `plans/`, `deferred/`). Every new file in this task
  except the design doc sits inside that scan surface. **Mitigation:** write
  `issue 186` or `maratik123/graphite-gp#186` (the char before `#` is
  alphanumeric, so the regex does not fire) — never a bare `#186` — outside
  `ai-docs/plans/`; run the guard in subtask 8.
  — `[measured: gh pr list --state all --limit 1 --json number --jq '.[0].number' → 185; bash .claude/skills/ai-audit/scripts/check-citations.sh → "PASS: every citation resolves for its reader." (GREEN on the clean tree); grep line in the guard: grep -rnoE '(^|[^a-zA-Z0-9/_-])#[0-9]+\b' .claude/ AGENTS.md ai-docs/]`
- **R2 — fixture `#N` in the test script trips the same guard.** The test's
  well-formed fixture carries an `**Issue:**` line. **Mitigation:** use `#42`;
  the guard's `[ "$n" -le "$LOCAL_MAX" ] && continue` fires and the line is
  skipped. **Stated as an invariant, not a comparison against a snapshot:**
  `LOCAL_MAX` is the local PR high-water mark, which only ever increases, so
  `42 ≤ LOCAL_MAX` holds for every future value — no re-measure is owed when new
  PRs land. (It measured `185` this round; the mitigation does not depend on
  that.) No runtime string assembly needed, unlike the precedent's date case where
  every in-range value is a hit.
  — `[measured: Read .claude/skills/ai-audit/scripts/check-citations.sh l.76 → `[ "$n" -le "$LOCAL_MAX" ] 2>/dev/null && continue`]`
- **R3 — `.claude/skills/task/SKILL.md` crossing 35,000 chars.** 1,191 chars of
  headroom against the planned delta of **≈ 737 chars** (Key decision 6's costed
  table — the rejected inline-blocks variant was ≈ 987; the "≈ 967" figure this
  risk carried until round 4 was neither, and the "≈ 717" it carried until round 5
  predated the third pointer). **Mitigation:** the hard budget and the
  now-required relief valve in Key decision 6; `wc -c` re-run is an AC16 command.
  — `[measured: wc -c .claude/skills/task/SKILL.md → 33809]`
- **R4 — AC16 is read DELTA-WISE. Product-owner decision, round 2; no spec
  amendment.** AC16 stands **verbatim**: no loaded instruction file crosses
  35,000 chars *as a result of this change*. `AGENTS.md` is recorded at
  **38,874** and **unchanged** — out of this task's scope, not a carve-out from
  the criterion. The design's earlier "unsatisfiable, re-scope to 40,000"
  reading is **withdrawn**; the 40,000 threshold plays no part in this task's
  verification. The operative constraint is Key decision 6's hard budget:
  `.claude/skills/task/SKILL.md` must stay under the 35,000 cap, verified against a
  `BEFORE` re-measured from the base commit rather than the design's recorded 33,809. This is the stricter reading and it costs no amendment round.
  — `[measured: wc -c AGENTS.md → 38874 (recorded as the unchanged baseline; git diff --stat over AGENTS.md must be empty at Step 9); wc -c .claude/skills/task/SKILL.md → 33809]`
- **R5 — `.claude/**` edits denied to the delegate.** Handled structurally by
  Key decision 2 (ordering + pre-planned in-thread takeover), not by a claim in
  either direction about whether the guard fires.
  — `[measured: cat .claude/settings.json → no deny entry matching .claude/**; sed -n '88,102p' ai-docs/learnings.md → the 2026-07-16 own-definition incident and its "apply in-thread" rule]`
- **R6 — no CI gate covers either shell script.** CI's `changes` job filters on
  `**/*.rs`, `**/Cargo.toml`, `Cargo.lock`, `clippy.toml`, `.github/workflows/**`,
  `rust-toolchain*`; a `.sh` diff triggers no Rust job, and no workflow runs
  `shellcheck` or the fixture test. The script's only gates are the ones this
  design names (subtask 4 + subtask 8). **Mitigation:** run
  `shellcheck` and the fixture test explicitly at Step 8 subtask 4 **and** again
  at Step 9. This matches the precedent's status exactly —
  `test-check-citations.sh` is likewise run by a skill (`/ai-audit` checklist P),
  never by CI.
  — `[measured: sed -n '1,45p' .github/workflows/ci.yml → the dorny/paths-filter `rust:` list; grep -rn 'shellcheck' .github/ → no hits]`
- **R7 — `objections` / `objections_reopened` count status CELLS, not distinct
  findings.** A finding objected in round 2 and re-objected in round 3 counts
  twice. This is the spec's chosen unit (reproducible from the file alone) and
  must be stated on the schema page so a future consumer does not read it as a
  distinct-finding count.
  — `[derived → discharged by the AC13 grep at Step 9]`
- **R8 — a `-D warnings`-style gate masking later failures does not apply here**;
  neither `shellcheck` nor the fixture test aborts on first failure — `shellcheck`
  reports all findings per file and the test harness accumulates `failures` and
  reports every case. **But** the *sweep* in subtask 8 does have this shape:
  fixing an AC15 hit can reveal another. Budget a re-run of the whole sweep after
  the last fix.
  — `[measured: Read .claude/skills/ai-audit/scripts/test-check-citations.sh → `failures=$((failures + 1))` accumulator, all four cases run unconditionally]`
- **R9 — line-shift key drift: `findings_first_seen` degenerates toward
  `findings` on exactly the runs it exists for** (spec § *Technical constraints*
  8; the row the round-5 risk table was missing). Mechanism, end to end: `/task`
  Step 11 applies fixes between rounds → any fix changing a file's line count
  shifts every line below it → `self-review` re-derives locations each round
  against the current tree → a carried-forward finding's `File:line` key changes
  → it re-counts as first-seen. Frequency is **common, not marginal**, and is
  *positively coupled* to the runs where the field matters (multi-finding files):
  the condition that makes the field/`findings` split worth anything is the
  condition that moves the lines. This is an **anti-correlation with usefulness**,
  not a bias that averages out. **Mitigation is documentation, not repair** —
  owner decision, settled: the key stays `File:line`, and the schema page carries
  the frequency statement, the ambiguity statement and the degeneracy signature
  (subtask 2 🔁, clauses (a)–(c)). The fixture **reaches** the failure rather than
  describing it: case 14 asserts the drifted row is re-counted, so the behaviour
  is pinned by a test, not by prose.
  — `[measured: awk 'NR==169' .claude/agents/self-review.md → "- On REJECT — every violation must have an exact file and line number." — the re-derivation rule that makes the key volatile; awk 'NR==177' → "  - Focus on remaining `⬜ Open` items plus anything newly introduced." — the carry-forward rule that makes the drift observable]`
- **R10 — the coupling clause erodes silently in a LATER edit.** The failure this
  risk names is not in this PR: it is a future editor deleting or softening the
  degeneracy-signature paragraph while leaving `findings_first_seen` in the
  schema, which converts a *labelled-degenerate* measure into a *silently-wrong*
  one — and does so mid-series, so every record before and after the edit means
  something different while looking identical. **Mitigation:** three layers, all
  mechanical. (1) `AC13a` grep (v) fails the AC on a missing signature. (2) The
  prescribed branch on that failure is **remove the field**, written into both the
  schema page (clause (c)) and Key decision 8, so an editor meets the instruction
  at the point of edit. (3) The field's schema-table row carries the coupling
  pointer, so the constraint is reachable from the field as well as from the
  narrative.
  — `[measured: grep -c 'ship together or not at all' ai-docs/plans/2026-07-31-task-run-telemetry.spec.md → 2 (the identity decision and AC13a both carry it — the spec deliberately states it twice so neither reader path misses it); grep -c 'ship together' ai-docs/task-run-schema.md → 0 (absent today; subtask 2 🔁 clause (c) adds it)]`

---

## Test Design

### Location and entry point

- **Location:** `.claude/skills/task/scripts/test-append-task-run.sh` — a
  standalone bash harness, invoked as
  `bash .claude/skills/task/scripts/test-append-task-run.sh`. Exit 0 = all cases
  pass; exit 1 = regression. Mirrors `test-check-citations.sh`'s `report()` +
  `failures` accumulator shape.
  [measured: `Read .claude/skills/ai-audit/scripts/test-check-citations.sh` → `report()` at l.40, `failures` accumulator, `exit 1` on any]
- **Entry point under test:** `.claude/skills/task/scripts/append-task-run.sh
  <progress-file> [<target-jsonl>]`.

### Fixture strategy — deliberately DIFFERENT from the precedent

`test-check-citations.sh` mutates a tracked file (`ai-docs/corrections-log.md`)
under `trap`-based backup/restore, and needed a dedicated case-4 to prove it did
not silently drop the file's mode bits.
[measured: `Read .claude/skills/ai-audit/scripts/test-check-citations.sh` l.32–38 (three traps + `mode_before`), l.105–114 (case 4)]

That complexity exists because that guard's *input is the repo*. Ours takes an
explicit path argument, so every fixture is written by heredoc into a
`mktemp -d` sandbox and the target JSONL lives there too. **Zero tracked files
are touched**, which removes the mode-preservation hazard, the `trap` triad, and
the restore-on-interrupt path outright. A single `trap 'rm -rf "$tmp"' EXIT INT TERM`
suffices. Case 13 asserts the no-mutation property directly rather than assuming it.

### Fixtures

| Name | Content |
|---|---|
| `F1` well-formed | Full header (`**Branch:**`, `**base_commit:**`, `**Issue:** #42`, `**Spec:**`, `**current_step:**`, `**last_passed_gate:**`); a `## Decisions log` **pinned to carry live bait** (below); a `## AC Status` table; a `## Files touched` section with 3 `` - `path` — desc `` lines; **three** `## Self-Review (Round N)` sections carrying **two** cross-round pairs — the **stable-key** carry-forward `src/b.rs:20` (AC9) and the **drifted-key** pair `src/g.rs:70` → `src/g.rs:73` (AC9a); and a **trailing `## Comment cycle round 1` decoy section after R3**. Section contents and the hand-counted expectations are tabulated below. |
| `F2` absent | Path inside the sandbox that does not exist. |
| `F3` no sections | Valid header + `## Files touched`, zero `## Self-Review` sections. |
| `F4` garbled | Two Self-Review headings; one section's table truncated mid-row, one severity cell reading `critical`, one section missing its `**Verdict:**` line, an `**Issue:**` line carrying a **URL** rather than `#N` (per progress-format.md's `[#number or URL]`), and **no `**base_commit:**` line** — so three degradation paths fire at once: `issue: null`, `"UNKNOWN"` verdict, and the merge-base trio fallback. |
| `F5` round-cap | Minimal 3-round file, verdicts `REJECT` / `REJECT` / **`REJECT`**. Exists solely so `hit_round_cap` is exercised in its **`true`** state — F1 only ever produces `false`, and a parser that hardcoded `false` would pass every other case. (The "would this still pass if I broke the thing it names?" check from `self-review.md` § Patterns 2.) |

#### F1 section layout and hand-counted expectations

Every row's `File:line` cell is pinned, because that cell **is** the cross-round
identity key for `findings_first_seen` — a fixture that left it unspecified could
not express the carry-forward case AC9 now mandates.

| Section | Rows (`File:line` · severity · status) |
|---|---|
| `## Self-Review (Round 1)` — `**Verdict:** REJECT` | `src/a.rs:10` · blocker · `✅ Fixed` <br> `src/b.rs:20` · major · `⚠️ Objected: <reason>` <br> `src/c.rs:30` · major · `✅ Fixed` <br> `src/d.rs:40` · nit · `✅ Fixed` <br> **`src/g.rs:15`** · minor · `✅ Fixed` ← *the fix that shifts `g.rs` line numbers*; **this row carries a MANDATORY inline comment in the fixture heredoc — see § *Two distinct fixture comments*, below** <br> **`src/g.rs:70`** · minor · `⬜ Open` · Finding text **`Missing doc comment`** ← *drift pair, round N* |
| `## Self-Review (Round 2)` — `**Verdict:** REJECT` | **`src/b.rs:20`** · major · `⬜ Open 🔁 Re-opened` ← *same key as R1 row 2: the **stable-key** carry-forward + the re-open of that objection* <br> `src/e.rs:50` · major · `⬜ Open` <br> `src/f.rs:60` · minor · `⚠️ Objected: <reason>` <br> **`src/g.rs:73`** · minor · `⬜ Open` · Finding text **`Missing doc comment`** ← *drift pair, round N+1 — same file, same Finding text, **shifted line** (AC9a)* |
| `## Self-Review (Round 3)` — `**Verdict:** APPROVE` | no rows |
| `## Decisions log` (**outside** every Self-Review section) | a bullet: `- **Step 11**: accepted the ⚠️ Objected rationale on src/b.rs:20; later 🔁 Re-opened in Round 2` |
| `## Comment cycle round 1` (**trailing decoy**) | `\| 1 \| f.rs:1 \| major \| decoy \| ⚠️ Objected: x \|` <br> `\| 2 \| f.rs:2 \| minor \| decoy \| ⬜ Open 🔁 Re-opened \|` |

Expected under **correct section bounding**, hand-counted:

| Field | Value | Derivation |
|---|---|---|
| `rounds` | `3` | three `## Self-Review (Round N)` headings |
| `verdicts` | `["REJECT","REJECT","APPROVE"]` | in round order |
| `hit_round_cap` | `false` | `rounds >= 3` **but** `verdicts[2] == "APPROVE"` |
| `findings` | `{blocker:1, major:4, minor:4, nit:1}` = **10** | Per bucket, counting every row in every bounded section: **blocker** = R1 `a.rs:10` → 1. **major** = R1 `b.rs:20` + `c.rs:30` (2) + R2 `b.rs:20` + `e.rs:50` (2) → 4 — `src/b.rs:20` counted **twice**. **minor** = R1 `g.rs:15` + `g.rs:70` (2) + R2 `f.rs:60` + `g.rs:73` (2) → 4. **nit** = R1 `d.rs:40` → 1. Row check: R1 6 + R2 4 + R3 0 = **10** = 1+4+4+1 ✓ |
| `findings_first_seen` | `{blocker:1, major:3, minor:4, nit:1}` = **9** | R1 contributes **all 6** rows (blocker 1, major 2, minor 2, nit 1). R2 is tested against R1's key set `{a.rs:10, b.rs:20, c.rs:30, d.rs:40, g.rs:15, g.rs:70}`: `b.rs:20` **is** in it → *not* first-seen (the AC9 de-duplication); `e.rs:50` is not → +1 major; `f.rs:60` is not → +1 minor; **`g.rs:73` is not** — R1 carries `g.rs:70`, a different byte string → **+1 minor, the AC9a over-count**. R2 contributes major 1, minor 2. Totals: blocker 1, major 2+1 = **3**, minor 2+2 = **4**, nit 1 → **9** ✓ |
| ↳ *per-bucket reading* | major **4 → 3**, minor **4 → 4** | The two buckets show the two regimes side by side in one fixture: **major** de-duplicates (the stable-key carry-forward `b.rs:20` is counted once), **minor** does **not** (the drifted `g.rs:70`→`:73` pair is counted twice), so `findings.minor == findings_first_seen.minor == 4`. That per-bucket equality is exactly the degeneracy signature at severity granularity, and it is what case 14 asserts. Whole-record ratio 9/10 < 1, correctly signalling "the key held for at least one row" — which it did, for `b.rs:20` |
| `objections` | `2` | R1 `src/b.rs:20` + R2 `src/f.rs:60` |
| `objections_reopened` | `1` | R2 `src/b.rs:20` |
| `files_touched` | the 3 paths | from `## Files touched` |
| `incomplete` | `false` | no trigger fires |

**`findings` 10 vs `findings_first_seen` 9 is the whole point of the field split**,
and `src/b.rs:20` is the row that separates them: twice in one, once in the other.
That single row discharges AC9's carry-forward requirement.

**The `g.rs` pair discharges AC9a, and it is deliberately the row that does NOT
separate them.** `src/g.rs:70` (R1) and `src/g.rs:73` (R2) are the same finding —
same file, same `Finding` text — under a `File:line` key that changed because the
`src/g.rs:15` fix above them shifted the numbering. The extractor therefore counts
it **twice in both** `findings` and `findings_first_seen`. That is the measured
behaviour of the shipped key, **not a defect**, and case 14 asserts it as expected
with the inline comment `AC9a` requires. AC9's `b.rs:20` row proves the
de-duplication works under a *stable* key; without the `g.rs` pair the fixture
would never touch the failure the key actually has — the gap `AC9a` exists to
close. The two rows are complementary: one proves the mechanism, the other pins
its known limit.

**Two distinct fixture comments — required, and NOT interchangeable.** A later
editor will be tempted to collapse them; they guard different actions.

| | Where | Guards against | Required by |
|---|---|---|---|
| **Comment A** | at the **case-14 assertion** | a reader seeing a wrong-looking expected value and "fixing" the parser | `AC9a` |
| **Comment B** | on the **`src/g.rs:15` fixture row** | a reader **deleting** the row that *causes* the drift | round-10 addendum |

**Comment A** tells a reader the failing-looking number is intended. **Comment B**
tells a reader not to delete the fixture row that produces it. Neither substitutes
for the other, and B is the one that matters when someone is staring at a red
test — **which is exactly the moment they are not reading this design.**

**Comment B, pinned verbatim** (gate and artefact copied from this one source):

```
# This row is the Step-11 fix whose line delta shifts src/g.rs:70 -> :73 between
# R1 and R2. It exists to instantiate key drift. Deleting it makes case 14 pass
# for the wrong reason and removes the only scenario findings_first_seen
# measures. If case 14 is red, the defect is in the parser or the gate - not in
# this row. See ai-docs/task-run-schema.md.
```

**Gate — wrap-insensitive, join lines before matching:**
`joined(){ tr '\n' ' ' < "$1" | sed 's/#//g' | tr -s ' '; }`, then
`joined test-append-task-run.sh | grep -c 'exists to instantiate key drift'` → `≥ 1`
**and** `… | grep -c 'not in this row'` → `≥ 1`.

**Round-20 fix — the raw form fails on correct input, and the proof is this design's own pinned text.**
The comment was reflowed by one word during implementation. The wrapping pinned *above*
breaks the phrase as `… the gate - not in` / `# this row.`, against which a raw
`grep -c 'not in this row'` reads **0**; the implemented file happens to break it one word
later and reads **1**. Which side passes is an accident of rendering, so the gate is made
wrap-insensitive rather than the wrap re-pinned — **re-pinning would name the remedy after
the current rendering, which is the generator.**
[measured, three inputs, mutation effectiveness asserted first (design-wrap variant differs from implemented `YES`, carries the split pin `1`; comment-removed variant lost the phrase `0`): **raw gate** — implemented `1` PASS, design-wrap `0` **FAIL on correct input**, removed `0` FAIL; **wrap-insensitive gate** — implemented `1` PASS, design-wrap `1` PASS, removed `0` FAIL]

**The general rule this instantiates** (§ *Gate self-audit* → round 10): *where a
gate pins a value a fixture also determines, the fixture is the source of truth
and the gate must derive from it or assert an equality; and where a fixture row is
load-bearing for a property, the row itself carries the prohibition against
removing it.* A protection that lives only in the design protects nothing at the
moment someone is looking at a red test.

**Why the fixture carries `src/g.rs:15` as well.** One row would be enough to make
the key drift, but the drift's *cause* is what the spec calls out — "several
findings in one file, some fixed, some carried" is both the condition that makes
the field split worth anything and the condition that moves the lines. `g.rs:15`
(`✅ Fixed`) is that condition instantiated: it is the Step-11 fix whose line
delta moves `g.rs:70` to `:73`. Without it the fixture would assert a shifted line
with no reason for the shift, and a later reader could mistake the pair for a
reviewer typo rather than the structural failure R9 describes.

Under a parser that drops bounding, the counters move — but **only one moves
shape-independently**, and the distinction is worth stating rather than glossing:

| Counter | Unbounded value | Depends on the unbounding parser's shape? |
|---|---|---|
| `findings` | **12** (`{blocker:1, major:5, minor:5, nit:1}` — the bounded `{1,4,4,1}` plus the two decoy rows, which land in the `major` and `minor` buckets) | **No.** Any parser matching `^\| [0-9]` rows picks up the decoy's two rows regardless of how it scopes sections. This is the shape-independent falsifier. Bounded **10** vs unbounded **12** stay distinct after the AC9a rows, so case 2 still fails loudly if bounding is dropped. |
| `objections` | **4** under a whole-file token grep (R1 1 + R2 1 + Decisions-log bullet 1 + decoy 1); **3** under a parser that drops bounding but still matches only `^\| [0-9]` rows — the `## Decisions log` bullet is prose, not a table row | **Yes** |
| `objections_reopened` | **3** under a whole-file token grep (R2 1 + bullet 1 + decoy 1); **2** under the row-matching variant | **Yes** |
| `findings_first_seen` | not stated — an unbounding parser's notion of "the preceding round" is undefined | **Yes, and undefinably so** |

The round-4 text called 4 / 3 "deterministic". They are not: they are
*whole-file-token-grep* values, and the same indeterminacy this design refuses to
paper over for `findings_first_seen` applies to them one degree less severely.
Case 2's assertions are on the **bounded** values (`objections` 2 / `objections_reopened` 1 / `findings` **10** / `findings_first_seen` **9**) and are
unaffected either way; `findings → 12` unbounded is what guarantees the case fails if
bounding is dropped, whatever shape the broken parser takes.

`findings_first_seen` also breaks, but its unbounded value is **not stated**: it
depends on how an unbounding parser assigns the decoy's rows to a "round", which
is undefined behaviour rather than a derivable number. Claiming a specific value
would be the round-3 defect repeated — an arithmetic assertion nobody can
reproduce. The case asserts the **bounded** value (`6`) exactly and lets the three
deterministic counters carry the falsification.

**The decoy's severity cells are load-bearing, and its row shape is deliberately
NOT realistic.** It borrows the *self-review* table shape (`| # | File:line |
Severity | Finding | Status |`) under a *comment-cycle* heading, on purpose: the
real `## Comment cycle round M` table is
`| Thread | path:line | Author | Category | Diff SHA | Reply | End state |`,
whose third cell is an **Author**, never a severity. A shape-realistic decoy
would therefore contribute to no `findings` bucket — leaving that counter at 7
whether or not the parser bounds sections, i.e. an inert decoy of exactly the
kind this design demoted `## AC Status` for. Realism is sacrificed on the row
shape precisely to keep the third counter falsifiable; the heading stays real so
the fixture still exercises a section the parser will genuinely meet.
[measured: `sed -n '125,133p' .claude/skills/pr-commented/SKILL.md` → `| Thread | path:line | Author | Category | Diff SHA | Reply | End state |`]

#### Why these three changes, specifically

- **The `## Decisions log` bait is not synthetic — it is mandated.**
  `.claude/skills/task/SKILL.md:218` (Step 11 sub-step 4) requires: *"append a
  `## Decisions log` bullet recording any `⚠️ Objected` rationale or
  Design-Amendment trigger"*. Therefore **every real progress file with an
  objection contains the literal `⚠️ Objected` outside any Self-Review section**,
  and an unbounded counter over-counts on *every real run* — not on a contrived
  input. This is the live discriminating case, and the round-1 fixture had no
  specified `## Decisions log` content at all.
  [measured: `Read .claude/skills/task/SKILL.md` l.218 → *"append a `## Decisions log` bullet recording any `⚠️ Objected` rationale or Design-Amendment trigger (one line, prefixed `Step 11:`; omit if none)"*]
- **The `## AC Status` table is an INERT decoy — it proves nothing.** Its rows are
  `| AC1 | PASS |`, i.e. they begin `| AC1`, so the `^\| [0-9]` row regex cannot
  match them with **or** without bounding. It is retained only so F1 has a
  realistic shape; the round-1 claim that case 2 "proves `## AC Status`
  exclusion" was an overclaim about an input that could never have failed.
  The `## Comment cycle round 1` decoy replaces it as the real discriminator —
  its **heading stem** is realistic, while its **row shape** is deliberately
  borrowed from the self-review table so its severity cells reach the `findings`
  buckets. See the note under § *F1 section layout* for why shape-realism would
  have re-created the inert-decoy defect.

  Precision on the heading: `/pr-commented` writes
  `## Comment cycle round M — PR #<N> (base <sha>, target <pending>)`, not a bare
  `## Comment cycle round 1`. The fixture uses the **stem only**, and the
  sufficient reason is that **the parser bounds on `^## `** — the suffix is not
  part of what the case tests, so carrying it would add noise without adding
  coverage.
  The `check-citations.sh` argument is **not** offered as a second ground, because
  it does not survive this design's own R2: the guard skips any `#N ≤ LOCAL_MAX`,
  and a fixture PR number would sit far below 185. Stating it as a reason would be
  a rationale contradicting a measurement recorded ten pages earlier in the same
  document.
  [measured: `grep -n 'Comment cycle round' .claude/skills/pr-commented/SKILL.md` → the full heading form at l.117; guard l.76 → `[ "$n" -le "$LOCAL_MAX" ] 2>/dev/null && continue`, `LOCAL_MAX` = 185 per R2]
  [measured: `Read ai-docs/templates/progress-format.md` l.49–53 → `| AC | Status |` / `| AC1 | PASS / FAIL / NOT_TESTED |`; l.86 → *"`/pr-commented` appends `## Comment cycle round M` sections"*]
- **Objections now span two sections, and the re-open has an antecedent.**
  Round 1 previously held both `⚠️ Objected` cells, so cross-section
  accumulation was never exercised; and R2's `⬜ Open 🔁 Re-opened` row had no
  R1 objection behind it, so the fixture contradicted this design's own Key
  decision 5 (the marker lands on the **new** round's row, re-opening a *prior*
  round's objection). Moving one objection into R1 fixes both at once and leaves
  every expected value unchanged.

`#42` is chosen over the real `#186` for R2's reason.

### Cases

| # | Scenario | Assertion |
|---|---|---|
| 1 | `F1` happy path (AC9) | exit 0; exactly one new line; each of `rounds` / `verdicts` / `hit_round_cap` / `findings` / `findings_first_seen` / `objections` / `objections_reopened` / `files_touched` / `incomplete` equals the hand-counted value in § *F1 section layout*, compared field-by-field via `jq -e`. A whole-object compare is deliberately avoided: `date`, `branch`, `instruction_corpus_lines` and the diff-size trio are environment-dependent. |
| 2 | `F1` — **section bounding** | The counters must equal `objections=2`, `objections_reopened=1`, `findings` total **10**, *despite* F1 carrying a `⚠️ Objected` + `🔁 Re-opened` bullet in `## Decisions log` and a trailing `## Comment cycle round 1` table whose two rows begin `\| 1 \|` / `\| 2 \|`, carry `major` / `minor` **severity cells**, and carry one token each. A parser that scans the whole file instead of bounding to `## Self-Review (Round N)` … next `^## ` yields 4 / 3 / **12** and **fails all three**. This case is the one that can actually fail; it does not claim to prove `## AC Status` exclusion, which is unfalsifiable (those rows begin `\| AC1`, unmatchable by `^\| [0-9]` either way). |
| 3 | `F1` **carry-forward** (AC9's explicit clause) | The row keyed `src/b.rs:20` appears in R1 and R2. Assert it is counted **twice** in `findings` (`major` 4, not 3) and **once** in `findings_first_seen` (`major` 3, not 4). Deleting the first-seen de-duplication makes `findings_first_seen.major` read 4 and this case fail; without this assertion the two fields are indistinguishable on any fixture where every row is unique. |
| 4 | `F2` absent (AC7) | exit **0**; one valid JSON line; `incomplete == true`; **all nine** `fallback-required` keys present — including `files_changed` / `insertions` / `deletions`, computed off `git merge-base main HEAD` since no `**base_commit:**` exists to read; `spec_base` equals the basename minus `.progress.md`; no optional key carries a bogus zero it could not have measured. |
| 5 | `F3` no sections (AC7) | exit **0**; `incomplete == true`; `rounds == 0`; `verdicts == []`; `hit_round_cap == false`. |
| 6 | `F4` garbled (AC7) | exit **0**; `jq -e . ` accepts the line; `incomplete == true`; the unknown `critical` severity is in no bucket; the verdict-less section contributes `"UNKNOWN"`; `issue == null` from the URL-form `**Issue:**` line — asserted as JSON `null`, **not** absent, since `issue` is `fallback-required`; and the trio is present despite the missing `**base_commit:**`. |
| 7 | `F5` round cap (**true** state) | `hit_round_cap == true` on `REJECT`/`REJECT`/`REJECT`. Paired with case 1's `false`, this is what makes the field's derivation testable in both directions. |
| 8 | Cannot append (AC8) | Target = `"$tmp/does-not-exist/task-runs.jsonl"`. Exit **non-zero**; stderr non-empty; no file created. A non-existent parent directory is used rather than `chmod 000`, because a root-run test would defeat a permission-based fixture. |
| 9 | Append-only / last-line-wins | Run case 1 twice against the same target: exactly 2 lines; line 1 byte-identical to the first run's output. |
| 10 | Trailing newline (AC1) | `tail -c1 "$target"` is `0x0a` after every appending case. |
| 11 | AC10 two-path containment | Derive three key sets **from the artefacts, mechanically**: `REQUIRED` = first-cell field names of the schema page's field-table rows whose class cell reads `fallback-required` (now **nine**, incl. the trio — derived from the page, never hardcoded, so the count cannot drift); `EXAMPLE` = `jq -r 'keys[]'` over the JSON block extracted by `awk '/^### Worked fallback example$/{f=1} f&&/^```json$/{g=1;next} g&&/^```$/{exit} g'`; `SCRIPT` = `jq -r 'keys[]'` over case 1's record. **Its cardinality is DERIVED from § *Record schema (v1)*** (rung 1): `F=$(awk '/^\| Field \| Type \|/{f=1;next} f&&/^\| `/{n++} f&&/^$/{print n;exit}' <design>)`, then assert `|SCRIPT| == F`. Assert `REQUIRED ⊆ EXAMPLE` and `EXAMPLE ⊊ SCRIPT` (proper — the strictness AC10 demands). **`sort` every side before `comm`, or use `jq -n --argjson a … --argjson b … '$a - $b == []'`.** `REQUIRED` is emitted in **field-table order** while `jq -r 'keys[]'` is **sorted**, and `comm` on unsorted input does not merely misreport — it writes `comm: file 1 is not in sorted order` to stderr *and* emits spurious rows, so the assertion silently reads garbage. [measured: unsorted `comm -23` on a 3-key set → two spurious rows plus two stderr warnings; the same inputs `sort`ed → empty, the correct result] |
| 12 | **AC11a — shortstat shapes (four sub-assertions)** | Build a throwaway `git init` repo inside the sandbox with purpose-made commits and run the script with that repo as cwd, asserting the trio per shape: (a) **deletions-only** → `insertions == 0`, `deletions == 3` (the shape a positional parse inverts); (b) **single insertion** → `files_changed == 1`, `insertions == 1`, `deletions == 0` (the singular noun *and* the absent deletions clause); (c) **no changes** → `0/0/0` from empty output; (d) **singular `deletion`** → ` 1 file changed, 2 insertions(+), 1 deletion(-)` must yield `insertions == 2`, `deletions == 1`. **Each shape's fixture pins `**base_commit:**` to the sandbox SHA** — see the note below. A real repo is used rather than a test-only "parse this string" hook in the script; adding an API surface that exists only for the test was rejected. Only the trio is asserted here; `instruction_corpus_lines` is meaningless in a sandbox repo and is not checked. |
| 13 | No tracked-file mutation | `git status --porcelain` captured before and after the whole run must be identical. Asserted, not assumed — the sandbox strategy makes it true by construction, and this case is what proves the construction held. |
| **14** | **`F1` key drift — the over-count asserted as EXPECTED (AC9a)** | The `src/g.rs:70` (R1) → `src/g.rs:73` (R2) pair is one finding under a drifted `File:line` key. Assert `.findings.minor == 4` **and** `.findings_first_seen.minor == 4` — i.e. the drifted row is counted twice in **both**, receiving **no** de-duplication, while case 3's `major` bucket shows the de-duplication working (4 → 3) on the stable key. Assert the equality explicitly (`.findings_first_seen.minor == .findings.minor`) so the *absence* of de-duplication is what the test states, not an incidental number. **The inline comment is part of the case, not commentary** — AC9a fails an assertion that lacks it. It must state that the over-count is *expected under the current `File:line` key and is not a defect*, and point at `ai-docs/task-run-schema.md`; the § *AC verification commands* AC9a row greps for both. Falsification direction is deliberate: "repairing" the key to path + `Finding` text would make `findings_first_seen.minor` read 3 and **fail this case**, routing the reader to the comment instead of letting the corpus change meaning mid-series. |
| **15** | **`F6` — a literal `\|` inside a `Finding` cell (M1 column-shift guard)** | `F6` is a minimal one-round file whose single row's `Finding` cell contains a literal `\|` (`uses \`a \| b\` here`), shifting every later column right by one. Assert exit **0**, `objections == 1` and `findings.major == 1`. Falsification direction: a parser that reads `Status` off a **fixed** cell index (`c[6]`) reads part of the *Finding* text as the status, never sees `⚠️ Objected`, and this case goes red. **The form that survives it is the ternary** `stat = (c[n] ~ /^[[:space:]]*$/ ? c[n - 1] : c[n])` (`append-task-run.sh:181`) — *the last cell, where a blank `c[n]` is read as the empty field a **trailing** pipe leaves behind*. It is **not** a bare `c[n - 1]`: that form also passes this case, and row 18 is what kills it — the two rows guard **disjoint** axes and neither subsumes the other. **Scope correction (round 3): what this row guards is the FIXED-INDEX axis, never the hazard's extent.** `\|` is cell *content*, not a delimiter, and may appear in **any** cell — `Status` included — so naming the hazard after the *`Finding`* cell is precisely the mis-naming that let the hole move twice (`c[6]` → shifted `Finding` cell; `c[n - 1]` → `\|` inside `Status`). This case passes **without** the mask at `append-task-run.sh:172`, because "last cell" already absorbs a shift *originating* in the `Finding` cell; row 19 is what guards the class. [measured, whole suite re-run under each mutation with the mutation asserted landed by `diff` before the run: `stat = c[6]` → **case 15 only** goes red (its `objections == 1`, 1 assertion) while every case-18 and case-19 assertion PASSes; `stat = c[n - 1]` → case 15 fully PASSes] This axis was blind before — the suite was green **on** the defect — so the case is a regression guard for a fault **no other fixture expressed at the time it was written**, every other fixture then carrying pipe-free `Finding` cells. `F10` (row 19) now walks the same escape through every cell position, so this row survives as the fixed-index member of a **four-way** disjoint set (rows 15 / 18 / 19 / 20 — it read *three-way* until row 20 landed), not as the class guard. |
| **16** | **`F7` — an unbucketed severity ALONE triggers `incomplete` (M2/M3)** | `F7` is clean in **every other respect** — parseable `#N` `**Issue:**`, resolvable `**base_commit:**`, a `## Files touched` section, a well-formed `**Verdict:**` — and its one row carries the severity `critical`. Assert exit **0**, `incomplete == true`, and `([.findings[]] \| add) == 0` (the row lands in no bucket). The isolation is the point: **no other trigger in this fixture can produce `incomplete`**, so the case can only pass for the reason it names. Case 6's `F4` fires three degradation paths at once and therefore cannot; this row and row 17 are what discharge that round-1 finding. |
| **17** | **`F8` — a verdict-less round ALONE triggers `incomplete` (M2/M3)** | `F8` is isolated the same way — clean header fields, a `## Files touched` section, both rows well-bucketed — and differs from a fully valid file in exactly one respect: `## Self-Review (Round 2)` carries **no `**Verdict:**` line**. Assert exit **0**, `incomplete == true`, and `verdicts == ["REJECT","UNKNOWN"]`, pinning the substituted token in round order rather than merely asserting its presence. Together with row 16 this splits `F4`'s three-triggers-at-once bundle into single-cause cases, so a parser that lost **one** of the two `incomplete` triggers still fails a case instead of being masked by its sibling. |
| **18** | **`F9` — a row with NO trailing pipe (M1' row-shape guard)** | GFM makes a row's **trailing** `\|` optional, and the row matcher requires only the **leading** one, so both shapes reach the parser. `F9` is a minimal one-round `REJECT` file whose two rows omit the trailing pipe: `\| 1 \| src/z.rs:99 \| major \| Do the thing \| ⚠️ Objected: reason` and `\| 2 \| src/y.rs:5 \| minor \| Another finding \| ⬜ Open 🔁 Re-opened`. Assert exit **0**, `objections == 1`, `objections_reopened == 1`, and `findings.major == 1 and findings.minor == 1`. Falsification direction, **disjoint from row 15's**: with no trailing pipe `split` leaves no trailing empty field, so `c[n]` **is** `Status` and `c[n - 1]` is the *`Finding`* cell — a parser hard-coded to `c[n - 1]` reads `Do the thing` / `Another finding` as the status, sees neither marker, and **only this case** goes red (`F6` and `F10` both carry trailing pipes and stay green under that same form). The two severity assertions are deliberately **not** the discriminating ones — `sev` is read at a fixed `c[4]`, upstream of the tail — they pin that the shape change perturbs the **status** cell only. **Read that as scoped to a row-SHAPE change and to nothing else:** an escaped `\|` in an earlier cell perturbs `c[4]` too, which is why row 19's row 6 exists; `c[4]` is safe here only because `F9` carries no escape. [measured, whole suite re-run under each mutation with the mutation asserted landed by `diff` before the run: reverting `append-task-run.sh:181` to `stat = c[n - 1]` → **case 18 only** goes red (its two objection assertions), cases 15 and 19 stay green; `stat = c[6]` → case 15 only goes red; deleting the mask at `:172` → case 19 only goes red] |
| **19** | **`F10` — an escaped `\|` walked through EVERY non-final cell (M1'' class guard)** | `F10` is a one-round `REJECT` file with **six** rows, each carrying an escaped `\|` in a different cell position: `#` (row 1), `File:line` (row 2), `Finding` (row 3), `Status` (row 4), `Finding` **and** `Status` together (row 5), `Severity` (row 6). Assert exit **0**, `objections == 4` (rows 1–4 carry the marker; row 5 is `🔁 Re-opened`, row 6 plain `⬜ Open`), `objections_reopened == 1`, `findings == {"blocker":1,"major":2,"minor":1,"nit":1}`, `([.findings[]] \| add) == 5`, and `incomplete == true` traceable to **row 6 alone** — the fixture carries a resolvable `**base_commit:**`, an `#N` `**Issue:**`, a `**Verdict:**` and a `## Files touched` section, so any other degradation path firing here is itself the bug. **The guarded form is the MASK**: before `split`, the row is copied to `row` and every backslash-escaped pipe is `gsub`'d to the sentinel byte `\002` (`append-task-run.sh:171–172`), the sentinel being restored on `key` / `sev` / `stat` after trimming (`:185–187`). The regex is deliberately *not* transcribed into this cell — a backslash-and-pipe literal cannot be written in a GFM table cell without an escaping convention that collides with this table's own (`\|` here denotes a literal pipe in cell content, per rows 15 and 18); read it live at the cited lines. It is the third shipped form of this fix and **the first named after a property of the INPUT** (`\|` is cell content, in *any* cell) rather than after the cell where the shift was last seen; a case pinning one more cell would have repeated rounds 1 and 2, so this fixture enumerates the positions instead. Falsification direction, **disjoint from rows 15, 18 and 20 — the four forms red four non-overlapping case sets** (it was three when this row was written; row 20 made it four, and the matrix is extended rather than replaced): [measured, whole suite re-run under each mutation with the mutation asserted landed by `diff` before the run: `stat = c[6]` → **case 15 only**, 1 assertion; `stat = c[n - 1]` → **case 18 only**, 2 assertions; the escaped-**pipe** mask stage deleted → **case 19 only**, 4 assertions; the escaped-**backslash** mask stage deleted → **case 20 only**, 1 assertion. The mask is now **two** `gsub` stages that red disjoint case sets, so "the `gsub` mask deleted" is no longer a single mutation — name the stage. Deleting *both* stages reds **case 19 only** (4 assertions), leaving case 20 green, because with no mask at all the delimiter after an escaped backslash is split on correctly] **What that implies is not what a reader would reason:** cases 15 and 18 pass **without** the mask, because "last cell" already absorbs a shift *originating* in the `Finding` cell. The mask is what handles an escape inside the **`Status` cell itself**, where the status text splits across cells and **no single index holds it** — the axis rounds 1 and 2 both left open. None of the **four** cases subsumes another; each owns exactly one property — fixed index (15), row shape (18), escaped-pipe cell position (19), escape-of-the-escape (20). **Row 6 is the mislocation discriminator:** the escape makes its severity cell unbucketable, so the row contributes to no bucket and trips `incomplete` — whereas a parser reading the *wrong* cell as severity would likely find a valid bucket name there and bucket the row, so `([.findings[]] \| add) == 5` is what fails if location is wrong. If a future edit reds **one** row of this fixture, read that row's cell position before touching the parser: it names the cell the edit dropped. |
| **20** | **`F11` — an escaped BACKSLASH adjacent to a REAL delimiter (M1''' class guard)** | `F11` is a one-round `REJECT` file with **two** rows. Row 1 is the discriminator: its `Finding` prose *names* the objection marker, the cell ends in an escaped backslash (`\\`), and the row's **real** `Status` is `✅ Fixed`. Row 2 is the complement — an escaped backslash **not** adjacent to a delimiter, sharing a cell with a genuine escaped pipe — so the fix is pinned to leave an ordinary `\\` intact and round-tripping, not merely to stop eating delimiters. Assert exit **0**, `objections == 1`, `findings == {"blocker":0,"major":1,"minor":1,"nit":0}`, and `incomplete == false`. **This class is a FALSE POSITIVE, not a parse failure — which is exactly why it outlived rows 15, 18 and 19.** A single-stage pipe mask matches the *second* backslash of `\\` together with the genuine delimiter that follows it, consumes that delimiter, and **merges `Finding` into `Status`**; the marker sitting in the Finding prose is then read as the row's status. The count moves in the direction that looks like **more** review work, `incomplete` stays `false`, and no gate flags it. **The guarded form is the two-stage mask and its ORDER**: escaped backslashes are consumed to the sentinel `\001` **first** (`gsub(/\\\\/, "\001", row)`), leaving only genuine escaped pipes for the second pass, both sentinels being restored on `key` / `sev` / `stat` after trimming. Falsification direction, and it is **narrower than "delete the mask"**: [measured, whole suite re-run per mutation against scratchpad copies of `append-task-run.sh`, each mutation asserted landed by `diff` before its run (exactly the intended lines deleted), the suite driven through a parametrised copy of the test script so no project file was touched: **escaped-backslash stage deleted — i.e. reverted to the single-stage mask** → **case 20 only** goes red, and within it exactly **one** assertion, `objections == 1`, the emitted record reading `"objections":2` alongside `"incomplete":false`; case 20's own `findings` and `incomplete` assertions still PASS, and all **13** assertions of cases 15, 18 and 19 PASS. **Both stages deleted** → case 20 stays **green** and only case 19 reds, because with no mask at all the delimiter after an escaped backslash is split on correctly. The fault is therefore not the mask's *absence* but the single-stage mask's *presence*, which is why stage ORDER — not stage existence — is the property this case owns] **Why row 19 could not catch it:** `F10` walks an escaped *pipe* through every cell position; this defect is an escaped *backslash* adjacent to a real delimiter. That is a different **input class**, not a different cell — no enumeration of cell positions over the `\|` class reaches it, so the gap was never a missing row in `F10`. The four cases now own four **disjoint** axes: fixed index (15), row shape (18), escaped-pipe cell position (19), escape-of-the-escape (20). |

**Why case 12 needs sub-assertion (d), and why it needs the pinned base.**

Sub-assertion (d) exists because without it **case 12 cannot fail for the reason it
exists**. Shapes (a)–(c) are all satisfied by a parser written `deletions\(-\)`
with no `?`: (a) says `3 deletions(-)` — plural; (b) and (c) expect `deletions == 0`,
which a non-matching pattern produces *by defaulting*. Such a parser would then
report `deletions == 0` on any real run that deleted exactly one line — a wrong
number, written once, into a durable append-only corpus that no later gate
re-derives. Only ` 1 deletion(-)` distinguishes the two patterns. This is
`self-review.md` § *Patterns* 2 applied to the same design that already invokes it
for F5's `hit_round_cap`: *would this still pass if I broke the thing it names?*
[measured: shape (d) reproduced on a purpose-built commit → ` 1 file changed, 2 insertions(+), 1 deletion(-)`]

The **pinned `**base_commit:**`** is what keeps the case measuring parsing rather
than environment. A `git init` sandbox has no `main` ref, so the script's
merge-base last resort is unavailable there:
[measured: in the throwaway repo, `git branch --show-current` → `master`; `git merge-base main HEAD` → `fatal: Not a valid object name main`, exit 128]

Without a pinned base the trio would degrade to `0/0/0` and all four sub-assertions
would fail for a reason that has nothing to do with keyword parsing — a false
signal pointing at the wrong defect. Pinning the base to the sandbox SHA keeps the
degradation path (now a documented `incomplete` trigger) out of this case entirely.

**Content-addressed, never line-pinned.** Case 11's extraction anchors on the
literal heading `### Worked fallback example` and on the field table's class
column — not on line numbers. This is the single lesson `check-citations.sh`'s
header documents at length: a line-pinned exclusion (`corrections-log.md:47`)
silently re-pointed at a dateless neighbour after an unrelated insertion, going
RED on a clean tree while *appearing* to work.
[measured: `Read .claude/skills/ai-audit/scripts/check-citations.sh` l.32–36 and l.148–152]

### Not tested, and why

- **The Step-12 integration itself.** There is no harness that can execute a
  `/task` sub-step in a test. Its coverage is threefold and none of it is a unit
  test: AC2's grep (ordering), the **Step-12 verification block** above, and this
  task's own Step 12 producing the log's first real line. That first line is what
  makes AC1's trailing-newline clause and AC14 evaluable **at all** — which is
  why both are pinned to Step 12 rather than Step 9. The fixture test covers the
  script; nothing covers the sub-step's placement except the grep.
- **Miri.** No Rust compiles in this diff, so no test needs
  `#[cfg_attr(miri, ignore)]`, and `.claude/agents/design.md` § Rules forbids
  specifying a routine local Miri run regardless.
  [derived → discharged at Step 9 by `git diff --name-only <base>..HEAD | grep -c '\.rs$'` → must be `0`. The Decomposition table listing no `.rs` file is a *derivation from this document*, not a measurement of the tree; only the post-implementation diff can settle it.]

---

## Open questions

The spec's four are **resolved** here, per `.claude/agents/design.md` § Workflow
(design decides; the owner overrides). Three genuinely-open items follow.

### Resolutions of the spec's Open questions

1. **Step-12 re-entry duplicates → keep append + last-line-wins.** The in-place
   rewrite alternative requires read-modify-write of the entire corpus file; a
   partial write corrupts every line, which is the exact failure AC13's
   hand-edit prohibition names (*"one malformed line breaks any `jq` read of the
   whole file"*). Append is also the shape `_inbox.jsonl` already establishes,
   and a duplicate line is itself signal — it records that a Step-12 re-entry
   happened, which an in-place rewrite would erase. AC1/AC13 wording stands
   unchanged. [measured: `tail -c 1 ai-docs/deferred/_inbox.jsonl | xxd` → `0a`, i.e. the sibling JSONL is newline-terminated append-only]
2. **PR number → omit, as the spec defaults.** `gh pr create` is Step 12
   sub-step 10; the commit carrying the record is sub-step 8. Recording it would
   force an amend or a second commit. Recovery is exact, not approximate, and
   uses a command already in-tree:
   `gh pr list --state all --head <branch> --json number --jq '.[0].number'` —
   the same derivation `cleanup-progress.sh` performs.
   [measured: `Read .claude/skills/pr-merged/scripts/cleanup-progress.sh` l.97 → `gh pr list --state merged --head "${PREV_BRANCH}" --json number --jq '.[0].number'`]
3. **Design-Amendment / nested-`/bugfix` round marking → not in v1**, as the
   spec defaults. `## Decisions log` bullets are free prose behind a `Step N:`
   prefix; detecting an amendment means substring-matching prose — precisely the
   silently-miscounting heuristic the spec rejected for `objections_reopened`.
   A wrong field is worse than an absent one. The `## Decisions log` is not
   parsed. [measured: `Read ai-docs/templates/progress-format.md` l.38–43 → the section is specified as one free-prose line per decision, prefixed by step]
4. **Rotation → none, as the spec defaults.** The file grows by exactly **one
   line per merged `/task` run**. The sibling JSONL surface is the scale
   reference and it is far busier — `_inbox.jsonl` accumulates one row per
   *deferred item*, and stands at **393 lines / 149,194 bytes (~380 B/line)**
   with no rotation mechanism and no observed pressure. A per-task file will
   take years to reach that size. Revisit only if a consumer needs a windowed
   read. [measured: `grep -c '' ai-docs/deferred/_inbox.jsonl` → `393`; `wc -c` → `149194`]

### Genuinely open — owner input welcome, none blocking

- **Q1 — should `ai-docs/task-run-schema.md` get an `AGENTS.md` § *Agent Docs*
  row?** Design says no (Key decision 7): no AC requires it, the spec places the
  invariants on the schema page itself, and leaving `AGENTS.md` untouched is what
  keeps its pre-existing 38,874 chars out of AC16's delta. Cost of "no": the page is discoverable only via
  `task/SKILL.md` and `claude-tools-hierarchy.md`. One-line override if the owner
  prefers the index entry.
- **Q2 — a pre-existing sub-step cross-reference drift, adjacent to our edit.**
  `.claude/skills/task/reference.md:272` and
  `.claude/skills/task/inbox-propagation.md:3` both call inbox propagation
  *"Step 12 sub-step 4"*; `SKILL.md` has it at sub-step **5**. Our `5a` insertion
  does not change that number and does not create the drift. Fixing it is a
  two-character edit in files we are already near, but it is **outside approved
  scope**, and AGENTS.md § *Communication* requires an ask rather than a
  notification. Not included.
  **Disposition (round 4): surface to the owner as a one-line ask at Step 8 entry**,
  before any implementation edit — not left to be noticed mid-flight, when the
  temptation to fold a "two-character fix" into an unrelated commit is highest.
  [measured: `grep -rn 'sub-step' .claude/` → `reference.md:272` and `inbox-propagation.md:3` say 4; `SKILL.md:230` is sub-step 5]
- **Q3 — should `/ai-audit` checklist P gain a row for the new fixture test?**
  `test-check-citations.sh` is run by `/ai-audit` Phase 2; nothing analogous will
  ever run `test-append-task-run.sh` unless a skill names it (R6). Adding the row
  is a one-line `/ai-audit` SKILL.md edit but is not in scope and touches a file
  no AC names.
  **Disposition (round 4): FILE AS A FOLLOW-UP ISSUE AT STEP 12** — a concrete
  deliverable of this task, not a memory item. Decay risk here is real and named:
  R6 establishes that *nothing* in CI runs this script or its test, so if no skill
  ever names `test-append-task-run.sh` it will rot silently and the only signal
  will be a future failure it should have caught. Step 12 already files inbox rows;
  this rides that step.
  [measured: `grep -n 'check-citations' .claude/skills/ai-audit/SKILL.md` → l.6 (`allowed-tools`) and l.116 (checklist P)]
- **Q4 — drift log against the spec (rounds 6–7). Items 1 and 3 are RESOLVED;
  2 and 4 stand.** All live in files this design does not own, so they are
  surfaced rather than edited (AGENTS.md § *Communication* requires an ask, not a
  unilateral fix):
  1. **RESOLVED in spec round 11 — both stale counts are gone.** Round 6 raised
     two: the § *Record-only observation* said *"18 acceptance criteria"* against a
     20-row table, and the `AC13b` disclosure said *"none of the **19** fields"*
     against an 18-field record. `spec-writer` fixed both by **deleting the count
     rather than refreshing it** — the stronger fix, since a refreshed number would
     have gone stale again at the next AC or field addition, and neither argument
     ever depended on the figure. Recorded as closed, not carried.
     [measured: `sed -n '72,73p' …spec.md` → *"This spec carries a **large acceptance-criteria set**"*; `sed -n '58p' …spec.md` → *"since **no field in the record** encodes handoff state"*. `rg -U -n '18\s*\n?\s*acceptance criteria|19\s*\n?\s*fields' …spec.md` → no match, confirming neither count survives]
  2. **"Subtask 3 not started" is not what the tree shows.** The handoff brief
     for this round states the fixture subtask has not begun. In fact a
     **complete 13-case draft exists in the working tree**, untracked and never
     committed. Nothing about amendment 2 changes because of it — the identity
     key is indeed not baked into any *committed* artefact, which is the property
     that made the amendment cheap — but subtask 3 is an **amend**, not a
     from-scratch write, and its Decomposition row now says so. Recorded because
     a claim about work-state that the tree contradicts is worth correcting at
     the point it is relied on.
     [measured: `git status --porcelain` → `?? .claude/skills/task/scripts/`; `wc -c .claude/skills/task/scripts/test-append-task-run.sh` → `18673`; `tail -1` of that file → `echo "PASS: all 13 cases green."`; `git ls-files .claude/skills/task/scripts/` → empty, so nothing is committed]
  3. **RESOLVED — the round-6 brief lagged the spec because two agents ran in
     parallel.** Round 6 was briefed at "19 ACs" while `spec-writer` was still
     editing; `AC13b` was read at *"At minimum **four**"*, then *"five"*, then
     *"six"*, and closed at **eight**. The coordinator has confirmed this was a
     scheduling error, now corrected: the spec is byte-stable and no agent is
     editing it. **The durable lesson is kept, because the mitigation round 6
     chose was the wrong shape:** it put a prose hedge ("the count is moving")
     *next to* a hard-coded `>= 6`, and prose beside a number does not stop the
     number from being executed. Round 7 replaces every such pairing with a
     run-time derivation — see § *Deriving `AC13b`'s entry floor* and § *Figures
     that move*. The concurrent-edit window is what exposed this, but the defect
     was independent of it: any spec-side count transcribed into a gate decays.
     [measured: spec byte-stable at `50979` across three `stat` samples; `grep -oE 'At minimum [a-z]+'` -> `At minimum eight`; distinct `**(roman)**` markers in the `AC13b` row -> `8`; the derivation cross-check reports `AGREE -> floor=8`]
  4a. **RESOLVED in spec round 13, plus ONE new instance (round 8).** Both round-7
     items are fixed: entry (vi)'s *"sharpest of the six"* ordinal was **removed
     rather than refreshed** (zero ordinals remain in `AC13b`), and the
     standing-warning sentence regained its governing clause. **New, surfaced not
     fixed:** § *Key decisions* now says *"The **seven** `AC13b` entries"* while
     `AC13b` states "At minimum eight" and enumerates eight — the same
     transcribed-ordinal class, relocated from the AC to the rationale row that
     cites it. Nothing is blocked: the design's floor is derived, so it read `8`
     regardless. **Note the scope limit this exposes** — the derivation only
     cross-checks *inside* the `AC13b` row, so a stale count in a **different**
     section is outside what it can catch. That is the correct boundary (the floor
     is `AC13b`'s to own), but it means prose elsewhere still needs a human read.
     [measured: `grep -c 'sharpest of the six' …spec.md` → `0`; `grep -n 'seven \`AC13b\` entries' …spec.md` → `291`; the derived gate on the same spec → `word=eight num=8 enum=8 floor=8`]
  4. **Two NEW stale figures inside `AC13b` itself (round 7) — see 4a for their resolution.**
     Both are spec-side; neither blocks implementation.
     **(a)** Entry (vi) still reads *"This is the sharpest of **the six**"* after
     the list grew to eight — the same transcribed-count class this round is
     removing from the design, now present in the spec's own prose.
     **(b)** The standing-warning sentence lost its subject during an edit.
     `AC13b` now runs *"...or whether committed history is simply the right unit.
     **in some form:** a clean `rounds` trend is not evidence..."* — the leading
     *"The section must also preserve the standing warning"* is gone, leaving a
     dangling fragment. The **requirement is still inferable**, and this design
     implements it (clause (d) keeps the standing warning; the `AC13b` gate greps
     for `cannot report otherwise`), so nothing is blocked — but the AC text
     should be repaired by its owner.
     [measured: `sed -n '382p' ...spec.md | grep -c 'sharpest of the six'` -> `1`; same line `grep -c 'The section must also preserve'` -> `0` while `grep -c 'in some form:'` -> `1`]
