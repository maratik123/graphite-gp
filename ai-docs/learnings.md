# Learnings

Append-only correction/validation log — the feed for `/improve`. See AGENTS.md § Learning Log for the rules (append-only; no escalation in the same turn) and the entry format, and [`corrections-log.md`](corrections-log.md) for the field glossary + forbidden skip-reasoning.

<!-- Append new entries below. NEVER edit, reorder, summarise, or delete an existing entry. -->

### 2026-07-13 — tooling — workspace `cargo fmt` reformatted out-of-scope sibling crates
**What happened:** Under a "geom.rs only" implementation scope (`/task` Step 8, Group A), I ran workspace-wide `cargo fmt`; it reformatted two pre-existing-unformatted files outside my scope — `crates/core/src/sim.rs` (import order, same crate as geom.rs) and `crates/ai/src/lib.rs` (signature wrap). I reverted both with `git restore`; neither was staged or committed.
**Rule:** When scoped to a single file in a multi-crate workspace, do NOT run workspace `cargo fmt` — it silently touches sibling crates. Write the target file already-formatted and VERIFY with `cargo fmt --check`, confirming the target file is ABSENT from the diff (pre-existing out-of-scope offenders may still appear, which is expected). `cargo fmt -p <crate>` is insufficient when the offender shares the target's crate.
**Kind:** correction
**Escalated?** no

### 2026-07-15 — tooling — asserted `cargo test --keep-going` support without verifying
**What happened:** Briefing the design + impl subagents for the `CARGO_BUILD_WARNINGS=deny` CI amendment (`/task` Step 8), I asserted from memory that "cargo test supports `--keep-going` (both build and test/docs support it)". It does NOT — `cargo test` has no `--keep-going` on cargo 1.97.0 (the pinned MSRV / CI floor); it errors `unexpected argument '--keep-going'`. The Group B impl subagent hit this, correctly STOPPED and reverted `ci.yml` rather than shipping a red `test` job. Correct idiom (cargo-test docs): `cargo build --tests --keep-going` + `cargo test --tests --no-fail-fast`. `--keep-going` is valid on `cargo build`/`cargo doc` only.
**Rule:** Before asserting an external tool's flag/capability in a spec/design/subagent brief, VERIFY it — run `cargo <cmd> --help` or the command itself, or read the offline docs at `~/.rustup/toolchains/stable-*/share/doc/`. Extends AGENTS.md § Dependency Versions ("query live state BEFORE asserting") from dependency VERSIONS to tool FLAGS/behavior.
**Kind:** correction
**Escalated?** no
