# gp-gen Ф5a: reachability substrate + V=1 liveness (+ core S/F bounded-chord fix)

**Source:** issue #28
**Date:** 2026-07-23
**Tracked in:** #28

## Scope

Add the passability-oracle **search substrate** and the **cheap V=1 liveness
check** to `gp-gen` (design doc §2 Ф5, §3 *Оракул проходимости*, *Счётчик
кругов*), **and** fix the `gp-core` S/F crossing test that the V=1 liveness
oracle depends on (see item 4 below). Concretely, in a new `gp-gen` module (design
places it — the phase convention is `phaseN.rs`):

1. **`forward_reachable`** — flood the `(x, y, vx, vy)` state space forward from a
   set of seed states, using core `legal_move` as the graph edge (5 transitions
   per state: the `Action` set), bounded so `|v| ≤ V_ceil`. Returns the set of
   reachable states.
2. **`backward_reachable`** — the same flood in reverse (predecessor edges of
   `legal_move`) from a goal set, bounded so `|v| ≤ V_ceil`. Returns the set of
   states from which the goal is forward-reachable. This is the `B` half of the
   full oracle's live-set `R ∩ B`; it is delivered here as substrate even though
   this issue does not build the full `Vmax` oracle.
3. **`oracle_liveness_V1(d, grid, sf, race_dir)`** — the binary "a lap exists"
   filter (design doc §2 `oracle_liveness_V1`): forward-flood from the `v = 0`
   start-grid seeds within `|v| ≤ 1`, and return `true` iff a **lap-closing**
   forward S/F crossing (by `race_dir`) is reachable. Sound **and** complete for
   the binary "a closed lap exists" at 4-connectivity (design doc §3: `(0,0)` is
   always legal, zero braking distance at `v = 1`, supercover admits no
   shortcut). It is deliberately blind to dynamics (run-out, chord-cut, hairpin
   at speed) — a fast Ф6-repair filter, **not** the final certifier.
4. **`gp-core` S/F crossing fix** — correct `gp_core::sim::LapCounter::register_move`
   (adjusting `gate_coord` / `crossing_event` / `register_move`, as design decides)
   so a forward/reverse S/F crossing is counted **only** when the swept `from → to`
   segment actually passes through the gate **chord's extent** — bounded by
   `sf.gate.behind` / the chord cells and their span *along* the chord — not merely
   the chord's infinite supporting line. Today `register_move` reads only
   `sf.gate.behind.first()` and projects via `gate_coord` **along `forward`'s axis
   only**, then tests `crossing_event` against a **global** infinite half-plane
   (`GATE_LINE`); the perpendicular-along-chord coordinate is ignored. This aligns
   the implementation with design.md §3's «полный хорд, разрезающий аннулюс в
   односвязную полосу» (a *bounded* chord) and makes `raw() >= 1` reachable via a
   physically-continuous lap around a closed ring. **`legal_move` itself stays
   UNCHANGED** — only the S/F crossing test in `register_move` changes.

