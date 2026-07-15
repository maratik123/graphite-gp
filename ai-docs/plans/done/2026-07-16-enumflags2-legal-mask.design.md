# Design: gp-core — adopt enumflags2 for legal_mask (typed 5-action bitflags)

**Issue:** [#51](https://github.com/maratik123/graphite-gp/issues/51)
**Date:** 2026-07-16
**Spec:** `ai-docs/plans/2026-07-16-enumflags2-legal-mask.spec.md`

## Approach

Replace `legal_mask`'s positional `[bool; 5]` return with a typed
`enumflags2::BitFlags<Action>`, by annotating the already-locked `sim::Action`
enum with `#[bitflags]` + `#[repr(u8)]`. The migration surface is tiny and fully
enumerated (4 code sites + the manifest, verified below), so this is a mechanical
type-swap with one correctness pivot (the `collect()` that builds the mask).

### Resolved open questions

**Q1 — bare `BitFlags<Action>` vs a named `LegalMask` newtype → bare `BitFlags<Action>`.**
A newtype would have to re-expose (delegate or `Deref`) `contains`, iteration,
the `|`/`&` operators, `empty()`, `PartialEq`, `Debug` — pure boilerplate — to
recover what `BitFlags<Action>` already gives for free, and there is **no domain
method to hang on it today** (`policy_action`, its only real consumer, is still a
`todo!` stub). Per AGENTS.md "Economy: YAGNI — no unnecessary abstractions" and
the spec's stated default, use the bare type. If a domain method ever earns its
keep, wrapping later is a clean, local change.

**Q2 — `#[repr]` int type → `#[repr(u8)]`.** 5 flags need 5 bits; `u8` (8 bits)
is the smallest sufficient unsigned int and enumflags2's natural default. The
`enumflags2::bitflags` macro requires an explicit integer `#[repr]` on its target
enum (`ai-docs/code-style.md` §*Enum repr*, case 1), and this is precisely the
first of the two sanctioned (anti-decorative) `#[repr]` cases — so `#[repr(u8)]`
satisfies AC3 by construction, not decoration.

**Q3 — manifest placement → direct in `crates/core/Cargo.toml [dependencies]`.**
`[workspace.dependencies]` currently holds **only internal path crates**
(`gp-core`/`gp-gen`/`gp-render`/`gp-ai`) — no external crate has been promoted
there yet. enumflags2 stays a **single-crate consumer** (see the re-export
decision below, which keeps `gp-ai` off a direct enumflags2 dep), so promoting to
the workspace table is premature (YAGNI). Declare it directly under `gp-core`.

### Key decision — re-export `BitFlags` from `gp-core`, don't add enumflags2 to `gp-ai`

`legal_mask` returns `BitFlags<Action>`, so that type becomes part of `gp-core`'s
**public API**. Rust API guideline C-REEXPORT: a crate should re-export the
foreign types that appear in its public signatures so consumers need not depend
on the underlying crate. Add a documented `pub use enumflags2::BitFlags;` in
`sim.rs`; `gp-ai::policy_action` then names `gp_core::sim::BitFlags` and requires
**no** direct enumflags2 dependency. This is a self-consistent decision cluster:
re-export ⇒ single consumer ⇒ direct-manifest placement (Q3).

### Migration surface (binding contract — verified `rg -U`)

| Site | File:line | Change |
|---|---|---|
| `legal_mask` definition | `crates/core/src/sim.rs:108` | return `[bool; 5]` → `BitFlags<Action>` |
| overflow test (i32::MAX) | `crates/core/src/sim.rs:218` | `== [false; 5]` → `== BitFlags::empty()` |
| overflow test (i32::MIN) | `crates/core/src/sim.rs:234` | `== [false; 5]` → `== BitFlags::empty()` |
| `policy_action` signature | `crates/ai/src/lib.rs:41` | `_mask: [bool; 5]` → `_mask: BitFlags<Action>` |

`rg -U -n '\[\s*(bool|true|false)\s*;\s*5\s*\]' --type rust` returns exactly these
4 lines and no others. No `gp-render` consumer exists (only a `render_frame`
`todo!`); no benches/examples exist; the only non-`.rs` `legal_mask` hits are
docs/spec prose, not code (verified 2026-07-16). AC4 ("no `[bool; 5]` mask type
remains") is satisfied by migrating exactly these 4.

### Rejected alternatives

- **`LegalMask` newtype** — rejected (Q1): boilerplate with no current domain method.
- **`gp-ai` imports `enumflags2::BitFlags` directly** — rejected: makes enumflags2
  a two-crate dep, undercutting Q3; the re-export is the idiomatic C-REEXPORT path.
- **Promote to `[workspace.dependencies]`** — rejected (Q3): single consumer; no
  external crate is in that table yet.
- **`bitflags` crate instead of `enumflags2`** — out of scope; the spec, the
  library survey (`ai-docs/library-survey.md`), and code-style §*Enum repr* all
  name `enumflags2` specifically (variant-typed flags over an existing enum).

## Decomposition

All five subtasks are **code** change-type. `Cargo.toml`/`Cargo.lock` are build
manifests for the Rust code and are implemented by the same (code) implementor;
they group with the `.rs` changes — the task touches **zero** instructions/harness
files, so the whole task is one homogeneous code group.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `enumflags2 = "0.7"` to `crates/core/Cargo.toml [dependencies]`; run `cargo update` then `cargo build` to refresh `Cargo.lock` and verify it resolves. (`0.7` pin per AGENTS.md § Dependency Versions — 0.x → pin minor only; spec verified max-stable `0.7.12` on 2026-07-16.) | `crates/core/Cargo.toml`, `Cargo.lock` | — |
| 2 | Annotate `sim::Action`: add attrs in order `#[bitflags]` → `#[repr(u8)]` → existing `#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]`; add `use enumflags2::bitflags;` and a `///`-documented `pub use enumflags2::BitFlags;` re-export. Leave variants without explicit discriminants (macro auto-assigns `1,2,4,8,16` in `ALL` order). Confirm `Action::ALL` (const) and `Action::accel()` (const fn) still compile unchanged. | `crates/core/src/sim.rs` | 1 |
| 3 | Change `legal_mask` return type to `BitFlags<Action>`, body to `Action::ALL.into_iter().filter(\|&a\| legal_move(d, s, a)).collect()` (preserves `ALL`/logit order). Update the two overflow tests' `assert_eq!(..., [false; 5])` → `assert_eq!(..., BitFlags::empty())`. Add one positive AC2 membership test (see Test Design). | `crates/core/src/sim.rs` | 2 |
| 4 | Update `gp-ai::policy_action` mask param `[bool; 5]` → `BitFlags<Action>`; add `BitFlags` to the `use gp_core::sim::{...}` import (no enumflags2 dep in `gp-ai`). Update the fn doc if it names the mask shape. | `crates/ai/src/lib.rs` | 2 |
| 5 | AC6 gate checkpoint: `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`; fix any lint/fmt/doc fallout (esp. the re-export doc — see Risks). | all changed files | 3, 4 |

M = 5 subtasks (≤ 15; no split needed). Subtask 4 depends on 2 (needs the
`gp_core::sim::BitFlags` re-export), not on 3 — `policy_action` is a stub that
nothing calls with `legal_mask`'s output, so its signature migrates independently
and each committed subtask leaves the workspace green.

## Handoff plan

Handoff grouping per `design.md` § Rules → handoff-grouping (a)–(h). M = 5 ≥ 1, so
this section is mandatory. All 5 subtasks are **code** change-type (`.rs` +
`Cargo.toml`/`Cargo.lock`), form a single dependency-ordered chain, and fit in one
group of ≤ 10 — so the minimum group count is **1**.

- **Group A (terminal)** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**,
  1M-token window, implemented via the `code-writer` subagent
  (`subagent_type="code-writer"`; its `model: sonnet` + `effort: medium` are
  frontmatter-pinned — no inline `model=`/effort override) — subtasks **1, 2, 3, 4, 5**
  (code change-type: `*.rs`, `Cargo.toml`, `Cargo.lock`). Size 5 (within `1..=10`;
  terminal group in range). Homogeneous (all code). Group-minimized: a single
  code change-type over one dependency chain ⇒ 1 group is the floor.
- **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) — the
  every-group handoff contract binds a reset at the start of **every** group,
  including the first and including this single-group design. No inter-group
  handoff follows (only one group). Group A completes Step 8 in its own
  `/context-reset` subagent.

Group count = 1 (≤ 4 default max; no user gate needed). The `design`,
`design-review`, `self-review`, and `spec-writer` subagents stay on Opus
regardless of the group's `sonnet`/`medium` implementor marker.

## Risks

- **`#[bitflags]` attribute ordering / discriminant collision.** The macro
  requires `#[bitflags]` outermost, an integer `#[repr]`, `Copy`, and **no**
  variant with value `0` (a flag must be a non-zero power of two). `Action` has no
  explicit discriminants (macro auto-assigns `1,2,4,8,16`) and already derives
  `Copy` — no collision. *Mitigation:* keep attrs in the order in subtask 2; the
  `cargo build` in subtask 1→3 catches any macro error immediately.
- **`accel()` / `ALL` semantics drift.** `#[bitflags]` changes variant
  *discriminant integers* (0..4 → 1,2,4,8,16), but `accel()` matches variant
  *identity* and `ALL` lists variant *values* — neither reads the discriminant as
  an integer, and there is no `as u8`/`from_u8` cast anywhere. *Mitigation:* no
  code change to `accel`/`ALL`; the existing overflow tests exercise both.
- **Re-export doc / intra-doc link gate.** The `RUSTDOCFLAGS="-D warnings"` doc
  gate denies broken intra-doc links. The `pub use enumflags2::BitFlags;` doc
  comment must **not** contain an intra-doc link that may fail to resolve — write
  it with a resolving `[`Action`]` link and refer to `enumflags2` in plain
  backticks (no `[enumflags2::BitFlags]` link). *Mitigation:* subtask 5 runs the
  doc gate explicitly; keep the doc-comment link-free except for `[`Action`]`.
- **Unused-dependency between subtasks.** After subtask 1 the dep is declared but
  unused until subtask 2. The workspace enables no `unused_crate_dependencies`
  lint (root `[workspace.lints]` has `missing_docs`, `broken_intra_doc_links`, and
  the clippy set), so `cargo build`/`-D warnings` stays green. *Mitigation:* none
  needed; verified against the root manifest.
- **`FromIterator` availability.** The `collect()` body relies on
  `BitFlags<Action>: FromIterator<Action>` (present in enumflags2 0.7). *Mitigation:*
  fallback is an explicit `fold(BitFlags::empty(), \|m, a\| m \| a)`; the
  code-writer picks whichever compiles under the resolved 0.7.x.
- **Live-version verification was offline.** The crates.io max-stable query
  returned null in this sandbox (network blocked); the spec's `0.7.12`-verified
  value stands and the manifest pins `0.7` (correct for any 0.7.x regardless of
  the exact patch). *Mitigation:* subtask 1's `cargo update` + `cargo build`
  resolves and locks the actual latest 0.7.x; re-verify live if network is up.

## Test Design

- **Location:** `crates/core/src/sim.rs` `#[cfg(test)] mod tests` (existing).
- **Migrated (subtask 3):** `..._i32_max_overflow...` and `..._i32_min_underflow...`
  — the per-action `assert!(!legal_move(...))` loop is unchanged (still tests
  `legal_move`); only the final `assert_eq!(legal_mask(&d, s), [false; 5])`
  becomes `assert_eq!(legal_mask(&d, s), BitFlags::empty())`. `BitFlags` resolves
  in the test module via `use super::*` (glob-imports `sim`'s `pub use` re-export);
  `empty()`'s `Action` type is inferred from the `BitFlags<Action>` return.
  Preserves the AC5 `i32::MAX`/`i32::MIN` no-panic guarantee (former sim AC4).
- **New — positive AC2 membership (subtask 3):** name e.g.
  `legal_mask_contains_exactly_the_legal_actions`. Build a `Corridor` carved (via
  `Corridor::set(p, true)`) into a shape that yields a **proper subset** of legal
  actions from a chosen interior `CarState` (some accelerations stay in `D`, some
  leave it). Assert, for each `a in Action::ALL`,
  `mask.contains(a) == legal_move(&d, s, a)`, and assert the mask is **neither
  empty nor `BitFlags::all()`** so the check is non-vacuous. This directly
  encodes AC2 ("contains exactly the actions for which `legal_move` is `true`",
  in `ALL` order) and guards the `collect()` (catches a dropped action or an
  inverted filter). Entry point: `legal_mask`. Fixture: a `Corridor` + one
  `CarState`; no other helpers.
- **`gp-ai` (subtask 4):** `policy_action` is a `todo!` stub — no test is added or
  required; the signature-type change is compile-checked by the workspace build.
- **Gates (subtask 5):** the full AC6 suite (`build`, `clippy --workspace
  --all-targets -D warnings`, `fmt --check`, `RUSTDOCFLAGS="-D warnings" doc`).

## Open questions

- None. The three spec-deferred design questions (bare `BitFlags` vs newtype;
  `#[repr]` int type; manifest placement) are resolved above (bare
  `BitFlags<Action>`; `#[repr(u8)]`; direct in `crates/core/Cargo.toml`), plus the
  `pub use` re-export decision that keeps enumflags2 a single-crate consumer.
