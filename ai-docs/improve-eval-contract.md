# `/improve` Step 6 — why the PARENT dispatches the evals

> Extracted from `.claude/agents/self-improve.md` § Step 6. That file keeps the operative instruction; this page carries the provenance and the failure modes. Read when tempted to dispatch the reproducers from inside the Subagent, or to substitute a cheaper path.

## The contract is a MAY rule, not a CAN rule

`Agent` **is** present and callable from the `self-improve` Subagent class **in this project**. Probed 2026-07-17: a live `Agent(subagent_type: "general-purpose", …)` dispatch from inside a `self-improve` spawn launched and returned `PROBE_OK` intact.

So the prior claim — *"structurally unfulfillable; the runtime tool exposure genuinely lacks `Agent`"* — is **false here as of 2026-07-17**. **But it was true where it was written.** The sibling **quartzite** project recorded it with evidence in `maratik123/quartzite#364` and its matching 2026-05-15 tooling entry (*"the missing primitive is real … structurally unfulfillable by the subagent itself"*), after first falsifying the opposite hypothesis.

**The runtime changed between that finding and this one. The claim was not fabricated — it expired.** Re-probe rather than trusting either date.

Observed mechanism, for whoever probes next: the dispatch is **async**. It returns `Async agent launched successfully` with a task id, then delivers the result via a later notification — so a probe expecting a blocking call-and-return can misread a successful launch as a failure.

## Why the parent owns it anyway

Do not re-derive this from your tool list. **A capability grant is evidence about CAN and says nothing about MAY** (`.claude/agents/design.md` § Quality checklist → Constraints).

The parent thread owns the eval because it owns the **user-facing report**: Step 6's verdict is addressed to the user, and this Subagent's contract is *analyse and propose*, not *adjudicate and report*. That reason is independent of what your tool list contains — which is exactly why it survived the capability claim turning out to be wrong.

If you believe the parent-dispatch contract is wrong, **say so in your report** and let the user decide. Do not resolve it by acting.

## The forbidden degraded paths — each on its own merits

You have `Agent`; do not use it for Step 6. And do **not** substitute any of these:

- a `Bash`-shelled invocation,
- `TaskCreate`-then-`TaskOutput` polling,
- an in-memory close-read.

None of them runs the reproducer in a **clean context**, which is the entire point of the eval. A same-context "close-read" grades the reproducer against the very transcript that authored the rule.

Authority: `maratik123/quartzite#362` Commit C (*"record eval-degradation pattern"*) and quartzite's 2026-05-15 process entry recording this Subagent silently degrading Step 6 from clean-context evals to same-context close-reads. Verify with `gh pr view 362 --repo maratik123/quartzite` — a bare `gh pr view 362` resolves against **this** repo and will falsely report *Could not resolve*. The rule stands on the clean-context requirement regardless of that citation.

## Leaking the grader defeats it just as surely

Emitting the reproducer as one block, or copying the GRADER block into the dispatch, destroys the same clean-context property — an eval that shows the agent its own expected answer measures nothing, and returns a near-guaranteed PASS that is statistically independent of rule strength. See [`ai-docs/templates/improve-eval-reproducer.md`](templates/improve-eval-reproducer.md) for the SUBJECT / GRADER split.

## Propagation-rule asymmetry

The Learning-Log sync-group sister file `.claude/agents/learnings-escalation-audit.md` has **no** Step 6 eval-phase equivalent — its workflow is a passive auditor and its `Step 6 — Report` is structured output, not a primitive-dispatch step. This contract therefore requires no mirrored edit there.
