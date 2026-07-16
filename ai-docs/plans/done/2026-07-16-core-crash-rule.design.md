# Design: gp-core crash rule (`sim::resolve_crash`)

**Issue:** #9 (block 3a, build-order 6/40)
**Date:** 2026-07-16

## Approach

Replace the `todo!("crash rule …")` stub in `crates/core/src/sim.rs` with the
finalized quench-with-scrub rule (design `docs/design.md` §3 "Краш", `[D4]`,
`[N5]` — all ФИНАЛИЗИРОВАНО). The crash is a search dead-end: `s = (x, y, vx, vy)`
with `s.pos() ∈ D` where `legal_mask(d, s)` is empty. `resolve_crash` produces the
post-crash kinematic state plus a scrub-tick marker; it is a pure function of
`(d, s)`, integer-only, non-panicking (and non-hanging — see § (5)).

**Precondition (documented on `resolve_crash`).** `A = s.pos() ∈ D`,
`legal_mask(d, s)` is empty, and the coast chord `A → B` lies in `supercover`'s
**bounded-chord domain** — a realistic single-move velocity (`|vx|, |vy| ≪
1.5×10⁹`). In any allocatable corridor `|v|` is bounded by the corridor's size, so
the sweep is short and the `i64` cross product cannot overflow; `resolve_crash`
**inherits** this precondition verbatim from `supercover` (`crates/core/src/geom/mod.rs`).
An adversarially astronomical `v` (a multi-billion-cell diagonal chord) is
out-of-domain — unsupported exactly as it is for `supercover`/`step`/`gate_coord`
(cf. R5). Within the domain the function is total; outside it, the `checked_add`
guard on `B` plus the `L ∈ D` fail-safe guard (§ 5) still prevent any panic **or**
hang for the residual `i32`-overflow edge — the function degrades, it never loops.

The whole rule is expressed over the coast/momentum segment `A = (x,y) →
B = (x+vx, y+vy)` and reuses the three existing primitives (`supercover`,
`Corridor::contains`, `legal_move`) — no second geometry or legality path
(single-legality-path invariant, as in `step`/`LapCounter`).

### The five delegated decisions, locked

**(1) Ordered coast walk + respawn cell `L`.**
`supercover(A, B)` is the source of truth for *which* cells the sweep touches, but
it yields the set in bounding-box scan order, not path order. Impose an integer
order by projection onto the coast direction `dir = (vx, vy)`:

```
proj(c) = vx·(c.x − A.x) + vy·(c.y − A.y)          # i64, monotone along the sweep
t_block = min { proj(c) : c ∈ cover, ¬d.contains(c) }   # nearest ¬D cell's projection
L       = argmax { proj(c) : c ∈ cover, proj(c) < t_block }   # furthest reachable D cell
```

Every ¬D cover cell has `proj ≥ t_block`, so **every** cell with `proj < t_block`
is in `D` — the prefix up to `L` is clean by construction, no per-cell membership
filter needed. `A` has `proj = 0 < t_block` (since `A ∈ D`), so the candidate set
is non-empty and `L` is well-defined; the selection falls back to `A` when the
candidate set is empty (a ¬D cell at `proj ≤ 0`) or `L = B` when `cover ⊆ D`
(non-crash input, `t_block = +∞`). Ties at the maximum projection (only the
symmetric 45° dual-vertex graze) break by lexicographic `(x, y)` for determinism
(AC6). This is exactly the spec's "furthest swept cell whose supercover-prefix is
⊆ `D`".

*Rejected:* (a) a DDA/Bresenham ordered cell walk — a second geometry path that
could disagree with `supercover` about touched cells; forbidden by the
single-geometry-path constraint. (b) per-candidate reachability
`supercover(A, c) ⊆ D` — **wrong** for off-axis side cells: for `c` not collinear
with `A→B` it computes the cover of a *different* sub-segment (e.g.
`supercover((0,0),(1,0))` for a diagonal coast). Projecting the real `A→B` cover
set is correct.

