# gp-gen Ф5b: full Vmax passability oracle (iterative deepening) + speed metrics + break_points

**Source:** issue #29
**Date:** 2026-07-23
**Tracked in:** #29

## Scope

Build the **full passability oracle** `phase5_full_oracle` in `gp-gen` (design
doc §2 Ф5b pseudocode, §3 *Оракул проходимости* / *Vmax_attain* / *Метрики
скорости*), composing the Ф5a substrate (`forward_reachable`,
`backward_reachable`, already committed in `crates/gen/src/phase5.rs`). Concretely:

1. **Iterative-deepening `Vmax` oracle.** A loop over `V_ceil = 1, 2, 4, 8, …`
   that on each iteration computes `R = forward_reachable(v=0 start seeds, |v| ≤
   V_ceil)`, `B = backward_reachable(lap-close goal, |v| ≤ V_ceil)`, and the
   live set `live = R ∩ B`. The loop **halts** when `Vpeak = max|v| over live <
   V_ceil` — reporting that `Vpeak` as the true `Vmax_attain` (the geometry, not
   `V_ceil`, is the binding limit). If a doubling still connects at `Vpeak ==
   V_ceil`, deepen (`V_ceil *= 2`); the final BFS may run to at most 2× the
   needed ceiling — a bounded, correct over-shoot (design §3).

2. **`R \ B` = provable crash, excluded.** A high-speed state reachable from the
   start (`∈ R`) but from which the lap-close goal is **not** backward-reachable
   (`∉ B`) — i.e. it cannot brake/finish in time — is dropped from `live`
   (design §3: «в `R`, но не в `B` → провабельный краш, отбрасывается»).

3. **`break_points` on no lap.** When `live` contains no lap-closing state,
   return `laps_exist = false` with a **non-empty** `break_points` = the
   *frontier gap* between `R` and the lap-close goal (design §2 `[N3]`
   `frontier_gap(R, goal)`) — the raw reachability-stall diagnostic. Translating
   a frontier gap into a concrete dual edge (`map_frontier_gap_to_edge`) is the
   Ф6 repair step and is **out of scope** (design §2 `[N3]`).

