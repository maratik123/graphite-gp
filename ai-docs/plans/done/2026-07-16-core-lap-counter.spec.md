# gp-core: signed S/F lap counter — half-open crossing test + valid-finish conjunction

**Source:** issue #8
**Date:** 2026-07-16
**Tracked in:** #8

## Scope

Implement the body of `LapCounter::register_move` in `crates/core/src/sim.rs`,
replacing the current `todo!("signed S/F crossing (design doc §3)")` stub. The
surrounding scaffolding already exists on `main` and is correct — do not
re-implement it (see *Out of scope*).

1. **`register_move` — signed half-open crossing test.** Score the move chord
   `from → to` against the timing gate carried by `sf.gate` and mutate
   `self.counter`:
   - `+1` when the chord crosses the gate **forward** (along `+race_dir`, i.e.
     the gate's `forward` direction).
   - `−1` when it crosses **reverse**.
   - no change when the chord does not cross.
   - **At most one event per move**, even for a long/fast chord — a straight
     segment's perpendicular coordinate is monotone, so it meets the gate line
     at most once.
2. **Half-open interval test over the half-grid gate (design §3 \[C2\], §2 Ф3).**
   The timing gate is a **dual edge on the half-grid**: the edge between each
   `behind[i]` cell and `behind[i] + forward.delta()`, placed one edge ahead of
   the front row so **all cars at `t=0` are strictly behind it** (design §2 Ф3,
   lines 48–49). Its supporting line therefore lies on the **half-grid**,
   halfway between the `behind` row and the `behind + forward` row — *not* on any
   integer cell line. Define the signed perpendicular coordinate of a point `p`
   along `forward`; the `behind` row and everything further back classify `−`,
   the `behind + forward` row and everything ahead classify `+`. The predicate is
   the half-open interval test the issue title names:
   - forward cross ⟺ `from` strictly `−` **and** `to` `+`-or-on-line;
   - reverse cross ⟺ `from` `+`-or-on-line **and** `to` strictly `−`.

   The `+`-or-on-line half (line ∈ `+`) makes forward/reverse exact mirrors, so a
   forward then a reverse cross telescope to net `0`. **Because the gate sits on
   the half-grid, no real integer car position ever lands on the line** (design
   §2 Ф3: «на линии» геометрически невозможно) — the `on-line` branch is a
   *defensive* part of the predicate definition, never exercised by real
   `Point`s. Every integer chord is scored purely by the `behind`(`−`) /
   `ahead`(`+`) partition.
3. **Score before collision resolution.** `register_move` scores the swept
   `from → to` of the *committed legal move* only; it performs no teleport and
   no collision/crash handling. The tick loop that would call it after
   `legal_move` and before `resolve_collisions` / `resolve_crash` is **not built
   in this task** (those remain `todo!`); the "teleports never touch the
   counter" guarantee is upheld here simply because `register_move` is the sole
   counter mutator and is only ever the crossing scorer.
4. **Valid-finish conjunction (design §3, *Валидный финиш*).** Encode the
   valid-finish rule as the conjunction *(the move is legal via `legal_move`)*
   ∧ *(the chord forward-crosses S/F)*, with **legality evaluated first**.
   `register_move` itself stays legality-agnostic — `legal_move` remains the
   single legality path (per #7's *single legality path* decision) — so the
   conjunction is expressed at the call site (optionally via a thin composing
   predicate; the exact shape is the design phase's call). A test must
   demonstrate that a would-be-crossing move that is **illegal** (its chord
   clips a wall — `legal_move` false) has its crossing **not** honored.
5. **Tests** — the from→to-vs-fixed-gate delta table plus a scripted
   move-sequence asserting exact counter values, including the `−1` init (see
   *Acceptance Criteria* and the test table below).

## Out of scope

Already on `main` — **do not re-implement**:

- `LapCounter` struct, `Default` (init `counter = -1`), `new`, `laps`
  (`counter.max(0)`), `raw` — all present and correct in `crates/core/src/sim.rs`.
- `StartFinish` / `TimingGate` / `RaceDir` / `Point` / `Side::delta` /
  `Orient` — the contract types this task consumes (finalized under #6).
- `legal_move` / `legal_mask` / `step` / `supercover` (#7): the legality path
  and kinematics. This task consumes `legal_move`; it does not modify it.

Not this task (sibling `todo!`s — leave untouched): `resolve_crash`,
`resolve_collisions`, and any tick/turn driver loop that would sequence
step → register_move → collision resolution.

## Deferred

- A concrete tick/turn driver that orders `legal_move` → `step` →
  `register_move` → collision/crash resolution and reads `laps()`/`raw()` for
  the win check | the ordering contract is documented here but the loop lands
  with the crash/collision tasks | **no new issue needed** — already covered by
  the existing `resolve_crash` / `resolve_collisions` sibling issues.

## Key decisions

| Question | Decision |
|---|---|
| Where does the gate's supporting line for the crossing test sit — the integer forward face, or the half-grid dual edge? | **The half-grid dual edge** between `behind[i]` and `behind[i] + forward.delta()` (design §3 \[C2\], §2 Ф3), placed one edge ahead of the front row. **Not** the integer forward-face line. Rationale: **(a)** product-owner directive to follow design §3 \[C2\] / §2 Ф3 literally — the gate is a dual edge on the half-grid, and design §2 Ф3 (lines 48–49) states an integer car position can **never** lie on the gate line («на линии» геометрически невозможно); **(b)** this is **behaviorally identical for lap counting** to the forward-face reading — under either line, `behind` cells classify `−` and `behind + forward` cells classify `+`, so every integer chord scores exactly the same `Δcounter`; the choice changes only the line's *representation* and whether an on-the-line position is reachable (it is not, on the half-grid); **(c)** accepted consequence: the issue's AC3 "ending exactly on the gate / starting on the line" cases are **geometrically vacuous** for integer `Point`s and are reconciled in AC3 below. |
| Which side owns the (unreachable) line? | The **`+` (forward) side**, per §3 \[C2\] "to on the `+` side **or on the line**". This keeps forward/reverse exact mirrors (back-and-forth telescopes to `0`) as a **defensive predicate convention**; on the half-grid no real `Point` reaches the line, so the convention is never exercised in play. |
| Integer perpendicular coordinate for a half-grid line. | A **doubled/scaled** perpendicular coordinate is the natural half-grid formulation: scale so `behind` cells → **even**, `behind + forward` cells → **even**, and the half-grid line → the **odd** midpoint between them. Real `Point`s (always on cell centers) only ever produce **even** values, so the odd line value is unreachable — the doubled coordinate keeps the whole test in **integers** (no fractional coordinate) while giving the half-open `on-line` branch a representable value to define against. Exact form (scale factor, origin) is the design phase's call. |
| Lateral extent — must the crossing point fall within the gate's cross-section span, or is a perpendicular-only test enough? | **Perpendicular-only, relying on the full-chord invariant.** S/F is a full chord cutting the annulus into a simply connected strip (design §3), and `legal_move` (checked first) keeps every scored chord inside `D`; so within legal play any perpendicular crossing *is* a gate crossing. The design phase may add a lateral-span guard for defensiveness, but it is not required for correctness on the intended (in-`D`, full-chord) domain. Tests keep each move's lateral coordinate within the fixture gate's span so they exercise the perpendicular rule directly. |
| Is the `race_dir: RaceDir` parameter needed, given `sf.gate.forward` already encodes the local `+race_dir` direction? | The crossing **sign derives from `sf.gate.forward`** (the local `+race_dir` projection), not from the global `Cw`/`Ccw` value. `race_dir` may therefore be redundant. Whether to **drop the parameter** (clean break, mirroring #7's dropped `d` param — AGENTS.md § *API Stability* permits it) or retain it is the design phase's call; if kept it must not become a permanently-unused `_race_dir` (`CARGO_BUILD_WARNINGS=deny`). |
| `register_move` return type. | Keep **`-> ()`** (mutate `self.counter`); the win check reads `laps()`/`raw()`. The design phase may return the signed event (`-1`/`0`/`+1`) if a call site needs the per-move delta, but no current caller does. |
| Empty / degenerate gate (`gate.behind` empty). | **No-op** (non-panicking, per AGENTS.md § *API Naming* default): with no edges there is no line to cross. The generator's invariant is a non-empty gate; `register_move` must not panic if handed an empty one. |

## Technical constraints

- **Integer-only, deterministic core (design §3a).** The crossing test uses
  integer arithmetic throughout — the perpendicular coordinate comes from
  `Side::delta()`'s integer `(dx, dy)`, and the half-grid line is represented via
  a doubled/scaled integer coordinate (behind → even, ahead → even, line → odd),
  never a fractional coordinate. Do **not** use `TimingGate::forward_unit()`
  (that unit accessor exists for the `s`-field tangent, a different concern).
- **`clippy::arithmetic_side_effects = "deny"`** is active workspace-wide. The
  perpendicular-coordinate computation subtracts / scales `Point` coordinates;
  use the established house pattern (a documented, test-justified fn-level
  `#[allow(...)]` with an overflow precondition, as `step` / `supercover` /
  `Size::area` do) **or** explicit-semantics ops (`checked_*` /
  `saturating_*` / `wrapping_*`). Exact form is the design phase's call.
- **`CARGO_BUILD_WARNINGS=deny`** (CI): no unused named parameter — reinforces
  the `race_dir`-drop-vs-keep decision above (no lingering `_race_dir`).
- Code and tests live in `crates/core/src/sim.rs` (`#[cfg(test)] mod tests`,
  the existing home of `LapCounter`). No new module or crate.
- `register_move` keeps a public-item `///` doc describing the half-open
  half-grid rule, the sign convention, and the score-before-collisions contract
  (the stub's doc is a starting point).
- **Gates:** `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`
  all pass.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | A chord that crosses the gate forward (`from` on the `−`/behind side, `to` on the `+`/ahead side of the half-grid edge) increments `counter` by exactly `+1`; a reverse crossing decrements by exactly `−1`; a chord that stays wholly on one side is a no-op. |
| AC2 | At most one event is registered per `register_move` call, even for a long chord spanning many cells across the gate (e.g. `from` two cells behind → `to` two cells ahead yields `+1`, not more). |
| AC3 | Half-open predicate + half-grid reconciliation: the crossing predicate is defined half-open (`from` strictly `−`, `to` `+`-or-on-line) so the line belongs to the `+` side and forward/reverse are exact mirrors. Because the gate is a **half-grid** dual edge, **no real integer `Point` ever lands on the line** (design §2 Ф3), so the on-the-line branch is **unreachable in play** — every integer chord is classified purely by the behind(`−`)/ahead(`+`) partition. The task tests the **reachable** behavior: a behind→ahead chord scores `+1`; a chord already ahead of the gate moving further ahead does **not** re-score (`from` already `+` → no double-count); the reverse mirror holds. It **may** additionally unit-test the raw half-open comparison at the odd (half-grid) line value to lock the line∈`+` convention, explicitly noting real `Point`s never produce that value. |
| AC4 | `counter` is `−1` at construction (`LapCounter::new()` / `default()`); `laps() == max(0, counter)`; the **first** forward crossing yields `counter == 0` and `laps() == 0` (race start, not a completed lap); the second yields `laps() == 1`. |
| AC5 | The valid-finish conjunction evaluates `legal_move` **before** the gate-cross: a would-be forward-crossing move whose chord is **illegal** (clips a wall — `legal_move` returns false) does **not** change `counter`. A legal forward-crossing move does. |
| AC6 | A scripted move sequence asserts exact `counter` / `laps()` values end to end, including the `−1` init and a back-and-forth pair (forward then reverse) telescoping to a net `0` counter delta. Parallel/tangent moves running along the gate (no perpendicular crossing) leave `counter` unchanged. |
| AC7 | `register_move` uses integer arithmetic only, introduces no `arithmetic_side_effects` clippy violation, and does not panic on a degenerate empty gate; the full gate suite (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, doc gate) passes. |

### Test table (illustrative — design phase finalizes fixtures)

Fixture gate: `behind = [(1,1)]`, `forward = East` → the half-grid dual edge
between `(1,1)` and `(2,1)`, i.e. the supporting line at `x = 1.5` (one edge
ahead of the front row). Cells with `x ≤ 1` classify `−` (behind); cells with
`x ≥ 2` classify `+` (ahead). No integer `x` equals `1.5`, so no `Point` is ever
on the line. Init `counter = −1`.

| from → to | side(from), side(to) | Δcounter | Why |
|---|---|---|---|
| (1,1) → (2,1) | −, + | +1 | forward; behind→ahead (AC1) |
| (0,1) → (4,1) | −, + | +1 | forward, long chord — still one event (AC2) |
| (3,1) → (1,1) | +, − | −1 | reverse (AC1) |
| (2,1) → (1,1) | +, − | −1 | reverse, adjacent (AC1) |
| (0,1) → (1,1) | −, − | 0 | stays behind — no cross |
| (2,1) → (3,1) | +, + | 0 | stays ahead — no cross (no double-count; AC3) |
| (2,0) → (2,3) | +, + | 0 | parallel along the gate (pure-`y`) — no perpendicular cross (AC6) |
| (1,1) → (2,1), then (2,1) → (1,1) | — | net 0 | telescoping forward+reverse (AC6) |

Scripted-sequence check (AC4/AC6): from a fresh counter (`raw() == −1`,
`laps() == 0`), a first forward cross → `raw() == 0`, `laps() == 0`; a reverse
cross → `raw() == −1`, `laps() == 0`; two forward crosses → `raw() == 1`,
`laps() == 1`.

## Open questions

None design-blocking. The `race_dir`-parameter drop-vs-keep, the exact doubled
perpendicular-coordinate form and overflow handling, an optional lateral-span
guard, and whether the valid-finish conjunction is a named predicate or inline
call-site composition are recorded as defaults in *Key decisions* and are the
design phase's to finalize.
