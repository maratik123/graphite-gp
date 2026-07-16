# Design: gp-core signed S/F lap counter — half-open half-grid crossing + valid-finish conjunction

**Issue:** #8
**Date:** 2026-07-16

## Approach

`LapCounter::register_move` scores the committed-legal chord `from → to` against the
timing gate `sf.gate` and mutates `self.counter` (`+1` forward, `−1` reverse, `0`
otherwise, ≤1 event per call). The gate's supporting line is the **half-grid dual
edge** one edge ahead of the `behind` row (design §2 Ф3 lines 46–49; §3 [C2] line 242),
so no integer `Point` ever lands on it. All scaffolding (`LapCounter`, `StartFinish`,
`TimingGate`, `Side::delta`, `legal_move`, `step`, `supercover`) already exists and
is correct — this task writes the `register_move` body (replacing `todo!`), plus the
integer-only crossing helpers and the AC1–AC7 tests. Everything lives in
`crates/core/src/sim.rs`.

### Chosen coordinate formulation — doubled signed perpendicular coordinate

Integer-only, deterministic (design §3a). Let `r = sf.gate.behind[0]` be the reference
cell (all `behind` cells share the same projection along `forward` — they are the gate's
cross-section, perpendicular to `forward`; documented reliance on the `TimingGate`
cross-section invariant the generator upholds) and `(dx, dy) = sf.gate.forward.delta()`
the integer unit axis vector (exactly one component is `±1`, the other `0`; via
`Side::delta`, **not** `TimingGate::forward_unit`, which is the f32 tangent accessor).

Define the **doubled** signed perpendicular coordinate of a point `p`:

```
gate_coord(p) = 2 * ((p.x − r.x)·dx + (p.y − r.y)·dy)
```

