# Design: gp-gen Ф3 — start/finish, accel zone, start grid, timing gate

**Issue:** #26
**Date:** 2026-07-23

## Approach

Ф3 adds one new file `crates/gen/src/phase3.rs` plus a two-line `lib.rs`
wiring (`mod phase3;` + `pub use phase3::*;`), mirroring the Ф1/Ф2 module shape
`[measured: grep -n "^mod \|^pub use" crates/gen/src/lib.rs → mod phase1; mod
phase2; pub use phase1::*; pub use phase2::*;]`. The public entry is

```
pub fn phase3_start_finish(d: Corridor, skel: &CoarseSkeleton, m: u32, v_target: i32) -> Phase3Output
```

returning a **named struct** (not a bare tuple) — the spec's open question
leaves the shape to design; a struct with three named public fields reads far
better at the future `generate()` call site than `(Corridor, StartFinish,
StartGrid)` positional unpacking and follows Ф1's own `CoarseSkeleton`
named-struct precedent `[measured: sed -n '48,58p' crates/gen/src/phase1.rs →
pub struct CoarseSkeleton { pub ring…, pub hole…, pub dir… }]`:

```
pub struct Phase3Output { pub d: Corridor, pub sf: StartFinish, pub grid: StartGrid }
```

`v_target: i32` is an **explicit parameter**, not read from `GenParams`
(Key decisions — D3 forbids conflating `V_target` with `GenParams.v_ceiling`;
`generate()` wiring that plumbs a future `GenParams.v_target` is out of scope,
per Deferred) `[measured: grep -n "v_ceiling\|v_target" crates/gen/src/lib.rs →
only `v_ceiling` present on GenParams]`. `m: u32` matches
`GenParams.start_finish_width()`'s return type `[measured: sed -n '40,42p'
crates/gen/src/lib.rs → pub const fn start_finish_width(&self) -> u32]`.

**Consumed gp-core types (unchanged):** `Corridor, Point, Side, Orient` from
`geom`; `StartFinish, TimingGate, StartGrid, RaceDir` from `track`;
`sim::LapCounter` in tests only (AC6). All verified to exist and carry the
fields the design uses `[measured: sed -n '27,109p' crates/core/src/track.rs →
TimingGate{behind:Vec<Point>, forward:Side}, StartFinish{chord:Vec<Point>,
orient:Orient, gate:TimingGate}, StartGrid{positions:Vec<Point>}]`.

### Pipeline inside Ф3 (six stages, all integer-only, total, no RNG)

The phase consumes Ф1's `skel` (with the coarse `ring`/`hole`/`dir`) and Ф2's
fine corridor `d`, and runs:

1. **`pick_straight_run`** — select a straight (non-corner) coarse ring segment
   deterministically. Operates on the **coarse** `skel.ring` + `skel.hole`
   (`BTreeSet<Point>` — deterministic iteration, no `HashSet` leakage, AC8)
   `[measured: grep -n "pub ring\|pub hole" crates/gen/src/phase1.rs → both
   BTreeSet<Point>]`. Returns a `Segment` descriptor: the travel axis
   (`Orient`), the **inward normal** `Side` (toward the hole), the fixed
   perpendicular coarse coordinate, and the along-axis coarse run range.
2. **`forward_side`** — project `skel.dir` onto the segment tangent to the
   local-forward `Side` (§ Forward-direction convention below).
3. **`thicken`** — additively push D's outfield wall out at the segment cross
   section until the chord width `≥ m` (no-op when already `≥ m`). Never carves
   (additive, topology-preserving, mirroring Ф2's Stage-2/2b discipline).
4. **`front_chord`** — extract the front-row cross-section from the (thickened)
   D: the maximal contiguous D run perpendicular to travel, from the outfield
   wall inward toward the hole. Builds `StartFinish { chord, orient, gate }`
   with `gate.behind = chord`, `gate.forward = forward_side`.
5. **`start_grid`** — lay out `rows = m.div_ceil(width)` rows front-to-back
   along `−forward_side`, `m` distinct positions, all in D.
6. **`phase3_start_finish`** — orchestrate 1–5, assemble `Phase3Output`.

Budget **measurement** (accel zone forward; grid-straight backward) is
**test-only** — the spec's Scope says Ф3 "emits only `(D, sf, grid)`" and Key
decisions rules the budgets **measured, not guaranteed/returned**; the measuring
helpers therefore live in `#[cfg(test)]`, discharging AC2/AC7 on adequate
fixtures rather than adding a production API.