**(2) Wall-normal classification at `L`.**
A local axis-neighbor probe in the travel direction (spec's chosen predicate):

```
into_wall_x = ¬d.contains(L + (vx.signum(), 0))    # signum ∈ {−1,0,1}; 0 ⇒ neighbor = L ∈ D
into_wall_y = ¬d.contains(L + (0, vy.signum()))
```

`into_wall_x ⇒ vx := 0`; `into_wall_y ⇒ vy := 0`; both ⇒ **corner** ⇒ both zeroed.
The surviving axis is along-wall. A zero velocity component has `signum = 0`, so
its probe returns `L` (in `D`) and never mis-classifies. Because `L` is the
*furthest* reachable cell, at least one forward axis-neighbor is ¬D in every
straight-wall/corner case, so the probe always zeroes ≥1 component (see Risks R1);
the fail-safe (5) is the guarantee for any residual degeneracy. This is consistent
with design `[D4]`'s "wall hit first along the swept segment": the nearest blocker
is adjacent to `L` along a travel axis.

**(3) `⌊t/2⌋` semantics.**
The surviving along-wall component `t` is damped with Rust integer division
`t / 2` — truncation toward zero, sign preserved (`5/2 = 2`, `−5/2 = −2`,
`1/2 = 0`). Verified lint-clean under `arithmetic_side_effects` (division by the
non-zero literal `2` never overflows or divides by zero). Locked by the exact
`(vx, vy)` assertion in AC2.

**(4) Crash-outcome type (clean break from `-> CarState`).**
`resolve_crash` has zero callers (verified), so AGENTS.md § *API Stability*
permits the signature change:

```rust
pub struct CrashOutcome { pub state: CarState, pub scrub: bool }

impl CrashOutcome {
    pub fn action_mask(self, d: &Corridor) -> BitFlags<Action>   // scrub ⇒ {Coast}, else legal_mask
    pub fn consume_scrub(self) -> CrashOutcome                    // total: scrub ⇒ step(Coast)+clear; else self
}

pub fn resolve_crash(d: &Corridor, s: CarState) -> CrashOutcome
```

Derives `Clone, Copy, PartialEq, Eq, Hash, Debug` (matches `CarState`, needed for
`assert_eq!`/`assert_matches!`). The scrub tick is a **real forced-`Coast` move**
(design "один ход без права реакселерации", `[N5]` "константная цена в один тик"):
`action_mask` returns the singleton `{Coast}` while `scrub` holds, then the full
`legal_mask` resumes. `consume_scrub` is **total**: when `scrub == true` it applies
the forced `Coast` (guaranteed legal by (5)) and clears the marker; when
`scrub == false` it is a no-op returning `self` unchanged (never double-advances a
spent outcome). This makes AC4 unit-testable entirely inside `gp-core`.
*Rejected:* encoding scrub as a `CarState` sentinel — not expressible, and AC4
needs the mask + advance operations.

**(5) Fail-safe (with the mandatory `L ∈ D` termination guard).**
The loop's termination proof **requires `L ∈ D`** (so that `Coast` at `v = (0,0)`
is legal — `supercover(L,L) = {L} ⊆ D`). Whenever `A ∈ D` (the precondition),
`L ∈ D`: the walk (1) only ever returns a `proj < t_block` cell (all `∈ D`), and
both `A`-fallbacks (`B`-overflow ⇒ `L = A`; and `respawn_cell`'s own `unwrap_or(A)`
when the candidate set is empty) return `A ∈ D`. The **one** way `L ∉ D` is an
out-of-precondition `A ∉ D` reaching either `A`-fallback — for which `Coast` from
`L` is illegal at *every* `v` (including `(0,0)`), i.e. an **infinite loop**. So
resolve_crash resolves `L` first, then guards the loop with `L ∈ D` (which covers
both `A`-fallback paths) and degrades safely otherwise:

```
if !d.contains(l) {
    // out of precondition (A ∉ D): cannot coast-check from L → do not enter the loop.
    return CrashOutcome { state: CarState { x: l.x, y: l.y, vx: 0, vy: 0 }, scrub: true };
}
let mut v = quench_velocity(d, l, s.vx, s.vy);
while !legal_move(d, CarState { x: l.x, y: l.y, vx: v.0, vy: v.1 }, Action::Coast) {
    v = (v.0 / 2, v.1 / 2);
}
```

`legal_move` is itself `checked_add`-based, so the loop is overflow-safe for any
`v`; with the guard ensuring `L ∈ D`, `(0,0)` is the guaranteed termination floor
(≤ ~31 iterations). This refines the design's looser "all moves illegal" to the
operative "the forced-`Coast` move is illegal". *Rejected:* re-quenching
(recomputing the normal) each iteration — the spec mandates blind whole-vector
halving; it is simpler and terminates.

`resolve_crash` always returns `scrub: true` — a crash never yields a penalty-free
controlled `v = 0` (`[N5]`; AC5).

### Arithmetic strategy (empirically verified)

`arithmetic_side_effects = "deny"` is active. A scratch-crate `cargo clippy` run
confirms: `t / 2`, `(vx / 2, vy / 2)`, and `x.checked_add(vx.signum())` are
**lint-free**; only i64 products (`vx·(c.x−A.x)`) trigger the lint. Therefore a
single scoped fn-level `#[allow(clippy::arithmetic_side_effects, reason = …)]` on
the projection helper (`respawn_cell`) — with a bounded-chord domain justification
mirroring `supercover` — is the entire allow surface. `B = (x+vx, y+vy)` uses
`checked_add` (overflow ⇒ respawn in place, `L = A`); neighbor probes use
`checked_add(signum)`; damping/halving use `/ 2`. No other `#[allow]`.

### Structure

Two private helpers in `sim.rs` keep the allow scoped and each piece unit-sized:
`respawn_cell(d, a, b) -> Point` (the ordered walk; carries the i64 allow) and
`quench_velocity(d, l, vx, vy) -> (i32, i32)` (probe + zero + `⌊t/2⌋`; lint-free).
`resolve_crash` computes `B` (`checked_add`) and **resolves `L` first** —
`L = if B-overflow { A } else { respawn_cell(d, A, B) }` — **then** applies the
`L ∈ D` guard (early-return `v=(0,0)` scrub outcome if it fails), **then** calls
`quench_velocity`, runs the fail-safe loop, and assembles `CrashOutcome`. No new
module or crate (spec constraint; YAGNI).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `CrashOutcome` struct (derives + `///`), `action_mask` + `consume_scrub` methods, and change `resolve_crash` to `-> CrashOutcome` with a temporary `todo!` body (keeps the crate compiling). | `crates/core/src/sim.rs` | — |
| 2 | (TDD, red) Add the AC1–AC6 + Robustness (no-panic/no-hang: axis-aligned overflow with `A∈D`, and the `A∉D` guard case) `#[cfg(test)]` unit tests with the verified fixtures/expected states from § Test Design. | `crates/core/src/sim.rs` | 1 |
| 3 | Implement private `respawn_cell(d, a, b) -> Point`: collect `supercover(a,b)`, i64 `proj`, `t_block`, max-`proj`/lex-tiebreak selection, `unwrap_or(a)`; scoped fn-level `#[allow(arithmetic_side_effects, reason=…)]` (bounded-chord domain, supercover-style). | `crates/core/src/sim.rs` | 1 |
| 4 | Implement private `quench_velocity(d, l, vx, vy) -> (i32,i32)`: axis-neighbor probe via `checked_add(signum)`, zero into-wall axes, damp survivor `⌊t/2⌋`. | `crates/core/src/sim.rs` | 1 |
| 5 | Assemble `resolve_crash` in order: `B` via `checked_add`; **resolve `L` first** (`L = if B-overflow { A } else { respawn_cell(d, A, B) }`); **then the `L ∈ D` guard** (early-return `v=(0,0)` scrub if `!d.contains(L)`, so an out-of-precondition `A ∉ D` reaching either `A`-fallback can never hang the loop); **then `quench_velocity`**; **then** the whole-vector-halving fail-safe loop via `legal_move`; return `CrashOutcome{ state, scrub: true }`. Write the full doc comment incl. the documented **precondition** (`A ∈ D`, `legal_mask` empty, bounded-chord `v` inherited from `supercover`) and the `v=0 + skip P ticks` fallback variant (AC7). | `crates/core/src/sim.rs` | 3, 4 |
| 6 | Run all gates (`cargo fmt --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`); turn the AC tests green; confirm the single justified `#[allow]` is the only arithmetic allow. | `crates/core/src/sim.rs` | 2, 5 |

