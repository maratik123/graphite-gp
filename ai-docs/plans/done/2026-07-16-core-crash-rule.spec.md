# gp-core: crash rule — quench with scrub-tick (normal→0, tangential→⌊t/2⌋)

**Source:** issue #9
**Date:** 2026-07-16
**Tracked in:** #9

## Scope

Implement `sim::resolve_crash` — today a `todo!("crash rule …")` stub in
`crates/core/src/sim.rs`. Replace it with the design's **finalized** crash rule
(design §3 "Краш", `[D4]`, `[N5]` — all marked ФИНАЛИЗИРОВАНО). A crash is a
search dead-end: from an otherwise-legal state `s = (x, y, vx, vy)` **every one of
the 5 actions is illegal** (`legal_mask(d, s)` is empty — all leave `D`). The
car's momentum (the coast trajectory `(x,y) → (x+vx, y+vy)`) carries it into a
wall; `resolve_crash` produces the post-crash kinematic state plus the scrub-tick
marker.

1. **Respawn position** — walk the coast segment `(x,y) → (x+vx, y+vy)` **in
   order** and place the car at the last cell still in `D` before the sweep leaves
   `D` (the furthest swept cell whose supercover-prefix stays ⊆ `D`). The start
   cell `(x,y)` is in `D`, so this always yields a valid respawn cell `L`.
2. **Velocity quench** — decompose the incoming velocity `(vx, vy)` into the
   wall's **normal** (into-wall) and **tangential** (along-wall) axes: zero the
   into-wall component, damp the along-wall component `t` to `⌊t/2⌋` (strong
   damping). The wall = the **first** `¬D` cell hit along the sweep; a **corner**
   (both the x- and y-neighbor of `L` are `¬D`) zeros **both** conflicting
   components.
3. **Scrub-tick** — the crash outcome carries a marker that forces the
   immediately-following move to be `Coast` (no re-acceleration) for **exactly one
   tick**; the tick after, the normal `legal_mask` applies again.
4. **Fail-safe** — if the forced `Coast` off the respawn state is illegal (the
   damped coast would re-exit `D`), halve the velocity again (toward zero) and
   re-check; repeat, in the limit reaching `v = (0,0)` (where `Coast` from the
   in-`D` cell `L` is always legal). The outcome is **always** a crash outcome
   (the scrub marker applies) — a crash never yields a penalty-free controlled
   `v = 0`.

Add whatever 3a-local type/helpers are needed so the scrub-tick (AC4) is
unit-testable inside `gp-core` (e.g. a crash-outcome carrying `CarState` + scrub
state, and a pure means to obtain the scrub tick's restricted `Coast`-only mask
and advance past it). Document the `v=0 + skip P ticks` fallback variant in the
`resolve_crash` doc comment.

## Out of scope

- **`P_crash` / reward shaping / progress-and-position penalty** — the design's
  *real* anti-abuse lever (`[N5]`: crash pins progress `s` and costs position)
  lives in the AI reward function (block 4) and the player's lost time/position,
  **not** in 3a kinematics. `resolve_crash` produces only the post-crash
  kinematic state + scrub marker.
- **Fallback variant implementation** — `v=0 + skip P ticks` (design §3 *Fallback*
  / variant 1) is only **documented** as the finicky-calibration alternative;
  implement the primary `⌊t/2⌋`-quench + scrub rule.
- **`resolve_collisions`** — the car-collision BFS placement is a sibling `todo!`
  / separate issue; leave it untouched.
- **The full tick/turn driver loop** — the running-game orchestration that
  *detects* a crash, calls `resolve_crash`, and counts the scrub down across turns
  (alongside `step`, `LapCounter`, collision resolution) is block 3b and needs the
  not-yet-built collision layer; the lap-counter task already deferred "the
  tick/turn driver loop lands with the crash/collision tasks". This task ships the
  pure crash resolution + scrub mechanism as 3a helpers, not the loop.
- Already on `main` — do **not** re-touch: `step`, `legal_move`, `legal_mask`,
  `CarState`, `LapCounter`, and all `geom` primitives.

## Deferred

- Calibration of the `⌊t/2⌋` damping (whether it proves "finicky" in play →
  rollback to variant 1) | empirical, decided by playtesting / AI-training runs,
  not on paper | **no** separate issue (design already names variant 1 as the
  documented fallback; the rollback is a code swap, not new scope).

## Key decisions

