# Design: gp-core corridor graph helpers — flood-fill, components, geodesic BFS, walls-from-boundary

**Issue:** #5
**Date:** 2026-07-15

## Approach

Extend `crates/core/src/geom.rs` (crate `gp-core`) with four cohesive
corridor-graph helpers over the existing dense-bitmap `Corridor`, plus one small
type change to make walls unambiguous. All are pure, deterministic, integer-only,
std-only (no new dependency), consistent with the deterministic-core rule
(design §3a; AGENTS.md § Code Style). Every path routes through `Corridor::index`
/ `Corridor::contains` (cells outside the box are `¬D`), reusing the existing
justified `#[allow(clippy::cast_sign_loss)]` pattern for `i32`↔`usize` index math.

### Public API added to `gp-core::geom`

| Item | Signature | AC |
|---|---|---|
| `Side` | `enum Side { East, West, North, South }` + `const ALL: [Self; 4]`, `const fn delta(self) -> (Coord, Coord)` | AC4 |
| `Wall` (changed) | `struct Wall { cell: Point, side: Side }` (was `{ cell, orient: Orient }`) | AC4 |
| `flood_fill` | `fn flood_fill(d: &Corridor, seed: Point) -> Vec<Point>` | AC1 |
| `component_count` | `fn component_count(d: &Corridor) -> usize` | AC1 |
| `bounded_complement_components` | `fn bounded_complement_components(d: &Corridor) -> usize` | AC2 |
| `CorridorScratch` | `struct CorridorScratch` + `fn new(d: &Corridor) -> Self` + `fn geodesic_bfs<B>(&mut self, d: &Corridor, seed: Point, visit: impl FnMut(usize, &[Point]) -> ControlFlow<B>) -> Option<B>` | AC3, AC6 |
| `geodesic_layers` | `fn geodesic_layers(d: &Corridor, seed: Point) -> Vec<Vec<Point>>` | AC3 |
| `walls_from_boundary` | `fn walls_from_boundary(d: &Corridor) -> Vec<Wall>` | AC4 |

One private `flood_component(d, in_set, visited, seed, out) -> touches_boundary`
traversal core backs `flood_fill`, `component_count`, and
`bounded_complement_components` (DRY — a single 4-conn flood parameterized by a
membership predicate + a box-boundary flag).

### Key decisions (this design owns the spec's open questions)

**1 — Wall representation: anchor to the `D`-cell + a 4-way outward `Side`.**
Today's `Wall { cell, orient: Orient }` cannot name *which* of a cell's four
sides an edge is on: a `D` cell with `¬D` to both east and west would produce two
indistinguishable `Wall { cell, Vertical }` values. Chosen representation: anchor
each boundary edge to its unique **drivable** cell plus the `Side` toward the
non-drivable neighbour. Because a `D↔¬D` adjacency has exactly one `D` side, this
yields **each boundary edge exactly once** by construction, with no half-grid
coordinate bookkeeping. A new 4-way `Side { East, West, North, South }` enum
carries the direction (mirrors `Point::neighbors4` order: `E, W, N, S`).
- *Rejected:* repurpose `Orient` into a 4-way enum — `Orient{Horizontal,Vertical}`
  is still used by `StartFinish.orient` as *chord* orientation (track.rs); a
  4-way `Orient` would corrupt that meaning. `Orient` stays; only its doc drops
  the Wall-specific wording. `Side::orient()` is intentionally **not** added
  (renderer/block-2 concern; pre-publish freedom lets block 2 add it — YAGNI).
- *Rejected:* half-grid edge coordinate (`(2x+1, 2y)` style) — extra coordinate
  space with no consumer; the `D`-cell anchor already guarantees once-only edges.
- Pre-publish clean break (AGENTS.md § API Stability): only downstream is
  `track.rs::TrackArtifact.walls` (a `Vec<Wall>`, never constructed yet), so no
  call sites break; `track.rs`'s `use crate::geom::{..., Orient, ..., Wall}` is
  unchanged (`Orient` still used by `StartFinish`, `Wall` keeps its name).

