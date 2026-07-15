# Design: Code-writing subagent with frontmatter-pinned effort

**Issue:** none — internal harness task (user opted out of a tracking issue)
**Date:** 2026-07-16
**Spec:** `ai-docs/plans/2026-07-16-code-writer-subagent-effort.spec.md`

## Approach

Create one file-based implementor subagent, `.claude/agents/code-writer.md`, whose
frontmatter pins `model: sonnet` + `effort: medium` (`tools` omitted → inherit-all),
then re-route every code-writing spawn that today runs on an *inline* `general-purpose`
`model="sonnet"` override to `subagent_type="code-writer"`. Because there is **no
per-invocation `effort` parameter** on the Agent/Task tool (Technical constraints:
model-resolution order is env → per-invocation `model` → frontmatter, and effort has no
per-invocation slot), frontmatter is the *only* lever that can pin effort. The current
inline spawn therefore cannot enforce "effort medium (pinned)" — the whole point of this
task is to move that pin into a file where the harness honours it.

**Why `subagent_type="code-writer"` (not `general-purpose` + "read code-writer.md").**
Only naming the subagent type activates its frontmatter `model`/`effort`. A
`general-purpose` spawn that merely reads the file in its prompt would still resolve model
via `general-purpose`'s own rules and could not pin effort at all — defeating the task.
Every routed spawn in this design is `subagent_type="code-writer"` with the inline
`model=`/effort override **dropped**.

**Two-mode body (design-phase item 2).** `code-writer` is spawned from two contexts, so
its body defines two modes selected by the spawn prompt:

- **Mode A — `/task` group-implementor** (spawned by `/context-reset` Handoff-protocol
  step 3 for a **code** group). Prompt shape: `"Read ai-docs/plans/<name>.progress.md and
  complete Group <X>'s subtasks <N>–<M>, then return"`. Contract (the current
  `general-purpose` implementor contract, unchanged): read the progress file end-to-end,
  do the group's subtasks sequentially in-context, run `cargo` gates + **`git commit`
  after each subtask**, update `.progress.md` at each subtask boundary, return.
- **Mode B — single-fix delegate** (spawned by `/bugfix`, `/main-ci-failed`,
  `/pr-ci-failed`, `/pr-commented`). Prompt shape: `"Single-fix delegate mode. Author the
  fix for: <fix intent/target + failing-test / root-cause context>. Run these gates:
  <list>. Do NOT commit; return a summary of edits + gate results."` Contract: `code-writer`
  **AUTHORS** the concrete edits from the orchestrator's planned fix intent (mirroring
  Mode A's write-the-code contract) — it does **NOT** transcribe a finished, pre-written
  diff. Transcription is pure latency overhead (the same anti-pattern this design rejects
  for mechanical `fmt`/`clippy` fixes — item 3 below) and would waste the pinned
  sonnet/medium *reasoning* tier the delegation exists to carry. The orchestrator supplies
  the fix intent/target + the failing-test / root-cause context, **not** a finished diff;
  `code-writer` stays within that target — no scope expansion — runs the named gates, and
  **returns WITHOUT committing**. The **orchestrator** owns self-review and the commit/push,
  so the fix can pass self-review *before* it is committed.

In **both** modes `code-writer` NEVER runs `self-review` and NEVER pushes; the only
behavioural difference is Mode A commits per subtask while Mode B commits nothing. The
mode contract lives once in `code-writer.md`; each spawner passes a thin, mode-selecting
prompt (DRY — no per-skill duplication of the contract text; satisfies design.md § Rules
≥3-site duplication, since the substantive contract sits in a single file that 5 spawn
sites reference).

**Delegation boundary (all four skills, Mode B).** Orchestrator keeps trace/root-cause,
classification, planning, self-review, thread-resolution, and commit/push; `code-writer`
writes only the fix code. This is a homogeneous, reused pattern across the four
`.claude/skills/**` conversions.

**Mechanical inline-fix-class routing — recommendation (design-phase item 3 / spec Open
question).** In `/main-ci-failed` and `/pr-ci-failed` Step 4, keep the existing
class split and refine it:

- **Mechanical single-command fixes STAY INLINE** with the orchestrator: `fmt`
  (`cargo fmt`), auto-`--fix`-able `clippy`, an `actionlint`-guided YAML one-liner, a
  `doc` typo. Rationale: (1) spawning a sonnet subagent to run `cargo fmt` is pure
  latency + context overhead with zero code-writing; (2) the fix is deterministic and
  needs no reasoning — `code-writer`'s value (a pinned sonnet/medium *reasoning* tier)
  is realised only when there is genuine code to write; (3) it keeps the boundary crisp
  and preserves today's behaviour for the common CI-red case (most reds are fmt/clippy).
- **Substantive code-writing DELEGATES to `code-writer`** (Mode B): a `clippy`/`doc` fix
  that needs a real code change (logic restructure to satisfy a lint, rewriting a broken
  doctest) — i.e. the non-mechanical subset of the current "Inline-fix classes" bucket.
- **`test` / `build` genuine regressions** continue to route to `/bugfix` (unchanged),
  which now reaches `code-writer` transitively via its own Step-5 conversion (AC5). This
  design does NOT change the test/build→`/bugfix` delegation.

**Q3 (opus/harness path) stays inline** (spec Key decision) — an instructions/harness
group keeps `Agent(subagent_type="general-purpose", model="opus", …)` with inherited
effort. No second frontmatter file: agent-file `model` is static and cannot vary per
group, so the opus path — which just needs a per-spawn `model="opus"` and inherited
effort — is correctly expressed inline. Only the code path (which additionally needs a
*pinned effort* the inline site cannot provide) moves to a file.

**Rejected alternatives.**
- *Two implementor files (`implementor-sonnet` / `implementor-opus`).* Rejected: the opus
  path needs no effort pin, so a file buys nothing there; duplicates the "read progress,
  run subtasks, commit each" contract. (Matches the prior `library-survey` design's
  rationale for keeping the spawn inline — the new fact that flips it *only for the code
  path* is the effort-pin requirement.)
- *Curate a restricted `tools` allowlist for `code-writer`.* Rejected for now (spec Open
  question, defaulted): inherit-all preserves the current `general-purpose` `*` surface;
  a denied-`Agent` allowlist to stop re-delegation is a future refinement, not needed for
  behaviour-preservation.
- *Route mechanical `fmt`/`clippy --fix` through `code-writer`.* Rejected — see item 3
  above.

### AC9 — `allowed-tools` gating (design-phase item 1): NO edit needed (verified)

Each of the four skills **already** spawns `Agent` subagents with its current
`allowed-tools` frontmatter (which lists only `Bash(...)` patterns, no `Agent`/`Task`):

| Skill | `allowed-tools` (Agent listed?) | Existing Agent spawn today |
|---|---|---|
| `/bugfix` | line 5 — Bash-only, **no** Agent | Step 1 `Agent(subagent_type="Explore")` (`:62`); Step 6.5 `Agent(subagent_type="general-purpose")` self-review (`:235`) |
| `/main-ci-failed` | line 5 — Bash+gh only, **no** Agent | Step 5 spawns `self-review` (`:224`); Step 4 delegates to `/bugfix` (Skill) |
| `/pr-ci-failed` | line 5 — Bash+gh only, **no** Agent | Step 5 spawns `self-review` (`:229`); Step 4 delegates to `/bugfix` |
| `/pr-commented` | line 5 — Bash+gh only, **no** Agent | Step 5 spawns `self-review` (`:209`); Patterns spawn `design` |

The `Agent`/`Task` tool is provably **not gated** by these skills' `allowed-tools` (if it
were, their existing `self-review`/`Explore` spawns would already fail).
`.claude/settings.json` `permissions.allow` likewise contains no `Agent`/`Task` entry yet
these spawns run in production. **Conclusion:** adding a new `subagent_type="code-writer"`
spawn requires **no** `allowed-tools` change to any of the four skills. AC9 is satisfied by
verification, not by an edit — the spec's conditional wording ("*where* that frontmatter
gates the spawn") resolves to "it does not gate it." (Fallback, not needed now: if a
future harness ever gates `Agent`, the fix is one `Agent` token added to each
`allowed-tools` — a separate concern.)

