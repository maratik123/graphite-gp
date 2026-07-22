# gp-gen Ф1 — coarse-block ring (infield-first) + a grouped seeded-RNG config

**Source:** issues #24, #49
**Date:** 2026-07-22
**Tracked in:** #24, #49 — **Closes #24, Closes #49**
**Discharges:** #50 (closed-as-superseded; its deferred *per-site* deterministic-
collection enforcement lands in this PR — **not** "Closes #50"). Governing policy:
`ai-docs/code-style.md` § Deterministic collections (fuller than #50's two-option
framing — it adds EnumMap/BitFlags and the fixed-`BuildHasher` footgun).

## Scope

Two coupled deliverables in one PR:

### A. Grouped seeded-RNG configuration (closes #49)

1. **Introduce one struct that groups four independently-seeded RNG sources**,
   each with its own seed, so the UI can configure all four seeds in one place:
   (1) car collision, (2) track generation, (3) AI learning, (4) AI inference.
   All sources are `rand_chacha::ChaCha8Rng` (matching the existing collision and
   generation code). `rand` + `rand_chacha` are **already** workspace deps and are
   already declared in `crates/core/Cargo.toml` and `crates/gen/Cargo.toml`
   (verified) — no new dependency is added.
2. **Wire two real consumers this PR:**
   - **Track generation → Ф1** (this PR's new consumer). Reconcile with the
     existing `GenParams { seed, .. }` / `GenParams::rng()` so generation has a
     single, non-divergent RNG path.
   - **Car collision → `gp-core` `sim::collision::resolve_collisions`, re-pointed
     this PR** to draw its RNG from the struct's collision source. (`resolve_collisions`
     has no production caller yet — only its own unit tests invoke it — so
     "re-point" means routing the collision seed through the struct and updating
     those determinism tests to seed via it; the `&mut ChaCha8Rng` signature need
     not change since the struct materializes a `ChaCha8Rng` from the collision seed.)
3. **Define, but do not consume, the AI-learning and AI-inference sources.**
   `gp-ai` (`crates/ai`) is a `todo!` stub; those two fields are defined-but-
   unconsumed this PR.

### B. Generation phase Ф1 — coarse skeleton (closes #24)

Implement `phase1_coarse_ring` (design doc §2) in `gp-gen`, at **coarse-block**
granularity (each block → `k×k` fine points in Ф2 — out of scope here). Produce
the skeleton `{ ring, P, dir }`:

4. **Draw a seeded, simply-connected polyomino `P`** (the infield / hole) from the
   grouped struct's track-generation source. Same seed ⇒ same `P`.
5. **Smooth `P`'s border** — remove 1-cell teeth everywhere, and guarantee at
   least one straight run long enough to host start/finish (see Q1 / AC3).
6. **Build the ring by construction:** `ring = minkowski_dilate(P, ≥1) \ P` — an
   annulus that is connected and has exactly one hole (`P`, ≥1 cell) by
   construction.
7. **Widen selected ring sides** (outward), preserving the annulus invariants.
8. **Fix traversal orientation** `dir ∈ {CW, CCW}` = the global `race_dir`
   (`gp_core::track::RaceDir`); store and return it.