## Handoff plan

Grouping is required for every `M ≥ 1` (here `M = 6`). Every subtask changes the
same code file `crates/core/src/sim.rs` (Rust `*.rs`) — a single homogeneous
**code** change-type — so the fewest-groups minimization yields exactly one group.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, 1M-token
  window, via the `code-writer` subagent (frontmatter-pinned `model: sonnet` +
  `effort: medium`; no inline override) — subtasks **1–6** (code change-type:
  `crates/core/src/sim.rs`). **Terminal group** (6 subtasks; within `1..=10`).
- **Handoff into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) at the
  start of the group. There is no inter-group handoff — the single group completes
  `/task` Step 8 in its own `/context-reset` subagent.

Group count = 1 (≤ 4; no user gate needed). The `design`, `design-review`,
`self-review`, and `spec-writer` subagents stay on Opus regardless of the group's
`sonnet` implementor marker.

## Risks

- **R1 — fail-safe loop-to-exactly-`(0,0)` is unreachable for a single wall.**
  Because `L` is the *furthest* reachable cell, the surviving axis's immediate
  neighbor is in `D` (that is why it survived), so a unit coast along it is always
  legal ⇒ a single-axis survivor's fail-safe terminal is `≥ 1`, never `0`. `(0,0)`
  terminals therefore arise from the **quench** (corner ⇒ both zeroed; or
  `⌊t/2⌋ = 0`), which is the fail-safe's guaranteed `(0,0)` floor with zero loop
  iterations. *Mitigation:* implement the whole-vector halving exactly as the spec
  states (the `(0,0)` floor branch is correct and defensive); AC5 asserts the two
  observable facts separately — (a) the loop **fires and reduces** a single-axis
  survivor to an exact state, and (b) a crash producing `v = (0,0)` **carries the
  scrub marker** (never penalty-free). This split is called out so review reads it
  as a deliberate, faithful realization of AC5, not an omission.
- **R2 — 45° dual-vertex graze ties `L`.** Two side cells can share the maximum
  projection. *Mitigation:* deterministic lexicographic `(x, y)` tiebreak (AC6);
  every asserted AC fixture uses a straight wall or a clean concave corner where
  `L` is unique, so the tie never affects an asserted state.
