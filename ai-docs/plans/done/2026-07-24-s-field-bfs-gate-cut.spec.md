# gp-gen Ф7: s-field — fold-free BFS distance on the annulus cut at the gate

**Source:** issue #32
**Date:** 2026-07-24
**Tracked in:** #32

## Scope

Implement the computation that fills the corridor's **s-field**: the monotone
integer progress coordinate `s` over `D`, defined as the 4-connected BFS graph
distance on `D \ gate` — the corridor with the timing-gate dual edges removed —
seeded from the gate's **forward (`+race_dir`) face**, with the gate edges
acting as barriers so propagation cannot wrap around the antipode. This yields a
single-valued, fold-free `0 → L` coordinate per drivable cell (design doc §2,
N1 / P1 / D2, and `phase7_output`'s `s_field = bfs_distance(D \ gate_edges(sf),
seed = forward_face(sf, skel.dir))`).

Concretely, this task delivers the algorithm that **populates
`SField.dist`** (`crates/core/src/track.rs`). The `SField` container and its
consumer-facing accessors (`scalar_at`, `gradient_at`, `tangent_at`) already
exist and are tested; today production has no producer — `SField::new` fills
`dist` from a caller-supplied per-cell closure (used only by unit tests). This
task adds the real BFS producer.

Inputs available to the producer:
- the corridor `D` (`Corridor`, dense bitmap over a bounding box),
- the timing gate (`TimingGate { behind: Vec<Point>, forward: Side }` on
  `StartFinish`), whose `forward` side already encodes `+race_dir`,
- the ready-made barrier predicate `TimingGate::separates(a, b)` — true iff the
  dual edge between `a` and `b` is one of the gate's implied cut edges
  (order-independent).

The **forward face** = `{ behind[i] + forward.delta() }` — the drivable cells
one step ahead of the gate's `behind` cross-section, on the `+race_dir` side.
These seed cells are at distance `0`; distance grows the long way around the
loop and reaches its maximum `L` at the `behind` cells, producing the intended
`L → 0` reset across the gate cut.

## Out of scope

- **Wiring the s-field into the full `generate()` pipeline.** `generate()` in
  `crates/gen/src/lib.rs` is still a `todo!()` stub (no phase Ф1–Ф7 is wired
  yet). This task delivers the s-field *producer* as an independently callable,
  independently tested unit; end-to-end pipeline assembly is a later task.
- The `SField` container, gradient/tangent/scalar accessors, and the gate
  `separates` predicate — already implemented and tested (`track.rs`).
- The parameterized **render centerline** (medial-axis racing line) — a separate
  Ф7 output (design doc D2); a different task.
- AI-frame (`∇s`) and reward (`Δs`) *consumption* — blocks 3a / 4.

## Deferred

- End-to-end `generate()` pipeline integration | `generate()` is a stub, and
  wiring one phase into a non-existent pipeline is premature | tracked by the
  remaining Ф1–Ф7 build-order issues; no new issue needed.

## Key decisions

| Question | Decision |
|---|---|
| Barrier predicate | Reuse the existing `TimingGate::separates(a, b)` (gp-core `track.rs`) — skip a neighbor expansion whenever `separates` is true. Do **not** re-derive the cut. |
| Seed set | The forward face `{ behind[i] + forward.delta() }`, all at BFS distance `0` (multi-seed BFS). `race_dir` enters only through `gate.forward`, already `+race_dir`-oriented. |
| Distance type | `u32` per cell (matches `SField.dist: Vec<Option<u32>>`). Integer arithmetic only, per the deterministic-physics rule (design §3a). |
| Out-of-band cells | `None` (`¬D`), exactly as `SField.dist` already encodes. |
| In-band cells unreachable after the cut | `None`. On a connected annulus, cutting the gate's chord of edges leaves `D \ gate` connected (the strip reachable the long way), so this should not occur; a cell the BFS never reaches simply stays `None` rather than being special-cased. |
| Producer placement / signature | **Leave to the `design` Subagent.** Candidates: a new barrier-aware, multi-seed 4-conn BFS helper in gp-core `geom/graph.rs` (alongside `geodesic_bfs` / `geodesic_layers`, which are single-seed and barrier-free), vs. a gp-gen Ф7 function. Either can own the `SField`-assembly. Not spec-constraining. |

## Technical constraints

- **Integer-only, deterministic** (design §3a): the BFS operates on `u32`
  distances and `Point` lattice coordinates; no floats anywhere in the producer.
  Given identical `(D, gate)` inputs the field is bit-identical across runs.
- **Reuse existing primitives**: the corridor's dense-bitmap membership
  (`Corridor::contains`), its bounding-box `Rect` (`SField.rect` mirrors it),
  and `TimingGate::separates` for the barrier test. The existing single-seed
  `CorridorScratch::geodesic_bfs` does not support multi-seed or barrier edges,
  so it cannot be used unchanged — extending it or adding a sibling is a design
  choice.
- **Barrier symmetry**: `separates(a, b) == separates(b, a)` already holds; the
  BFS must consult it on every 4-neighbor step so the cut blocks both traversal
  directions (a one-directional barrier would leak the antipode fold back in).
- The producer must fill **every** in-`D` cell of `SField.rect` with `Some(d)`
  and every `¬D` cell with `None`, upholding the `dist.len() == rect.area()`
  invariant that `SField::scalar_at` / `gradient_at` rely on.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The s-field is the 4-connected BFS distance on `D \ gate`, seeded (distance `0`) from the forward face `{ behind[i] + forward.delta() }`, with every gate dual edge (`TimingGate::separates`) treated as an impassable barrier in both directions. |
| AC2 | Forward-face cells read `s = 0`; distance increases with graph distance the long way around the loop; the gate's `behind` cross-section reaches the maximum `L`. Every in-`D` cell of `rect` is `Some`, every `¬D` cell is `None` (`dist.len() == rect.area()`). |
| AC3 | On a symmetric ring fixture, every forward (`+race_dir`) unit move has `Δs ≥ 0`, **except** the single `L → 0` step across the gate — i.e. no antipode fold where forward motion decreases `s` (the fold a naive full-ring BFS from the gate would produce). |
| AC4 | The only `s` discontinuity around the loop is the intended `L → 0` reset at the gate; nowhere else does `s` jump. |
| AC5 | `s` is single-valued per cell (one distance per drivable cell — no projection-onto-polyline folding); asserted on a wide-pocket / hairpin fixture where a nearest-point-on-centerline definition would fold. |
| AC6 | Deterministic exact field values on a small annulus fixture (hand-computed distances asserted cell-by-cell); rerunning the producer on identical inputs yields identical output. |
| AC7 | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass; the new logic carries `#[cfg(test)] mod tests`; any Miri-aborting or cost test is gated per AGENTS.md (the pure-integer BFS is expected Miri-clean). |

## Open questions

- None design-blocking. The producer's home crate and exact signature
  (gp-core BFS helper vs. gp-gen Ф7 function) are deliberately left to the
  `design` Subagent per the Key Decisions row above.