9. Return `{ ring, P, dir }` as the Ф1 output type.
10. **Deterministic collections + their dependency consequence (discharges #50).**
    Every set/map whose iteration reaches Ф1 output follows
    `ai-docs/code-style.md` § Deterministic collections (see the Key Decision).
    Apply the chosen container's consequence **in this PR**: if a `BTreeSet`/
    `BTreeMap` keyed on `gp_core::geom::Point` is used, add the `PartialOrd, Ord`
    derive to `Point` (it currently derives `Eq + Hash` but **not** `Ord` —
    verified); if an `IndexSet`/`IndexMap` is used, add `indexmap` as a **new
    direct** workspace dependency (it is only transitive today — verified),
    pinned per AGENTS.md § Dependency Versions (`cargo update` + `cargo build`
    after). A membership-only set never iterated into output needs no ordered
    container and no such change.

## Out of scope

- Ф2 rasterization to the fine corridor `D`; Ф3 start/finish + gate + grid (#26);
  Ф4 static validation (#27); Ф5 oracle; Ф6 local repair; Ф7 export.
- The `generate_track` outer seed-budget / reseed loop (#34). Downstream phases
  (Ф3 #26 / Ф4 #27) enforce full run-out / accel-zone length and reseed via that
  loop; Ф1 only seeds **one** long-enough straight as the S/F candidate.
- **Consuming** the AI-learning / AI-inference RNG sources — `gp-ai` is a stub;
  they are defined-only this PR.
- Producing a `TrackArtifact` (that is Ф7).

## Deferred

- Full fine-grid run-out enforcement (`accel_zone ≥ ~V_target²/2` points) | fine-
  point Ф3 concern | tracked by #26 — no new issue.
- Consuming the AI RNG sources in `gp-ai` | crate is a stub | happens with the
  block-4 work — no new issue.
- Polyomino shape-distribution tuning | Ф1 needs only a valid deterministic draw |
  revisit if tracks are dull — no issue yet.

## Key decisions

| Question | Decision |
|---|---|
| **Q1 — `L_min` straight guarantee (resolved r1)** | `L_min` is in **coarse-block** units at Ф1. Ф1 de-teeths **everywhere** (every maximal straight run ≥ 2 blocks) **and** guarantees **≥1** straight run ≥ `L_min` blocks (the S/F candidate). Full run-out length is enforced downstream (Ф3 #26 / Ф4 #27) with reseed via `generate()` (#34). Do **not** over-constrain Ф1 to "every straight ≥ `L_min`". |
| Grouped-RNG struct location | **`gp-core`** — both wired consumers need it: the collision consumer lives in `gp-core`, and `gp-gen` (the generation consumer) already depends on `gp-core`, plus the two AI sources are reachable since `gp-ai` also depends on `gp-core`. gp-core is the only crate all consumers already share, so it is the home. Exact module/path within `gp-core` → design. |
| Struct shape | Default: hold **four `u64` seeds**, materialize a `ChaCha8Rng` on demand per source (mirrors the existing `GenParams::rng()` pattern) for clean UI seed config; alternative is four live `ChaCha8Rng` values. → design. |
| Struct / field naming | → design (e.g. `Seeds` / `RngConfig` with `collision` / `generation` / `ai_learning` / `ai_inference`). |
| `GenParams` reconciliation | The generation seed is sourced from the grouped struct's generation field; whether `GenParams` embeds the struct or the struct feeds `GenParams::rng()` → design. No duplicate divergent generation RNG path may remain. |
| Skeleton output type & location | New type in `gp-gen` (e.g. `CoarseSkeleton { ring, hole /*P*/, dir }`); `dir` is `gp_core::track::RaceDir`. Exact shape → design. |
| Coarse-cell set representation | Design chooses the container (reuse `gp_core::geom::Corridor` at coarse granularity, or a dedicated cell-set), but **must follow `ai-docs/code-style.md` § Deterministic collections** for any set/map whose iteration reaches output: `BTreeSet`/`BTreeMap` by default; `IndexSet`/`IndexMap` **only** where insertion-order iteration genuinely drives output; **never** a `std` `HashSet`/`HashMap` iterated into output (a fixed/seeded `BuildHasher` is **not** an escape hatch — it pins hash values, not slot layout); and **prefer `enum_map::EnumMap` / `enumflags2::BitFlags`** when a key/element is a closed enum (e.g. a direction/side). Membership-only sets (never iterated into output) are exempt. This discharges #50's deferred per-site policy. |
| Polyomino draw scale (P size / bounds) | Design picks a bounded, deterministic draw; no AC constrains size; the snapshot test captures whatever a fixed seed yields. |
| `widen_selected_sides` / `choose_orientation` | Selection, amount, and the CW/CCW rule → design; both must be deterministic under a fixed seed and preserve the annulus invariants. |
| Ф1 failure handling | Whether Ф1 retries the draw internally (bounded attempts on the same stream) or is fallible → design; the by-construction ACs must hold for whatever it returns. |
| **Q2 — collision-RNG rewire scope (resolved r2)** | **"Also collision"** — re-point `gp-core`'s existing collision RNG through the grouped struct **this PR**. The product owner accepts the larger diff into `gp-core` sim + its (test-only, for now) call sites for one unified RNG config now. |

## Technical constraints

- **Integer-only & deterministic** (design doc §3a): the skeleton (`P`, `ring`,
  block coordinates) is pure integer geometry, matching the deterministic
  integer physics of the engine. The Ф1 draw uses no real-number arithmetic.
- **Replay determinism** is a hard contract for **both** wired consumers: the
  skeleton is a pure function of the generation seed, and collision resolution is
  a pure function of the collision seed. **No `thread_rng`, no OS entropy on any
  path fed by the grouped struct** (generation or collision). No `HashMap`
  iteration order leaking into the draw.
- **Cross-platform / cross-toolchain bit-for-bit reproducibility** of the
  generation path (docs/design.md §2 [N4], §5 [M3]): a replay stores only the
  seed and *regenerates* the same skeleton, so identity must hold not just for
  the same binary but **across platforms and toolchain/`hashbrown` versions**.
  This is exactly why iteration-order-into-output must use a deterministic
  collection per `ai-docs/code-style.md` § Deterministic collections (scope
  bullet 10) — the integer generation path is bit-deterministic by construction.
- **Depends on #5 (CLOSED).** `gp-core` connectivity helpers are available for
  by-construction assertions and tests: `component_count`,
  `bounded_complement_components`, `flood_fill` (`crates/core/src/geom/graph.rs`).
- AGENTS.md conventions: strict clippy (`-D warnings`), `///` on every public
  item, magic numbers → module `const`, `#[cfg(test)] mod tests` for ~50+-line
  logic, `thiserror` for any new error enum.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Ф1 returns a coarse skeleton `{ ring, P (hole), dir }`; `dir` is a `RaceDir`. |
| AC2 | For a fixed seed, `ring` is **connected** (one 4-conn component), has **exactly one hole**, and the hole `P` has **≥1 cell** — asserted via the #5 helpers. `ring = dilate(P) \ P` holds by construction. |
| AC3 | Border smoothing guarantees **(a)** no 1-cell teeth — every maximal straight border run is **≥ 2 coarse blocks**, **and (b)** at least **one** straight run **≥ `L_min` coarse blocks** exists (the S/F candidate). Tests assert both; Ф1 is **not** constrained to "every straight ≥ `L_min`". |
| AC4 | `race_dir` (`CW`/`CCW`) is fixed, returned in the skeleton, and stable across repeated same-seed calls. |
| AC5 | **Replay:** two Ф1 runs with the same generation seed produce identical skeletons (same `P`, `ring`, `dir`); different seeds differ in at least one field. Identity must be **bit-for-bit and independent of platform / toolchain** (design doc §2 [N4], §5 [M3]) — no `HashSet`/`HashMap` iteration order reaches the skeleton. |
| AC6 | A **snapshot test** pins the exact coarse cells (`P` and `ring`) and `dir` for one known small seed. |
| AC7 | One struct groups four independently-seeded `ChaCha8Rng` **sources** — collision, generation, AI-learning, AI-inference — each with its own seed, configurable in one place. |
| AC8 | The struct's **generation** source feeds Ф1; a generation seed reproduces the same skeleton (AC5 holds through the struct). No `thread_rng` / OS entropy on the generation path. |
| AC9 | The **AI-learning** and **AI-inference** sources are defined and reachable/constructible (no consumer required — `gp-ai` is a stub). |
| AC10 | The existing `GenParams.seed` / `GenParams::rng()` is reconciled with the struct — a single generation RNG path, no divergent duplicate. |
| AC11 | **Collision re-point:** `sim::collision::resolve_collisions` draws its RNG from the struct's **collision** source; collision replay-determinism is preserved — the existing `gp-core` collision determinism tests still pass, updated to seed via the struct. No `thread_rng` / OS entropy on the collision path. |
| AC12 | **Deterministic collections (discharges #50):** every set/map iterated into Ф1 output conforms to `ai-docs/code-style.md` § Deterministic collections (no `std` `HashSet`/`HashMap` into output; `BTreeSet`/`BTreeMap` default; `IndexSet`/`IndexMap` only for genuine insertion-order output; `EnumMap`/`BitFlags` for closed-enum keys). The chosen container's dependency consequence is applied this PR: `BTreeSet<Point>` ⇒ `Point` gains `PartialOrd, Ord`; `IndexSet` ⇒ `indexmap` added as a pinned direct dep, followed by `cargo update` + `cargo build` per AGENTS.md § Dependency Versions. |
| AC13 | Closes #24 and #49; `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and the doc gate pass; the new logic carries a `#[cfg(test)] mod tests`. |

## Open questions

None. Q1 (r1) and Q2 (r2) are resolved and recorded in Key decisions; remaining
undecided items are internal design choices delegated to the design phase, not
open product questions.