- **R3 — large-`v` robustness, reconciled with `supercover`'s bounded chord.**
  `resolve_crash` inherits `supercover`'s **bounded-chord precondition** (realistic
  single-move `v`, `|v| ≪ 1.5×10⁹`; see § Approach → *Precondition*). An
  adversarially astronomical *diagonal* chord (e.g. `A=(0,0)`, `v≈(2×10⁹, 2×10⁹)`
  with `B` still `< i32::MAX`) is therefore **out-of-domain** — like R5, not
  defended: `checked_add` on `B` does not trip, but `respawn_cell → supercover`
  would scan/collect ~10¹⁸ cells (and overflow `2·cr` in `i64`). This is the same
  domain limit `supercover`/`step`/`gate_coord` already carry. *What is still
  guaranteed within-domain:* the residual `i32`-overflow edge — an **axis-aligned**
  near-`i32::MAX` `v` (e.g. `car(1,0,i32::MAX,0)`) where `B.x = x+vx` overflows
  `i32` — is handled non-panickingly and non-hangingly: `checked_add` fails ⇒
  `L = A` (walk skipped, so no giant `collect`), and because that chord is
  axis-aligned (`dy=0` ⇒ `cr=0`, no `i64` overflow) the fail-safe's `legal_move`
  short-circuits at the first `¬D` cell and halves `v` to a legal value. The
  `L ∈ D` guard (§ 5) additionally prevents any hang if a caller violates the
  `A ∈ D` precondition. *Mitigation summary:* `checked_add` on `B` + `L ∈ D` guard
  + inherited bounded-chord precondition; covered by the two Robustness tests
  below (axis-aligned overflow with `A ∈ D`; and the `A ∉ D` guard case).
- **R4 — `arithmetic_side_effects`.** Verified: only the i64 projection lints ⇒
  one scoped fn-level `#[allow]` with a bounded-chord domain reason on
  `respawn_cell`; everything else is `checked_add`/`/2` (lint-free). AC7.
- **R5 — non-crash input.** `resolve_crash` documents the precondition (`A ∈ D`,
  `legal_mask(d, s)` empty). A degenerate non-crash input (`cover ⊆ D`) yields a
  safe best-effort outcome (`L = B`, damped velocity) and never panics; behavior
  outside the precondition is documented as unsupported, not defended per-branch.
- **R6 — pedantic cast lints in test fixtures.** `--all-targets` clippy also lints
  test code; a fully-drivable-rect helper must avoid `usize as i32`
  (`cast_possible_truncation`). *Mitigation:* iterate `0..i32::try_from(dim).unwrap()`
  (test `unwrap` is exempt) — spelled out in § Test Design.

## Test Design

All tests live in the existing `#[cfg(test)] mod tests` of
`crates/core/src/sim.rs`. Fixtures below are hand-verified (cover set, `t_block`,
`L`, quench, fail-safe traced by hand).

**Shared helper** (lint-clean fully-drivable rect):

```rust
fn filled(w: usize, h: usize) -> Corridor {
    let mut d = Corridor::new(Point::new(0, 0), w, h);
    for y in 0..i32::try_from(h).unwrap() {
        for x in 0..i32::try_from(w).unwrap() {
            d.set(Point::new(x, y), true);
        }
    }
    d
}
```
Reuse the existing `fn car(x, y, vx, vy) -> CarState` helper.

- **AC1 — respawn position (exact `pos()`).** Entry: `resolve_crash`. Fixture:
  `filled(3, 4)` (D = `x∈{0,1,2}`, `y∈{0,1,2,3}`), `car(1, 0, 3, 2)`. Sweep
  `(1,0)→(4,2)`, `t_block = 8` at `(3,1)`, `L = (2,1)`. Assert
  `out.state.pos() == Point::new(2, 1)`. (First assert `legal_mask(&d, s)` is empty
  — the input is a genuine crash.)
- **AC2 — straight glancing wall (exact `(vx,vy)`).** Same `filled(3, 4)` /
  `car(1, 0, 3, 2)`. At `L=(2,1)`: `(3,1)∉D ⇒ vx→0`; `(2,2)∈D ⇒` survivor `vy`,
  `⌊2/2⌋ = 1`. Assert `out.state == CarState { x: 2, y: 1, vx: 0, vy: 1 }`. Also
  add a **head-on** row (`vy = 0` survivor ⇒ `⌊0/2⌋ = 0`) to lock the `t/2`
  sign/truncation, and a sign case (negative survivor ⇒ negative `⌊t/2⌋`).
- **AC3 — concave corner ⇒ `v=(0,0)`.** Fixture `filled(3, 3)`, `car(0, 0, 3, 3)`.
  Sweep `(0,0)→(3,3)`, `L=(2,2)`; both `(3,2)∉D` and `(2,3)∉D` ⇒ corner. Assert
  `out.state == CarState { x: 2, y: 2, vx: 0, vy: 0 }`.