| Question | Decision |
|---|---|
| Which segment is "the swept segment"? | **The coast/momentum trajectory** `(x,y) → (x+vx, y+vy)`. The design's singular "the swept segment" and single into-wall/along-wall *incoming* velocity both refer to the no-input momentum vector; the 4 accel choices are the failed avoidance attempts, not the sweep. Design finalizes the exact ordered walk. |
| Respawn cell `L`. | The **furthest cell along the ordered coast segment whose supercover-prefix is ⊆ `D`** — the last valid cell before the sweep leaves `D`. Always ∈ `D` (start cell `(x,y) ∈ D` ⇒ ≥1 valid cell). Reuse `supercover` + `Corridor::contains`; do not add a second geometry path. |
| Wall-normal classification. | At `L`, the into-wall axis is the axis of the **first `¬D` cell** along the sweep relative to `L`: `L+(sgn(vx),0) ∉ D` ⇒ x is into-wall (zero `vx`); `L+(0,sgn(vy)) ∉ D` ⇒ y is into-wall (zero `vy`); both `¬D` ⇒ **corner** ⇒ zero both. The surviving axis is along-wall. Exact predicate is the design phase's call. |
| `⌊t/2⌋` integer semantics for the along-wall component `t`. | **Integer division `t/2` (truncation toward zero, sign preserved)** — halves the along-wall speed without flipping direction. The design's `⌊t/2⌋` is shorthand for "halve the along-wall speed"; the exact rounding must be pinned by design and locked by an exact-state test. Integer-only (no non-integer numeric types); the `/2` literal is self-evident (const-exempt). |
| Scrub-tick temporal model + representation. | **Model: the scrub tick is a real forced-`Coast` move** (design "один ход без права реакселерации" / `[N5]` "константная цена в один тик"): right after the crash the only permitted action is `Coast`; the following tick resumes the full `legal_mask`. `resolve_crash` returns a small **crash outcome** (post-crash `CarState` + scrub state); 3a provides a pure means to obtain the scrub tick's `Coast`-only mask and advance past it, so AC4 is unit-testable in `gp-core`. Exact type/shape is the design phase's call; the existing `-> CarState` signature changes (AGENTS.md § *API Stability* permits the clean break — `resolve_crash` has **zero current callers**). |
| Fail-safe predicate + unit. | Because the immediately-following move is forced `Coast`, the fail-safe halves the **whole velocity vector** (component-wise toward zero) while **`Coast` is illegal** from the fixed respawn cell `L`, re-checking each iteration, until `Coast` is legal or `v=(0,0)` (`Coast` from `L ∈ D` at `v=0` is trivially legal ⇒ termination guaranteed). This refines the design's looser "all moves illegal" to the operative "the forced-`Coast` move is illegal" — the two coincide except in the "`Coast` illegal but an accel legal" case, where the forced `Coast` still requires halving. |
| Head-on / corner crash producing `v=(0,0)`. | Allowed and correct: it is a **crash** `v=0` (scrub marker applies, progress pinned) — distinct from a player-chosen `Coast`-to-rest. AC5's "never a free controlled `v=0`" means the quench/fail-safe outcome always carries the crash marker, never an indistinguishable penalty-free stop. |
| Precondition (non-crash input). | `resolve_crash(d, s)` assumes `s` is a genuine crash (`legal_mask(d, s)` empty). Invoking it when some action is legal is a documented precondition; the guard form is the design phase's call. Non-panicking API posture per AGENTS.md § *API Naming*. |

## Technical constraints

- **Integer-only, deterministic core** (design §3a): all arithmetic integer, no
  RNG, no I/O — `resolve_crash` is a pure function of `(d, s)`. `⌊t/2⌋` and the
  fail-safe halving are integer division.
- **`clippy::arithmetic_side_effects = "deny"`** is active in the root
  `[workspace.lints.clippy]`: any velocity/position arithmetic uses
  `checked_*`/`saturating_*` or the documented fn-level
  `#[allow(clippy::arithmetic_side_effects, reason = …)]` house pattern with a
  domain-bound justification + covering tests (as in `step`, `supercover`,
  `Size::area`, `Rect::index`).
- **Reuse existing geom/sim** — `supercover` for the swept-cell walk,
  `Corridor::contains` for `∈ D`, and `legal_move`/`legal_mask` for the fail-safe
  legality re-check. Do **not** introduce a second legality/geometry path
  (single-legality-path invariant, as in `step`/`LapCounter`).
- Code + tests live in `crates/core/src/sim.rs` (`#[cfg(test)] mod tests`) — the
  existing home of `resolve_crash`. A new crash-outcome type may live there; no
  new crate/module unless design justifies one.
- Every new public item gains a ≥1-line `///`; the fallback variant is documented
  in `resolve_crash`'s doc.
- Gates: `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`
  all green.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | **Respawn position** is the last valid cell along the ordered coast sweep `(x,y) → (x+vx, y+vy)` before it leaves `D` — the furthest swept cell whose supercover-prefix is ⊆ `D`. Asserted as an exact post-crash `CarState.pos()` on a hand-built corridor. |
| AC2 | **Straight-wall crash** (glancing or head-on into a flat wall): the into-wall (normal) velocity component becomes `0` and the along-wall (tangential) component becomes `⌊t/2⌋` (integer `t/2`, sign preserved). Asserted as exact post-crash `(vx, vy)`. |
| AC3 | **Concave-corner crash** — both the x- and y-neighbor of the respawn cell are `¬D`: **both** conflicting components are zeroed ⇒ `v = (0,0)`. Asserted exactly. |
| AC4 | The **scrub-tick blocks re-acceleration for exactly one tick**: immediately after the crash the only legal action is `Coast` (the scrub-aware mask == `{Coast}`), and on the following tick the full `legal_mask` applies again. Asserted via the 3a crash-outcome + scrub mask. |
| AC5 | **Fail-safe**: when the forced `Coast` off the respawn state is illegal, the velocity is halved (toward zero) repeatedly, re-checking `Coast` each iteration, until `Coast` is legal or `v=(0,0)`; an over-speed entry whose damped coast still exits `D` terminates at exactly `v=(0,0)`. The outcome always carries the crash/scrub marker — **never** a penalty-free controlled `v=0`. Asserted as the exact terminal `CarState`. |
| AC6 | `resolve_crash` is **pure and deterministic** (no RNG, no I/O): identical `(d, s)` inputs yield an identical crash outcome (asserted). |
| AC7 | No `arithmetic_side_effects` / clippy violation (`-D warnings` green); `resolve_crash`'s doc records the `v=0 + skip P ticks` fallback as the documented alternative if `⌊t/2⌋` proves finicky. |

## Open questions

None design-blocking. The swept-segment ordered walk, wall-normal predicate,
crash-outcome type shape, `⌊t/2⌋` rounding convention, and the scrub-mask
mechanism are recorded as defaults in **Key decisions** for the design phase to
finalize. The `⌊t/2⌋`-vs-`v=0+skip P` calibration is genuinely empirical
(**Deferred**), not resolvable on paper.