### Forward-direction convention (the crux of AC5)

`skel.dir` is a **declared** orientation label, drawn randomly by Ф1's
`choose_dir(rng)` — it is **not** derived from the ring's geometric winding, and
the ring is stored as an unordered `BTreeSet` with no traversal order
`[measured: sed -n '253,260p' crates/gen/src/phase1.rs → choose_dir returns
Cw/Ccw from rng.random_range(0..2), independent of ring cells]`. Consequently
"the Side a `skel.dir`-traversing car heads along the straight" (AC5) has meaning
**only** through a fixed projection convention that Ф3 defines. The convention,
in the y-up grid (`y` increases northward `[measured: sed -n '27,31p'
crates/core/src/geom/mod.rs → "increasing northward"]`), with `inward` =
the unit delta toward the hole:

- **CCW** (interior on the left of travel): `forward.delta() = (inward.y, −inward.x)`.
- **CW**  (interior on the right of travel): `forward.delta() = (−inward.y, inward.x)`.

Worked example (discharges the sign): a rectangular ring, bottom (south) arm,
hole to the north ⇒ `inward = North = (0, 1)`. CCW gives `forward = (1, 0) =
East`; CW gives `forward = (−1, 0) = West`. This matches the CCW unit-square
edge order `(0,0)→(1,0)` (East along the bottom) — the standard positive-signed-
area convention `[derived → AC5 asserts forward equals this formula on a
hand-built fixture; AC6 asserts the resulting gate/grid geometry is
LapCounter-self-consistent regardless of which Side forward resolves to]`. Since
no independent geometric winding is stored, AC5 is a **definitional** check
against this formula and AC6 a **self-consistency** check — together they fully
pin `forward`.

**Cross-phase convention warning (REC 1).** Because AC5 is definitional and AC6
is direction-agnostic, an *inverted* `forward` would pass **both** Ф3 tests
silently — harmless *within* Ф3 (the grid/gate stay mutually consistent whichever
Side `forward` resolves to), but **not** harmless downstream: the eventual Ф7
centerline and the AI progress/reward consumer both orient by this same
`race_dir` and MUST adopt **this exact projection convention**; a future phase
re-deriving the opposite sign would flip lap progress against the gate.
`forward_side` therefore carries a **required doc comment** (subtask 1) stating
the two rotation formulas above and this warning, so the sign is a single source
of truth rather than re-guessed per consumer.

### Straight-selection algorithm (`pick_straight_run`)

Deterministic, coarse, tie-broken for byte-reproducibility (AC8):

- A ring cell `c` is **hole-facing on side `s`** iff `c + s.delta() ∈ hole`.
- A **straight inner run** along an axis `A` is a maximal set of ring cells
  contiguous along `A`, all hole-facing on the *same* perpendicular side `s`
  (the inward normal), none of them a corner (a corner faces the hole on two
  distinct sides). Enumerated over `Side::iter()` order (fixed) and the
  `BTreeSet` ring order (fixed) `[measured: sed -n '20,32p'
  crates/core/src/geom/graph.rs → Side derives strum::EnumIter, order
  East,West,North,South]`.
- **Chosen run:** the longest such run (most forward headroom, per Key
  decisions), tie-broken by `(inward-side enum order, fixed-coord, run-start)` —
  a total order over deterministic keys, so same-`skel` runs reproduce
  bit-for-bit.

The along-axis coarse run length `× k` bounds the fine straight available; the
S/F front row is placed toward the **back** of the run (leaving `rows` cells of
grid straight behind it) so the forward accel headroom to the first corner is
maximized. Selection is coarse; the chord/grid/gate are built at fine D
coordinates (below).