- **AC4 — scrub tick blocks re-accel for exactly one tick.** Reuse `filled(3, 4)`
  / `car(1, 0, 3, 2)` (crash ⇒ `state=(2,1,0,1)`). Assert:
  `out.scrub == true`; `out.action_mask(&d) == BitFlags::from(Action::Coast)`;
  `let out2 = out.consume_scrub();` ⇒ `out2.scrub == false`,
  `out2.state == CarState { x: 2, y: 2, vx: 0, vy: 1 }`,
  `out2.action_mask(&d) == legal_mask(&d, out2.state)`, and (non-vacuous)
  `legal_mask(&d, out2.state) != BitFlags::from(Action::Coast)` (at `(2,2,0,1)` the
  mask is `{Coast, West, South}`).
- **AC5 — fail-safe.** Two assertions:
  - *(a) loop fires + reduces survivor.* Fixture `filled(4, 2)` (D = `x∈{0,1,2,3}`,
    `y∈{0,1}`), `car(0, 0, 4, 3)`. `L=(2,1)`, quench `⇒ (2,0)`; `Coast(2,0)` from
    `(2,1)` lands at `(4,1)∉D` ⇒ halve `⇒ (1,0)`; `Coast(1,0)` → `(3,1)∈D` legal.
    Assert `out.state == CarState { x: 2, y: 1, vx: 1, vy: 0 }` and
    `out.scrub == true`.
  - *(b) crash-`v0` is scrub-marked (never penalty-free).* Reuse the AC3 corner
    outcome: assert its velocity is `(0,0)` **and** `out.scrub == true` — the
    guaranteed `(0,0)` floor carries the crash marker, distinguishing it from a
    player-chosen `Coast`-to-rest.
- **AC6 — pure/deterministic.** `resolve_crash(&d, s) == resolve_crash(&d, s)` on
  the AC2 fixture (derive `PartialEq`).
- **Robustness (R3) — no panic *and no hang*.** Two assertions:
  - *(a) axis-aligned `i32::MAX` overflow, `A ∈ D`.* `filled(5, 5)`,
    `car(1, 0, i32::MAX, 0)`: `A=(1,0) ∈ D`, `legal_mask` empty (mirrors the
    existing `legal_move` overflow fixtures), `B.x = 1 + i32::MAX` overflows `i32`
    ⇒ `checked_add` fails ⇒ `L = A` (walk skipped, so no giant `collect`). The
    chord is axis-aligned (`dy=0`), so `legal_move`'s `supercover` scan
    short-circuits at the first `¬D` cell (`x=5`) and the fail-safe halves `v` to a
    legal value in ≤ ~31 iterations. Assert it returns (no panic, no hang) and
    `out.scrub == true`. **Do NOT** use a `CarState { x: i32::MAX, .. }` here: that
    puts `A ∉ D`, and without the guard the `L ∉ D` fail-safe would loop forever.
  - *(b) `A ∉ D` guard.* `Corridor::new(Point::new(0,0), 5, 5)` (empty — nothing
    drivable, or any corridor not containing `A`), `CarState { x: i32::MAX, y: 0,
    vx: i32::MAX, vy: 0 }`. The `L ∈ D` guard (§ 5) fires: assert `resolve_crash`
    returns promptly with `out.state.vx == 0 && out.state.vy == 0` and
    `out.scrub == true` (safe degradation, no hang) — directly exercises the guard.
- **AC7** — no unit test: the `-D warnings` clippy + doc gates and the
  `resolve_crash` doc-comment recording the `v=0 + skip P ticks` fallback variant.

## Open questions

None design-blocking. The `⌊t/2⌋` vs `v=0 + skip P` calibration is genuinely
empirical (spec **Deferred**), documented in the `resolve_crash` doc-comment, not
resolved here.