**2 — Complement counting: bounded ⟺ does not touch the box boundary.**
Count 4-conn components of `¬D` over the box; a component is **unbounded**
(outfield) iff any of its cells lies on the box border (`dx∈{0,w-1}` or
`dy∈{0,h-1}`) — the standard "exterior is one connected region" trick — and
**bounded** (an infield hole) otherwise. `bounded_complement_components` returns
the bounded count; Ф4's "exactly one bounded hole of ≥1 cell" is `== 1` (every
component has ≥1 cell, so the "≥1" clause is automatically satisfied — the count
is the whole test). Works regardless of box margin (a ring flush to all four
edges still has its infield hole strictly interior → bounded). Cites design §2 Ф4.

**3 — Geodesic BFS confined to `D` == "never crossing a wall".** A wall is only
ever the boundary between a drivable and a non-drivable cell (design §1 duality);
the edge between two adjacent `D` cells is therefore never a wall, and "внутри
`D` стен нет" (no walls inside `D`, design §3). So a 4-conn BFS that never steps
to a `¬D` cell provably never crosses a wall — no separate wall test is needed.
Layers group cells by strictly increasing 4-conn geodesic distance; equal-distance
cells share a layer (the caller's seeded RNG — design §3, in `sim`, out of scope —
picks among ties). Deterministic intra-layer order (fixed `neighbors4` order,
fixed frontier order) satisfies AC5; the caller must not depend on that order for
correctness (it only picks among a tie set), but it is reproducible.

**4 — Geodesic shape: a reusable-scratch visitor + an eager convenience.**
`CorridorScratch::geodesic_bfs` is the primary primitive — it reuses buffers
across queries (AC6) and takes a `FnMut(distance, &[Point]) -> ControlFlow<B>`
visitor so the future `sim::resolve_collisions` can **stop at the first layer
containing a free cell** (nearest-free placement) instead of materializing the
whole component, and can radius-limit via `distance` (design §3 optional cap).
`geodesic_layers(d, seed) -> Vec<Vec<Point>>` is a thin eager wrapper (allocates
its own scratch, collects all layers) for tests and non-perf-critical callers —
the standard "control vs convenience" pair, not speculative surface.
- *Rejected:* eager-only `Vec<Vec<Point>>` — allocates fresh per query and can
  materialize the whole component under a dense pack, violating AC6's
  reuse-for-repeated-queries intent.
- *Rejected:* a borrowing `Iterator` over BFS — self-referential over the
  scratch, awkward/unidiomatic in Rust; the `ControlFlow` visitor gives the same
  early-stop with none of the lifetime pain.

**5 — Scratch reuse mechanism: generation-stamped visited (AC6).**
`CorridorScratch` owns a `Vec<u32>` stamp buffer sized to the box (allocated
once), a monotone `generation: u32`, and two `Vec<Point>` frontier buffers.
"visited" ⟺ `stamp[i] == generation`; each query bumps `generation`, so the
per-query reset is **O(1)** (no per-query `O(area)` clear). On the rare
`u32` wrap (`checked_add` returns `None`), fill the stamp with `0` and restart at
`1` — an `O(area)` clear amortized once per ~4·10⁹ queries. The one-shot topology
helpers (`flood_fill`, `component_count`, `bounded_complement_components`) each
scan the whole box exactly once per call, so they use a local `Vec<bool>` visited
(reuse buys nothing there) — this keeps the generation-stamp complexity confined
to the sole **repeated-query** path (collision resolution), which is exactly what
AC6 targets. `CorridorScratch::new(d)` binds the buffer to `d`'s box; `geodesic_bfs`
`debug_assert!`s the passed corridor's dimensions match (misuse guard;
`resolve_collisions` reuses one scratch against one corridor).
- *Alternative noted:* reset only the touched cells via a `Vec<bool>` +
  touched-list — equivalent `O(cells-visited)` reset, no wrap logic; the
  generation stamp is chosen as the more idiomatic single-buffer form.

