# Design: gp-gen Ф5b — full Vmax passability oracle (iterative deepening) + speed metrics + break_points

**Issue:** [#29](https://github.com/maratik123/graphite-gp/issues/29)
**Date:** 2026-07-23

## Approach

Build `phase5_full_oracle(d, grid, sf, race_dir) -> OracleResult` as an
iterative-deepening driver that composes the already-committed Ф5a substrate
(`forward_reachable` / `backward_reachable` / `within_v_ceil`, in
`crates/gen/src/phase5.rs`) — never reimplementing the flood edge (`legal_move`)
or the crossing test (`register_move`) (design §3; AC5). Signature mirrors the
committed `oracle_liveness_v1(d, grid, sf, race_dir)` exactly
[measured: `grep -n oracle_liveness_v1 crates/gen/src/phase5.rs` → `142: pub fn oracle_liveness_v1(`, params `d: &Corridor, grid: &StartGrid, sf: &StartFinish, race_dir: RaceDir`].

### The four Open-questions design calls (spec § Open questions)

**(1) `live = R ∩ B` lap-close mechanism — plain floods for `R`/`B` + a
confined augmented BFS for lap detection; NO Ф5a signature change.**

The tension the spec names: `backward_reachable` takes plain `goals: &[CarState]`
and returns `HashSet<CarState>`, but "lap close" vs "race start" is a *counter*
distinction (a forward S/F crossing is `-1→0` race-start **or** `0→1` lap-close —
the same geometric crossing), which a plain `CarState` set cannot encode. Ф5a
resolved the identical problem by threading a `LapCounter` through the flood
(`oracle_liveness_v1`) rather than scanning a plain set. The design call splits
the two concerns so the plain Ф5a floods are reused **unchanged** and the counter
appears only where it must:

- **`R` (plain):** `R = forward_reachable(start_seeds@v=0, V_ceil)` — the Ф5a
  function, verbatim. All states reachable from the start regardless of lap phase.
- **`G` (lap-close goal states):** enumerate over `R`: for each `s ∈ R` and each
  `a ∈ Action::iter()`, if `legal_move(d, s, a)` and the swept move
  `s.pos() → step(s,a).pos()` crosses the S/F **forward**, collect `step(s, a)`.
  The forward-crossing test reuses core `register_move` (see (helper)
  `crosses_sf_forward` below) — one crossing code path (AC5).
- **`B` (plain):** `B = backward_reachable(&G, V_ceil)` — the Ф5a function,
  verbatim. States from which *some* forward crossing is reachable.
- **`live = R ∩ B`** (`HashSet` intersection). AC2 holds by construction: a
  high-speed state in `R` from which no forward crossing is reachable (`∉ B` —
  the provable crash) is dropped.
- **Lap existence + `fastest_lap` + phase-0 region:** a **new** confined augmented
  BFS `fastest_lap_through_live(d, seeds, sf, &live, V_ceil)` — the Ф5a
  `oracle_liveness_v1` product-state `(CarState, LapCounter)` pattern, but
  (a) expansion restricted to `s2 ∈ live`, (b) BFS parent-tracking for path
  reconstruction, (c) returning **both** the fewest-move `Vec<Point>` path from a
  start seed to the first `raw() >= 1` transition (`None` if no lap) **and** the
  **phase-0 reached cell set** `P0 = { s.pos() : a visited augmented state
  (s, φ) has φ == 0 }` — the post-race-start, pre-lap-close region the
  `NotLappable` branch's `frontier_gap` consumes (see (3)). It calls the
  identical `legal_move` / `step` / `register_move` triple (AC5) — it does **not**
  reimplement `forward_reachable`; it is a distinct product-graph traversal.
  Confining to `live` cannot drop a real lap: every state on a genuine
  start→…→lap-close path is reachable-from-start (`∈ R`) and can reach that
  lap-close crossing (`∈ B`), hence `∈ live`.

**Ф5a signature impact: none.** `forward_reachable` / `backward_reachable`
signatures are untouched; the only Ф5a edit is widening `within_v_ceil` from
private `const fn` to `pub(crate) const fn`
[measured: `crates/gen/src/phase5.rs:55` → `const fn within_v_ceil(...)`, no
`pub`] so `phase5b` bounds its goal enumeration and augmented BFS on the *same*
L∞ box the floods enforce (`redundant_pub_crate = "allow"` in the workspace lints
[measured: `Cargo.toml` `[workspace.lints.clippy]` → `redundant_pub_crate = "allow"`],
so `pub(crate)` inside a private module is not linted). This is a visibility
widening, not a signature/behaviour change — flagged here per the spec's
"any Ф5a signature change is a design call."

**(2) Concrete `lap_length` measure — the fewest-move lap length at `V_ceil = 1`.**
The oracle runs before Ф7 builds `s_field`/`centerline`, so it cannot read a
geometric arc length. The path-independent measure: the move count of the fastest
lap computed at the **first** deepening iteration (`V_ceil = 1`). At `|v| ≤ 1`
every move advances at most one cell, so the fewest-move V=1 lap = the tightest
cell cycle = the loop's geometric perimeter in cells — an extremum (minimum over
laps), hence path-independent, a fixed track property. It is captured for free
during iteration 1 (no separate pass). Then
`tempo = lap_length / len(fastest_lap)` (V=1 move count ÷ final-ceiling move
count) reads as *average cells-per-move* — `1.0` on a track drivable only at V=1,
`> 1.0` when speed can be carried around the loop, i.e. exactly design §3's
"honest fastness scalar integrating straights and braking." `len(fastest) ≤
lap_length` always (a wider velocity box can only shorten the fewest-move lap), so
`tempo ≥ 1.0`. Well-definedness: whenever a lap exists at the final `Vmax`, a V=1
lap also exists — design §3 states V=1 liveness is *sound + complete* for "a
closed lap exists" at 4-connectivity
[derived → AC7 exact-metrics + long-straight fixtures discharge it: both are
lappable and assert a concrete `lap_length`/`tempo`].

