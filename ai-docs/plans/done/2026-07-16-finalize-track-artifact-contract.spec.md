# gp-core: finalize the TrackArtifact contract — s_field + start grid + gate segment + centerline sampling

**Source:** issue #6
**Date:** 2026-07-16
**Tracked in:** #6

Complete the block-1 → {3a, 4} track-artifact contract in `crates/core/src/track.rs`
(design doc §2 Ф7, §2 Exported artifact, §2 P2/D2/N1, §6). This issue defines the
**contract types and their read accessors**, exercised on hand-filled fixtures. It
does **not** build the generator that populates them (block 1, Ф7 `phase7_output`) nor
the lap-crossing logic that consumes the gate (`sim::LapCounter::register_move`, a
separate `TODO(3a)`).

## Scope

1. Extend `TrackArtifact` so it carries **all** of `{ corridor D, walls, sf (with its
   gate segment), race_dir, s_field, start grid, centerline, metrics }` — the two
   currently-missing members are the s-field and the start grid.
2. Finalize `StartFinish` (removing its `TODO(1)`): add the exact **timing-gate
   segment(s)** — dual edges on the half-grid, placed one edge ahead of the front row
   and spanning the corridor cross-section — sufficient, together with `race_dir`, for
   `LapCounter::register_move`'s half-open signed-crossing test (design §3, [C2]).
   Also make the gate's **forward (`+race_dir`) direction** determinable from the type.
3. Add the **s-field type**: a monotone integer scalar `0..=L` per drivable cell (the
   BFS distance on `D \ gate` seeded from the gate's forward face — design §2 N1), plus
   a **gradient/tangent accessor** that returns a defined unit tangent for **every**
   `D` cell in the band. Per P2/M1: `t̂ = normalize(∇s)`, `n̂ ⟂ t̂`; on the gate cells
   the tangent equals the forward `race_dir` direction (the cut is not differenced
   across).
4. Add the **start-grid type**: an ordered list of distinct `v = (0,0)` positions in
   `D`, front-to-back along `−race_dir` (design §2 Ф3, [C2]).
5. Implement `Centerline::at(s)` (removing its `todo!`): wrap `s` around the closed
   loop by `length`, linearly interpolate `pos` and `tangent` between the bracketing
   samples, tangent oriented along `race_dir`.
6. One-line `///` on every new public item; a `#[cfg(test)] mod tests` in the artifact
   module asserting the accessors' exact outputs on hand-filled fixtures.

## Out of scope

- **Populating** the artifact (the generator, block 1 Ф7 `phase7_output` /
  `generate_track`). This issue supplies the types + read accessors, tested on
  hand-built fixtures — not the pipeline that fills them.
- The `LapCounter::register_move` **crossing logic** (`sim.rs` `TODO(3a)`). This issue
  only guarantees `StartFinish` *carries* the data that logic will need.
- **Racing-line construction** (medial axis → trim-to-loop → arc-length resample). Only
  `Centerline::at` sampling of an already-populated sample list is implemented here.
- Rendering (block 2) and oracle-metrics computation (`TrackMetrics` stays as-is).

## Deferred

- Field population by the generator | block 1 (Ф7) is not yet built | no new issue —
  already covered by the block-1 build-order issues.

## Key decisions

| Question | Decision |
|---|---|
| Scalar / discrete-gradient numeric type | Integer: `s` is a BFS hop distance `0..=L`; the discrete gradient is integer neighbor differences. |
| Unit-direction numeric type in the artifact | `(f32, f32)`, mirroring the existing `CenterlineSample.tangent`. The deterministic physics modules (`sim`, `geom`) stay integer-only (design §3a); only the render/AI-facing artifact carries fractional directions, exactly as the current `Centerline` already does. |
| Accessor shape (one method vs `gradient_at` + `tangent_at`; return types) | Design's call (API surface). Contract: some accessor yields the unit tangent for every `D` cell, and the raw gradient **direction** is observable for the fixture test. |
| s-field storage | Design's call; a dense `Rect`-backed grid mirroring `Corridor` is the natural default. |
| Gate representation | Design's call — either a `Vec` of half-grid dual-edge unit segments, or a (line coordinate + span + forward normal). Must support the half-open crossing test. |
| Forward-direction source at the gate | Derived from `orient` + `race_dir`, or stored explicitly on `StartFinish`; design chooses. Needed both for the gate special-case in the tangent accessor and for `register_move`'s sign. |
| Start-grid element type | `Point` (with `v = (0,0)` implicit and documented) vs `CarState`; design's call. |
| Gradient scheme at band-boundary cells | Design's call (one-sided / central differences over in-band neighbors). Contract only requires the tangent be *defined for every `D` cell*. |
| Module placement / file split | Design's call; split `track.rs` into a `track/` dir if it crosses the soft size limit. |

## Technical constraints

- `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check`
  clean; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` clean (no broken
  intra-doc links).
- Every new public item carries a one-line `///` (doc-convention).
- The deterministic physics modules (`sim`, `geom`) remain integer-only and
  deterministic (design §3a). The artifact directions use `(f32, f32)` as the current
  centerline sample already does.
- Accessors are **pure** functions of the stored artifact (deterministic).
- New logic ≥ ~50 lines requires a `#[cfg(test)] mod tests` block; respect the soft
  file-size limits (500 excl. / 800 incl. tests).
- No new dependency is expected — BFS uses the existing `geom` helpers, and
  `enumflags2` is already present; verify (`grep` + `cargo tree`) before adding any.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `TrackArtifact` exposes public members for all of `{ corridor D, walls, sf (carrying the gate segment(s)), race_dir, s_field, start grid, centerline, metrics }`; the workspace builds. |
| AC2 | The s-field type exposes a monotone integer `0..=L` scalar per `D` cell and a gradient/tangent accessor returning a **defined** unit tangent for **every** `D` cell in the band; a fixture whose `s` increases along `+x` yields a tangent pointing `+x` (gradient direction assertion). |
| AC3 | On the gate cells, the tangent accessor returns the forward `race_dir` direction (unit vector perpendicular to the chord, `+race_dir` sense) — asserted on a fixture; the field is **not** differenced across the gate cut. |
| AC4 | `Centerline::at(s)` returns `None` on an empty centerline; wraps `s` modulo `length` (`at(length)` ≡ `at(0)`, `at(length + x)` ≡ `at(x)`); linearly interpolates `pos` and `tangent` between bracketing samples with tangent oriented along `race_dir` — asserted on a small hand-built sample list. |
| AC5 | The start-grid type holds an ordered list of distinct `v = (0,0)` positions in `D`, front-to-back along `−race_dir`; a fixture asserts the ordering, and the type documents the distinct-and-in-`D` invariant (upheld by the generator). |
| AC6 | `StartFinish` carries the exact timing-gate segment(s) (one dual edge ahead of the front row, spanning the cross-section) and enough to determine the forward `+race_dir` direction — sufficient for `LapCounter::register_move`'s half-open signed-crossing test; its `TODO(1)` is removed. |
| AC7 | Every new public item has a one-line `///`; a `#[cfg(test)] mod tests` in the artifact module covers the accessors' exact outputs on hand-filled fixtures (s-field gradient direction, gate tangent = `race_dir`, centerline wrap/interpolation, start-grid ordering). Doc and clippy gates above are green. |

## Open questions

None blocking design. Representation choices (gate encoding, s-field storage, accessor
signatures, start-grid element type) are enumerated in *Key decisions* as design-phase
selections with defensible defaults, not user-facing questions.