- `behind` row (`r`'s row) → `0` (even); `behind + forward` row → `+2` (even); each
  further row ±`2`. The half-grid gate line falls at `+1` — the **odd** midpoint.
- Real integer `Point`s always yield an **even** `gate_coord`, so the odd line value is
  unreachable in play (design §2 Ф3: «на линии геометрически невозможно»). The doubling
  is exactly what makes the half-grid line an addressable *integer* (`GATE_LINE = 1`)
  without any fractional coordinate — the spec's requested "behind→even, ahead→even,
  line→odd" form, expressed relative to `r` so the origin is the gate itself.

`const GATE_LINE: i32 = 1;` — the odd midpoint; the line belongs to the `+` (forward)
side, per §3 [C2] "`to` on the `+` side **or on the line**". The signed event:

```
crossing_event(from_c, to_c):
    forward (+1) ⟺ from_c <  GATE_LINE  ∧  to_c >= GATE_LINE   // from strictly −, to +-or-on-line
    reverse (−1) ⟺ from_c >= GATE_LINE  ∧  to_c <  GATE_LINE   // from +-or-on-line, to strictly −
    else 0
```

Because the coordinate is evaluated only at the two endpoints and a straight segment is
monotone in the perpendicular coordinate, the endpoint-only test yields **at most one**
event even for a long chord (AC2) — there is no per-cell scan to double-count. The
`>= GATE_LINE` (`+`-or-on-line) half makes forward/reverse exact mirrors, so a
forward-then-reverse pair telescopes to net `0` (AC6). Verified against the full spec
test table (fixture `behind=[(1,1)]`, `forward=East` ⇒ `gate_coord(p)=2·(p.x−1)`,
`GATE_LINE=1`): all 8 rows reproduce the tabulated `Δcounter`.

`register_move` body:

1. **Empty-gate guard** — `if sf.gate.behind.is_empty() { return; }` (no edge ⇒ no line;
   non-panicking no-op, per Key Decision + AC7).
2. `from_c = gate_coord(from)`, `to_c = gate_coord(to)`.
3. `event = crossing_event(from_c, to_c)`.
4. `self.counter = self.counter.saturating_add(event)`.

### Key decisions (design-phase calls the spec left open)

- **Drop `race_dir`.** The crossing sign derives entirely from `sf.gate.forward` (the
  local `+race_dir` projection), never from the global `Cw`/`Ccw`. `race_dir` is
  redundant, and `CARGO_BUILD_WARNINGS=deny` forbids a lingering `_race_dir`. Clean break
  per AGENTS.md § API Stability, mirroring #7's dropped `d`. New signature:
  `pub fn register_move(&mut self, sf: &StartFinish, from: Point, to: Point)`. Verified
  (`rg -Un 'register_move'`, multiline-aware): **no callers exist** (only the def + two
  doc mentions in `track.rs`), so the drop is safe. `use crate::track::StartFinish;`
  (drop `RaceDir`); add `Side` to the geom `use` for the `gate_coord` signature.

- **Valid-finish conjunction = inline, no new production predicate.** Design §3 frames it
  as «не отдельное правило» — a conjunction of two *existing* predicates, with `legal_move`
  the single legality path (#7). `register_move` stays `-> ()` and legality-agnostic; its
  `///` states the caller's obligation to gate on `legal_move` first. AC5 demonstrates the
  ordering at the call site (`if legal_move(&d, s, a) { lap.register_move(&sf, from, to); }`).
  Adding a `valid_finish` wrapper would materialise the "separate rule" the design
  deliberately avoids and re-wrap the single legality path — rejected (YAGNI, no caller).

- **`register_move` keeps `-> ()`.** No caller needs the per-move delta; the win check
  reads `laps()`/`raw()` (deferred tick loop). Rejected: returning the signed event.

- **Lateral extent — perpendicular-only.** S/F is a full chord cutting the annulus into a
  simply connected strip (design §3), and `legal_move` (checked first in the conjunction)
  keeps every scored chord in `D`; so any perpendicular crossing of a legal in-`D` chord
  *is* a gate crossing. No lateral-span guard (not required for correctness on the
  intended domain; would be speculative). Tests keep each move's lateral coordinate within
  the fixture gate's span.

- **Overflow — house `#[allow]` on `gate_coord`, `saturating_add` for the counter.**
  `gate_coord` carries a fn-level `#[allow(clippy::arithmetic_side_effects, reason=…)]`
  with a documented in-`D` precondition (`from`/`to`/`behind[0]` are grid-realistic,
  allocatable-corridor coordinates — the same domain `supercover`/`Size::area`/`step`
  document; pairwise coordinate differences and the `×2` doubling stay within `i32`).
  This matches the adjacent physics fns `register_move` composes with (`step`,
  `supercover`, `SField::gradient_at`). The counter update uses `saturating_add` (explicit
  semantics; lap counts never approach `i32::MAX` in any real game; panic-free) so
  `register_move` itself needs no `#[allow]`. `crossing_event` does only comparisons and
  literal returns — no arithmetic side effects, so it is a `const fn` with no `#[allow]`
  and is directly unit-testable at the odd line value (AC3).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Implement `register_move`: drop `race_dir` (final signature), fix `use` (`StartFinish` only; add `Side`), rewrite the `///` to the final contract (half-open half-grid rule, sign convention, score-before-collisions, caller's `legal_move`-first obligation, empty-gate no-op). Add `const GATE_LINE`, `fn gate_coord` (fn-level `#[allow]` + in-`D` precondition doc), `const fn crossing_event`, and the body (empty-gate guard → two `gate_coord` → `crossing_event` → `saturating_add`). | `crates/core/src/sim.rs` | — |
| 2 | Verify `register_move` (tests AC1–AC7) in the existing `#[cfg(test)] mod tests`: delta table, long-chord single event, no-rescore, `crossing_event` raw odd-line unit test, `−1` init / `laps`, legality-gated valid-finish, scripted telescoping sequence + parallel move, empty-gate no-op. | `crates/core/src/sim.rs` | 1 |

`M = 2` (well under the 15-task split threshold). **TDD note:** AGENTS.md TDD (test-first)
is exercised *inside* the implementor's edit loop (write a failing check, watch red, make
green); the committed subtask boundary lands impl-then-confirming-tests to keep each
commit's gate green — matching this crate's established `feat`→`test` two-commit rhythm
(commits `20af…` implement → `7a6f…`/`c7d6…` confirm). Both subtasks touch only
`crates/core/src/sim.rs`.

## Handoff plan

Per the every-group handoff contract (`.claude/skills/task/SKILL.md` Step 8; design.md
§ Rules → handoff-grouping (a)–(h)): grouping is required for every `M ≥ 1` (a); max group
size is **10** consecutive subtasks (b); the handoff destination is `/context-reset` per
`.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) (c); the terminal
group is sized `1..=10` (d); every group is change-type-homogeneous (e); groups are
minimized to the fewest possible (f); each group is marked with its implementor model +
effort (g); the default max is 4 groups (h). Both subtasks are **code** (`*.rs`) and share
one file, so they cluster into a single homogeneous group — the minimum.

- **Handoff into Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md`
  § Compaction recovery (re-entry) before starting subtask 1.
- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, 1M-token window,
  via the `code-writer` subagent (frontmatter-pinned `model: sonnet` + `effort: medium`;
  no inline override) — subtasks 1–2 (code change-type: `crates/core/src/sim.rs`).
  **Terminal group** (2 subtasks; within the `1..=10` range). Only group — no inter-group
  handoff; the group completes Step 8 in its own `/context-reset` subagent.

## Risks

- **`behind` cells not co-linear along `forward` (malformed gate).** `gate_coord` uses
  `behind[0]` as the projection origin. Mitigation: the `TimingGate` cross-section
  invariant (all `behind` cells perpendicular to `forward`, generator-upheld) is a
  documented precondition; behavior stays *defined* (uses `behind[0]`'s row) even if
  violated. Empty `behind` is guarded first (no-op).
- **Overflow off the in-`D` domain (adversarial `Point`s).** Mitigation: `gate_coord`'s
  fn-level `#[allow]` documents the grid-realistic/allocatable precondition (matching
  `supercover`/`step`/`Size::area`); the counter uses `saturating_add`; the empty-gate
  branch is guarded. No panic anywhere in-domain; `crossing_event` is pure comparison.
- **`clippy::arithmetic_side_effects = deny`.** Mitigation: the only side-effecting ops are
  the two subtractions + `×2` in `gate_coord` (under the justified `#[allow]`) and the
  counter's `saturating_add` (explicit). `crossing_event` and the empty-gate guard add none.
- **Doc gate (`RUSTDOCFLAGS="-D warnings"`).** Mitigation: the new `///` uses the in-crate
  intra-doc links `[`legal_move`]` / `[`StartFinish`]` (both in scope) and keeps design-doc
  citations (§2 Ф3, §3 [C2]) as plain text, not links.
- **`race_dir` drop breaking a future caller.** Mitigation: none exists today (verified);
  the deferred tick loop is written against the new signature from the start.

## Test Design

All tests live in `crates/core/src/sim.rs` `#[cfg(test)] mod tests`. A helper builds the
fixture `StartFinish` (only `sf.gate` is read by `register_move`): `gate = TimingGate {
behind: vec![Point::new(1,1)], forward: Side::East }`, with a sensible `chord`/`orient`
(unused by the counter). `gate_coord(p) = 2·(p.x−1)`, so `x ≤ 1 ⇒ −`, `x ≥ 2 ⇒ +`,
`GATE_LINE = 1` (no integer `x` reaches it). Entry points: `LapCounter::register_move`,
`LapCounter::{new, default, laps, raw}`, private `crossing_event`, and `legal_move` (AC5).

- **AC1 — forward / reverse / no-cross.** Table-driven over the spec rows: `(1,1)→(2,1)`
  `+1`; `(3,1)→(1,1)` and `(2,1)→(1,1)` `−1`; `(0,1)→(1,1)` `0`. Fresh counter per case;
  assert `raw()` delta exactly.
- **AC2 — single event on a long chord.** `(0,1)→(4,1)` yields exactly `+1` (not `+2`);
  symmetric long reverse `(4,1)→(0,1)` yields exactly `−1`.
- **AC3 — no re-score + raw half-open at the odd line.** `(2,1)→(3,1)` (ahead→ahead) `0`;
  mirror `(1,1)→(0,1)` (behind→behind) `0`. Plus a direct unit test of the private
  `crossing_event`: `crossing_event(0, 1) == 1` (from `−`, to on-line ⇒ forward — locks
  line ∈ `+`), `crossing_event(1, 0) == −1`, `crossing_event(2, 4) == 0`, with a comment
  noting real `Point`s never produce the odd `1` value (design §2 Ф3).
- **AC4 — init and `laps`.** `LapCounter::new()` and `default()` ⇒ `raw() == −1`,
  `laps() == 0`; first forward cross ⇒ `raw() == 0`, `laps() == 0`; second ⇒ `raw() == 1`,
  `laps() == 1`.
- **AC5 — valid-finish conjunction, legality first.** Corridor `D = {(1,0),(1,1),(2,1)}`
  (deliberately **not** `(2,0)`), same fixture gate. *Illegal* would-be forward-crosser:
  car `(1,0)`, `v=(0,1)`, `Action::East` ⇒ `step`→`(2,1)`, chord `(1,0)→(2,1)` crosses
  forward but `supercover` hits the dual-vertex tie incl. off-`D` `(2,0)` ⇒
  `legal_move == false`; guarded `if legal_move {…}` skips ⇒ `counter` unchanged (assert
  `(2,1) ∈ D` so rejection is non-vacuous, mirroring `legal_move_rejects_wall_clipping_chord`).
  *Legal* forward-crosser: car `(1,1)`, `v=(0,0)`, `Action::East` ⇒ `(2,1)`, `supercover
  {(1,1),(2,1)} ⊆ D` ⇒ `legal_move == true` ⇒ `register_move` runs ⇒ `counter` `+1`.
- **AC6 — scripted telescoping sequence + parallel move.** From a fresh counter: forward ⇒
  `raw()==0, laps()==0`; reverse ⇒ `raw()==−1, laps()==0`; two forwards ⇒ `raw()==1,
  laps()==1`. A back-and-forth pair (`(1,1)→(2,1)` then `(2,1)→(1,1)`) nets `0`. A parallel
  move `(2,0)→(2,3)` (pure-`y`, constant `gate_coord`) leaves `counter` unchanged.
- **AC7 — empty-gate no-op.** `gate.behind = vec![]` ⇒ `register_move` leaves `counter`
  unchanged and does not panic. (The clippy / doc / build gates are enforced by CI.)

Fixtures/helpers: a `fn sf_east_gate() -> StartFinish` builder; a `fn car(x,y,vx,vy) ->
CarState` shorthand for AC5; reuse of the existing `Corridor::new` + `set` pattern.

## Open questions

None design-blocking. The `race_dir` drop, the doubled-coordinate exact form + overflow
handling, the perpendicular-only (no lateral-span) choice, and the inline valid-finish
conjunction are all resolved above per the spec's *Key decisions* defaults.
