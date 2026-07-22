# gp-gen Ф3 — start/finish, accel zone, start grid, timing gate

**Source:** issue #26
**Date:** 2026-07-23
**Tracked in:** #26

## Scope

Implement Block 1 phase **Ф3** in `gp-gen` (`crates/gen/src/phase3.rs`, `pub use phase3::*;` from `lib.rs`) — the `phase3_start_finish` step of the design-doc §2 pipeline. Given Ф2's fine corridor `D` and Ф1's `CoarseSkeleton` (with the fixed global `RaceDir`), the phase:

1. **Picks a straight run** of the ring (not a corner) to host the start/finish.
2. **Orients the S/F across the corridor** (`Orient::Horizontal` or `Orient::Vertical`, perpendicular to the segment's tangent) and **thickens** that cross-section to width `≥ m` (`m = GenParams.cars`).
3. **Derives the local forward direction** as the global `RaceDir` (`skel.dir`) projected onto the segment tangent — i.e. the `Side` the cars drive toward.
4. Builds the two **distinct** [C2] objects:
   - **Start grid** ([`StartGrid`]): `m` distinct start positions in `D`, all implicitly `v = (0, 0)`, laid out as `ceil(m / width)` rows from a front row backward along `−race_dir`, ordered front-to-back.
   - **Timing gate `sf`** ([`StartFinish`] carrying its [`TimingGate`]): a half-grid dual edge placed **one edge ahead of the front row**, so every start cell is strictly behind it at `t = 0`. `chord` = the front-row cross-section (width `≥ m`); `gate.behind` = that cross-section; `gate.forward` = the local forward `Side`.
5. **Measures** the two straight-length budgets and exposes them for the ACs (see Key decisions on enforcement vs. measurement):
   - accel zone `≥ ~V_target²/2` fine points **forward** (along `race_dir`) to the first corner;
   - grid straight `≥ ceil(m/width)` rows **backward** (along `−race_dir`) inside `D`.

The phase returns the (possibly thickened) corridor `D`, the `StartFinish` (`sf`), and the `StartGrid` — the design's `(D, sf, gr)` triple. All new Ф3 arithmetic is integer-only (the deterministic integer discipline of `docs/design.md` §3a). Follows the established Ф1/Ф2 posture: deterministic, no RNG of its own beyond what `skel` already fixed, and **total / infallible** (no `Result`, no production panic).

## Out of scope

- `s_field` (Ф7 / N1 gate-cut BFS distance field), `centerline` (Ф7 render curve), and oracle `metrics` (Ф5) — those are later phases; Ф3 emits only `(D, sf, grid)`.
- Static validation (Ф4), passability oracle (Ф5), and local repair (Ф6) — including any **seed-retry / rejection** when a ring has no straight long enough for the accel zone or the grid rows. Ф3 measures those budgets; enforcing them is the not-yet-built pipeline loop's job (see Deferred).
- Wiring Ф3 into the top-level `generate()` (still `todo!()`), and any change to `TrackArtifact` assembly.
- New gp-core types — `StartFinish`, `TimingGate`, `StartGrid`, `RaceDir`, `Orient`, and `LapCounter` already exist (deps #6, #8 landed) and are consumed as-is.

## Deferred

| What | Why | Separate issue? |
|---|---|---|
| Add a `v_target` field to `GenParams` | Ф3 takes `v_target` as an explicit parameter (matching the `phase3_start_finish(D, skel, m, V_target)` pseudocode). `GenParams` today carries only `v_ceiling` (= `V_ceil`, the oracle BFS scaffold — a **distinct** quantity from `V_target` per design D3; the two must never be conflated). Plumbing `V_target` into `GenParams` belongs with the `generate()` wiring, which is out of scope here. | Folds into the future `generate()` / Ф4–Ф7 wiring task; no separate issue needed now. |
| Enforce accel-zone / grid-straight budgets by seed-retry | Requires the Ф4/Ф5/Ф6 loop, which is unbuilt. | Belongs to the Ф4–Ф6 tasks already on the block roadmap. |

## Key decisions

| Question | Decision |
|---|---|
| How is `V_target` supplied to Ф3? | As an explicit `v_target: i32` parameter to `phase3_start_finish`, **not** reused from `GenParams.v_ceiling` (design D3 forbids conflating `V_target` with `V_ceil`). `accel_zone` threshold is the integer `v_target² / 2`. |
| Fallible (`Result`) or total? | **Total / infallible**, mirroring `phase1_coarse_ring` and `phase2_rasterize` (both total, no panic). Ф3 always produces a best-effort `(D, sf, grid)` from the chosen straight. |
| Are accel-zone (`≥ V_target²/2`) and grid-straight (`≥ rows`) **guarantees** or **measured outputs**? | **Measured**, not guaranteed by Ф3. Ф3 selects the straight, thickens, and lays out grid + gate; it does not reject or retry when a budget is short. The ACs assert these budgets on fixtures whose ring provides an adequate straight; enforcement is deferred to the Ф4–Ф6 loop. This resolves the pseudocode's `ensure …` into "measure and expose" for this slice. |
| Which straight is picked when several qualify? | The straight run offering the most forward headroom (longest run available for the accel zone), chosen **deterministically** (stable tie-break) so same-`skel` runs reproduce bit-for-bit — consistent with Ф1/Ф2 determinism. Exact selection algorithm is design's call. |
| Does Ф3 mutate `D` (thicken)? | Yes — thickening the S/F cross-section to `≥ m` is Ф3's job (pseudocode `thicken(D, seg, width ≥ m)`). It is **additive** (push the outer wall out; never carve), mirroring Ф2's additive, topology-preserving discipline. When the corridor is already `≥ m` wide at the segment (e.g. `k ≥ m`), thickening is a no-op. |
| Relationship of `sf.chord`, `gate.behind`, and the grid front row | `sf.chord` = the front-row cross-section points (width `≥ m`); `gate.behind` = the same cells; the gate's implied dual edges sit one edge **forward** of them; the grid's front row occupies `sf.chord`, with `ceil(m/width) − 1` further rows behind along `−race_dir`. This places every start cell strictly behind the gate line (`gate_coord < GATE_LINE`), so `LapCounter` reads `raw() == −1` for all cars at `t = 0`. |

## Technical constraints

- **Integer-only arithmetic.** All new Ф3 logic uses integer arithmetic only — no floating-point (the deterministic integer discipline of `docs/design.md` §3a; `v_target² / 2` is integer division). The existing unit-vector accessor on `TimingGate` is untouched.
- **Zero production panic**, `saturating_*` / `try_from(..).unwrap_or(..)` discipline as in Ф1/Ф2.
- **Consumes existing gp-core types unchanged:** `Corridor`, `Point`, `Side`, `Orient`, `RaceDir`, and `track::{StartFinish, TimingGate, StartGrid}`. `gate.forward` is the local-forward `Side`; `sf.orient` is the chord orientation (`Horizontal`/`Vertical`).
- **Gate ↔ lap-counter contract.** The produced `sf` must satisfy `gp_core::sim::LapCounter`'s half-open crossing test: `gate.behind` is the front row (doubled `gate_coord == 0`), the gate line is the odd midpoint `+1`, so every start cell is at `gate_coord ≤ 0 < GATE_LINE`. This is the geometric root of the "all start cars read `counter = −1`" invariant.
- **File-size / test discipline:** a `#[cfg(test)] mod tests` block; soft 500 / hard 1000 production lines (Ф1 `phase1.rs` and Ф2 `phase2.rs` are near-precedent for the incl.-tests budget). Strict clippy (`-D warnings`), `cargo fmt`.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The S/F sits on a straight run of the ring (not a corner), is oriented **across** the corridor (`sf.orient` perpendicular to the segment tangent), and `sf.width() ≥ m` (`m = cars`). |
| AC2 | On a fixture whose chosen straight is long enough, the measured acceleration zone forward of the S/F (along `race_dir`, up to the first corner) is `≥ v_target² / 2` fine points. |
| AC3 | The start grid holds exactly `m` **distinct** positions, all in `D`, laid out as `ceil(m / width)` rows from a front row backward along `−race_dir`, `positions` ordered front-to-back. Each is implicitly `v = (0, 0)` (per `StartGrid`'s contract). |
| AC4 | The timing gate is the half-grid dual edge one edge ahead of the front row: `gate.behind` equals the front-row cross-section, `gate.forward` is the local-forward `Side`, and **every** start-grid cell is strictly behind the gate (`gate_coord(cell) < GATE_LINE`, i.e. `≤ 0`), for **every** row. |
| AC5 | `race_dir` (`sf`'s local forward) equals the global ring orientation (`skel.dir`) projected onto the segment tangent: `gate.forward` is the `Side` a `skel.dir`-traversing car heads along that straight; the artifact's `race_dir` stays `skel.dir` (global CW/CCW). |
| AC6 | Feeding the produced `sf` into a fresh `gp_core::sim::LapCounter` (init `−1`), no start position has yet crossed the gate: every start car reads `raw() == −1` / `laps() == 0` at `t = 0`, and a first forward move from the front row registers a `+1` crossing (half-open test self-consistent for every row). |
| AC7 | The grid-straight budget is measured: `D` provides `≥ ceil(m / width)` rows of straight behind the front row along `−race_dir` on an adequate fixture (all grid rows fit inside `D`, distinct, no overlap with the gate/¬D). |
| AC8 | Determinism: `phase3_start_finish` on the same `(D, skel, m, v_target)` yields byte-identical `(D, sf, grid)` across repeated calls (no `HashSet`-order or RNG-order leakage), consistent with Ф1/Ф2. |
| AC9 | Ф3 is total: no `Result`, no production panic on any Ф1→Ф2 output; the workspace stays green (`cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -D warnings`, doc gate). |

## Open questions

- None blocking design. The precise straight-selection algorithm, the thickening mechanism (and whether it needs a Ф2-style topology-safe pocket-absorption pass), and the internal return shape (tuple vs. small struct) are left to the `design` Subagent.