4. **Speed metrics on success (almost free on top of `R ∩ B`).** When a lap
   exists, return `laps_exist = true` plus:
   - `Vmax_attain = Vpeak` (peak attainable speed);
   - `fastest_lap` = the fewest-**move** path through `live` from a start seed to
     a lap-close crossing (BFS/shortest-path over the live-state graph);
   - `tempo = lap_length / (move count of `fastest_lap`)` — the honest fastness
     scalar (design §3: integrates straights and braking);
   - `speed_heatmap` = for each corridor point, the max `|v|` over the live
     states at that point (design §3: where's-fast/slow, diagnoses run-out).

The oracle edge is core's `legal_move` and the lap-close test is core's
signed-crossing logic (`LapCounter::register_move`, fixed to bounded-chord
semantics in Ф5a) — reused via the Ф5a substrate, **never reimplemented**
(design §3; carries forward Ф5a AC3).

## Out of scope

- **`map_frontier_gap_to_edge` / Ф6 local repair** (design §2 Ф6, `[N3]`). This
  issue emits the raw `frontier_gap` diagnostic; mapping it to the dual edge to
  shift, and the repair loop, are the separate Ф6 build-order item. `[N3]` names
  this the riskiest unproven step — explicitly deferred.
- **`generate()` pipeline wiring.** Calling `phase5_full_oracle` from the
  `generate()` loop (design §2 pseudocode `res = phase5_full_oracle(...)` on
  green) stays deferred, exactly as Ф5a deferred wiring `oracle_liveness_v1`.
  `generate()` remains `todo!`; this issue adds a callable oracle, not its call
  site.
- **Ф7 output assembly** (`phase7_output`: `walls` / `s_field` / `centerline`).
  Those artifact members already exist independently (`crates/core/src/track.rs`);
  this issue does not build the Ф7 assembler nor populate a whole `TrackArtifact`.
  It computes the oracle result / `TrackMetrics` fields only.
- **Any `gp-core` change.** `legal_move` and the crossing test are consumed
  unchanged; the bounded-chord `register_move` fix already landed in Ф5a (#28).
- **`V_target` / design-input sizing.** `V_ceil` is the oracle's sliding search
  boundary (a BFS scaffold), never the design input `V_target` (design §2 D3) —
  this issue does not read or size against `V_target`.

## Deferred

- Frontier-gap → dual-edge mapping + Ф6 repair | risky, distinct build-order
  item (design `[N3]`) | yes — tracked by the Ф6 build-order issue; no new issue
  from this spec.
- `generate()` / Ф7 wiring of the oracle result | separate integration item |
  no new issue needed (design build order covers it).

## Key decisions

| Question | Decision |
|---|---|
| What does this issue build on top of Ф5a? | The **deepening loop + metrics**, composing the already-committed `forward_reachable` / `backward_reachable` (Ф5a, `phase5.rs`). Ф5a delivered the two floods and `oracle_liveness_v1`; Ф5b delivers `phase5_full_oracle`. No re-implementation of the floods. |
| `max|v|` / `Vpeak` norm | **L∞ (Chebyshev): `max(|vx|, |vy|)`**, consistent with Ф5a's `within_v_ceil` L∞ box bound (`|vx| ≤ V_ceil ∧ |vy| ≤ V_ceil`). `Vpeak = max over live states of max(|vx|, |vy|)`; the halt test `Vpeak < V_ceil` therefore reads directly against the same bound the floods enforce. |
| `live = R ∩ B` — how is "lap-close goal" defined, and does it need the counter? | The live set must distinguish a **lap-close** crossing from the race-start crossing — the same reason Ф5a threaded a `LapCounter` through its flood rather than scanning a plain reachable set (design; Ф5a rejected-alternative). Reusing core's `register_move` for the crossing test is required (one code path). The exact mechanism (augmented `(CarState, LapCounter)` flood for `B` / `live`, vs. an explicit goal-state enumeration just ahead of the gate) is a **design call**, constrained to reuse core's crossing logic. |
| `tempo` semantics | Per design §3: `tempo = lap_length / (move count of fastest lap)`, where **`lap_length` is the fixed loop length** (a track property, path-independent) and the move count varies with speed — so a track you can carry speed around completes the fixed loop in fewer moves → higher tempo. This is the "honest" scalar that integrates straights and braking, distinct from the peak `Vmax_attain`. |
| Concrete `lap_length` measure | The oracle runs **before** Ф7 builds `s_field` / `centerline`, so it cannot read those. The concrete path-independent loop-length measure the oracle derives (e.g. loop length in cells via a cycle length, or the geometric length of a canonical lap) is a **design call** — see Open questions; it fixes exact fixture values but not the oracle's architecture. |
| `fastest_lap` path metric | Fewest **moves** (turns), not fewest cells — a BFS/shortest-path over the *live-state* graph (nodes = live `CarState`s, edges = `legal_move`), from a start seed to the first lap-close crossing. `len(fastest)` in `tempo` is this move count. |
| `speed_heatmap` shape | `point → max |v| over live states at that point` (L∞ norm, integer). Container shape (`Vec<(Point, i32)>` matching the existing `TrackMetrics.speed_heatmap`, or a map) is a design call; the semantic is fixed. |
| `break_points` / `frontier_gap` shape | The raw reachability-stall diagnostic between `R` and the unreached lap-close goal (design `[N3]`). Concrete shape (e.g. `Vec<Point>` of frontier positions) is a **design call** — it feeds Ф6's `map_frontier_gap_to_edge` later, which is out of scope. Only the invariant matters here: **non-empty** exactly when `laps_exist = false`. |
| Return type | A new oracle-result type is needed: `TrackMetrics` (`crates/core/src/track.rs`) carries `vmax_attain` / `tempo` / `fastest_lap` / `speed_heatmap` but **not** `laps_exist` or `break_points`. Shape (an enum `{ Lappable(metrics) \| NotLappable { break_points } }`, or a struct with a `laps_exist` flag) is a design call; on success it populates the existing `TrackMetrics` fields. Whether the new type lives in `gp-gen` or `gp-core` is a design call. |
| New dependencies | None — deepening and shortest-path use `std` collections; `gp-gen` already depends on `gp-core`. |
| Pipeline integration | Deferred (matches Ф5a): deliver the callable `phase5_full_oracle`, not its `generate()` call site. |

## Technical constraints

- **Integer-only, deterministic, total search.** The deepening loop, the `R` /
  `B` floods, `Vpeak`, `speed_heatmap` speeds, `fastest_lap` move counts, and
  the shortest-path search are integer arithmetic throughout (design §3a;
  `gp-gen` is pure-integer). `tempo` populates the existing artifact-contract
  metric field of the same name (`crates/core/src/track.rs` `TrackMetrics`) and
  is the sole derived-ratio output — its representation matches that
  already-established field, unchanged by this issue. No production panic; no RNG.
- **Termination (AC).** Each inner flood is over a finite state space (corridor
  cells × the bounded L∞ velocity box), so each terminates. The outer deepening
  halts because `Vmax_attain` is geometry-bounded (a speed whose braking
  distance exceeds the longest straight is unreachable on a completable lap,
  design §3) and the doubling runs to at most 2× the true ceiling — a bounded
  over-shoot. Deterministic: fixed `Action` order + deterministic worklist
  discipline (as in Ф5a) → identical inputs yield identical results.
- **One code path (AC, carried from Ф5a AC3).** The oracle edge is core
  `legal_move` (via `forward_reachable` / `backward_reachable`) and the lap-close
  test is core's `register_move` crossing logic — verified by construction
  (the floods are the Ф5a functions) and a shared assertion in tests. No parallel
  legality or crossing rule is introduced.
- **Composes Ф5a unchanged.** `forward_reachable` / `backward_reachable` /
  `within_v_ceil` are reused as-is; if the deepening needs a lap-close-aware
  variant of the backward flood (counter-threaded goal), it extends the Ф5a
  substrate rather than forking a second flood. Any Ф5a signature change is a
  design call flagged here, not assumed.
- **Miri.** `gp-gen` is the sanctioned crate-level `--exclude` from the Miri
  gate (#134 cost carve-out); the new code is pure integer and Miri-clean and
  rides that carve-out — no per-test gating needed.
- **File-size / per-fn limits** per AGENTS.md; a `#[cfg(test)] mod tests` block
  is required (substantial logic). `phase5.rs` already exists — placement of the
  new oracle (same file vs. a new `phase5b`/`oracle` module) is a design call
  under the file-size limits.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `phase5_full_oracle(d, grid, sf, race_dir)` runs iterative deepening `V_ceil = 1, 2, 4, 8, …`, computing `R`, `B`, and `live = R ∩ B` (via the Ф5a floods) each iteration, and **halts** when `Vpeak = max|v| over live < V_ceil`, reporting that `Vpeak` as `Vmax_attain`. |
| AC2 | A state in `R` but **not** in `B` (high speed with no in-time braking/finish — a provable crash) is **excluded** from `live`; `live = R ∩ B` exactly. |
| AC3 | When `live` holds no lap-closing state, the oracle returns `laps_exist = false` with a **non-empty** `break_points` = the frontier gap between `R` and the lap-close goal. |
| AC4 | On success the oracle returns `laps_exist = true`, `vmax_attain = Vpeak`, `fastest_lap` (fewest-move path through `live` from a start seed to a lap-close crossing), `tempo = lap_length / len(fastest_lap)`, and `speed_heatmap` (per-corridor-point max `|v|` over live states). |
| AC5 | The oracle edge is the identical `legal_move` and the lap-close test is core's signed-crossing logic — reused via the Ф5a substrate, no reimplementation (shared test assertion pins this, as in Ф5a AC3). |
| AC6 | The search is integer-only, deterministic, and terminates on every input (finite bounded state space; deepening over-shoots by at most 2×). |
| AC7 | Tests: **(long straight)** on a track with one long straight, `Vmax_attain` is dominated by that straight and `tempo` is lower than `Vmax_attain` alone implies (tempo integrates the required braking). **(provable crash)** a high-speed state that cannot brake in time is asserted **absent** from `live` (in `R`, not in `B`). **(untraversable ring)** `break_points` is non-empty and `laps_exist = false` on a broken/untraversable ring. **(exact metrics)** deterministic exact `Vmax_attain` / `tempo` / `fastest_lap` / `speed_heatmap` values on a small hand-built fixture. A shared assertion pins AC5 (same `legal_move` / crossing path as sim). |

## Open questions

None blocking design. Design-subagent calls, each constrained by a Key-decisions
default above and none altering the oracle's overall shape:

- **Concrete `lap_length` measure** for `tempo` (the semantic — fixed loop length
  ÷ fastest move count — is pinned by design §3; the concrete path-independent
  measure the oracle derives pre-Ф7 fixes exact fixture values only).
- **`live = R ∩ B` lap-close mechanism** — augmented `(CarState, LapCounter)`
  flood (as Ф5a) vs. explicit goal-state enumeration ahead of the gate — and
  whether it extends the Ф5a `backward_reachable` signature.
- **Concrete shapes / homes** of the new oracle-result type, `break_points`, and
  `speed_heatmap` container (semantics fixed above; exact types are design's).
- **Module placement** of the new oracle (extend `phase5.rs` vs. a new module)
  under the file-size limits.