**(3) Concrete shapes / homes.**
- **`OracleResult` — new enum, home `gp-gen` (`phase5b`).** Shape:
  ```
  pub enum OracleResult {
      Lappable(gp_core::track::TrackMetrics),   // populates existing fields
      NotLappable { break_points: Vec<Point> }, // gen-internal Ф6 diagnostic
  }
  ```
  Rationale for `gp-gen` (not `gp-core`): `TrackMetrics` is the artifact contract
  and already lives in `gp-core::track`
  [measured: `crates/core/src/track.rs:310-320` → `pub struct TrackMetrics { vmax_attain: Option<i32>, tempo: Option<f32>, fastest_lap: Vec<Point>, speed_heatmap: Vec<(Point, i32)> }`];
  `break_points` is a raw reachability-stall diagnostic that only feeds Ф6's
  `map_frontier_gap_to_edge` (out of scope), so it does not belong in the exported
  contract type. An enum (vs. a `laps_exist` bool + `Option` soup) makes the
  "`break_points` non-empty ⟺ not lappable" invariant (AC3) unrepresentable-when-violated.
  `gp-gen` already depends on `gp-core` [measured: `crates/gen/Cargo.toml`
  `[dependencies]` → `gp-core = { workspace = true }`], and no new dependency is
  needed (spec Key-decisions).