## Decomposition

All subtasks are **instructions/harness** change-type (edit only `.claude/**` +
`ai-docs/**`; no `*.rs`). AC9 and AC12 are verification-only (no edit) and are covered in
Test Design.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Create `code-writer.md`** (AC1, AC12). Frontmatter `model: sonnet`, `effort: medium`, `tools` omitted (inherit-all); body defines Mode A (group-implementor: read `.progress.md`, do subtasks sequentially, gate + `git commit` per subtask, return) and Mode B (single-fix delegate: **author** the concrete edits from the orchestrator's fix intent/target + failing-test / root-cause context — NOT transcribe a finished diff — gate, return WITHOUT committing). Body states: mode selected by spawn prompt; never runs self-review; never pushes; Mode B never commits. Confirm name non-clashing (AC12). | `.claude/agents/code-writer.md` | — |
| 2 | **Route `/task` code-group spawn** (AC2). `/context-reset` Handoff-protocol step 3 (`:40–43`): code branch (`:41`) → `Agent(subagent_type="code-writer", …)`, drop inline `model="sonnet"`, state model+effort are frontmatter-pinned; opus branch (`:42`) unchanged; fix the `:40` lead-in ("spawn the group's implementor") and the `:43` closing sentence (only the instructions/harness spawn takes an inline `model=` override; quality gates stay Opus). | `.claude/skills/context-reset/SKILL.md` | 1 |
| 3 | **Update `/task` Step-8 handoff prose** (AC3). `:163` "Per-group implementor model selection" — code group (marked `code-writer`) → `subagent_type="code-writer"`, model+effort frontmatter-pinned (no inline `model=`/effort); instructions/harness group (marked `opus`) → `general-purpose`+`model="opus"`+inherited effort. Remove any wording implying an unenforceable inline `effort medium pinned` on a `general-purpose` code spawn. Keep orchestrator-unchanged + quality-gates-stay-Opus clauses. (Task/Design sync pair with #2.) | `.claude/skills/task/SKILL.md` | 1 |
| 4 | **Update `design` ↔ `design-review` markers** (AC4). `design.md` code-group marker prose (`:67`, `:71` unchanged instr example, `:73`, `:77`, `:113` (f) example tags, `:114` (g) definition) + `design-review.md` marker-check (`:34`): code group → implementor **`code-writer`** (frontmatter sonnet/medium)/1M; instructions/harness → `opus`/inherited/1M (unchanged). Edit BOTH files in this subtask (design↔design-review sync group — must not drift). | `.claude/agents/design.md`, `.claude/agents/design-review.md` | 1 |
| 5 | **Convert `/bugfix` Step 5 (Fix)** (AC5). `:200–208` — delegate the fix's code-writing to `Agent(subagent_type="code-writer", …)` in Mode B: `code-writer` **authors** the concrete edits from the Step-4-planned fix intent/target + the failing-test / root-cause context (it does NOT transcribe a finished diff), runs the gates, returns without committing. Steps 1–4 (Reproduce/Trace, Root Cause, Failing Test, Plan), Step 6 (Verify), Step 6.5 (self-review), Step 7 (cleanup) stay with the orchestrator. **Step 5's bail rules stay ORCHESTRATOR-side** — the **One-file rule** (`>3` files touched → STOP, back to Step 2) and the **One-attempt rule** (a new bug appeared in the same place after the fix → STOP, draw a full system diagram, show the user) are user-facing control flow the orchestrator retains as part of its planning; `code-writer`'s Mode-B return surfaces the "new bug in the same place" signal back to the orchestrator, which then applies the bail. Do NOT push the bail logic into `code-writer`'s prompt. (Spec's "Step 6 (self-review)" maps to the skill's Step 6.5.) | `.claude/skills/bugfix/SKILL.md` | 1 |
| 6 | **Convert `/main-ci-failed` Step 4** (AC6). `:189–218` — substantive code-writing → `code-writer` (Mode B); mechanical single-command lint fixes (`fmt`/auto-`clippy`/`doc` typo/`actionlint` one-liner) stay inline; `test`/`build` regressions stay routed to `/bugfix`. Classification, reproduction, Step-5 self-review, branch/commit/push/PR orchestration stay with the orchestrator. | `.claude/skills/main-ci-failed/SKILL.md` | 1 |
| 7 | **Convert `/pr-ci-failed` Step 4** (AC7). `:209–223` — same contract as #6. | `.claude/skills/pr-ci-failed/SKILL.md` | 1 |
| 8 | **Convert `/pr-commented` Step 4 (Fix)** (AC8). `:178–205` — delegate `fix`-classified code-writing to `code-writer` (Mode B: apply all `fix` edits, gate, return without committing); orchestrator then does the single commit. Comment classification (Steps 2–3), architectural-bail routing (Step 3), Step-5 self-review, single-commit/push (Step 6) stay with the orchestrator. | `.claude/skills/pr-commented/SKILL.md` | 1 |
| 9 | **Update `claude-tools-hierarchy.md`** (AC10). Add a `code-writer` row to the Subagents table (`:6–15`) listing all spawners (`/task` via `/context-reset` code group; `/bugfix` Step 5; `/main-ci-failed` Step 4; `/pr-ci-failed` Step 4; `/pr-commented` Step 4) + its two-mode role. Update the PART-5 note (`:31`): code group → `subagent_type="code-writer"` (frontmatter sonnet+medium, no inline override); instructions/harness group → `general-purpose`+`model="opus"`+inherited; orchestrator unchanged; quality gates stay Opus. | `ai-docs/claude-tools-hierarchy.md` | 1–8 |
| 10 | **Propagation-grep verification + cleanup** (AC11, and AC9/AC12 confirmation). Run the AC11 grep over the **live** scope only (see Test Design). Confirm no stale references to the old inline-`model="sonnet"` / unenforceable-`effort medium pinned` code-group contract remain in live instruction files; `general-purpose`/`medium`/`sonnet` retained only where legitimate: the opus branch, self-review spawns, `code-writer` frontmatter, marker shorthand, **and the quality-gate/analysis `general-purpose` spawns this task does NOT route** — `.claude/skills/interview/SKILL.md:122` (spec-writer cold-spawn), `.claude/skills/ai-audit/SKILL.md:44` (learnings-escalation-audit spawn), `.claude/skills/project-review/SKILL.md:67` (review-findings spawn) and `:115` (self-review spawn). These four sites MUST stay `general-purpose`; enumerate them so the `general-purpose` grep has zero ambiguity. Do NOT edit historical records (see Risks). | (verification; edits only if a stray live reference is found) | 1–9 |

M = 10 subtasks.

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. This task is **homogeneous
instructions/harness change-type** (every subtask edits `.claude/**` or `ai-docs/**`; no
`*.rs`), so per PART-5 its implementor group(s) are marked **`opus` / effort inherited**
(NOT `sonnet`). All 10 subtasks are the same change-type and fit within the size cap
(10 ≤ 10), so group-minimization (f) yields the FEWEST possible groups: **one**.

- **Group A** — model `opus`, effort inherited from the orchestrator (typically xHigh) —
  **NOT** pinned, 1M-token window — subtasks 1–10 (instructions/harness change-type:
  `.claude/**`, `ai-docs/**`). Ordered so `code-writer.md` is created first (subtask 1,
  the shared reference), the two sync pairs are adjacent (2–3 context-reset↔task; 4
  design↔design-review edited together in one subtask), the hierarchy doc reflects the
  final contract (9), and the propagation grep runs last (10). Terminal group (10
  subtasks; within the `1..=10` range). This is the first **and** only group — `/task`
  Step 8 spawns `/context-reset` per `.claude/skills/context-reset/SKILL.md`
  § Compaction recovery (re-entry) at the start of Group A. No inter-group handoff
  (single group). The single group completes Step 8 in its own `/context-reset` subagent.

Group count = 1 (≤ 4; no user approval needed). The `design`, `design-review`,
`self-review`, and `spec-writer` quality-gate subagents stay on Opus regardless of the
group marker.

## Risks

- **AC11 grep over-reach into historical records (highest risk).** `grep -rn` for the
  changed keywords (`model="sonnet"`, `effort medium`, code-group markers) also matches
  files that legitimately quote the OLD contract and MUST NOT be edited:
  - `ai-docs/plans/done/2026-07-15-library-survey-workflow-tuning.spec.md` / `.design.md`
    and `.../2026-07-15-integer-safety-audit.design.md` — completed plan records
    describing what those PRs did at the time. Editing them falsifies history (append-only
    ethos; `done/` is immutable).
  - `ai-docs/plans/INDEX.md:10` — the library-survey changelog row (historical summary).
  - `ai-docs/plans/2026-07-16-code-writer-subagent-effort.spec.md` / `.design.md` — THIS
    task's own spec/design, which deliberately quote the old contract to describe the
    change.
  - `ai-docs/instruction-file-validation.md:474` (`model = "sonnet"`) — a spawn snippet
    in the dual-model instruction-clarity *validation harness*, a different-purpose spawn,
    not the `/task` code-group implementor. Verify and leave unless it is in fact the
    code-group spawn.
  - `.claude/skills/task/reference.md` — its `general-purpose` snippets (`:83/93/118/128`)
    are `design`/`design-review`/`spec-writer` re-spawns (quality gates — stay
    `general-purpose`); its § Every-group handoff (`:212–223`) does not restate the
    code-group spawn contract; `:219` "sonnet-model orchestrator" is a historical incident
    narrative about the *orchestrator*, not the implementor spawn. **OUT of scope.**
  **Mitigation:** subtask 10 scopes the grep to the LIVE targets only (the 5 spec-named
  files); Test Design pins the exact IN/OUT file list; the implementor treats a match in
  any OUT-of-scope file as expected, not a defect.
- **design.md self-reference.** Subtask 4 edits the very marking contract this design's
  Handoff plan uses. No circularity: this task has no code group (it is
  instructions/harness), so its own `opus`/inherited marker is unaffected by the
  code-group marker edit; the edit changes only how FUTURE code groups are marked.
- **Mode ambiguity in `code-writer` body.** If the body under-specifies mode selection,
  a Mode-B spawn could wrongly commit (Mode-A behaviour). **Mitigation:** the body states
  the mode is chosen by the spawn prompt and makes the commit/no-commit distinction the
  first, explicit rule of each mode; each spawner's prompt names the mode.
- **Sync-group drift.** context-reset↔task (2,3) and design↔design-review (4) must land in
  the same PR. **Mitigation:** all in Group A / one PR; #4 edits both files atomically.
- **AXIOM — instruction-file 40k-char cap.** `code-writer.md` is a new small file (well
  under cap); the edits to the five existing files are net near-zero (swap inline override
  for a subagent name). **Mitigation:** subtask 10 runs `wc -c` on any file that grew
  materially; none is expected near 35k.

## Test Design

No `*.rs`; verification is grep / `wc -c` / file-exists / manual read. Run after Group A.

**Per-AC checks:**

- **AC1** — `test -f .claude/agents/code-writer.md`; frontmatter contains exactly
  `model: sonnet` and `effort: medium` (`grep -E '^(model|effort):'`), and **no** `tools:`
  line (`! grep -q '^tools:'`). Read the body: Mode A block (read `.progress.md`,
  sequential subtasks, gate + `git commit` per subtask) AND Mode B block (apply fix, gate,
  return WITHOUT committing) both present; body states never-self-review / never-push.
- **AC2** — `grep -n 'subagent_type="code-writer"' .claude/skills/context-reset/SKILL.md`
  in the step-3 code branch; confirm `model="sonnet"` no longer appears on that branch
  (`! grep -q 'model="sonnet"'`); opus branch still reads
  `subagent_type="general-purpose", model="opus"` with effort NOT set.
- **AC3** — `grep -n 'code-writer' .claude/skills/task/SKILL.md` at `:163`; confirm no
  residual `model="sonnet"`/`effort medium pinned` on a `general-purpose` *code* spawn;
  orchestrator-unchanged + gates-stay-Opus clauses intact.
- **AC4** — `grep -n 'code-writer' .claude/agents/design.md .claude/agents/design-review.md`;
  both describe the code group's implementor as `code-writer`; instructions/harness marker
  still `opus`. Confirm both files changed in the same subtask/commit (sync group).
- **AC5** — `grep -n 'subagent_type="code-writer"' .claude/skills/bugfix/SKILL.md` inside
  Step 5; Steps 1–4, 6, 6.5, 7 unchanged in ownership (orchestrator).
- **AC6 / AC7** — `grep -n 'code-writer' .claude/skills/main-ci-failed/SKILL.md`
  `.../pr-ci-failed/SKILL.md` inside Step 4; mechanical `fmt`/`clippy`/`doc`/`actionlint`
  single-command path still described as inline; `test`/`build`→`/bugfix` unchanged.
- **AC8** — `grep -n 'code-writer' .claude/skills/pr-commented/SKILL.md` inside Step 4;
  single commit still orchestrator-owned; Steps 2–3 classification/bail + Step 5
  self-review + Step 6 push/resolve unchanged.
- **AC9** — confirm **no** `allowed-tools` change in the four skills' frontmatter
  (`git diff` shows line 5 of each unchanged); evidence table above proves each already
  spawns `Agent`. Verification-only.
- **AC10** — `grep -n 'code-writer' ai-docs/claude-tools-hierarchy.md`: Subagents table
  row present with all five spawners; PART-5 note (`:31`) reflects the routed contract.
- **AC11 (scoped grep)** — run:
  `grep -rn 'model="sonnet"\|effort medium pinned' .claude/ AGENTS.md`,
  `grep -rn 'code-writer\|model="sonnet"' .claude/agents/ .claude/skills/ ai-docs/claude-tools-hierarchy.md`,
  and (routing sweep for the `general-purpose` keyword)
  `grep -rn 'general-purpose' .claude/skills/ .claude/agents/`.
  **IN scope (must be clean/updated):** `.claude/skills/context-reset/SKILL.md`,
  `.claude/skills/task/SKILL.md`, `.claude/agents/design.md`,
  `.claude/agents/design-review.md`, `ai-docs/claude-tools-hierarchy.md`, plus the four
  converted skills. **OUT of scope (matches expected — do NOT edit):**
  `ai-docs/plans/done/**`, `ai-docs/plans/INDEX.md`, this task's own `*.spec.md`/
  `*.design.md`, `ai-docs/instruction-file-validation.md`, `.claude/skills/task/reference.md`,
  and the quality-gate/analysis `general-purpose` spawns this task does NOT route —
  `.claude/skills/interview/SKILL.md:122` (spec-writer cold-spawn),
  `.claude/skills/ai-audit/SKILL.md:44` (learnings-escalation-audit spawn),
  `.claude/skills/project-review/SKILL.md:67` (review-findings spawn) and `:115`
  (self-review spawn). Confirm both named sync groups updated in the same PR.
- **AC12** — `ls .claude/agents/` shows no prior `code-writer`; not a built-in
  (`general-purpose`/`Explore`/`Plan`/`fork`/`general-purpose`) or existing agent
  (`spec-writer`/`design`/`design-review`/`self-review`/`review-findings`/`self-improve`/
  `learnings-escalation-audit`/`triage-runner`). Verified at design time; re-confirm.
- **Char-cap AXIOM** — `wc -c` on any of the six touched instruction files that grew;
  expect all < 35,000.

## Open questions

- None design-blocking. The two spec Open questions are resolved in this design:
  mechanical inline-fix classes **stay inline** (Approach → item 3); `tools` frontmatter
  **stays inherit-all** (Approach → rejected alternatives). Both are the spec defaults,
  confirmed with justification.