**6 — File size.** Non-test `geom.rs` is ~189 lines; the additions land it near
the 200–400 target ceiling (AGENTS.md § File size). Handled concretely, not
preemptively: task 6 measures the final non-test line count and **iff it exceeds
400**, `git mv geom.rs geom/mod.rs` and move the four helpers + `Side` +
`CorridorScratch` (and their tests) into `geom/graph.rs`, re-exported via
`pub use graph::*;` — every `crate::geom::*` path stays valid, no consumer edits.
Below 400 → keep the single file (YAGNI).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `Side` enum (`ALL`, `delta`); change `Wall` to `{ cell, side: Side }`; update the file-top duality doc + `Orient` doc to drop Wall-specific wording (`Orient` remains for `StartFinish`). `cargo build` + doc gate confirm `track.rs` still compiles unchanged. | `crates/core/src/geom.rs` | — |
| 2 | Private `flood_component` traversal core (predicate + boundary flag) + `flood_fill` + `component_count`; unit tests (AC1). | `crates/core/src/geom.rs` | — |
| 3 | `bounded_complement_components` (reuses `flood_component` with the `¬D` predicate, counts non-boundary-touching components); unit tests (AC2). | `crates/core/src/geom.rs` | 2 |
| 4 | `CorridorScratch` (generation-stamped visited + frontier buffers) + `geodesic_bfs` visitor + eager `geodesic_layers`; unit tests (AC3, AC5, AC6). | `crates/core/src/geom.rs` | — |
| 5 | `walls_from_boundary` (anchor each `D↔¬D` edge to its `D`-cell + `Side`); unit tests (AC4). | `crates/core/src/geom.rs` | 1 |
| 6 | Consolidation gate: run `cargo test` / `clippy --workspace --all-targets -D warnings` / `RUSTDOCFLAGS="-D warnings" cargo doc`; determinism cross-check (AC5); measure non-test line count and split into `geom/graph.rs` **iff** > 400 (Key decision 6); confirm `lib.rs` re-exports intact. | `crates/core/src/geom.rs` (→ `geom/{mod,graph}.rs` iff split), `crates/core/src/lib.rs` | 1,2,3,4,5 |

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping: grouping is required
for **every M ≥ 1** (this design is `M = 6`); non-terminal groups are **exactly 3
consecutive** subtasks; the terminal group is sized within **1..=3**; each group
boundary — including entry into the first group — hands off to `/context-reset`
per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).

- **Handoff into Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) before starting subtask 1.
- **Group A:** subtasks 1–3 — `Side`/`Wall` representation, flood core + `flood_fill`/`component_count`, `bounded_complement_components` (non-terminal group; exactly 3).
- **Handoff after Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Parent `/task` resumes in Group B with fresh context.
- **Group B:** subtasks 4–6 — geodesic BFS + scratch, `walls_from_boundary`, consolidation gate (terminal group; 3 subtasks, within the 1..=3 range).

## Risks

- **`Wall` field change is a breaking API edit.** Mitigation: pre-publish clean
  break (AGENTS.md § API Stability); the only reference is `track.rs`'s
  `Vec<Wall>` field + its import, neither of which constructs a `Wall` or names
  `orient`, so nothing beyond the type def changes. `cargo build` + doc gate in
  task 1 confirm.
- **File size crosses the 400-line target after the additions.** Mitigation:
  task-6 measured split into `geom/graph.rs` with a `pub use` re-export (Key
  decision 6); public paths preserved, no consumer edits.
- **Clippy pedantic + nursery are `deny` (workspace lints).** New `i32`↔`usize`
  index/stamp math will trip `cast_sign_loss` / `cast_possible_truncation`.
  Mitigation: funnel index math through `Corridor::index`/local helpers and reuse
  the existing justified `#[allow(clippy::cast_sign_loss)]` comment pattern
  (already in `Corridor::new`/`index`); no *blanket* allows. Any public fn that
  can `panic!`/`assert!` gets a `# Panics` doc (`missing_panics_doc`); the
  `CorridorScratch` box-match guard is a `debug_assert!` (no panic in release, no
  doc obligation).
- **Generation-stamp wraparound** could mistake stale `0` stamps for visited.
  Mitigation: `checked_add`-with-fill fallback (Key decision 5); covered by a
  reasoning note, not a 4-billion-iteration test.