- **`break_points`: `Vec<Point>`** (the **goal-aware** reachability-stall
  frontier). Semantic per design [N3] `frontier_gap(R, goal)` and spec AC3
  ("frontier gap between `R` **and the lap-close goal**"): the **outer 4-frontier
  of the phase-0 reachable region `P0` within `proj(R)`**, where `P0 = { p :
  some visited augmented state (s, φ=0) has s.pos() == p }` is the set of cells
  occupiable **after** the race-start crossing but **before** a lap-close, emitted
  by `fastest_lap_through_live`'s augmented `(CarState, LapCounter)` flood
  (subtask 5). Concretely
  `break_points = { c ∈ proj(R) : c ∉ P0 ∧ ∃ c' ∈ P0 with |c − c'|₁ == 1 }`, and
  the **driver** falls back to the start-seed cells whenever this frontier is empty
  — which happens in **both** degenerate cases: `P0 == ∅` (no forward crossing
  reachable at all) **and** `P0 == proj(R)` (the phase-0 region already covers the
  whole drivable component, so it has no outer frontier — § driver pseudocode).
  This is **goal-aware**: the phase distinction (ahead-of-gate / post-race-start
  vs. behind the gate) is exactly the lap-close-vs-race-start awareness the
  `LapCounter` encodes — **NOT** the earlier drivability-vs-`R` boundary, which was
  **provably always empty** (see § Risks). Finer dual-edge localization is Ф6's job
  (`map_frontier_gap_to_edge`, out of scope).

  **Non-emptiness when `NotLappable` (AC3) — the seed-cell FALLBACK is the
  unconditional guarantor; the P0-frontier is the meaningful DIAGNOSTIC in the
  normal case.** AC3 requires only the one direction `NotLappable ⟹ break_points ≠
  ∅` (the converse is vacuous — `break_points` is returned **only** in the
  `NotLappable` arm, never on `Lappable`, so emptiness-when-`Lappable` is a scoping
  fact, not a proof obligation). The driver guarantees that one direction
  **unconditionally through the seed-cell fallback**, which fires exactly when the
  P0-frontier `frontier_gap(&proj(R), &P0)` is empty — and `frontier_gap` is empty
  in **both** degenerate cases: `P0 == ∅` (no forward crossing reachable at all) and
  `P0 == proj(R)` (the phase-0 region already covers the whole drivable component,
  so it has no outer frontier). Neither the corrected step 1 below nor any
  `NotLappable` topology rules out `P0 == proj(R)`, so it is the **fallback**, not
  the frontier, that discharges AC3 in every case; `seed_cells` is non-empty
  (`grid.positions` is non-empty by generator contract), hence `break_points ≠ ∅`
  unconditionally. ∎ (AC3)

  The **P0-frontier itself** is the meaningful diagnostic in the *normal*
  (non-degenerate) `NotLappable` case — a non-loopable topology whose phase-0 arc
  stalls **inside** `proj(R)`, leaving a proper `∅ ⊊ P0 ⊊ proj(R)` whose outer
  4-frontier localizes the reachability stall for Ф6 (a strictly better signal than
  the seed-cell fallback). Steps 1–4 establish that this normal case is precisely
  the one the broken-ring fixture exhibits, and that the frontier is non-empty
  there:
  1. **`proj(R)` is the full 4-connected drivable component of the seeds.** At
     `V_ceil ≥ 1` the unit-distance step between two 4-adjacent drivable cells is
     legal — its `supercover` is exactly the two endpoints, both ∈ D
     [measured: `legal_move`/`supercover` `crates/core/src/sim/mod.rs:89,107`;
     design §3 `[C4]`] — and from any rested cell you step to a drivable 4-neighbor
     and decelerate back to rest there, so every component cell is reachable and no
     off-component cell is. A larger `V_ceil` only *adds* states, never new cells,
     so `proj(R)` is that same 4-connected component at every ceiling.
  2. **On a NON-LOOPABLE topology, `P0` is a proper subset of `proj(R)` excluding
     the behind-gate seed cells.** When **no closed lap exists** (the `NotLappable`
     precondition), a car that has crossed the gate forward into phase 0 can never
     return to a behind-gate cell *at phase 0* via a counted re-crossing: netting
     back to the gate's behind side while advancing forward would require a closed
     forward loop, which by hypothesis does not exist here. So on a non-loopable
     topology the behind-gate seed cells — where every start seed sits ("строго
     позади ворот при `t=0`", design §3) — stay at phase −1, `seed_cells ⊆ proj(R)
     \ P0`, and `P0 ⊊ proj(R)` is a proper subset. **This scoping is load-bearing:**
     the claim is FALSE on a *valid* (loopable) ring. The bounded-chord
     `register_move` counts a forward crossing **only inside the behind-span**
     [measured: `register_move` + `crossing_within_span`
     `crates/core/src/sim/mod.rs:218-251`; the Ф5a bounded-chord fix], so on a
     loopable ring a car can drive a full loop — crossing the gate LINE at the far
     wall *outside* the counted span, which does **not** flip the counter — and
     re-enter a behind-gate cell still at phase 0. Concretely on the 5×5 testfix
     ring (behind `[(2,0)]`, forward East, seed `(2,0)@rest`): cross forward
     `(2,0)→(3,0)` into phase 0, drive the loop, arrive `(1,0)→(2,0)` at `v=(1,0)`
     without re-crossing the counted span, so `(2,0) ∈ P0` at phase 0. But a valid
     ring is `Lappable` and never reaches this branch — the argument is needed only
     for `NotLappable`, where the loop is absent by hypothesis.
  3. **`P0 ≠ ∅` whenever a race-start is reachable.** A start seed at rest steps
     forward across the gate in one legal move (the race-start `−1 → 0` crossing),
     producing a phase-0 state `s2`. `s2 ∈ live`: `s2 ∈ R` (reachable from the
     seed), and `s2 ∈ B` because from `s2` the trivial reverse-then-forward
     back-and-forth at the gate reaches a forward crossing (`B` is counter-blind —
     § Approach (1)), so the `live`-confined augmented flood **does** reach `s2` at
     phase 0. On the degenerate hand-fixture with **no** forward-crossable gate,
     `P0 == ∅` and the driver's seed-cell fallback fires (non-empty;
     generator-unreachable, since the generator always emits a valid
     forward-crossable full-chord S/F).
  4. **A non-empty proper subset `P0` of a finite 4-connected set `proj(R)` has a
     non-empty outer 4-frontier.** Connectivity forces at least one 4-edge from `P0`
     to `proj(R) \ P0` (else `P0` would be a union of whole components — all-or-
     nothing in a connected set); that edge's outer endpoint ∈ `break_points`. In
     the normal `NotLappable` case (steps 2–3 give `∅ ⊊ P0 ⊊ proj(R)`) the
     P0-frontier is therefore non-empty and is the diagnostic; when instead `P0 ==
     proj(R)` (frontier empty though `P0 ≠ ∅`) the seed-cell fallback above supplies
     non-emptiness. ∎

  Steps 1–4 are **`V_ceil`-independent** (they hold at any ceiling), though the
  `NotLappable` branch in fact only fires at `V_ceil == 1` (V=1 completeness — the
  same monotonicity noted below the pseudocode). They ground `NotLappable`'s
  "genuinely no lap" meaning in design §3's V=1 **sound + complete** liveness claim:
  `fastest_lap_through_live` returning `None` at `V_ceil == 1` is the same augmented
  V=1 flood `oracle_liveness_v1` certifies with (confining to `live` cannot drop a
  real lap — § Approach (1)), so "no path to `raw() >= 1`" = "no lap exists."
- **`speed_heatmap`: `Vec<(Point, i32)>`** — the exact type of the existing
  `TrackMetrics.speed_heatmap` field, emitted sorted by `Point` (`Point` derives
  `Ord`, `x` then `y` [measured: `crates/core/src/geom/mod.rs:29` → `#[derive(... PartialOrd, Ord, ...)] pub struct Point`]) for deterministic output (AC6).
- **`fastest_lap`: `Vec<Point>`** — the existing `TrackMetrics.fastest_lap` type.

**(4) Module placement — new `crates/gen/src/phase5b.rs`.** `phase5.rs` is
already 506 lines (188 non-test 1–188, 318 test 189–506)
[measured: `wc -l crates/gen/src/phase5.rs` → `506`;
`grep -n 'mod tests' crates/gen/src/phase5.rs` → `190: mod tests`]. The new
oracle (deepening driver, confined augmented BFS with path tracking, goal
enumeration, `frontier_gap`, heatmap, `OracleResult`) plus its tests is
substantial; appending it would push `phase5.rs` past the soft 500/800
(excl./incl. `#[cfg(test)]`) limits toward the 1000/1500 hard cap (AGENTS.md
§ Code Style). A sibling `phase5b.rs` keeps the phase-naming scheme and imports
the substrate via `use crate::phase5::{forward_reachable, backward_reachable, within_v_ceil}`.

### The deepening driver (design §2 Ф5b pseudocode / §3)

```
phase5_full_oracle(d, grid, sf, race_dir):        # race_dir: signature fidelity [N4], unused (crossing sign is sf.gate.forward — as Ф5a)
  seeds = grid.positions @ v=0
  lap_length = None
  V_ceil = 1
  loop:
    R    = forward_reachable(d, seeds, V_ceil)
    G    = lap_close_goals(d, sf, &R, V_ceil)
    B    = backward_reachable(d, &G, V_ceil)
    live = R ∩ B
    (fastest, p0) = fastest_lap_through_live(d, seeds, sf, &live, V_ceil)  # (Option<Vec<Point>>, P0 = phase-0 cells)
    if fastest is None:                              # no lap in live
        bp = frontier_gap(&proj(R), &p0)             # outer 4-frontier of P0 within proj(R) [N3] — the diagnostic
        if bp.is_empty(): bp = seed_cells            # unconditional AC3 guarantor; fires when frontier empty: P0 == ∅ OR P0 == proj(R)
        return NotLappable { break_points: bp }
    if V_ceil == 1: lap_length = moves(fastest)      # capture V=1 measure once
    Vpeak = max over live of vnorm(state)            # L∞
    if Vpeak < V_ceil: break                         # geometry no longer binds
    V_ceil = V_ceil.saturating_mul(2)
  # success:
  metrics = TrackMetrics {
      vmax_attain: Some(Vpeak),
      tempo: Some(lap_length as f32 / moves(fastest) as f32),
      fastest_lap: fastest,
      speed_heatmap: speed_heatmap(&live),
  }
  return Lappable(metrics)
```

`vnorm(s) = max(|s.vx|, |s.vy|)` (L∞ / Chebyshev), matching Ф5a's `within_v_ceil`
box bound so the halt test `Vpeak < V_ceil` reads against the same bound the
floods enforce (spec Key-decisions).

The `NotLappable` branch can only fire at `V_ceil == 1` (V=1 completeness, (2)
above): if a lap exists at V=1 it survives every larger `V_ceil` (reachability is
monotone in the box). Keeping the per-iteration check matches the pseudocode and
is harmless.

### Rejected alternatives

- **Augmented product-state flood for everything (drop the plain `R`/`B`).**
  Rejected: the spec/AC mandate `live = R ∩ B` via the Ф5a substrate and
  explicitly want `R \ B` (provable crash) excluded (AC2) — an all-augmented
  design would reimplement, not compose, the floods and would fail AC5's
  "reused via the Ф5a substrate, no reimplementation."
- **Extend `backward_reachable` to thread a counter (goal = `raw()==1` states).**
  Rejected: a backward counter is ill-defined (the counter is a forward path
  invariant); it would fork a second flood and change the Ф5a signature. The
  plain-`B` + confined-augmented-BFS split reuses the substrate verbatim.
- **`lap_length = |D|` (corridor cell count).** Rejected: that is *area*, which
  conflates corridor width; the V=1 fewest-move lap is a true *length*.
- **`OracleResult` in `gp-core`.** Rejected: `break_points` is a gen-internal Ф6
  input, not part of the exported `TrackArtifact` contract.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | New `phase5b` module skeleton + `mod phase5b; pub use phase5b::*;` in `lib.rs`; widen `within_v_ceil` to `pub(crate) const fn` in `phase5.rs`; define `OracleResult` enum (doc + `Lappable`/`NotLappable`). Compile-smoke test. | `crates/gen/src/phase5b.rs`, `crates/gen/src/lib.rs`, `crates/gen/src/phase5.rs` | — |
| 2 | Lift Ф5a test fixtures (`ring_corridor`, `ring_sf`, `ring_grid`, `car`) into a shared `#[cfg(test)] pub(crate) mod testfix` (new file); update `phase5.rs` tests to `use crate::testfix::*`. Keeps Ф5a tests green. | `crates/gen/src/testfix.rs`, `crates/gen/src/lib.rs`, `crates/gen/src/phase5.rs` | 1 |
| 3 | `crosses_sf_forward(sf, from, to) -> bool` (reuses core `register_move`) + `lap_close_goals(d, sf, &R, v_ceil) -> Vec<CarState>` (goal enumeration over `R`, bounded to the box). Tests incl. AC5 crossing-path pin vs direct `register_move`. | `crates/gen/src/phase5b.rs` | 1, 2 |
| 4 | `vnorm(s) -> i32` (L∞, total abs, `const fn`); `speed_heatmap(&live) -> Vec<(Point,i32)>` (per-point max `vnorm`, sorted by `Point`); `frontier_gap(r_cells: &HashSet<Point>, p0_cells: &HashSet<Point>) -> Vec<Point>` — **pure** outer 4-frontier of `p0` within `r` (goal-aware via `p0`; **replaces** the committed-but-provably-empty `frontier_gap(d, &R)` — see § Risks). Full driver wiring of `p0` lands in subtask 6; subtask 4 builds/tests the pure helper against hand-supplied `r`/`p0` cell sets. **ALSO** rewrite the stale committed `OracleResult::NotLappable` doc comment (`crates/gen/src/phase5b.rs:33` currently "non-empty by the generator-guarantee dependency") to the new P0-frontier-diagnostic + seed-cell-fallback rationale (fallback is the unconditional AC3 guarantor; frontier is the diagnostic). Tests. | `crates/gen/src/phase5b.rs` | 1, 2 |
| 5 | `fastest_lap_through_live(d, seeds, sf, &live, v_ceil) -> (Option<Vec<Point>>, HashSet<Point>)` — confined augmented `(CarState, LapCounter)` BFS with parent-tracking; returns the fewest-move path to first `raw()>=1` **and** `P0` (phase-0 reached cells, for `frontier_gap`). Tests: valid ring → `(Some(path), non-empty P0)`, broken ring → `(None, non-empty P0)`, lone race-start ≠ lap. | `crates/gen/src/phase5b.rs` | 1, 2 |
| 6 | `phase5_full_oracle(d, grid, sf, race_dir) -> OracleResult` deepening driver composing 3/4/5 (`R`/`G`/`B`/`live`, `Vpeak` halt, V=1 `lap_length` capture, `tempo`, assemble); on `NotLappable` wires `break_points = frontier_gap(&proj(R), &P0)` with the `P0 == ∅` → seed-cells fallback. Tests: AC1 halt+`Vmax`, AC2 `R\B` excluded, AC6 determinism. | `crates/gen/src/phase5b.rs` | 3, 4, 5 |
| 7 | Cross-cutting AC tests: AC3 broken ring → `NotLappable` + non-empty `break_points`; AC4 exact hand-fixture metrics + long-straight `tempo < Vmax` implication; AC5 shared `legal_move`/crossing assertion; AC7 provable-crash absent from `live`. | `crates/gen/src/phase5b.rs`, `crates/gen/src/testfix.rs` | 6 |

## Handoff plan

- **(a)** `M = 7 ≥ 1`, so this `## Handoff plan` is mandatory.
- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, via the
  `code-writer` subagent, 1M-token window — subtasks **1–7**. Change-type: **code**
  (all edits are Rust `*.rs` under `crates/gen/src/`: `phase5b.rs`, `phase5.rs`,
  `lib.rs`, `testfix.rs`). Homogeneous **(e)**; single group of 7 ≤ 10 **(b)**;
  minimized **(f)** — one code change-type, so one group is the fewest possible.
  **Terminal group** (7 subtasks; within `1..=10` **(d)**).
- **Entry into Group A (c):** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) — the
  every-group handoff binds even the first/only group.
