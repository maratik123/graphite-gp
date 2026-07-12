# Learnings

Append-only correction/validation log — the feed for `/improve`. See AGENTS.md § Learning Log for the rules (append-only; no escalation in the same turn) and the entry format, and [`corrections-log.md`](corrections-log.md) for the field glossary + forbidden skip-reasoning.

<!-- Append new entries below. NEVER edit, reorder, summarise, or delete an existing entry. -->

### 2026-07-13 — tooling — workspace `cargo fmt` reformatted out-of-scope sibling crates
**What happened:** Under a "geom.rs only" implementation scope (`/task` Step 8, Group A), I ran workspace-wide `cargo fmt`; it reformatted two pre-existing-unformatted files outside my scope — `crates/core/src/sim.rs` (import order, same crate as geom.rs) and `crates/ai/src/lib.rs` (signature wrap). I reverted both with `git restore`; neither was staged or committed.
**Rule:** When scoped to a single file in a multi-crate workspace, do NOT run workspace `cargo fmt` — it silently touches sibling crates. Write the target file already-formatted and VERIFY with `cargo fmt --check`, confirming the target file is ABSENT from the diff (pre-existing out-of-scope offenders may still appear, which is expected). `cargo fmt -p <crate>` is insufficient when the offender shares the target's crate.
**Kind:** correction
**Escalated?** no
