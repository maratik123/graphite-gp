# gp-core: sim step + legal_move/legal_mask verification

**Source:** issue #7
**Date:** 2026-07-16
**Tracked in:** #7

## Scope

The issue body was auto-generated before two reworks landed on `main`
(`integer-safety-audit` #48 and `enumflags2-legal-mask` #51). Against the current
`crates/core/src/sim.rs`, the genuine delta is small: **implement `step`** (the
only remaining `todo!` in the movement path) plus the confirmation tests the
issue's Test notes call for. `legal_move`, `legal_mask`, and the 5-action `Action`
enum already exist and are correct.

1. **Implement `sim::step`** — the pure kinematic update (design §3 *Движение*):
   apply the action's acceleration to velocity first (`vx' = vx + ax`,
   `vy' = vy + ay`), then advance position by the **new** velocity
   (`x' = x + vx'`, `y' = y + vy'`), returning the resulting `CarState`. No
   legality check, no I/O, no RNG. Replaces the current `todo!("step …")` stub.
2. **step tests** — exact-state assertions: from `v = (0,0)` the five actions
   (Coast + four axes); at least one general `(x, y, vx, vy)` + action case that
   exercises the accelerate-then-advance ordering (i.e. a non-zero starting
   velocity, so "advance by the *new* velocity" is distinguishable from "advance
   by the old velocity"); a determinism assertion (same input → same output).
3. **legal_move confirmation test** — the issue's Test notes: a corridor where a
   fast chord clips a wall (its supercover leaves `D`) is illegal, while a clear
   chord through `D` is legal. This locks the already-implemented
   `supercover ⊆ D` rule (design §3 C4); it does not change `legal_move`'s code.
4. **legal_mask confirmation** — assert `legal_mask` returns a `BitFlags<Action>`
   holding exactly the actions for which `legal_move` is true, over `Action::ALL`.
   Already implemented and covered by `legal_mask_contains_exactly_the_legal_actions`;
   this task keeps that guarantee green (no code change).

## Out of scope

Already on `main` — **do not re-implement**:

- `legal_move` body (checked_add off-grid⇒false; #48).
- `legal_mask` `BitFlags<Action>` migration and the `BitFlags` re-export (#51).
- `Action` enum / `Action::ALL` / `Action::accel` (5 von-Neumann actions locked;
  diagonal acceleration is unrepresentable by construction).
- `CarState`, `CarState::pos`.

Other `sim.rs` stubs belonging to sibling issues — leave `todo!` untouched:

- `LapCounter::register_move` (signed S/F crossing).
- `resolve_crash` (crash rule — design §3 has this **finalized** as scrub-t200
  damping, but it is a separate task/id, not this one).
- `resolve_collisions` (car-collision BFS placement).

## Deferred

- None. No new follow-up issues are surfaced by this task.

## Key decisions

| Question | Decision |
|---|---|
| The stub's `d: &Corridor` parameter — the kinematic update reads no `D` (design §3 formula is `(x+vx', y+vy')`). | **Default: drop it** → `step(s: CarState, a: Action) -> CarState`. `step` has zero current callers (grep-confirmed), so the change breaks nothing; AGENTS.md § *API Stability* permits the clean break; `CARGO_BUILD_WARNINGS=deny` forbids shipping a permanently-underscored `_d`. Design may retain `d` if it identifies a concrete forward-looking need. |
| `step` return type. | Infallible **`-> CarState`** (matches the issue and the assumed-legal precondition). No `Option`/`Result` — legality is `legal_move`'s job; `step` is the assumed-legal kinematic update. |
| Overflow under `arithmetic_side_effects = "deny"`. | On the assumed-legal domain the four adds are overflow-free (proven by `legal_move`'s `checked_add` chain: a legal action has `vx+ax`, `vy+ay`, `x+vx'`, `y+vy'` all in `i32`). Satisfy the lint via the established house pattern — a documented fn-level `#[allow(clippy::arithmetic_side_effects, reason = …)]` with the assumed-legal precondition + covering tests (as in `supercover`, `Size::area`, `Rect::index`) — **or** an explicit-semantics op (`wrapping_add`/`saturating_add`). Exact form is the design phase's call. |
| Single legality path. | `step` must **not** call `legal_move` or `supercover` and must perform no legality check. `legal_move` remains the sole legality predicate shared by player, AI mask, and oracle; `legal_mask` is its only fan-out. |

## Technical constraints

- **Integer-only, deterministic core** (design §3a): `step` uses integer
  arithmetic throughout (no non-integer numeric types), with no RNG and no I/O —
  consistent with the rest of gp-core.
- **`clippy::arithmetic_side_effects = "deny"`** is active in the root
  `[workspace.lints.clippy]`. `step`'s velocity/position updates must not emit a
  raw unguarded add; use the documented `#[allow]`-with-precondition house pattern
  or an explicit-semantics op (see Key decisions).
- **`CARGO_BUILD_WARNINGS=deny`** (CI): no unused named parameter — reinforces
  dropping `d` over a lingering `_d`.
- Code and tests live in `crates/core/src/sim.rs` (`#[cfg(test)] mod tests`), the
  existing home of `step`/`legal_move`/`legal_mask`. No new module or crate.
- `step` consumes `Action::accel()` for the `(ax, ay)` acceleration; it does not
  re-encode the action table.
- Gates: `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`
  all pass. `step` gains a one-line-minimum `///` doc (public item) with the
  accelerate-then-advance rule.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `step` applies acceleration to velocity **first** (`vx' = vx + ax`, `vy' = vy + ay` from `Action::accel`), **then** advances position by the new velocity (`x' = x + vx'`, `y' = y + vy'`), returning that `CarState`. A case with non-zero starting velocity distinguishes "advance by new v" from "advance by old v". |
| AC2 | `step` is pure and deterministic: no I/O, no RNG; identical `(state, action)` inputs yield an identical `CarState` (asserted). |
| AC3 | `legal_move` returns `false` when `p1 ∉ D` **or** any supercover cell of the chord `pos → p1` is `∉ D`, and `true` for a fully-in-`D` chord — confirmed by a fast-chord-clips-wall (illegal) vs clear-chord (legal) test on a hand-built corridor. |
| AC4 | `legal_mask(d, s)` returns a `BitFlags<Action>` containing exactly the actions `a ∈ Action::ALL` for which `legal_move(d, s, a)` is `true` (typed bitflags, **not** `[bool; 5]`). |
| AC5 | From `v = (0,0)`: `Coast` yields `(x, y, 0, 0)` (car in place); `East`/`West`/`North`/`South` yield position shifted by exactly `(+1,0)`/`(-1,0)`/`(0,+1)`/`(0,-1)` with velocity set to the same delta. |
| AC6 | `step` introduces no `arithmetic_side_effects` violation — the velocity/position updates use an explicit-semantics op or a documented, test-justified `#[allow]` per the house pattern; `cargo clippy --workspace --all-targets -- -D warnings` passes. |

## Open questions

None design-blocking. The `d`-parameter retention and the exact overflow-handling
form are recorded as defaults in Key decisions and are the design phase's to
finalize.