### Thickening (`thicken`)

At the chosen segment, scan the perpendicular (cross-section) run of D from the
outfield wall inward; if its length `< m`, additively `d.set(.., true)` further
outfield cells (away from the hole) across the segment's full tangent span until
the run reaches `m`. Additive-only (never clears a cell), and the added region
is a solid rectangle attached to an existing straight wall, which cannot enclose
a new bounded pocket `[derived → the fixture topology test (§ Test Design,
subtask 3/6) asserts component_count(&d)==1 && bounded_complement_components(&d)
==1 after phase3, using the same gp-core predicates Ф2's Stage-3 uses]`. No-op
when the run is already `≥ m` (Key decisions). No Ф2-style pocket-absorption
pass is added — a straight-wall outward rectangle needs none, and the topology
test is the gate that would catch a violation rather than a bare assertion.

### Chord, gate, and grid construction

- **`front_chord`**: the maximal contiguous D run perpendicular to travel at the
  front-row tangent coordinate, starting at the outfield wall cell and walking
  inward toward the hole, stopping before the first `¬D` (hole) cell. Length =
  corridor width `≥ m` after thickening. `sf.orient` = perpendicular to travel
  axis (`Orient::Vertical` for an east–west straight, `Orient::Horizontal` for a
  north–south straight — the chord spans across the corridor, AC1).
- **`StartFinish`**: `chord` = front-row cross-section points; `gate.behind` =
  the same points; `gate.forward` = `forward_side`. The gate's implied dual
  edges sit one edge forward of the front row (`TimingGate` contract) `[measured:
  sed -n '18,60p' crates/core/src/track.rs → gate.behind holds drivable
  cross-section cells; implied edge {cell: behind[i], side: forward} one edge
  ahead]`.
- **`StartGrid`**: `rows = m.div_ceil(width)` (`div_ceil` is the same total
  integer op `GenParams::min_width` uses `[measured: grep -rn div_ceil crates →
  crates/gen/src/lib.rs:37 self.cars.div_ceil(2)]`); the front row occupies the
  first `min(m, width)` chord cells, each further row is the same cross-section
  shifted one cell along `−forward_side`, kept only where the cell ∈ D; collect
  distinct positions ordered front-to-back. With thickening making `width ≥ m`,
  `rows == 1` in the common case (a single abreast row); the general `div_ceil`
  layout is retained (cheap, matches the `StartGrid` contract).
- **`start_grid` degrade contract (NOTE 1).** On an adequate straight the result
  is exactly `m` distinct positions (AC3). When D is width-capped and cannot host
  `m` cells behind the front row (a short/narrow fixture), `start_grid` returns
  **as many distinct front-to-back cells as fit inside D — never a duplicate,
  never a `¬D` cell** — rather than padding to `m` with off-corridor or repeated
  points. Every emitted position is filtered on `d.contains(..)` and appended in
  front-to-back order with dedup, so the length simply falls below `m` in the
  degenerate case. Totality is intact (no panic, no `Result`); the AC3 "exactly
  `m`" guarantee is asserted only on the adequate fixtures, consistent with the
  spec's measured-not-enforced posture for the straight budgets.

Because `gate.behind` is the front row, every front-row cell has `gate_coord ==
0` and every cell one row back has `gate_coord == −2`, while `GATE_LINE == 1`
`[measured: sed -n '219,267p' crates/core/src/sim/mod.rs → GATE_LINE=1;
gate_coord doubles the signed perpendicular offset; crossing_event forward iff
from_c<1 && to_c>=1]`. So **every** start cell sits at `gate_coord ≤ 0 <
GATE_LINE` — strictly behind the gate — and a first forward move (front row →
one cell ahead, `gate_coord 0 → 2`) scores exactly `+1` (AC4/AC6), giving the
`LapCounter` init `−1` its documented self-consistency for all rows.

### Rejected alternatives

- **Fully coarse-ring selection *and* coarse construction, then scale to fine.**
  Rejected: Ф2's taper/absorption already moved D's outfield wall off exact
  `k`-block boundaries, so a coarse-derived chord would mis-locate the fine wall.
  Selection stays coarse (where "not a corner" is crisp) but chord/grid/gate are
  read from the actual fine D.
