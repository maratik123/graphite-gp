# gp-core: adopt enumflags2 for legal_mask (typed 5-action bitflags)

**Source:** issue #51
**Date:** 2026-07-16
**Tracked in:** #51

Replace `legal_mask`'s positional `[bool; 5]` return with a typed, `#[repr]`-backed
bitflags set over the 5 von-Neumann actions, using the `enumflags2` crate. The
5-action set is now locked, so the flag enum will not churn (this adoption was
verdicted **defer-to-issue** in `ai-docs/library-survey.md` pending exactly that).

## Scope

1. Add `enumflags2 = "0.7"` to `crates/core/Cargo.toml` `[dependencies]`. (Max
   stable `0.7.12` verified on crates.io 2026-07-16; `0.x` pin per AGENTS.md §
   Dependency Versions.)
2. Give the locked 5-action set (`sim::Action`) an `enumflags2::bitflags`
   `#[repr]`-backed bit representation, so a set of legal actions is a real
   bitflags value (compose/test cheaply) rather than a positional bool array.
3. Change `legal_mask` in `crates/core/src/sim.rs` to return the typed mask
   (`BitFlags<Action>`) instead of `[bool; 5]`, containing exactly the actions
   for which `legal_move` is true, preserving `Action::ALL` order.
4. Update the mask parameter of `policy_action` in `gp-ai`
   (`crates/ai/src/lib.rs`) from `[bool; 5]` to the typed mask.
5. Update the two overflow no-panic tests in `sim.rs` that assert
   `legal_mask(...) == [false; 5]` to the typed-mask empty-set equivalent.
6. After the manifest change: run `cargo update` then `cargo build` to refresh
   `Cargo.lock` and verify.

## Out of scope

- `legal_move` (single-action `-> bool`) — unchanged; only `legal_mask` changes.
- The `step` / `resolve_crash` / `resolve_collisions` `todo!` stubs.
- **A `gp-render` change.** The issue lists a "MovePad legality in `gp-render`"
  call-site, but no such consumer exists today: `crates/render/src/lib.rs` has
  only the `render_frame` `todo!` stub and no `legal_mask` / MovePad reference
  (verified 2026-07-16). There is no render call-site to migrate; MovePad, when
  implemented, will consume the typed mask from the start.
- Promoting `enumflags2` to `[workspace.dependencies]` — only `gp-core` uses it
  today; the root table currently holds internal path crates only.

## Deferred

- (none — nothing carried to a separate issue.)

## Key decisions

| Question | Decision |
|---|---|
| Which enum carries `#[bitflags]`? | Annotate the existing `sim::Action` (the locked 5-action set) — single source of truth, no parallel flag enum. `Action::ALL` order (Coast, East, West, North, South) is preserved as the canonical bit/logit order. Design may instead introduce a distinct set newtype if it prefers a named mask type. |
| `legal_mask` return type | `BitFlags<Action>` directly (the issue's primary option). The "small newtype" (e.g. a `LegalMask` wrapper) alternative is left to the `design` subagent. |
| Version / pin | `enumflags2 = "0.7"` (observed max stable `0.7.12`; `0.x` pinning). |
| `#[repr]` int type | enumflags2's default (a `u8` repr suffices for 5 flags) — exact repr left to design. |
| Manifest placement | Declared directly in `crates/core/Cargo.toml` (single-crate consumer). Design may promote to `[workspace.dependencies]` if it judges that the workspace convention. |

## Technical constraints

- `gp-core` is integer-only and deterministic (design doc §3a); `enumflags2` is a
  pure-integer bitflags crate with no floating-point surface — compatible.
- The `#[repr]` this adds is justified by the `enumflags2::bitflags` contract, the
  first of the two sanctioned cases in `ai-docs/code-style.md` § *Enum repr* (not
  a decorative annotation).
- `#[bitflags]` requires its target enum be `Copy` with an integer `#[repr]`;
  `Action` already derives `Copy`. `Action::ALL` (const) and `Action::accel()`
  (const `fn`) must continue to compile and keep the same order/values semantics.
- Preserve `Action::ALL` ordering as the canonical logit order — the AI policy
  masks per-logit, so the mask must be testable per action in that order.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `enumflags2 = "0.7"` is present in `crates/core/Cargo.toml`; `cargo update` and `cargo build` succeed with the refreshed `Cargo.lock`. |
| AC2 | `legal_mask(d, s)` returns a typed `enumflags2` mask over `Action` (not `[bool; 5]`) that contains exactly the actions for which `legal_move(d, s, a)` is `true`. |
| AC3 | The `Action` set has a `#[repr]`-backed `enumflags2::bitflags` representation, satisfying the anti-decorative `#[repr]` rule (`ai-docs/code-style.md` § *Enum repr*). |
| AC4 | `gp-ai::policy_action` consumes the typed mask; no `[bool; 5]` mask type remains anywhere in the workspace. |
| AC5 | The two `sim.rs` overflow tests (`..._i32_max_overflow...`, `..._i32_min_underflow...`) assert the empty typed mask (equivalent to the former `[false; 5]`), preserving the `i32::MAX`/`i32::MIN` no-panic guarantee (former AC4 of the sim work). |
| AC6 | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass; every changed public item keeps its `///` doc. |

## Open questions

- Named newtype (`LegalMask`) vs bare `BitFlags<Action>` for the `legal_mask`
  return — left to the `design` subagent; bare `BitFlags<Action>` is the default.
