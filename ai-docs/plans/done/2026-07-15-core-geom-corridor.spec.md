# gp-core corridor graph helpers — flood-fill, components, geodesic BFS, walls-from-boundary

**Source:** issue #5
**Date:** 2026-07-15
**Tracked in:** #5

## Scope

Extend `crates/core/src/geom.rs` (crate `gp-core`) beyond the existing
`Corridor` / `Point` / `Wall` / `supercover` scaffold with the shared
corridor-graph helpers every downstream block reads. All operate over the
existing dense-bitmap `Corridor`, are pure, deterministic, and integer-only
(no floats, no RNG in geom), and need no new dependency (std-only).

1. **4-connected flood-fill** — from a seed point, the set (and/or count) of
   `D`-cells 4-reachable from the seed without leaving `D`. `neighbors4`
   already exists; 4-connectivity is the analysis metric of design §1, *not*
   car movement.
2. **Connected-component counting** — the number of 4-conn components of `D`,
   and of its complement `¬D`. Complement counting must express the design §2
   Ф4 annulus check ("complement has exactly one **bounded** component of
   ≥1 point"), i.e. distinguish a bounded infield hole from the unbounded
   outfield.
3. **In-`D` geodesic BFS (nearest-free layer expansion)** — from a seed, emit
   `D`-cells grouped by strictly increasing 4-conn geodesic distance
   (layer-by-layer), never crossing a wall or leaving `D`. Equal-distance cells
   surface in the **same** layer, so the collision-resolution caller's seeded
   RNG (design §3) can pick among ties. Consumed by `sim.rs::resolve_collisions`
   (currently a `todo!()`).
4. **`walls_from_boundary`** — derive the exact set of dual edges on the
   `D ↔ ¬D` boundary: one edge per adjacent (drivable, non-drivable) cell pair,
   each edge exactly once, none passing through a point (by construction of the
   duality, design §1). Feeds `TrackArtifact::walls` (Ф7).

## Out of scope

- The seeded-RNG tie-break and occupied-set filtering of car-collision
  resolution — those stay in `sim.rs::resolve_collisions` (design §3). geom
  emits deterministic layers; the RNG pick among an equal-distance layer is the
  caller's. (The deterministic integer core forbids RNG inside geom.)
- `step`, crash resolution, the signed lap counter, and the passability oracle
  — later Block-3a tasks in `sim.rs`.
- Distance-transform / medial-axis / cross-section **width** metric (the other
  half of §2 Ф4) — a distinct primitive; this task ships the flood-fill /
  component / BFS / wall helpers Ф4 composes, not the width measure.
- `Centerline` / `s_field` construction — block-1 products in `track.rs`.
- No change to `supercover`, `Corridor` membership, or `legal_move` /
  `legal_mask` — already implemented; consumed as-is.

## Deferred

- §2 Ф4 cross-section **width** distance-transform | not in this issue's ACs, but Ф4 needs it and it's a separate primitive | yes — recommend a follow-up gp-core issue (note for `/triage`).

## Key decisions

| Question | Decision |
|---|---|
| Connectivity | 4-connected throughout (design §1 — analysis metric, not car movement). Reuse `Point::neighbors4`. Manhattan/geodesic distance is 4-conn. |
| Complement semantics | Count `¬D` components over the bitmap box, treating everything **outside** the box as one connected unbounded outfield; a complement component is *unbounded* iff it touches the box boundary, *bounded* (an infield hole) otherwise. This makes the §2 Ф4 "exactly one bounded hole ≥1 cell" test expressible. Works regardless of box margin. |
| Determinism / no RNG / no floats | gp-core is integer-only and deterministic (design §3a; AGENTS.md § Code Style). BFS emits layers in a fixed, reproducible order; any RNG tie-break belongs to the caller (`sim`). Applied silently per convention. |
| Geodesic BFS output | Emit `D`-cells grouped by 4-conn geodesic distance layer; same-distance cells share a layer (so a seeded caller can choose among ties). Single-source covers the collision-resolution need; whether to also accept multiple seeds (a superset) is design's call. Concrete shape (`Vec<Vec<Point>>` / iterator / callback / distance array) → design. |
| Wall representation & anchoring | Today's `Wall{cell, orient}` with 2-variant `Orient{Horizontal,Vertical}` cannot by itself name *which* of a cell's four sides an edge is on. `walls_from_boundary` must emit unambiguous, once-only boundary edges. Whether design extends `Orient` to a 4-way side, adopts a canonical anchoring convention (e.g. anchor each edge to its `D`-cell + the outward neighbour direction — which also yields each boundary edge exactly once), or uses a half-grid edge coordinate is design's call. Pre-publish API is freely breakable (AGENTS.md § API Stability); the only downstream field is `track.rs::TrackArtifact.walls`, contained within gp-core. |
| Scratch reuse | A single traversal's `visited` set is O(area) and unavoidable. The "without per-query O(area) scratch where avoidable" constraint targets **repeated** queries (collision resolution places multiple cars): reuse one traversal buffer / generation-stamped `visited` across queries rather than reallocating a full-box buffer per car. Exact mechanism → design. |
| Test placement | In-file `#[cfg(test)] mod tests` in `geom.rs`; exact integer assertions on hand-built corridors (annulus, rectangular ring). AGENTS.md Rust Test Conventions. |

## Technical constraints

- Rust, crate `gp-core`; edit `crates/core/src/geom.rs` (+ its `#[cfg(test)]`
  module). May touch `crates/core/src/track.rs` / `lib.rs` re-exports **only**
  if the wall representation changes.
- Integer-only, no floating-point; deterministic; no RNG in geom. std-only —
  no new dependency (gp-core's `[dependencies]` is empty; BFS / flood-fill /
  wall-derivation need no crate).
- Operate over the dense bitmap; cells outside the box are `¬D` (consistent
  with `Corridor::contains`).
- Strict clippy (`-D warnings`), `cargo fmt`; every public item keeps a `///`
  doc; no broken intra-doc links (`RUSTDOCFLAGS="-D warnings" cargo doc`).
- File size: `geom.rs` is ~330 lines incl. tests; four helpers + tests may push
  the non-test count toward the 200–400 target — design may split into a
  submodule if it grows past the cap (AGENTS.md § File size).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Flood-fill from a seed returns exactly the 4-conn component of `D` containing the seed (empty when the seed ∉ `D`); component counting returns the exact number of 4-conn components of `D`. Verified on hand-built corridors with known counts. |
| AC2 | Complement (`¬D`) component counting returns the exact number of 4-conn components and distinguishes **bounded** (infield hole) from **unbounded** (outfield, touching the box boundary), so the §2 Ф4 check "exactly one bounded hole of ≥1 cell" is expressible. Verified: solid disk → 0 bounded holes; annulus → 1; two-hole shape → 2. |
| AC3 | Geodesic BFS emits `D`-cells layer-by-layer in strictly increasing 4-conn geodesic distance from the seed, never crossing a wall or leaving `D`; every cell at equal distance appears in the **same** layer (ties grouped, for the caller's seeded pick). Exact layer membership asserted on a small annulus, including at least one equal-distance tie in a single layer. |
| AC4 | `walls_from_boundary` yields exactly the set of dual edges on the `D ↔ ¬D` boundary — one edge per adjacent (drivable, non-drivable) cell pair, each edge exactly once, none passing through a point — asserted as an exact edge set for a rectangular ring. |
| AC5 | All helpers are integer-only and deterministic (no floats, no RNG): identical output for identical input on every run. |
| AC6 | Helpers avoid O(area) per-query scratch where avoidable — repeated queries (e.g. placing multiple cars) reuse traversal scratch rather than reallocating a full-box buffer per query. (Design/review-checked; not a unit-test assertion.) |
| AC7 | The full test table ships as in-file unit tests (D & complement component counts, BFS layer membership on an annulus with an equal-distance tie, exact wall-edge set for a rectangular ring); `cargo test` passes, `cargo clippy --workspace --all-targets -- -D warnings` is clean, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` is clean. |

## Open questions

- **Exact API surface of each helper** (return `Vec<Point>` vs iterator vs a
  component-label array; single- vs multi-source geodesic BFS; whether
  flood-fill and component-counting share one internal traversal). Design's
  call; defensible defaults recorded above. Not design-blocking.
- **Wall representation** — extend `Orient` to a 4-way side, adopt a canonical
  `D`-cell anchoring convention, or introduce a half-grid edge coordinate.
  Design picks; the observable contract (unambiguous, once-only boundary edges,
  none through a point) holds either way. Touches only `track.rs::walls`,
  contained in gp-core.
- **Whether the §2 Ф4 width distance-transform ships here** — recommended as a
  follow-up (outside this issue's ACs); noted for `/triage`.