- **Returning a bare `(Corridor, StartFinish, StartGrid)` tuple.** Rejected for
  readability at the future `generate()` site; a named `Phase3Output` matches
  Ф1's struct precedent.
- **Adding a production accel/grid budget API.** Rejected — spec Scope emits only
  `(D, sf, grid)` and Key decisions rules the budgets measured (test-only), not
  returned.
- **A Ф2-style pocket-absorption pass after thicken.** Rejected as YAGNI for a
  straight-wall additive rectangle; the topology test is the gate.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Module scaffold + `lib.rs` wiring: `mod phase3; pub use phase3::*;`; module doc; `Phase3Output` struct; `Segment` descriptor; `phase3_start_finish` signature (best-effort, total); module `const`s; pure `const fn` helpers `forward_side(dir, inward)` (RaceDir+Side→Side match/rotation — carries the **required convention doc comment** per REC 1) and axis/perp `Orient`↔`Side` helpers — with their unit tests | `crates/gen/src/phase3.rs`, `crates/gen/src/lib.rs` | — |
| 2 | `pick_straight_run` over coarse `ring`/`hole` → `Segment` (travel axis, inward normal, fixed coord, along-axis range); deterministic tie-break | `crates/gen/src/phase3.rs` | 1 |
| 3 | `thicken` — additive outward widen of D at the segment to cross-section width `≥ m`; additive/hole-safe/no-op-when-wide; topology preserved | `crates/gen/src/phase3.rs` | 1, 2 |
| 4 | `front_chord` + `StartFinish`/`TimingGate` assembly (chord, `orient`, gate.behind, gate.forward) | `crates/gen/src/phase3.rs` | 3 |
| 5 | `start_grid` — `rows = m.div_ceil(width)`, `m` distinct front-to-back positions in D | `crates/gen/src/phase3.rs` | 4 |
| 6 | Orchestrate `phase3_start_finish`; test-only budget-measurement helpers (accel-zone forward, grid-straight backward); fixtures + end-to-end AC tests (AC2, AC6, AC8 determinism/snapshot, AC9 totality) | `crates/gen/src/phase3.rs` | 1–5 |

M = 6, all **code** (`*.rs`) — one homogeneous change-type.

## Handoff plan

Per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry),
the `/task` Step-8 every-group handoff contract binds a `/context-reset` handoff
at the start of every design-defined group, including the first and including
single-group designs.