- **Complement "unbounded ⟺ touches box boundary" assumes a connected exterior.**
  Valid by construction (the box is the analysis frame; anything off-box is the
  single outfield). Documented on `bounded_complement_components`; the two-hole
  and flush-to-edge test cases exercise it.
- **`CorridorScratch` reused against a differently-sized corridor** would index a
  wrong-sized buffer. Mitigation: `debug_assert!` dimensions match in
  `geodesic_bfs`; documented precondition ("bind the scratch to the corridor it
  queries"). `resolve_collisions` uses one scratch per corridor.

## Test Design

All in-file `#[cfg(test)] mod tests` in `geom.rs` (moving with the code into
`geom/graph.rs` iff task 6 splits), exact integer assertions on hand-built
corridors; set comparisons via `HashSet` where order is an implementation detail
(matching the existing `cover_set` test style). Helper: a small builder that sets
a list of `(x, y)` cells drivable on a `Corridor::new(origin, w, h)`.

- **`flood_fill` / `component_count` (AC1)** — `crates/core/src/geom.rs` tests.
  - Entry points: `flood_fill`, `component_count`.
  - Scenarios: single solid block → `component_count == 1`, `flood_fill` returns
    exactly that block's cells (as a set); two disjoint blocks → count `2`,
    `flood_fill` from a seed in one returns only that block; seed ∉ `D` → empty
    `Vec`; empty corridor → count `0`.
  - Fixtures: cell-list builder.
- **`bounded_complement_components` (AC2)** — `geom.rs` tests.
  - Entry point: `bounded_complement_components`.
  - Scenarios (from AC2): solid filled rectangle → `0`; rectangular annulus
    (single interior hole) → `1`; two-hole shape (two separate interior holes) →
    `2`; empty corridor → `0` (whole box is the unbounded outfield); a hole flush
    consideration — ring flush to all box edges still yields `1` (interior hole
    bounded).
  - Fixtures: annulus builder (outer rectangle minus inner block); two-hole
    builder.
- **`geodesic_layers` / `geodesic_bfs` (AC3, AC5, AC6)** — `geom.rs` tests.
  - Entry points: `geodesic_layers` (eager) and `CorridorScratch::geodesic_bfs`
    (visitor).
  - Scenarios: straight 1-wide corridor → layers are the exact distance bands
    from the seed; **rectangular annulus with an equal-distance tie** — seed at
    one side's midpoint, assert exact per-layer sets and that the opposite
    midpoint's two equidistant approaches land in the **same** layer (≥2 cells in
    that layer); seed ∉ `D` → empty layers; `geodesic_bfs` `ControlFlow::Break`
    at a chosen layer returns `Some(payload)` and stops (early-stop path);
    reusing one `CorridorScratch` across two successive `geodesic_bfs` calls
    yields identical layers (AC6 reuse-correctness — a second query is not
    polluted by the first's stamps).
  - Fixtures: annulus builder; a collecting visitor closure.
- **`walls_from_boundary` (AC4)** — `geom.rs` tests.
  - Entry point: `walls_from_boundary`.
  - Scenarios: solid 2×2 block → the exact set of 8 outward `Wall{cell, side}`
    (the square's perimeter, each cell's two outward sides); rectangular ring →
    the exact set of outer **and** inner boundary edges (each `D↔¬D` pair once,
    none between two `D` cells); assert as a `HashSet<Wall>` and additionally
    that `walls.len()` equals the deduped length (each edge exactly once, mirrors
    the existing `no_duplicate_cells` supercover test).
  - Fixtures: block + ring builders.
- **Determinism (AC5)** — a compact test that calls each of `flood_fill`,
  `component_count`, `bounded_complement_components`, `geodesic_layers`,
  `walls_from_boundary` twice on the same fixture and asserts byte-identical
  (`Vec`-equal, order included) output.

## Open questions

None blocking. Defensible defaults are recorded above for every spec open
question (helper API shapes, single-source geodesic BFS — multi-source is a
superset not needed by the collision-resolution consumer and is YAGNI here, the
scratch-reuse mechanism, and the wall representation). The §2 Ф4 cross-section
**width** distance-transform remains a recommended follow-up gp-core issue
(spec Deferred; note for `/triage`) — out of this task's ACs.