The S/F lap-close test is the identical signed-crossing path used at runtime
(core's `LapCounter::register_move` keyed on `sf.gate` + `race_dir`) — one code
path, matching the `legal_move` reuse. Because the oracle consumes core's
crossing logic unchanged, the core fix in item 4 is what makes the oracle's AC4
reachable. The initial forward crossing as a car leaves the start grid is the
**race start** (`counter −1 → 0`), *not* a lap; a lap-closing crossing is the
forward crossing that completes the full loop (`counter 0 → 1`). Correctness rests
on the S/F chord cutting the annulus into a simply-connected strip (design doc §3
*Счётчик кругов*), so a second forward crossing is reachable only after
traversing the whole ring.

### Why the core fix is required (finding)

For **any** closed-loop (annulus) track, the chord's infinite supporting line is
also crossed on the **far wall** of the ring — a reverse crossing. With the
current unbounded half-plane test, `raw()` therefore nets **zero** per lap, and
`raw() >= 1` is **topologically unreachable** via continuous driving. Two
independent confirmations:

- **Topology:** a closed curve crosses any infinite line a net-zero signed number
  of times; single-half-plane crossings strictly alternate sign, so from the
  `−1` init the counter cycles `−1, 0, −1, 0, …` and never reaches `+1`.
- **Core's own tests:** the existing `register_move` unit tests only ever reach
  `raw() == 1` by calling `register_move` **twice** with the identical
  `(1,1)→(2,1)` pair back-to-back (a non-physical scripted sequence) — never a
  continuous lap.

This is a real bug in #8's `register_move` (unbounded half-plane) relative to
design §3's bounded-chord intent. It blocks Ф5a's oracle (AC4) and would break
real runtime lap counting once `LapCounter` is wired into the game loop, so it is
fixed in this same PR.

## Out of scope

- **The full `Vmax` oracle / `phase5_full_oracle`** (design doc §2 Ф5b, §3
  *Vmax_attain*): iterative deepening `V_ceil = 1, 2, 4, …`, the live-set
  `R ∩ B` intersection as a shipped oracle result, and `Vmax_attain` extraction.
  This issue delivers the two reachability primitives that Ф5b will *compose*,
  plus V=1 liveness; it does not build the deepening loop. (`forward_reachable`
  and `backward_reachable` are in scope; wiring them into an intersection-based
  certifier is not.)
- **Oracle-derived metrics** — `tempo`, `fastest_lap`, `speed_heatmap`
  (`TrackMetrics`, design doc §3 *Метрики скорости*).
- **Pipeline integration** — calling `oracle_liveness_V1` from the `generate()`
  loop (design doc §2 pseudocode `if issues.empty and not oracle_liveness_V1…`)
  and the Ф6 repair loop. `generate()` stays `todo!`; this issue adds callable
  primitives, not their call sites.
- **Core `legal_move` and the reachability edge stay unchanged.** The **only**
  core change in scope is correcting the S/F crossing test in `register_move`
  (`gate_coord` / `crossing_event`) to bound crossings to the chord extent per
  design §3 — the minimal core change needed to align it with the design. The
  full `Vmax` oracle (Ф5b), metrics, and `generate()`-loop wiring remain out of
  scope.

## Deferred

- Full `Vmax` oracle + metrics | large, distinct build-order item (Ф5b) | yes — a
  separate issue already tracks the full oracle (design build order); no new
  issue needed from this spec.

## Key decisions

| Question | Decision |
|---|---|
| Does #28 include `backward_reachable`, or only forward + V=1 liveness? | Both `forward_reachable` **and** `backward_reachable` are in scope (the substrate); the full `Vmax` deepening oracle that intersects them is the separate Ф5b issue. Issue body is explicit. |
| Edge relation | Core `legal_move` (5 `Action` transitions) — the identical runtime rule, reused, never reimplemented (AC: one code path). Backward search uses its predecessor relation, still gated on the same `legal_move`. `legal_move` is **not** modified by this issue. |
| S/F crossing test — global half-plane vs bounded chord | **Finding (blocking, confirmed against source):** `register_move` scores S/F crossings via `gate_coord` (projection along `forward`'s axis only) + `crossing_event` against the **global** infinite `GATE_LINE` half-plane, reading only `sf.gate.behind.first()` — it never bounds the crossing to the chord's extent. On any closed ring the supporting line is re-crossed on the far wall (reverse), so `raw()` nets zero per lap and `raw() >= 1` is topologically unreachable via continuous driving (verified by the net-zero-crossings theorem and by core's own tests reaching `raw()==1` only via a scripted duplicate crossing). **Decision:** fix `register_move` so a crossing counts **iff** the swept segment passes through the chord's extent (bounded by `sf.gate.behind` / the chord cells and their span along the chord) — aligning with design §3's «полный хорд». This makes `raw() >= 1` reachable via a real lap. Exact bounding geometry is a design call. |
| S/F lap-close test (oracle) | The identical signed half-open crossing path from core (`LapCounter::register_move` / `sf.gate` + `race_dir`), never a parallel reimplementation. Fixing core (above) fixes the oracle — that is the point of the single code path. |
| `|v| ≤ V_ceil` bound representation | Default: per-axis (Chebyshev/L∞ box) `|vx| ≤ V_ceil ∧ |vy| ≤ V_ceil`, giving a finite `(2·V_ceil+1)²`-velocity domain. At `V_ceil = 1` this stays consistent with the design's "4-conn" completeness argument (a superset of cardinal velocities cannot make the binary liveness unsound or incomplete). Design confirms/refines the exact velocity domain for the V=1 check. |
| V=1 liveness role | Fast Ф6 filter, blind to dynamics — not the final certifier (design doc §3). |
| New dependencies | None — BFS/flood uses `std` collections; `gp-gen` already depends on `gp-core`. |

## Technical constraints

- Integer-only, deterministic, total. Integer arithmetic throughout the search
  and the corrected crossing test (design doc §3a; `gp-core` `geom`/`sim` are
  integer-only). No production panic; no RNG.
- **Core change touches #8's merged code.** The fix is confined to the S/F
  crossing test (`gate_coord` / `crossing_event` / `register_move`); `legal_move`
  and every other core symbol stay untouched. Existing core `LapCounter` tests
  must be updated/extended: the current scripted-duplicate tests that reach
  `raw() == 1` via a repeated identical crossing should be revisited so the suite
  reflects the corrected bounded-chord semantics, and a **new physically-continuous
  closed-loop lap test** (a real lap around a ring reaching `raw() >= 1`) added.
- **Miri:** `gp-core` **is** in the Miri gate (only `gp-gen` is the sanctioned
  crate-level `--exclude`, #134 cost carve-out). `register_move` and the crossing
  helpers are pure integer — the new/updated core tests are Miri-clean and must
  stay so (no FFI/GPU, no per-test gating needed). The `gp-gen` substrate is pure
  integer too and rides the `gp-gen` carve-out.
- **Termination + determinism (AC):** the state space is finite (corridor cells ×
  the bounded velocity box), so the flood visits each state at most once and
  terminates. Iteration order over states/actions is deterministic (fixed
  `Action` order; a deterministic visited-set/queue discipline) so results are
  reproducible run-to-run.
- **One code path (AC):** the oracle edge is core `legal_move` and the lap-close
  test is core's crossing logic — verified by construction (direct calls) and by
  a shared assertion in tests (design doc test notes: "the oracle uses the same
  supercover-legality as sim").
- Reachability results are returned as an inspectable set of `CarState` (or
  equivalent) so Ф5b and tests can query membership and compute `R ∩ B`; the
  exact container/visibility is a design call.
- File-size and per-fn limits per AGENTS.md; a `#[cfg(test)] mod tests` block is
  required (module has substantial logic).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `forward_reachable` explores the 4D `(x, y, vx, vy)` state space with `legal_move` edges (5 transitions), bounded so `|v| ≤ V_ceil`, from a given seed-state set; returns the reachable-state set. |
| AC2 | `backward_reachable` explores the same 4D space with the reversed `legal_move` (predecessor) relation, bounded so `|v| ≤ V_ceil`, from a given goal-state set; returns the set from which the goal is forward-reachable. |
| AC3 | The oracle edge is the identical `legal_move` used at runtime, and the S/F crossing uses core's signed-crossing logic — one code path, no reimplementation (verified by a shared test assertion). |
| AC4 | `oracle_liveness_V1(d, grid, sf, race_dir)` returns `true` iff a lap-closing forward S/F crossing (by `race_dir`) is reachable from the `v = 0` start-grid seeds within `|v| ≤ 1`; it distinguishes the race-start crossing from the lap-closing crossing. On a valid hand-built ring this is **`true`**. |
| AC5 | The search terminates on every input (finite bounded state space) and is deterministic (identical inputs → identical results). |
| AC6 | **(Core fix)** `register_move` counts a forward/reverse S/F crossing **iff** the swept `from → to` segment passes through the gate chord's **extent** (not merely its infinite supporting line): a full physically-continuous lap around a closed ring reaches `raw() >= 1`, while a crossing of the supporting line **outside** the chord extent (e.g. the far wall of the ring) does **not** change the counter. |
| AC7 | Tests: **(oracle)** on a small hand-built valid ring, V=1 liveness is `true`; on a broken ring (gap in the corridor), `false`; forward∩backward set membership is asserted for a few known states; a shared assertion pins AC3 (same supercover-legality as sim). **(core)** a gp-core-level test drives a physically-continuous closed loop and reaches `raw() >= 1` (a real lap), and asserts an off-chord supporting-line crossing is a no-op; existing scripted-duplicate `LapCounter` tests are updated to the corrected bounded-chord semantics. |

## Open questions

- Exact velocity-domain shape for the V=1 check (full L∞ box vs. cardinal-only),
  the container/visibility of the returned reachable sets, and the exact
  chord-extent bounding geometry for the corrected `register_move` (which of
  `gate_coord` / `crossing_event` / `register_move` carries the bound, and how the
  along-chord span is derived from `sf.gate` / `sf.chord`) are design-subagent
  calls, constrained by the Key-decisions defaults above; none blocks design.
</content>
</invoke>