- **(a) Grouping required:** M = 6 ≥ 1 — this `## Handoff plan` is mandatory.
- **Group A** — code change-type (`crates/gen/src/phase3.rs`,
  `crates/gen/src/lib.rs` — all Rust `*.rs`) → routes to the `code-writer`
  subagent, whose `model: sonnet` (sonnet-5) + effort `medium` are
  frontmatter-pinned (no inline model/effort override), 1M-token window —
  subtasks **1–6**. **Entry into Group A** spawns `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
  Terminal group (6 subtasks; within the `1..=10` range), completes Step 8 in
  its own `/context-reset` subagent.

Boundary sizing check: (b) max group size 10 — Group A holds 6 ≤ 10; (d)/(f)
terminal group is 6 ∈ `1..=10`; (e) homogeneous — every subtask edits Rust
`*.rs` only, no `*.md`/`.claude/**` in scope; (f) minimized — a single
change-type collapses to ONE group, the fewest possible; (h) 1 group ≤ 4, no
user gate needed. The `design`, `design-review`, and `self-review` gates stay on
Opus regardless of Group A's `sonnet` implementor marker.

## Risks

- **AC5 forward-sign convention could be inverted.** Mitigation: the worked
  rectangle example above pins the sign, and the test asserts `forward` against
  the explicit formula rather than intuition. — `[derived → subtask 1 unit test
  `forward_side` truth-table (Cw/Ccw × 4 inward sides) + subtask 6 AC5 fixture
  test]`.
- **Thicken creates a bounded pocket / disconnects D.** Additive straight-wall
  rectangle should not, but "should not" is not a check. — `[derived → subtask
  3/6 topology test: component_count(&d)==1 && bounded_complement_components(&d)
  ==1 post-phase3, via the gp-core predicates Ф2 Stage-3 already uses (measured
  crates/core/src/geom/graph.rs:122,151)]`.
- **`HashSet`-order or RNG leakage breaks determinism (AC8).** Ф3 draws no RNG
  and iterates only `BTreeSet` (ring/hole) + row-major D box scans. — `[derived →
  subtask 6 determinism test: two `phase3_start_finish` calls on identical
  `(d, skel, m, v_target)` compare byte-equal `Phase3Output`; plus an exact
  snapshot pin in the Ф1/Ф2 snapshot style]`.
- **Integer overflow on coordinate arithmetic.** Follow Ф1/Ф2:
  `saturating_*` / `try_from(..).unwrap_or(..)`, no raw ops without a justified
  `#[allow(clippy::arithmetic_side_effects, reason=…)]`. — `[derived → `cargo
  clippy --workspace --all-targets -- -D warnings` (arithmetic_side_effects is
  deny, measured Cargo.toml [workspace.lints.clippy]) + AC9]`.
- **`missing_const_for_fn` forces `const fn` on the pure helpers.** `forward_side`
  and the axis/perp `Orient`↔`Side` mappers are pure `match`/integer bodies with
  no conditionally-const callee (no `bool::then`, no closures over non-const
  calls), so they are const-eligible ⇒ MUST be `const fn` or clippy reds. —
  `[derived → the same clippy gate; nursery `missing_const_for_fn` is deny
  (measured Cargo.toml), precedent `pub const fn side_unit_f32`/`start_finish_width`
  are const]`.
- **File-size cap.** Ф1/Ф2 are 482/340 production lines (`< 500` soft) and
  1110/1064 incl-tests `[measured: wc -l crates/gen/src/phase{1,2}.rs → 1110,
  1064; grep -n "^#\\[cfg(test)\\]" → tests at 483/341]`; the spec cites them as
  the incl-tests-budget precedent. Ф3 is a smaller phase; keep production `< 500`
  and refactor helpers before merge if the incl-tests file nears the Ф1 size. —
  `[derived → self-review file-size check + AGENTS.md § Code Style]`.

## Test Design

All tests in `crates/gen/src/phase3.rs` `#[cfg(test)] mod tests`, mirroring
Ф1/Ф2 (unit + fixture + property + snapshot). Fixtures are hand-built
`CoarseSkeleton`s (Ф2's `fixture_jog` style) fed through
`crate::phase2_rasterize` to get D, then `phase3_start_finish`. Choose a small
`v_target` (e.g. `2` ⇒ threshold `v_target²/2 = 2`) so a modest fixture provides
an adequate straight for AC2/AC7.

- **Subtask 1 — `forward_side` + axis/perp helpers.** Entry: `forward_side`,
  `Orient`/`Side` mappers. Scenarios: full truth table Cw/Ccw × {East, West,
  North, South} inward against the documented rotation formula; `Orient`
  perpendicular round-trips. No fixtures.
- **Subtask 2 — `pick_straight_run`.** Entry: `pick_straight_run`. Scenarios:
  chosen run is a straight (both endpoints hole-facing on one side, non-corner);
  longest-run selection; determinism (two calls equal, byte-identical `Segment`);
  a rectangular fixture with a known longest arm returns that arm. Fixture:
  hand-built rectangular ring + hole.
- **Subtask 3 — `thicken`.** Entry: `thicken` (or its effect through
  `phase3_start_finish`). Scenarios: cross-section width `≥ m` after thicken;
  additive (`D_before ⊆ D_after` — no cell cleared); hole cells never set
  drivable; no-op when already `≥ m` (`k ≥ m` fixture); topology preserved
  (`component_count==1`, `bounded_complement_components==1`).
  **Boundary-margin fixture (REC 2):** assert the same two topology invariants
  (`component_count(&d)==1` **and** `bounded_complement_components(&d)==1`) on a
  second fixture where the outward push reaches D's bounding-box margin — the one
  case where added cells could touch the box border — not only on an interior
  fixture. (Ф2 pads its bounding box by `BBOX_PAD=1` `[measured: sed -n '18,23p'
  crates/gen/src/phase2.rs → const BBOX_PAD: i32 = 1]`; if thicken's push would
  otherwise exceed the margin, thicken must grow D's box or clamp so no push
  lands off-box — the boundary fixture is what forces that path to be exercised.)
  Helper: cross-section-width scan (test-only). **NOTE 2 — duplication:** this
  helper duplicates Ф2's `cross_section_width` `[measured: sed -n '982,1014p'
  crates/gen/src/phase2.rs → fn cross_section_width]`; this is the **2nd** copy
  (below the ≥3-site extraction threshold), so a per-file copy is acceptable
  here — but if a **3rd** gen phase needs it, hoist it to a shared
  `#[cfg(test)]` gen test-util module rather than adding a 3rd copy (§ Rules,
  ≥3-site duplication → shared util).
- **Subtask 4 — chord + gate (AC1, AC4, AC5).** Entry: `front_chord` /
  `StartFinish` assembly. Scenarios: AC1 `sf.orient` perpendicular to travel and
  `sf.width() ≥ m`, S/F on a straight (not a corner); AC4 `gate.behind` == front
  chord, `gate.forward` == `forward_side`, every start cell `gate_coord(cell) <
  GATE_LINE` for every row; AC5 `forward_side` equals the projection formula for
  `skel.dir`. Fixture: the AC-adequate straight fixture.
- **Subtask 5 — `start_grid` (AC3, AC7).** Entry: `start_grid`. Scenarios: AC3
  exactly `m` distinct positions, all in D, `rows = m.div_ceil(width)`, ordered
  front-to-back; AC7 D provides `≥ rows` cells of straight behind the front row
  along `−forward_side`, grid rows fit in D with no overlap with `¬D`/gate.
  Helper: grid-straight backward measurement (test-only).
- **Subtask 6 — end-to-end (AC2, AC6, AC8, AC9).** Entry: `phase3_start_finish`.
  Scenarios:
  - **AC2** — measured accel zone forward of S/F along `forward_side` to the
    first corner `≥ v_target²/2` on an adequate fixture (test-only forward-run
    measurement helper; **no float** — `v_target*v_target` then `/2` integer, or
    `saturating_mul` under the arithmetic-safety posture).
  - **AC6** — feed the produced `sf` into a fresh `gp_core::sim::LapCounter`
    (init `−1`): every start position reads `raw() == −1` / `laps() == 0` at
    `t = 0` (no start cell has crossed); a first forward move from the front row
    (`from` a chord cell, `to` one cell along `forward_side`) registers a `+1`
    crossing (`raw()` `−1 → 0`), asserted for **every** row via `register_move`
    `[measured: sed -n '210,267p' crates/core/src/sim/mod.rs → register_move
    scores forward iff from_c<GATE_LINE && to_c>=GATE_LINE; init −1 via Default]`.
  - **AC8** — determinism: two calls on identical `(d, skel, m, v_target)` yield
    byte-identical `Phase3Output` (compare drivable-point set of `d`, `sf.chord`,
    `sf.gate.behind`/`forward`, `sf.orient`, `grid.positions`); plus one exact
    snapshot pin (counts + boundary extremes, Ф1/Ф2 snapshot style) for a fixed
    fixture.
  - **AC9** — totality: `phase3_start_finish` returns without panic on Ф1→Ф2
    output for a sweep of `phase1_coarse_ring` seeds (à la Ф2's
    `property_sweep`); the workspace stays green under `cargo build`,
    `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and
    the doc gate `[derived → those four commands are the AC9 gate]`.

## Open questions

- None blocking. The straight-selection tie-break, the thickening span, and the
  `Phase3Output` struct shape are resolved above; all remaining budget/enforcement
  concerns are explicitly deferred to the Ф4–Ф6 loop by the spec.
</content>
</invoke>
