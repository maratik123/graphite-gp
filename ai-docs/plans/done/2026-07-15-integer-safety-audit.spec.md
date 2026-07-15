# Integer overflow- and signedness-safety audit

**Source:** issue #48
**Date:** 2026-07-15
**Tracked in:** #48

## Scope

Sweep every integer arithmetic operation and type conversion in the workspace's
**production** code and bring each into compliance with the overflow/signedness
rule (`ai-docs/learnings.md`, 2026-07-15 code-style entry, landed via PR #47).
For each site, either:

- **(a) convert** to an explicit-semantics form matching intent — `checked_*` →
  `Option`; `saturating_*` → clamp; `wrapping_*`/`overflowing_*` → modular;
  `strict_*` → always-panic invariant; `abs_diff` → unsigned magnitude;
  `carrying_*`/`borrowing_*` → multi-word; or widen to a strictly wider integer
  type (`i64`/`i128`) where that provably prevents overflow; **or**
- **(b) retain the raw op**, but only where operands are knowingly bounded so no
  overflow / signedness issue / division-by-zero can occur — with a doc-comment
  stating the domain bound **and** a test that exercises that bound. `supercover`'s
  bounded-chord precondition is the model pattern.

Type conversions: prefer `usize::try_from(i32)?` / `u32::try_from(...)` over `as`
casts wherever an out-of-domain input could overflow or lose sign; no
`#[allow(clippy::cast_sign_loss)]` / `cast_possible_truncation` without a `reason`.

**Enforcement deliverable (round-2 decision):** add `arithmetic_side_effects =
"deny"` to the root `[workspace.lints.clippy]` table and annotate every
deliberately-retained raw op with a justified
`#[allow(clippy::arithmetic_side_effects, reason = "...")]`, so the rule is
mechanically enforced across every crate from now on.

**Substantive target = `gp-core`** (`crates/core`: `geom`, `sim`, `track`) — the
only crate with real integer logic today. Concrete production sites found (starting
inventory; design confirms completeness):

| Site | Current form | Disposition |
|---|---|---|
| `geom/mod.rs` `Point::neighbours` (~L40–43) | raw `self.x ± 1`, `self.y ± 1` (i32) | overflows at `i32::MAX`/`MIN` — needs (a) or (b) |
| `geom/mod.rs` `Size::area` (~L101) | `self.width * self.height` (u32) | u32 overflow — needs (a) or (b) |
| `geom/mod.rs` `Rect::index` (~L133–146) | `checked_sub` + `try_from`; `dy * width + dx` (usize) | conversion already compliant; confirm the `usize` index mul bound + test |
| `geom/mod.rs` `Rect::far_corner` (~L161–162) | `try_from` + `saturating_add` | reference-compliant; confirm covering test |
| `geom/mod.rs` `Rect::on_border` (~L176) | `dx + 1 == w`, `dy + 1 == h` (usize) | bounded (`dx < w`) — retain (b): document bound + test |
| `geom/mod.rs` `supercover` (~L295–302) | `i64::from` widening; `dx * (cy − a.y)`, `2 * cr.abs()` | reference (b) pattern — confirm bounded-chord precondition doc + covering test; verify i64 products cannot overflow within the stated bound |
| `sim.rs` `legal_move` (~L85–86) | raw `s.vx + ax`, `s.x + vx2` (i32) | overflows for adversarial coords — needs (a) or (b) |
| `geom/graph.rs` neighbour build (~L283) | raw `cell.x + dx`, `cell.y + dy` (i32) | overflows at box-adjacent extreme coords — needs (a) or (b) |
| `gen/lib.rs` `GenParams::min_width` (~L28) | `self.cars.div_ceil(2)` (u32) | already overflow-safe (`div_ceil`) — confirm only |

## Out of scope

- Implementing the `todo!()` stubs (`sim::step` / `register_move` / `resolve_crash`
  / `resolve_collisions`; the `gen`/`render`/`ai`/`game` pipelines). They contain
  no arithmetic yet; their integer logic is audited when it lands.
- Test-fixture literals and self-evident constants (`0`/`1`/`-1`/`2`) — exempt per
  AGENTS.md § Code Style.
- Changing physics semantics. This audit hardens *how* an op is computed, never
  *what* it computes; every existing test must still pass unchanged.

## Deferred

- Re-audit of `gp-gen` / `gp-render` / `gp-ai` / `gp-game` as their integer logic
  lands | scaffolds have no risky integer ops today (only `div_ceil`, a no-op
  `self.cars` return) | standing follow-up on this same issue #48 — no new issue.

## Key decisions

| Question | Decision |
|---|---|
| Which crates carry substantive changes? | `gp-core` only. Scaffold crates (`gen`/`render`/`ai`/`game`) get a confirming sweep — verified free of unsafe integer ops today; no forced edits. |
| Accepted safety strategies | The explicit-semantics integer methods (`checked_*`/`saturating_*`/`wrapping_*`/`overflowing_*`/`strict_*`/`abs_diff`/`carrying_*`/`borrowing_*`), `try_from` for conversions, and widening to a strictly wider integer type. Real-number arithmetic is not admissible — gp-core is integer-only (`docs/design.md` §3a). |
| Reference-compliant patterns already in tree | `Rect::index` (`checked_sub`+`try_from`), `Rect::far_corner` (`try_from`+`saturating_add`), `supercover` (i64 widening + bounded-chord precondition). These are the model; the audit confirms each has a documented bound + covering test. |
| Per-site method choice (checked vs saturating vs widen) | Left to the `design` Subagent per each call-site's caller semantics, following the rule's intent→method table. |
| MSRV method availability | 1.97.0. The full illustrative list — incl. `strict_*`, `carrying_*`/`borrowing_*`, `abs_diff` — verified to compile on the pinned stable toolchain; all are available. |
| Starting-point cast/allow inventory | No `as` numeric casts and no `#[allow(clippy::cast_*)]` / `arithmetic_side_effects` exist anywhere in the workspace today (verified). |
| Machine-enforcement via `clippy::arithmetic_side_effects` | **Enabled workspace-wide (round-2 owner decision).** Add `arithmetic_side_effects = "deny"` to the root `[workspace.lints.clippy]` table — a single restriction lint, not the whole `restriction` group; a specific lint at default priority, so it sits above the `-1` group denies without conflict. Every deliberately-retained bounded raw op carries a per-site `#[allow(clippy::arithmetic_side_effects, reason = "...")]`. Owner accepts the allow-churn now, while the codebase is small, for a mechanical guarantee across all crates. Level `deny` matches the existing posture (`missing_docs`, `large_stack_frames`) and the CI `-D warnings` floor. |

## Technical constraints

- **gp-core is integer-only, deterministic, std-only** (`docs/design.md` §3a) —
  real-number arithmetic is not an admissible safety strategy in `geom`/`sim`.
- **No production panics.** Public gp-core functions must not panic on any in-range
  input, including adversarial extremes (`i32::MAX`/`i32::MIN`, `u32::MAX`). Where a
  raw op is retained under (b), its safety must rest on a documented, test-covered
  bound — not an implicit assumption.
- **Lint floor + new restriction lint.** This audit adds `arithmetic_side_effects =
  "deny"` to the root `[workspace.lints.clippy]` table. `cargo clippy --workspace
  --all-targets -- -D warnings` must stay clean with it active — and because
  `--all-targets` lints `#[cfg(test)]` code too, raw arithmetic in **test** targets
  also needs either a checked form or a justified `#[allow(clippy::arithmetic_side_effects,
  reason = "...")]`. The workspace `[lints]` already denies clippy `pedantic` +
  `nursery` (cast lint families); every `#[allow(...)]` added needs a `reason = "..."`.
- **Behaviour-preserving.** No change to any existing test's expected values; new
  tests only add coverage.
- **Verify method MSRV-stability (PROC-1).** Before the design/impl reaches for any
  integer method, confirm it compiles on the pinned toolchain (all currently-listed
  methods already verified above).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Every raw integer arithmetic op and `as` numeric cast in `gp-core` production code (`geom`, `sim`, `track`) is either converted to an explicit-semantics/widened form matching intent, or retained under a documented, test-covered domain bound. No unjustified overflow / signedness / division-by-zero site remains. |
| AC2 | Conversions that could lose sign or truncate use `try_from`, not `as`; no `#[allow(clippy::cast_sign_loss \| cast_possible_truncation \| arithmetic_side_effects)]` exists without a `reason = "..."`. |
| AC3 | Each retained raw op (incl. `supercover`'s bounded-chord products and `Rect::on_border`) carries a doc-comment stating its domain bound and has a test exercising that bound. |
| AC4 | New/updated tests cover the adversarial edges of every newly-hardened public geom/sim method — `i32::MAX`/`i32::MIN` coordinate inputs and `u32` overflow inputs return the intended `Option`/clamp/wrapped result without panicking. |
| AC5 | Scaffold crates (`gen`/`render`/`ai`/`game`) are confirmed free of unsafe integer ops; no substantive edits made to them; the scaffold re-audit is recorded as deferred on #48. |
| AC6 | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass. |
| AC7 | `arithmetic_side_effects = "deny"` is present in the root `[workspace.lints.clippy]` table, and every deliberately-retained raw arithmetic op — in **production and test** targets — carries a `#[allow(clippy::arithmetic_side_effects, reason = "...")]`. `cargo clippy --workspace --all-targets -- -D warnings` passes with the restriction lint active. |

## Open questions

_None. Q1 (machine-enforcement scope) was resolved in round 2 — the
`clippy::arithmetic_side_effects` lint is enabled workspace-wide with justified
per-site allows (see Key decisions and AC7)._