- No inter-group handoff (single group). **(h)** 1 group ≤ default max 4 — no user
  gate needed. No instructions/harness change-type appears anywhere, so no Opus
  implementor group exists; the `design`/`design-review`/`self-review` gates stay
  on Opus regardless **(g)**.

## Risks

- **`missing_const_for_fn` (nursery, `deny`) forces `const fn` on `vnorm`.**
  `vnorm` is a pure integer accessor (`max(|vx|,|vy|)`), so the lint will *require*
  `const fn` — but **only a branchless body is const-callable on stable**. Neither
  `<i32 as Ord>::max` nor `i32::try_from` is const-stable
  (`Ord::max` → E0658 "Ord is not yet stable as a const trait", tracking
  rust-lang/rust#143874; `try_from` likewise non-const — VERIFIED by compile), so
  the earlier `vx.saturating_abs().max(vy.saturating_abs())` /
  `i32::try_from(...unsigned_abs().max(...))` snippets do **not** compile as
  `const fn`. Use the branchless form (compiles as `const fn`, and
  `missing_const_for_fn` still fires to require it — VERIFIED):
  ```
  let a = vx.saturating_abs();
  let b = vy.saturating_abs();
  if a >= b { a } else { b }
  ```
  `saturating_abs` (not plain `i32::abs`) keeps the body clear of
  `arithmetic_side_effects` (also `deny`) — plain `i32::abs` overflows at `i32::MIN`.
  Velocities are box-bounded (`|v| ≤ V_ceil`) so overflow is unreachable in
  practice, but totality keeps the zero-production-panic posture — `[derived → cargo clippy --workspace --all-targets -- -D warnings discharges both the const-eligibility and the abs-overflow lint]`.
- **Zero production panic (gp-core posture, `ai-docs/panic-index.md` intentionally
  minimal for `gp-gen` — no `gp-gen` rows [measured: `cat ai-docs/panic-index.md` → only `render`/`game` rows, no `gp-gen`]).** No `unwrap`/`expect`/panicking
  index in the new oracle: `V_ceil` doubling uses `saturating_mul`; `tempo`'s
  `len(fastest)` is `≥ 1` on the success path (a lap has ≥ 1 move) so the ratio
  never divides by zero; `vnorm` is total. `[derived → cargo clippy + the panic-gate hook discharge it]`.
- **Termination / bounded 2× over-shoot (AC6).** Each inner flood is over a finite
  space (corridor cells × the bounded L∞ velocity box) so terminates; the outer
  loop halts because `Vmax_attain` is geometry-bounded (a speed whose braking
  distance exceeds the longest straight is unreachable on a completable lap,
  design §3), and `V_ceil` doubling over-shoots that ceiling by at most 2×.
  `[derived → AC1 halt test (finite Vmax on the long-straight fixture) + AC7 untraversable-ring test (returns before the loop deepens) discharge it]`.
- **Determinism despite `HashSet` intermediates (AC6).** `R`/`B`/`live` are
  `HashSet`s, but every *output* is order-independent: `Vpeak`/heatmap are `max`
  aggregates, `speed_heatmap` is sorted by `Point`, and `fastest_lap_through_live`
  is a BFS seeded in `grid.positions` order expanding in `Action::iter()` order
  (Ф5a's determinism discipline). `[derived → AC6 determinism test asserts identical repeated results]`.
- **`frontier_gap` non-emptiness (AC3) — CORRECTED definition; the original was
  provably always empty.** The design's ORIGINAL `frontier_gap(d, &R)` (drivable
  cells `∉ proj(R)` with a 4-neighbor `∈ proj(R)`) is **PROVABLY ALWAYS EMPTY**: at
  `V_ceil ≥ 1`, `proj(R)` is the *entire* 4-connected drivable component of the
  seeds (unit-step legality + brake-to-rest — § Approach non-emptiness proof step 1,
  verified against `legal_move`/`supercover` `crates/core/src/sim/mod.rs:89,107`),
  so **no** drivable cell is ever 4-adjacent-to-`R`-yet-outside-`R`. The earlier
  premise "R stalls at the break; cells just past it are drivable-but-unreached" is
  **false** — deleting one cell from a ring leaves a still-fully-connected *path*,
  not two components, so `R` floods the whole remaining component and the frontier
  is empty. The redefined **goal-aware** form (outer 4-frontier of the phase-0
  region `P0` within `proj(R)`, § Approach (3)) gives AC3 non-emptiness through the
  **driver's seed-cell fallback as the unconditional guarantor** — the P0-frontier
  is the meaningful *diagnostic* in the normal (non-degenerate) `NotLappable` case
  (`∅ ⊊ P0 ⊊ proj(R)`, non-empty by connectivity), but the fallback fires whenever
  the frontier is empty, covering **both** `P0 == ∅` **and** `P0 == proj(R)` (the
  latter not ruled out for every `NotLappable` topology). The behind-cells-stay-φ−1
  argument that keeps the broken-ring frontier non-empty is scoped to **non-loopable
  topologies** (§ Approach (3) step 2): on a *valid* loopable ring it is FALSE —
  the bounded-chord `register_move` [measured: `register_move` +
  `crossing_within_span` `crates/core/src/sim/mod.rs:218-251`] does not count the
  far-wall crossing, so a full loop re-enters a behind-gate cell at phase 0 and that
  cell **can** be in `P0`; but a valid ring is `Lappable` and never reaches this
  branch. `[derived → AC3 broken-ring test asserts non-empty break_points against
  the corrected phase-0 definition + a phase-0 witness; subtask-4 helper test
  asserts the outer frontier of a proper `P0 ⊊ R` is non-empty and of `P0 == R` is
  empty]`.
- **Test-fixture duplication (≥2-site, open-ended trajectory).** The Ф5a ring
  fixtures are needed by `phase5` **and** `phase5b` tests now, with Ф6/Ф7 upcoming
  — lifted to a shared `#[cfg(test)] pub(crate) mod testfix` (subtask 2) rather
  than copied, per the ≥2-with-more-coming shared-helper rule. Call sites at merge:
  2 (`phase5`, `phase5b`). `[measured: grep -n 'fn ring_corridor\|fn ring_sf\|fn ring_grid' crates/gen/src/phase5.rs → the three helpers currently live in phase5.rs's #[cfg(test)] mod]`.

## Test Design

All tests are `#[cfg(test)] mod tests` blocks inside `crates/gen/src/phase5b.rs`,
plus the shared `crates/gen/src/testfix.rs` fixtures. `gp-gen` is the sanctioned
crate-level Miri `--exclude` (#134 cost carve-out; pure-integer, Miri-clean) — no
per-test `#[cfg_attr(miri, ignore)]` needed
[measured: AGENTS.md § Rust Test Conventions → "`gp-gen` is excluded from the Miri gate"].

- **Subtask 3 — `crosses_sf_forward` / `lap_close_goals`:**
  - Entry: `crosses_sf_forward`, `lap_close_goals`.
  - Scenarios: a forward move across the ring gate `(2,0)→(3,0)` returns `true`;
    the reverse returns `false`; an off-gate move returns `false`. `lap_close_goals`
    over a small `R` yields the expected post-crossing states, all within the box.
  - **AC5 pin:** `crosses_sf_forward(sf, from, to)` agrees with a direct
    `LapCounter::new(); c.register_move(sf, from, to); c.raw() == 0` (the shared
    core crossing path — a forward crossing takes the fresh `-1` counter to `0`)
    [measured: `crates/core/src/sim/mod.rs:218` `register_move` + `crossing_event`
    returns `+1` for `from` strictly behind / `to` ahead-or-on-line].
  - Fixtures: shared `ring_sf`/`ring_corridor` from `testfix`.

- **Subtask 4 — `vnorm` / `speed_heatmap` / `frontier_gap`:**
  - Entry: each function.
  - Scenarios: `vnorm(car(_,_,2,-3)) == 3`; `vnorm` total at `i32::MIN`.
    `speed_heatmap` over a hand-built `live` set produces per-point max `vnorm`,
    sorted ascending by `Point`. `frontier_gap(&r, &p0)` (pure, on hand-built
    `HashSet<Point>`): a proper `p0 ⊊ r` over a connected `r` yields a non-empty
    outer 4-frontier listing only cells in `r \ p0` that are 4-adjacent to `p0`;
    `p0 == r` yields empty; `p0 == ∅` yields empty (the driver — subtask 6 — not
    this pure helper, supplies the seed-cell fallback).
  - Fixtures: hand-built `HashSet<Point>` (`r`/`p0`) + `HashSet<CarState>` for
    `speed_heatmap`.

- **Subtask 5 — `fastest_lap_through_live`:**
  - Entry: `fastest_lap_through_live`.
  - Scenarios: valid ring + its `live` → `(Some(path), P0)` where the path's
    first/last cells are a start seed / a lap-close crossing and its moves = the
    tightest V=1 lap; broken ring → `(None, P0)` with `P0` non-empty (race-start
    reached, lap not); the Ф5a dead-end fixture (permits race-start crossing,
    dead-ends after) → `(None, P0)` with `P0` non-empty (a lone `-1→0` crossing is
    not a lap). Pin the § Approach proof step 2 invariant **ONLY on the non-loopable
    fixtures** (broken ring, dead-end), where it genuinely holds: `P0` never
    contains a strictly-behind-gate start-seed cell. **Do NOT** assert this on the
    lappable ring — there a full loop re-enters a behind-gate cell at phase 0
    (bounded-chord `register_move` does not count the far-wall crossing —
    § Approach (3) step 2), so `(2,0) ∈ P0` and the invariant is FALSE.
  - Fixtures: shared `ring_*` + a `dead_end`-style fixture (mirror Ф5a's
    `dead_end_corridor`).

- **Subtask 6 — `phase5_full_oracle` (AC1/AC2/AC6):**
  - Entry: `phase5_full_oracle`.
  - Scenarios: **AC1** on a lappable fixture the loop halts and returns
    `Lappable` with `vmax_attain == Vpeak` and `Vpeak < V_ceil` at halt.
    **AC2** a hand-constructed high-speed state that is in `R` but not `B` is
    absent from the intersected `live` (assert via a direct `R`/`B` recompute in
    the test). **AC6** two identical calls return equal `OracleResult` (derive or
    assert field-wise; `Vec` fields compared directly, heatmap sorted).
  - Fixtures: shared `ring_*` (a small lappable ring), plus a corridor with a
    reachable-but-un-brakeable pocket for AC2.

- **Subtask 7 — cross-cutting AC (AC3/AC4/AC5/AC7):**
  - **AC3:** broken ring (`ring` with one straight cell cleared, mirroring Ф5a's
    `d.set(Point::new(4,2), false)`) → `NotLappable { break_points }` with
    `!break_points.is_empty()`, computed as the outer 4-frontier of the phase-0
    region `P0` within `proj(R)` (the CORRECTED definition — § Approach (3)).
    Derivation for this fixture (gate `behind=[(2,0)]`, `forward=East`): the
    race-start crosses `(2,0)→(3,0)` into phase 0, the phase-0 arc floods CCW along
    the bottom-right ring to the cleared `(4,2)` and stalls, so
    `P0 = {(3,0),(4,0),(4,1)}`; `proj(R)` is the whole remaining ring component, so
    the outer 4-frontier is non-empty — e.g. the behind-gate cell `(2,0)`
    (4-adjacent to phase-0 `(3,0)`, but itself reachable only at phase −1, hence
    `∈ proj(R) \ P0`). Assert `!is_empty()` and, non-vacuously, that a known
    behind-gate frontier cell (`(2,0)`) is present.
  - **AC4 (exact metrics + long straight):** a small hand-built track with one
    long straight; assert exact `vmax_attain`, exact `fastest_lap` cell sequence,
    exact `speed_heatmap`, and `tempo` computed as `lap_length / len(fastest)`.
    Assert `tempo` is strictly less than `vmax_attain as f32` — the peak speed
    reached on the straight is not sustained through the required braking, so the
    honest scalar is lower (design §3). All values are integer-derived and
    deterministic; `tempo` compared with an epsilon (`f32`).
  - **AC5:** a shared assertion that the oracle's crossing decision equals a
    direct core `register_move` call on the same `from→to` and that its edge is
    `legal_move` (reuse the subtask-3 pin; assert on the ring fixture).
  - **AC7 (provable crash):** construct a state reachable at high speed into a
    corner it cannot brake out of in time; assert it is in `forward_reachable`
    but absent from the final `live` (in `R`, not in `B`).
  - Fixtures: shared `ring_*`, a broken ring, and a purpose-built long-straight
    track (new `testfix` helper).

## Open questions

None. The four spec Open-questions calls are resolved in § Approach:
`lap_length` = fewest-move V=1 lap length; `live = R ∩ B` via plain Ф5a floods +
a confined augmented BFS (no Ф5a signature change, only `within_v_ceil` visibility
widening); `OracleResult` enum homed in `gp-gen`, `break_points: Vec<Point>`,
`speed_heatmap: Vec<(Point,i32)>` sorted; module = new `phase5b.rs`;
`break_points` = the **goal-aware** outer 4-frontier of the phase-0 region within
`proj(R)` (design amendment — supersedes the drivability-vs-`R` `frontier_gap(d,&R)`
that was provably always empty). Surfacing scope notes for the orchestrator (not
blocking): (1) subtask 2 edits the committed Ф5a `phase5.rs` test module (fixture
lift) — a mechanical refactor kept green by re-running `cargo test -p gp-gen` after
the move; (2) **subtasks 1–4 are already committed on the branch**, so this
amendment's redefinition means the resumed **subtask 4** rewrites the committed
`frontier_gap(d, &R)` + its tests to the pure `frontier_gap(&r, &p0)` set helper
**and** rewrites the stale committed `OracleResult::NotLappable` doc comment
(`phase5b.rs:33`, currently "non-empty by the generator-guarantee dependency") to
the P0-frontier-diagnostic + seed-cell-fallback rationale, **subtask 5** widens
`fastest_lap_through_live`'s return to `(Option<Vec<Point>>, HashSet<Point>)`
(adding `P0`), and **subtask 6**'s driver wires the two together with the seed-cell
fallback firing on **both** `P0 == ∅` **and** `P0 == proj(R)`.
