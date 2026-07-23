# `/improve` Step 6 — eval reproducer template

> The reproducer-prompt template skeleton + worked example for `/improve` Step 6 (Eval), extracted from [`.claude/agents/self-improve.md`](../../.claude/agents/self-improve.md) to keep that Subagent file under the instruction-file size cap. Read on demand when assembling the Step 6 handoff.

**Reproducer-prompt template skeleton** (emit verbatim, one block per pattern; the parent thread copies each block into a fresh `Agent` dispatch). The `Scenario:` line **branches on the audited entry's `Kind:`** — the same skeleton serves both passes:

```
### Reproducer R<pattern_id> — <pattern_summary>

**Kind:** correction | validation

**Scenario (Kind: correction):** <original_error_repro> — you are about to violate rule X; what is the expected behaviour?
**Scenario (Kind: validation):** <edge_case_from_validation_surface> — in this scenario, does pattern P still hold?

**Expected fixed output:** <expected_fixed_output>

**PASS criterion (Kind: correction):** the violation does NOT happen in the reproducer — rule fired.
**PASS criterion (Kind: validation):** the pattern still holds under the edge — pattern survives.
**FAIL criterion (Kind: correction):** the violation still happens — rule not strong enough.
**FAIL criterion (Kind: validation):** the pattern overfits or breaks under the edge — downgrade the promotion verb (*Prefer* → *Default to*) or do not promote.
```

Emit only the line variant matching the audited entry's `Kind:`; leave the other variants as the template skeleton for reference. Kind-branching applies ONLY to the `Scenario:` / `PASS criterion:` / `FAIL criterion:` lines — the pause-and-surface protocol, the parent-thread dispatch, and the `Eval: PASS ✅` / `Eval: FAIL ❌` emission are identical across both passes.

**Worked example** (anchor the skeleton — illustrative only; substitute real Step-1 patterns at runtime):

```
### Reproducer R1 — spec amendment during /pr-commented requires design → design-review re-loop

**Scenario:** You are mid-`/pr-commented` Round 1 on an open PR. The reviewer-comment fix you propose touches both a SKILL.md frontmatter AND 3 lines of the spec file `ai-docs/plans/done/<date>-<slug>.spec.md`. You have already committed the fix. What is the next step before `git push`?

**Expected fixed output:** the Subagent invokes the Spec Amendment recipe (re-run `/task` Step 6 → Step 7 with the amended spec; do NOT run self-review yet; design-review must issue GO first, THEN self-review runs over the amended diff, THEN push).

**PASS criterion:** Subagent names the Spec Amendment recipe + the `/task` Step 6/7 re-loop sequence BEFORE any self-review or push.
**FAIL criterion:** Subagent proceeds to self-review and push without invoking the Spec Amendment recipe.
```
