# Design: gp-gen Ф7 s-field — fold-free BFS distance on the annulus cut at the gate

**Issue:** #32
**Date:** 2026-07-24

## Approach

### What the deliverable is

Populate `SField.dist` (`crates/core/src/track.rs`) with the monotone integer
progress coordinate `s`: the 4-connected BFS graph distance on `D \ gate` — the
corridor with the timing-gate dual edges removed — seeded (distance `0`) from the
gate's forward (`+race_dir`) face, with `TimingGate::separates` acting as an
impassable barrier in both directions. The `SField` container and its accessors
(`scalar_at` / `gradient_at` / `tangent_at`), plus the `separates` barrier
predicate, already exist and are tested; today production has no producer —
`SField::new` fills `dist` from a caller-supplied closure used only by tests
`[measured: rg -n "SField::new" crates/core/src/track.rs → lines 426,490,510,633
all under #[cfg(test)]/track_artifact test]`. This task adds the real BFS
producer.

### Chosen placement — split across the two natural owners (gp-core only)

The producer decomposes into a **generic geom-graph primitive** and a **thin
SField wrapper**, both in **gp-core** — gp-gen is **not** touched (its
`generate()` is a `todo!()` stub and wiring is explicitly out of scope):

1. **`barrier_distance_field` — a new multi-seed, barrier-aware 4-conn BFS in
   `crates/core/src/geom/graph.rs`**, alongside the single-seed, barrier-free
   `geodesic_bfs` / `geodesic_layers`. Signature:

   ```rust
   pub fn barrier_distance_field(
       d: &Corridor,
       seeds: impl IntoIterator<Item = Point>,
       barrier: impl Fn(Point, Point) -> bool,
   ) -> Vec<Option<u32>>
   ```

   Returns a row-major `Vec<Option<u32>>` over `d`'s bounding box
   (`len == d.area()`, same layout as `Corridor`'s cells and `SField.dist`):
   `Some(distance)` for reachable in-`D` cells, `None` for `¬D` and for in-`D`
   cells the cut leaves unreachable. It takes a **generic** `barrier(a, b) ->
   bool` predicate, **not** a `TimingGate` — so `geom` acquires no dependency on
   the higher-level `track` module (no layering inversion). This is the piece
   that genuinely must live in `geom`: it needs `Corridor`'s **private**
   `index` / `area` helpers to size and address the dense buffer, exactly as the
   existing `flood_fill`/`flood_component` do `[measured: rg -n
   "d.area\(\)|d.index" crates/core/src/geom/graph.rs → flood_component calls
   d.index at 60/75 (not d.area()); flood_fill sizes vec![false; d.area()] at
   107; component_count's d.area() is at 123]`, and `graph` is a child module of
   `geom` so it has that access.

2. **`SField::from_gate_bfs(d: &Corridor, gate: &TimingGate) -> SField` in
   `track.rs`** — the domain wrapper that knows about `TimingGate` and `SField`
   (both live in `track.rs`). It computes the seed set from `gate.forward_face()`
   and passes `|a, b| gate.separates(a, b)` as the barrier:

   ```rust
   let dist = barrier_distance_field(d, gate.forward_face(), |a, b| gate.separates(a, b));
   Self { rect: d.rect(), dist }
   ```

3. Two small supporting members feed the wrapper:
   - **`TimingGate::forward_face(&self) -> impl Iterator<Item = Point>`** (track.rs)
     — the geometric seed set `{ behind[i] + forward.delta() }`, overflow-filtered
     with `checked_add` exactly as the existing `separates`/`ahead_of` does
     `[measured: sed track.rs:52-54 → ahead_of uses checked_add(dx)?/checked_add(dy)?]`.
     Named + `pub` so AC2 ("forward-face cells read `s = 0`") can assert against
     the seed set precisely.
   - **`Corridor::rect(&self) -> Rect`** (geom/mod.rs) — a `const` accessor
     returning the bounding box, so `from_gate_bfs` (in `track.rs`, which cannot
     see `Corridor`'s private `rect` field) can set `SField.rect`. This makes the
     already-documented "`SField.rect` mirrors `Corridor`'s bounding-box storage"
     relationship `[measured: sed track.rs:119-120 → doc "Dense grid mirroring
     Corridor's bounding-box storage"]` explicit instead of hand-rebuilt from
     `origin()/width()/height()`, and is reused by every future Ф7-export /
     render caller that needs the box.

### Why this split over the alternatives

- **Rejected: a gp-gen Ф7 function owning the BFS.** gp-gen can only reach
  `Corridor` through its **public** API (`contains`, `origin/width/height`) — the
  `index`/`area`/`box_points` helpers are private `[measured: sed geom/mod.rs:
  320-337 → index/area/box_points/on_border all lack pub]`. It would therefore
  re-derive the row-major index it cannot borrow and duplicate the box-iteration
  geom already owns, while the deliverable's home is named as `track.rs`
  (gp-core). Placing the BFS in gp-gen splits the producer away from `SField`'s
  crate for no benefit.

- **Rejected: extending `CorridorScratch::geodesic_bfs` in place.** It is
  single-seed, barrier-free, and layer-visitor-shaped; adding multi-seed +
  barrier + a full-field return would change its signature and force its one
  caller (`geodesic_layers`) to adapt, for a one-shot field build that needs no
  reusable scratch. A sibling function is cleaner and mirrors the existing
  `geodesic_layers` "allocates its own scratch" convenience form
  `[measured: sed graph.rs:285-299 → geodesic_layers allocates a fresh
  CorridorScratch]`.

- **Rejected: `SField::from_gate_bfs` taking a `TimingGate` inside `geom`.**
  That would make `geom` depend on `track`, inverting the layering (`track`
  depends on `geom`, never the reverse `[measured: sed track.rs:5 → use crate::
  geom::{...}]`). Keeping the barrier generic in `geom` and doing the
  `gate.separates` wiring in `track.rs` preserves the direction.

### Algorithm (barrier_distance_field)

Layer-by-layer double-buffered BFS, mirroring `geodesic_bfs`'s frontier style so
the traversal and its determinism match the established primitive:

1. `let mut dist = vec![None; d.area()];` and two `Vec<Point>` frontiers.
2. Seed layer 0: for each `seed`, if `let Some(i) = d.index(seed)` **and**
   `d.contains(seed)` **and** `dist[i].is_none()` → set `dist[i] = Some(0)`,
   push to `frontier`.
3. `let mut distance = 0u32;` while `frontier` non-empty: build `next` by, for
   each frontier cell `p` and each `n in p.neighbors4()`, stepping to `n` iff
   `d.index(n)` is `Some(i)` **and** `dist[i].is_none()` **and** `d.contains(n)`
   **and** `!barrier(p, n)`; mark `dist[i] = Some(distance.saturating_add(1))`,
   push `n`. Swap frontiers, `distance = distance.saturating_add(1)`.
4. Return `dist`.

`barrier_distance_field`'s **doc comment MUST state the `dist.len() == d.area()`
postcondition explicitly** (one entry per bounding-box cell, row-major). This is
load-bearing: `SField::from_gate_bfs` pairs that `dist` with `d.rect()` (whose
`area()` equals `d.area()`), so the postcondition is exactly what upholds
`SField`'s `dist.len() == rect.area()` invariant that `scalar_at` / `gradient_at`
rely on `[measured: sed track.rs:143-150 → scalar_at doc "even if dist is shorter
than rect.area()" + the invariant note]`.

`distance.saturating_add(1)` is a **total** form — no `#[allow(clippy::
arithmetic_side_effects)]` needed, upholding gp-core's zero-panic posture — and
in the grid-realistic domain never saturates (a `u32` distance is reached only
past ~4·10⁹ cells in a line) `[derived → cargo clippy --workspace --all-targets
-- -D warnings in AC7]`. This is the one deliberate divergence from
`geodesic_bfs`, which stores distance as `usize` under a bounded
`#[allow]` `[measured: sed graph.rs:235-240,278-279 → usize distance +
arithmetic_side_effects allow]`; `u32` (to land directly in `SField.dist:
Vec<Option<u32>>`) + saturating is the cleaner total form here.

Membership/barrier are consulted on **every** 4-neighbor step, so — because
`separates` is symmetric `[measured: rg -n "separates_is_symmetric" track.rs →
existing test at 387]` — the cut blocks traversal from either endpoint (AC1's
both-direction requirement). Direct `dist[i]` indexing is provably in-bounds
(`i = d.index(p) < d.area() == dist.len()`), the identical invariant
`flood_component` relies on for `visited[i]`; not a new panic-index entry.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `pub const fn rect(&self) -> Rect` accessor to `Corridor` (returns the bounding box; `const` is forced by `missing_const_for_fn` on this field-access body). Unit test. | `crates/core/src/geom/mod.rs` | — |
| 2 | Add `barrier_distance_field(d, seeds, barrier) -> Vec<Option<u32>>`: multi-seed, barrier-aware 4-conn BFS over `D`, row-major over `d`'s box, `None` off-band/unreachable. `#[cfg(test)] mod` additions. | `crates/core/src/geom/graph.rs` | — |
| 3 | Add `TimingGate::forward_face(&self) -> impl Iterator<Item = Point>` — `{ behind[i] + forward.delta() }`, `checked_add`-filtered. Unit tests. | `crates/core/src/track.rs` | — |
| 4 | Add `SField::from_gate_bfs(d, gate) -> SField` wiring `forward_face` seeds + `separates` barrier + `d.rect()` into `barrier_distance_field`. AC-level tests (annulus, ring, hairpin, determinism). | `crates/core/src/track.rs` | 1, 2, 3 |

Scope: 4 tasks (≤ 15). No split needed.

## Handoff plan

Grouping per `.claude/agents/design.md` § Rules (handoff-grouping) (a)–(h). All
four subtasks change **code** only (Rust `*.rs` under `crates/core/src/`), so the
change-type is homogeneous → they cluster into the **fewest possible groups**:
one. `M = 4`.

- **Group A** — model **`sonnet`** (sonnet-5), effort **`medium` (pinned)**, via
  the `code-writer` subagent, 1M-token window — subtasks **1, 2, 3, 4** (code
  change-type: `*.rs`). Ordering respects dependencies: 1, 2, 3 are independent
  and precede 4, which depends on all three. Terminal group (4 subtasks; within
  the `1..=10` range and ≤ the size cap of 10). Routes to
  `subagent_type="code-writer"` whose `model: sonnet` + `effort: medium` are
  frontmatter-pinned — no inline `model=`/effort override.
- **Handoff into Group A:** at the start of Group A, spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Being
  the sole (and terminal) group, Group A both begins under a fresh
  `/context-reset` subagent and completes Step 8 there; there is no inter-group
  handoff. Group count = 1 (≤ the default max of 4; no user gate needed).

## Risks

- **Layering inversion (geom → track):** avoided by the generic `barrier: impl
  Fn(Point, Point) -> bool` — `geom` never names `TimingGate`; the
  `gate.separates` wiring stays in `track.rs` — `[derived → cargo build in AC7:
  a `use crate::track` in geom/graph.rs would be visible in the diff/build]`.
- **One-directional barrier leaks the antipode fold back in** (spec Technical
  constraints): mitigated by consulting `barrier(p, n)` on every neighbor step
  and by `separates`'s existing symmetry — `[measured: rg -n
  "separates_is_symmetric" crates/core/src/track.rs → test present at line 387]`;
  the AC3 ring test asserts the no-fold property end-to-end.
- **`u32` distance overflow / panic:** `saturating_add(1)` is total (defined at
  the ceiling, unreachable in-domain) — no `#[allow]`, no panic-index entry —
  `[derived → cargo clippy --workspace --all-targets -- -D warnings + the empty
  ai-docs/panic-index.md invariant in AC7]`.
- **Out-of-`D` / unreachable cells:** BFS pre-fills `None` and only overwrites
  reached in-`D` cells, so `¬D` and cut-isolated cells stay `None` with no
  special-casing — `[derived → the annulus + hairpin unit tests in Test Design]`.
- **Miri:** pure-integer, std-only, no FFI/GPU → expected Miri-clean, no
  `#[cfg_attr(miri, ignore)]` gating — `[derived → MIRIFLAGS=-Zmiri-tree-borrows
  cargo miri test -p gp-core (gp-core is on the Miri gate; run per AGENTS.md
  § Rust Test Conventions)]`.
- **BFS-loop count in `geom` (informational, no consolidation now):**
  `barrier_distance_field` is the **third** BFS-shaped loop in `geom`, alongside
  `CorridorScratch::geodesic_bfs` and `DistanceTransform`'s inline BFS
  `[measured: sed crates/core/src/geom/distance.rs:41-72 → seed pass + VecDeque
  relaxation loop]`. It does **not** meet the ≥3-site *mechanical-copy* bar: the
  three have genuinely distinct semantics — visitor + reusable generation-stamped
  scratch (`geodesic_bfs`); `u32`-with-`0`-sentinel wall distance seeded from the
  `¬D` boundary (`DistanceTransform`); multi-seed `Vec<Option<u32>>` + generic
  barrier (this one). No consolidation is warranted; a future **4th** BFS variant
  should trigger a consolidation look.
- **`Corridor::rect()` single caller (YAGNI):** justified by the documented
  `SField.rect`-mirrors-`Corridor`-box relationship and reuse by future
  Ф7-export/render box consumers; kept as its own reviewable subtask —
  `[measured: sed track.rs:119-121 → "Dense grid mirroring Corridor's
  bounding-box storage"]`.

## Test Design

### Subtask 1 — `Corridor::rect()`
- Location: `crates/core/src/geom/mod.rs` `#[cfg(test)] mod tests`.
- Entry point: `Corridor::rect`.
- Scenarios: an off-origin `Corridor::new((2,3), 4, 5)` returns
  `Rect { origin: (2,3), size: {4,5} }`; the returned `rect.index`/`points`
  agree with the corridor's own membership box.
- Fixtures: reuse the existing `corridor(...)` helper style; no new fixture.

### Subtask 2 — `barrier_distance_field`
- Location: `crates/core/src/geom/graph.rs` `#[cfg(test)] mod tests` (reuse the
  existing `corridor` / `ring_3x3` / `rect` helpers).
- Entry point: `barrier_distance_field`.
- Scenarios:
  - **Single seed, no barrier, straight 1-wide corridor** → `dist` reads
    `Some(0), Some(1), …` along the run, `None` off-band; `barrier = |_,_| false`.
  - **Multi-seed** (two seeds) → each cell carries the distance to its *nearest*
    seed (min), verified on a straight corridor seeded at both ends.
  - **Barrier blocks a step** → a cut edge on a straight corridor forces the
    far cell to `None` (no other path) or the long-way distance (ring).
  - **Ring annulus, one cut edge** (the AC6 fixture below) → distances grow the
    long way; the antipode reads the maximum; the cell across the cut from the
    seed reads `L`, not `1`.
  - **Unreachable in-`D` cell** → stays `None`.
  - **Seed ∉ `D`** → contributes nothing (skipped).
  - **Saturated-self-neighbor edge case is harmless** → a frontier cell on the
    grid boundary whose `neighbors4()` saturates to itself (a cell at
    `i32::MAX`/`i32::MIN`, per `Point::neighbors4`'s saturating add/sub) does not
    self-loop: the saturated self is caught by the `dist[i].is_none()` guard (its
    own cell is already `Some`) and skipped. Mirrors the existing
    `walls_*_i32_min/max` edge tests, pinning the `neighbors4`-saturation
    interaction `[measured: rg -n "saturates_at_i32" crates/core/src/geom/mod.rs
    → neighbors4 saturation tests present]`.
  - **Determinism** → two calls on identical input return identical `Vec`.
- Fixtures: the `ring_3x3()` 8-cell annulus already in `graph.rs` tests.

### Subtask 3 — `TimingGate::forward_face`
- Location: `crates/core/src/track.rs` `#[cfg(test)] mod tests`.
- Entry point: `TimingGate::forward_face`.
- Scenarios: `behind = [(1,1)], forward = East` → `{(2,1)}`; each `Side` shifts
  by its `delta()`; `behind = []` → empty; a `behind` cell at `i32::MAX` with
  `forward = East` → the overflowing seed is filtered out (no panic), mirroring
  `separates`'s `checked_add`.
- Fixtures: inline `TimingGate` literals (as the existing `separates` tests do).

### Subtask 4 — `SField::from_gate_bfs` (AC1–AC6)
- Location: `crates/core/src/track.rs` `#[cfg(test)] mod tests`.
- Entry point: `SField::from_gate_bfs`; read back via `scalar_at`.
- **AC6 exact fixture (hand-computed).** The 8-cell 3×3 ring
  `{(1,1),(2,1),(3,1),(1,2),(3,2),(1,3),(2,3),(3,3)}` (center `(2,2)` excluded),
  `gate = { behind: [(1,1)], forward: East }` → forward face `{(2,1)}`, cut edge
  `(1,1)–(2,1)`. Expected distances the long way around the 8-cycle:
  `(2,1)=0, (3,1)=1, (3,2)=2, (3,3)=3, (2,3)=4, (1,3)=5, (1,2)=6, (1,1)=7`
  (so `L = 7`, at the `behind` cell). Assert every cell cell-by-cell; assert the
  cell across the cut `(1,1)` reads `7`, **not** `1` (proves the barrier).
- **AC1/AC2:** on that fixture: forward-face `(2,1) = Some(0)`; `behind`
  cross-section `(1,1) = Some(7) = max`; every in-`D` cell `Some`, `(2,2)` (¬D)
  `None`; `dist.len() == rect.area()`.
- **AC3 (no antipode fold):** walking the ring in `race_dir` order from the seed
  — `(2,1)→(3,1)→(3,2)→(3,3)→(2,3)→(1,3)→(1,2)→(1,1)` — every forward unit step
  has `Δs = +1 ≥ 0`; the only decrease is the closing `(1,1)→(2,1)` gate step
  (`7 → 0`). Assert `Δs ≥ 0` for all non-gate forward steps.
- **AC4 (single discontinuity):** assert the only `|Δs| > 1` / decrease around
  the loop is the `7 → 0` gate step; every other adjacent forward pair differs by
  exactly `1`.
- **AC5 (single-valued, no projection fold):** a wide-pocket / hairpin (U-shaped)
  corridor fixture where a nearest-point-on-centerline definition would fold two
  arms onto one `s`; assert each cell holds exactly one `Some(u32)` and that `s`
  increases monotonically along the true corridor path through the pocket
  (structural: `Vec<Option<u32>>` is single-valued by construction; the fixture
  demonstrates a projection-based definition *would* fold where BFS does not).
- **AC6 determinism:** `from_gate_bfs(&d, &gate) == from_gate_bfs(&d, &gate)`
  (compare `dist`).
- Fixtures: the ring built via the existing `corridor(...)`-style helper; a new
  small U/hairpin corridor literal for AC5.

All new logic carries a `#[cfg(test)] mod tests`; AC7's `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` are run before
commit. The pure-integer BFS is expected Miri-clean (no gating), verified with
the workspace Miri command per AGENTS.md § Rust Test Conventions.

## Open questions

- None. The producer's home crate + signatures (the sole open Key-Decisions row)
  are resolved above: a generic `barrier_distance_field` in gp-core
  `geom/graph.rs` + an `SField::from_gate_bfs` wrapper in `track.rs`, with
  supporting `TimingGate::forward_face` and `Corridor::rect`.
