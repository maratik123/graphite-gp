# Delegation — the four-phase lifecycle

> Extracted from `AGENTS.md` § *Workflow*. That file keeps the binding rules; this page carries the mechanics and the incidents that produced them. **Read before any Subagent spawn that commits, edits protected files, or runs long.**

Delegation fails in the middle, not at the ends. Most instruction text covers phase 1 (decide to delegate) and phase 4 (read the result); phases 2 and 3 are where the observed failures actually land.

## Phase 1 — Fit: can this delegate really do this work?

Check the delegate's **charter** AND its **environment**. A step that says "delegate" is evidence about CAN, never about fit, and neither is a `tools:` grant.

- **Charter fit.** `code-writer` is a *code* implementor. A predominantly-prose diff (`.claude/**`, `ai-docs/**`, `*.md`) has no code to delegate — author it in-thread. Its cargo gates are also meaningless on a diff with no `.rs`.
- **Environment fit.** A *background* Subagent cannot answer an interactive or self-modification permission prompt. A protected-file edit therefore **fails closed** regardless of what `Edit(...)` allow-lists say. Apply those in-thread.

## Phase 2 — Hand-off state: leave the index CLEAN

**`git commit` captures the whole index, not just what the delegate's own `git add` added.** Anything you pre-staged — a `git mv`, a half-finished edit — lands in the delegate's first commit, attributed to its work and mixed into its diff.

Before delegating to a subagent that commits: either commit your own staged work first, or `git restore --staged <path>` to keep it working-tree-only.

## Phase 3 — While it runs: waiting is not stuck

A delegate that ends its turn to wait on a long job is **waiting**, not hung.

Do **not** start a parallel investigation of the same question. It duplicates the work and steals CPU from the very job you are impatient about — the two racing processes make each other slower, which reads as further confirmation that it is stuck.

Verify what it is actually doing (`ps`, its committed output) before judging. If it lacks information you already have, send it that information plus an explicit decision rule. Take over only after stopping it — and say so.

## Phase 4 — Take-over and return: the summary is a claim

**A subagent's RETURN SUMMARY is a claim, not a record.** After any group or subagent returns, verify every gate / PASS / "I did X" assertion against the **durable** record — the `.progress.md`, the commit body, `git log` / `git diff`, the file itself. When they disagree, trust the durable record, and close the gap in-thread (e.g. run `image-check` yourself if the progress file shows it was skipped).

This applies with extra force to a delegate that also writes shared files. Both of these have proven false against the durable record in this repo: a `code-writer`'s *"I appended a learning"*, and its *"golden verified by `image-check` → PASS"*.

Treat a delegate's stated **reason** for skipping a gate as suspect too — especially when its other observed behaviour contradicts it.

**If you revert and re-author its work by hand**, treat its touched-file list as a **checklist**. The sub-edits — propagations, sync-group siblings — are exactly what a manual redo drops. Run the `AGENTS.md` § *Propagation Rule* grep before commit, unconditionally.
