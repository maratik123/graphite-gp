# Design: gp-gen Ф1 coarse-block ring + grouped seeded-RNG config

**Issue:** #24 (Closes), #49 (Closes), #50 (discharges — not "Closes")
**Date:** 2026-07-22
**Spec:** `ai-docs/plans/2026-07-22-gp-gen-phase1-coarse-ring.spec.md`

## Approach

Two coupled deliverables, one PR. All changes are Rust (`.rs`); no instruction-file
or workflow edits, and — per the container decision below — **no `Cargo.toml`
dependency edit**.

### A. Grouped seeded-RNG config (`Seeds`) in gp-core

**Struct location** — `gp-core`, fixed by spec. New top-level module
`crates/core/src/rng.rs`, exposed as `pub mod rng;` from `crates/core/src/lib.rs`,
type re-exported as `gp_core::rng::Seeds`. gp-core is the only crate every consumer
(collision lives here; gp-gen and gp-ai both depend on gp-core) already shares
`[measured: cat crates/{gen,ai}/Cargo.toml → both list gp-core = { workspace = true }]`.

**Struct shape** — hold **four `u64` seeds**, materialize a fresh `ChaCha8Rng` per
source on demand (the default in Key decisions; mirrors the existing
`GenParams::rng()` pattern `[measured: crates/gen/src/lib.rs:41-43]`). Four named
`u64` fields (`collision`, `generation`, `ai_learning`, `ai_inference`) with four
accessor methods (`collision_rng`, `generation_rng`, `ai_learning_rng`,
`ai_inference_rng`) each returning `ChaCha8Rng::seed_from_u64(self.<field>)`. This
gives clean one-place UI seed config (AC7) and constructible/reachable AI sources
without a consumer (AC9).

*Why named fields, not `EnumMap<RngSource, u64>`* — `ai-docs/code-style.md`
§ Deterministic collections prefers `EnumMap`/`BitFlags` for a closed-enum key
**only when the map is iterated into output**. `Seeds` is a config record accessed
by name, never iterated into any generated artifact, so the closed-enum-container
rule does not apply; four named fields are clearer for UI config and match the
existing `GenParams` idiom. Recorded so review can audit the trade-off.

*Const-eligibility* — the accessors call `ChaCha8Rng::seed_from_u64`, which is
**not** `const` on stable, so `missing_const_for_fn` (nursery, deny) does **not**
fire; the accessors are plain `fn`, exactly like today's non-const
`GenParams::rng()` `[derived → cargo clippy --workspace --all-targets -- -D warnings]`.
Derives: `Clone, Copy, Debug, Default, PartialEq, Eq, Hash` (4×`u64`, so `Copy` is
cheap; `Default` = all-zero seeds for UI).

### A2. `GenParams` reconciliation (single generation RNG path)

**Decision: `GenParams` embeds a `Seeds` field**, replacing the standalone
`seed: u64` `[measured: crates/gen/src/lib.rs:24 pub seed: u64]`. The old
`GenParams::rng()` becomes `GenParams::generation_rng(&self) -> ChaCha8Rng`,
delegating to `self.seeds.generation_rng()`. The generation stream is now
materialized in exactly one place (`Seeds::generation`), so no divergent duplicate
path remains (AC10). The same `Seeds` value's `collision` seed feeds collision
(A3), unifying all four sources behind one configurable record (AC7/AC8). The three
non-seed coarse params (`cars`, `min_straight`, `v_ceiling`, `block_size`) are
unchanged.

### A3. Collision RNG re-point (test-only call sites this PR)

`resolve_collisions` keeps its `rng: &mut ChaCha8Rng` signature — the struct
*materializes* a `ChaCha8Rng` from the collision seed, so the function is unchanged
(spec §A.2). `resolve_collisions` has **no production caller**
`[measured: grep -rn resolve_collisions crates → only crates/core/src/sim/collision.rs
tests + the mod.rs re-export + a gp-render doc comment]`, so "re-point" = update the
collision determinism unit tests to seed via `Seeds { collision: N, ..Default::default() }
.collision_rng()` instead of the bare `ChaCha8Rng::seed_from_u64(N)`. Behaviour and
the byte-identical determinism guarantee are preserved because both produce the same
`ChaCha8Rng` from the same `u64` (AC11).

### B. Generation phase Ф1 — `phase1_coarse_ring`

Implements design doc §2 Ф1 pseudocode
(`P = random_simply_connected_polyomino(rng)` → smooth border → `ring =
minkowski_dilate(P,1) \ P` → `widen_selected_sides` → `dir = choose_orientation`)
`[measured: docs/design.md §2 lines 86-92]`. All at **coarse-block** granularity;
Ф2 fine expansion (`k×k`) is out of scope.

New module `crates/gen/src/phase1.rs` (`mod phase1; pub use phase1::*;` from
`lib.rs`). Public surface:

```
pub struct CoarseSkeleton { pub ring: BTreeSet<Point>, pub hole: BTreeSet<Point>, pub dir: RaceDir }
pub fn phase1_coarse_ring(l_min: i32, rng: &mut ChaCha8Rng) -> CoarseSkeleton
```

`dir` is `gp_core::track::RaceDir` `[measured: crates/core/src/track.rs:11-16]`.
Signature mirrors the pseudocode's `phase1_coarse_ring(k, L_min, rng)` minus `k`
(the block size is a Ф2 concern the coarse skeleton does not consume). `l_min` comes
from `GenParams::min_straight` (Q1: coarse-block units); the future `generate()`
caller does `let mut r = params.generation_rng(); phase1_coarse_ring(params.min_straight, &mut r)`.

**Container decision (AC12, discharges #50): `BTreeSet<Point>`, add `PartialOrd, Ord`
to `Point`.** `Point` today derives `Clone, Copy, PartialEq, Eq, Hash, Debug, Default`
but **not** `Ord` `[measured: crates/core/src/geom/mod.rs:21]`; `indexmap` is only a
transitive dep `[measured: grep -rn indexmap --include=Cargo.toml → empty; cargo tree
--invert indexmap → naga → wgpu → egui-wgpu chain only]`. `ring`/`hole` **reach
output** (returned, iterated by tests and by future Ф2), so per § Deterministic
collections they cannot be `std` `HashSet`. Choosing `BTreeSet<Point>` costs one
additive derive on `Point` and **zero** new dependencies; `IndexSet` would add
`indexmap` as a direct dep for insertion-order semantics we do not need (grid cells
carry no meaningful insertion order). The derived `Ord` on `Point{x,y}` orders by
`x` then `y` — fine for deterministic, cross-platform iteration (AC5).

**Determinism crux (AC5 cross-platform).** Every random pick draws an **index into a
deterministically-ordered candidate list**, never into `HashSet` iteration order.
Growth-candidate frontiers are enumerated as `BTreeSet<Point>` / sorted `Vec<Point>`
before the rng picks; the pick index is drawn as a fixed-width **`u32`**
(`rng.random_range(0u32..n)`), mirroring `resolve_collisions`' documented cross-arch
`u32` policy `[measured: crates/core/src/sim/collision.rs:99-108 + its
"Determinism contract" doc]`. This is what makes the skeleton bit-identical across
32-/64-bit targets and `hashbrown`/toolchain versions (design doc §2 [N4], §5 [M3],
cited by AC5). `use rand::RngExt;` for `random_range` (same import collision.rs
uses).

**Ф1 pipeline (deterministic under the fixed stream; per-attempt consumption order
fixed as: growth draws → widen draws; then ONE orientation draw after the loop
settles). "Coarse block" throughout = one coarse cell — the unit of `l_min` and of
AC3's run lengths (Q1); the k×k fine expansion is Ф2.**

**`l_min` domain clamp (reviewer NOTE 2 — bounds work on BOTH the primary and
fallback paths).** `GenParams::min_straight` is an unbounded `i32`; a value near
`i32::MAX` would otherwise drive a saturating, enormous coarse-grid allocation on
both the base strip and the fallback rectangle. Ф1 **first** clamps to its documented
sane coarse-block domain: `let l_eff = l_min.clamp(MIN_COARSE_STRAIGHT,
MAX_COARSE_STRAIGHT);` (`const`s; the documented supported domain is
`MIN_COARSE_STRAIGHT..=MAX_COARSE_STRAIGHT` coarse blocks — e.g. `2..=256`). Every
later use of the straight length reads `l_eff`, never raw `l_min`, so both paths have
a fixed work ceiling `≤ MAX_COARSE_STRAIGHT` cells. This is the same
grid-realistic/allocatable-domain precondition posture gp-core already uses for
`supercover`/`Size::area` — AC3(b)'s "≥ L_min" holds exactly for in-domain `l_min`; a
pathological out-of-domain `l_min` (not a real coarse-grid parameter) is a
documented, **tested** degrade to the ceiling, not an AC change
`[derived → Task 9 clamp-boundary test at `l_min ∈ {i32::MAX, i32::MIN}` → bounded
work, valid skeleton]`.

1. **Base strip — guarantees AC3(b) by construction.** Seed `P` with a 2-cell-tall
   strip occupying block-row 0, cells `[0, base_w) × {0, 1}`, where `base_w =
   max(l_eff, MIN_BASE)` rounded **up to even** (`usize::try_from(l_eff.max(MIN_BASE))`
   is infallible in-domain since `l_eff ≥ MIN_COARSE_STRAIGHT ≥ 0`; then
   `base_w += base_w & 1`). The strip's **south edge** (the dual edges on the
   `y == 0` cells' `Side::South`) is a straight border run of length `base_w ≥ l_eff`.

2. **Growth on the even sublattice, restricted to the outward half-plane `y ≥ 2`
   (fixes Issue 2; underpins AC3(a) on `P`'s border).** `P` grows by adding
   even-aligned **2×2 cell blocks** (`{(2i,2j), (2i+1,2j), (2i,2j+1), (2i+1,2j+1)}`)
   drawn from the **block-4-adjacent frontier** of `P`'s blocks, **restricted to
   block-rows `j ≥ 1` (`y ≥ 2`) — strictly above the base strip.** Each addition
   draws a fixed-width **`u32`** index into the frontier enumerated as a **sorted
   `Vec<Point>`** (never `HashSet` order), up to `TARGET_BLOCKS` (`const`).
   - *Issue 2 resolution (reviewer NOTE 1 — enclosure, not neighbor-count):* growth
     never adds a cell at `y ≤ 1`, and the pre-dilation hole-fill (step 3) fills only
     **bounded** complement components. The entire half-plane `y ≤ −1` opens downward
     into the **unbounded outfield** (nothing in `P` ever sits below the base strip),
     so `y ≤ −1` is **never an enclosed/bounded component** and hole-fill can **never**
     cover the base strip's south edge. The edge therefore stays a straight border run
     `≥ base_w ≥ l_eff`. **AC3(b) holds by construction.** The reviewer's objection —
     "keeping cells in `P` ≠ keeping the base as a border" — is met by the `y ≥ 2`
     **keep-out half-plane**, which preserves the *edge* on the border, not merely the
     base cells in `P`. The implementor encodes this as an **enclosure-based**
     `debug_assert!` — every `y == 0` cell's `Side::South` dual edge is present on the
     ring/`P` boundary (`walls_from_boundary`) — **not** a neighbor-count assertion
     (the previous parenthetical's neighbor-count reason was irrelevant: step 3 is
     enclosure-based `bounded_complement_components`, which never inspects neighbor
     counts).
   - *Even-sublattice rationale (AC3(a), `P`'s border only):* every boundary turn of
     a union of even-aligned 2×2 blocks lies on an edge-line `x = 2k − ½` /
     `y = 2k − ½` (spacing 2), so consecutive corners on any straight edge are ≥ 2
     cells apart ⇒ **every maximal straight run of `P`'s border ≥ 2 by construction**
     — a supporting claim only; the *guarantee* for AC3(a) is the step-6 check on the
     full ring, so even if this were wrong the check + fallback still hold
     `[derived → Task 6 asserts `max_straight_runs(P)` min ≥ 2; Task 9 asserts it on
     the ring across N seeds]`. `P` stays 4-connected by accretion from the
     (connected) base strip.

3. **Hole-fill BEFORE dilation (AC2 ordering, explicit).** Fill every bounded
   complement component of `P` into `P` so `P` is **simply connected**, detected with
   `bounded_complement_components` over a `Corridor` built from `P`
   `[measured: crates/core/src/geom/graph.rs:151 pub fn bounded_complement_components(&Corridor)]`.
   Runs **before** dilation.

4. **Ring — Moore (3×3 / Chebyshev-1) dilation.** `ring = dilate_moore(P) \ P`. The
   structuring element is the 3×3 Moore neighborhood, **not** the 4-neighborhood
   plus-shape — a plus-dilation ring of a thin feature is 4-disconnected (cells touch
   only diagonally), whereas the Moore shell is a 4-connected loop enclosing exactly
   `P`. For a non-empty simply-connected `P` this yields **by construction** (design
   doc §2 Ф1 "связно, ровно одна дырка, дырка ≥1"): `component_count(ring) == 1`,
   `bounded_complement_components(ring) == 1`, `|hole| ≥ 1` (AC2); `hole = P`. `P` is
   non-empty (base strip).

5. **Widen selected sides (outward-only).** For each `Side` in `Side::iter()` (fixed
   order `[measured: crates/core/src/geom/graph.rs:20-30 East,West,North,South]`),
   draw a `u32` amount `0..=WIDEN_MAX` (`const`) and append that many **outward**
   cell-layers **spanning the full length of that side's outer border run** (a
   whole-run strip). Widening is applied **before** the check (step 6) so the check
   validates the *returned* ring. The chosen-sides set is an internal decision,
   **not iterated into output**, so it needs no ordered container and no
   `BitFlags<Side>` (Side is not `#[bitflags]` today; adding that repr to gp-core is
   out of scope).
   - *AC2 is NOT preserved by construction here (design amendment, post-implementation
     — commit `d0f665e`, seed-48 regression).* The earlier revision claimed
     "outward-only ⇒ annulus invariants preserved by construction." **That claim is
     FALSE for concave rings.** For a concave ring whose extremal straight run has
     **disjoint arms**, widening one arm's extremal cells outward can **pinch off a
     second bounded complement component** — 2 holes instead of 1 — violating AC2
     (`component_count`/`bounded_complement_components == 1`). Outward-only guarantees
     neither one-hole nor connectivity for concave shapes. Post-widen AC2 is therefore
     **VERIFIED** (step 6), not assumed — the same "checked on the actual output, not
     assumed" posture the design already adopted for the outer-border run-length
     (reviewer Issue 1), now extended to post-widen connectivity/one-hole.

6. **Post-widen verification (run-length AND connectivity) + bounded same-stream
   retry.** On the **actual, post-widen** ring, verify **both**:
   - **(a) run-lengths** — the maximal straight runs of the **entire** ring border,
     via `walls_from_boundary(&ring_corridor)` grouped by `(side, fixed-coordinate)`
     into contiguous runs
     `[measured: crates/core/src/geom/graph.rs:308 walls_from_boundary(&Corridor) -> Vec<Wall>]`,
     have `min_run ≥ 2` (AC3a) and `max_run ≥ l_eff` (AC3b, redundant with steps 1–2); and
   - **(b) connectivity / one-hole** — `component_count(&ring) == 1 &&
     bounded_complement_components(&ring) == 1` (AC2), which widening can break for
     concave rings (step-5 amendment, seed-48 regression) and is therefore checked
     here, not assumed
     `[measured: crates/core/src/geom/graph.rs:122 component_count / :151 bounded_complement_components]`.

   On **failure of either (a) or (b)**, redraw from the **next segment of the SAME
   generation stream** — no new entropy, fully deterministic (same seed ⇒ same attempt
   sequence ⇒ same first success) — up to `MAX_ATTEMPTS` (`const`). Implemented as the
   `phase1_coarse_ring_attempts` retry check `[measured: commit d0f665e — green]`.
   - *Why a check, not a by-construction claim, for the OUTER border (reviewer Issue
     1):* the even-sublattice argument (step 2) covers only `P`'s border, which is
     the ring's **inner** border. Moore dilation breaks even-alignment, so the ring's
     **outer** border run-lengths are **verified, not assumed** — a concave corner
     rounded by dilation is the one realistic source of an outer length-1 run. The
     false "neighbor-count fixpoint ⇒ all runs ≥ 2" claim from the previous revision
     is **removed**; a staircase can no longer slip through because the invariant is
     *checked on the actual output*, then retried/fallen-back.

7. **Guaranteed-terminating deterministic fallback — makes Ф1 total.** If all
   `MAX_ATTEMPTS` are exhausted, return a **rectangular annulus**: outer
   `[0, W) × [0, H)` minus inner `[1, W−1) × [1, H−1)`, with `W = max(l_eff + 2,
   MIN_RECT_W)` and `H = MIN_RECT_H` (`const`s, both ≥ 4; `W ≤ MAX_COARSE_STRAIGHT + 2`,
   so the fallback allocation is bounded — reviewer NOTE 2). A rectangular annulus
   satisfies **every AC by construction, provably**: connected (one 4-conn loop),
   exactly one hole (`(W−2)(H−2) ≥ 1`), every maximal straight border run = a full
   side length `≥ 2`, and the bottom side `W − 2 ≥ l_eff` (AC3b, within the documented
   `l_min` domain) `[derived → Task 9's forced-fallback test asserts these directly]`.

8. **Orientation.** `dir = if rng.random_range(0u32..2) == 0 { RaceDir::Cw } else
   { RaceDir::Ccw }` — one fixed-width `u32` draw after the loop settles (success or
   fallback), so `dir` is seeded on every path. AC4 stability follows from Ф1 being a
   pure function of the stream.

**Ф1 failure handling (Q — resolved here): INFALLIBLE via a guaranteed-terminating
terminal — NOT a blanket "by construction for all ACs".** The reviewer correctly
rejected the blanket by-construction claim for AC3. The precise revised contract:
- **AC2** (connected, one hole ≥ 1): **by construction for the PRE-widen
  Moore-dilated ring** (Moore dilation of a non-empty simply-connected `P`; hole-fill
  precedes dilation), then **RE-VERIFIED (checked-with-fallback) after widening** —
  outward widening can pinch off a second bounded component on a concave ring
  (step-5 amendment, commit `d0f665e`, seed-48 regression), so AC2 on the *returned*
  ring is enforced by the step-6 `component_count == 1 && bounded_complement_components
  == 1` check + bounded same-stream retry, not purely by construction. The rectangular
  fallback trivially satisfies AC2 (one 4-conn loop, one hole).
- **AC3(b)** (≥ 1 straight run ≥ `l_min`): **by construction** — base strip + `y ≥ 2`
  growth keep-out — **and** re-verified in step 6.
- **AC3(a)** (all runs ≥ 2): **verified** on the actual ring border (step 6), not
  assumed; bounded same-stream retry maximizes the random path's success; the
  rectangular fallback (step 7) is the **defined terminal** where it holds trivially.
- **Termination:** each attempt is bounded work (`TARGET_BLOCKS` additions + bounded
  widen + one bounded border scan); `MAX_ATTEMPTS` is a `const`; the fallback is
  `O(W·H)` and unconditional ⇒ Ф1 always returns in bounded time
  `[derived → cargo test phase1 completes with no hang]`.
- **Reconciliation with the panic-index binding constraint:** the terminal is a
  *guaranteed-terminating construction*, so Ф1 needs **no `Result`, no `thiserror`,
  and no `unwrap`/`expect`/`panic!`/panicking index** — the zero-production-panic
  posture is preserved with a defined value on every path
  `[measured: ai-docs/panic-index.md intentionally empty per AGENTS.md]`. (The outer
  seed-budget/reseed loop #34 may later prefer to *reseed* rather than fall back;
  that is out of scope — Ф1 is self-contained and total today.) Internal
  `debug_assert!`s document the by-construction AC2/AC3(b) invariants without
  shipping a production panic.

**Integer-safety (#48 posture).** Ф1 is `gp-gen` but the workspace lints
(`arithmetic_side_effects = "deny"`, `pedantic`+`nursery` deny) apply crate-wide
`[measured: Cargo.toml [workspace.lints.clippy]]`. All coordinate arithmetic uses
`checked_`/`saturating_` forms or a documented `#[allow(clippy::arithmetic_side_effects,
reason=…)]` under a stated coarse-grid bound; any const-eligible pure integer helper
is `const fn` (nursery `missing_const_for_fn`) `[derived → cargo clippy … -D warnings]`.

### Rejected alternatives

- **`IndexSet<Point>` for ring/hole** — rejected: adds `indexmap` as a direct dep
  for insertion-order semantics grid cells don't need; `BTreeSet` + one `Point`
  derive is cheaper and dependency-free.
- **Store ring/hole as `Corridor`** — rejected as the *returned* type: `Corridor`
  has no public cell iterator (`box_points` is private
  `[measured: crates/core/src/geom/mod.rs:322 fn box_points]`), so it is awkward for
  downstream Ф2 iteration; a `Corridor` is still built *internally/in tests* to run
  the #5 connectivity helpers, but the skeleton carries `BTreeSet<Point>`.
- **Plus-shape (4-neighbor) dilation** — rejected: produces a 4-disconnected ring
  for thin features, failing AC2's `component_count == 1`.
- **Neighbor-count smoothing (fill cells with ≥3 in-`P` neighbors / erode ≤1)** —
  **rejected (reviewer Issue 1):** a staircase boundary on a thick body is a stable
  fixpoint — every staircase cell has exactly 2 in-`P` neighbors (neither filled nor
  eroded) — yet each step is a length-1 run, so the fixpoint does **not** guarantee
  AC3(a). Replaced by even-sublattice growth (inner border ≥ 2 by construction) +
  a run-length **check** on the full ring (step 6).
- **Full concave-corner fill to fixpoint** — rejected: a rectilinear simply-connected
  region with no concave corners **is a rectangle**, so filling every concave corner
  collapses every `P` to its bounding rectangle, destroying all shape variety (the
  fallback rectangle is acceptable as a rare terminal, but not as the *only* output).
- **Fallible Ф1 (`Result`) + `thiserror`** — rejected in favor of the
  guaranteed-terminating rectangular fallback: AC3(b) is by construction, AC2 is
  by-construction pre-widen then checked post-widen, and AC3(a) is checked — all
  three with the same bounded-retry-then-fallback terminal, so a total function with
  a defined terminal is simpler for the (out-of-scope, `todo!`) `generate()` caller
  than error propagation, and keeps the zero-panic posture. (Per the reviewer's note,
  this is the "guaranteed-terminating construction" branch of the
  Result-vs-construction choice.)

## Decomposition

All 9 subtasks are Rust code (`.rs`). TDD: write each task's `#[cfg(test)]` cases
before/with its production code (§ Test Design).

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `Seeds` struct (4 `u64` seeds + 4 `*_rng()` accessors) in new `rng` module; `pub mod rng;` in lib.rs; unit tests (AC7, AC9) | `crates/core/src/rng.rs`, `crates/core/src/lib.rs` | — |
| 2 | Re-point collision determinism tests to seed via `Seeds::collision_rng()` (signature unchanged) (AC11) | `crates/core/src/sim/collision.rs` | 1 |
| 3 | Reconcile `GenParams`: embed `Seeds`, replace `seed`/`rng()` with `generation_rng()`; update gen tests (AC8, AC10) | `crates/gen/src/lib.rs` | 1 |
| 4 | Add `PartialOrd, Ord` to `Point`'s derive (BTreeSet key; discharges #50 dep consequence) (AC12) | `crates/core/src/geom/mod.rs` | — |
| 5 | Define `CoarseSkeleton { ring, hole, dir }` + `phase1` module scaffold; `mod phase1; pub use phase1::*;` (AC1) | `crates/gen/src/phase1.rs`, `crates/gen/src/lib.rs` | 4 |
| 6 | `P` construction: **clamp `l_min`→`l_eff` (`MIN`/`MAX_COARSE_STRAIGHT` domain, NOTE 2)** + base strip + even-sublattice 2×2 growth (`y ≥ 2` keep-out) + hole-fill-BEFORE-dilation + **enclosure-based AC3(b) `debug_assert`** (NOTE 1); `u32`/sorted-`Vec` determinism; unit tests (AC3b, connectivity, simple-connectivity) | `crates/gen/src/phase1.rs` | 5 |
| 7 | Moore-dilate ring, outward widen, post-widen **check = run-length AND connectivity/one-hole** (`component_count==1 && bounded_complement_components==1`, amendment d0f665e) + bounded same-stream retry + rectangular fallback (`W` from `l_eff`, bounded) + orientation; assemble `phase1_coarse_ring`; unit tests (AC2, AC3a, AC4) | `crates/gen/src/phase1.rs` | 6 |
| 8 | Ф1 AC-level suite: replay determinism (AC5), minted snapshot (AC6), returned-container type (AC12) | `crates/gen/src/phase1.rs` | 1, 3, 7 |
| 9 | **Multi-seed property test** over N seeds: `component_count(ring)==1`, `bounded_complement_components(ring)==1`, `|hole| ≥ 1`, `min_run ≥ 2`, `max_run ≥ l_eff`; **assert fallback rate ≤ FALLBACK_RATE_MAX** (not merely "not all"; recommendation); a **clamp-boundary test** (`l_min ∈ {i32::MAX, i32::MIN}` → bounded work + valid skeleton, NOTE 2); a forced-exhaustion test asserting the rectangular fallback's invariants (AC2, AC3) | `crates/gen/src/phase1.rs` | 7 |

Scope: 9 tasks (≤ 15). No issue split needed.

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. `M = 9`. Every subtask is
**code** change-type (`*.rs`) — homogeneous — and the dependency chain
(1,4 → 2,3,5 → 6 → 7 → 8,9) fits within one `≤ 10` group, so the minimum group count
is **1**.

- **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) before
  starting the group (every-group handoff contract, sub-point (a)/(c)).
- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned in frontmatter)**,
  1M-token window, via the `code-writer` subagent (code change-type routes to
  `subagent_type="code-writer"`; no inline model/effort override) — subtasks **1–9**
  (all `*.rs`). Terminal group (9 subtasks; within the `1..=10` range).

No inter-group handoff (single group). Total groups: **1** (≤ 4 default max; no user
gate needed). The `design`, `design-review`, and `self-review` Opus gates review
Group A's output regardless of the implementor marker.

## Risks

- **Cross-platform non-determinism from `HashSet` iteration or width-varying index
  draws (AC5) is the primary hazard.** Mitigation: candidate frontiers are
  `BTreeSet<Point>`/sorted `Vec`, and every pick draws a fixed-width `u32` index —
  mirroring the collision code's documented cross-arch policy
  `[measured: crates/core/src/sim/collision.rs:99-108]`. Test AC5 asserts
  same-seed identity; the Miri workspace job + CI on two arches would surface a
  regression `[derived → cargo test + CI]`.
- **Moore vs plus dilation correctness (AC2).** A plus-dilation ring is
  4-disconnected for thin features; the design fixes the structuring element to
  Moore 3×3. Mitigation: AC2 test runs `component_count == 1` and
  `bounded_complement_components == 1` on the minted skeleton
  `[derived → cargo test phase1]`.
- **Widen pinches off a second hole on a concave ring (AC2) — realized regression,
  now checked.** `[measured: commit d0f665e — the multi-seed test at seed 48,
  l_min=3 pinched a concave arm into a 2-hole ring, failing AC2]`. Outward-only
  widening does **not** preserve one-hole/connectivity for concave rings with
  disjoint extremal arms, so the earlier "by construction" claim was false.
  Mitigation (in code, green): the step-6 retry check now also verifies
  `component_count == 1 && bounded_complement_components == 1` on the actual post-widen
  ring, with the same bounded same-stream retry → rectangular fallback; Task 9's
  multi-seed sweep (including seed 48) is the regression guard
  `[derived → cargo test phase1 multi-seed]`.
- **AC3(a) outer-border length-1 run from Moore-rounding a concave corner
  (reviewer Issue 1).** The even-sublattice construction proves runs ≥ 2 only for
  `P`'s border (the ring's inner border); the ring's OUTER border is **verified** by
  the run-length check (step 6), not assumed. Mitigation: bounded same-stream retry
  redraws on failure; the rectangular fallback is the guaranteed terminal. Risk that
  many seeds fall back → rectangle-heavy output (correct but dull): Task 9 asserts the
  **fallback rate ≤ `FALLBACK_RATE_MAX`** (not merely "not all") — a construction bug
  making Moore-rounding fail the step-6 check on most seeds would pass a "not all"
  check yet spike the rate; the threshold catches it `[derived → cargo test phase1
  multi-seed]`.
- **AC3(b) base-edge covered by growth (reviewer Issue 2).** Keeping base *cells* in
  `P` is insufficient — a cell added beneath the base turns its south edge interior.
  Mitigation: growth is confined to the `y ≥ 2` half-plane, and hole-fill fills only
  **bounded** complement components while `y ≤ −1` opens to the unbounded outfield
  (enclosure argument, NOTE 1), so the base south edge stays a border run ≥ `l_eff`
  **by construction**; the run-length check re-verifies `max_run ≥ l_eff`
  `[derived → cargo test phase1 (AC3b) + multi-seed]`.
- **Termination of the retry loop.** `MAX_ATTEMPTS` bounded × bounded per-attempt
  work + unconditional `O(W·H)` fallback ⇒ no unbounded loop / hang
  `[derived → cargo test phase1 completes]`.
- **Unbounded `l_min` → enormous allocation (reviewer NOTE 2).** `GenParams::min_straight`
  is an unbounded `i32`; a value near `i32::MAX` would saturate `base_w`/fallback `W`
  into a multi-billion-cell allocation on both paths. Mitigation: `l_min` is clamped
  to `l_eff ∈ MIN_COARSE_STRAIGHT..=MAX_COARSE_STRAIGHT` before any length use, so
  work is capped at `MAX_COARSE_STRAIGHT`; Task 9's clamp-boundary test drives
  `l_min = i32::MAX`/`i32::MIN` and asserts a bounded, valid skeleton
  `[derived → cargo test phase1 clamp-boundary]`.
- **`phase1.rs` file-size soft limit (500 excl. tests / 800 incl.)
  `[measured: ai-docs/code-style.md § File size]`.** Ф1 + helpers + tests may
  approach it. Mitigation: if the production half crosses ~500 lines, split helpers
  into a `phase1/` submodule (`polyomino.rs`, `ring.rs`) by responsibility, not by
  line count `[derived → wc -l + reviewer check]`.
- **`arithmetic_side_effects`/`missing_const_for_fn` denies on new Ф1 integer code
  `[measured: Cargo.toml [workspace.lints.clippy] nursery+pedantic+arithmetic_side_effects
  = deny]`.** A `-D warnings` gate aborts on the first hit, masking later ones;
  budget a re-run after each cleanup. Mitigation: checked/saturating forms or
  documented allows; const-eligible pure helpers marked `const fn`
  `[derived → cargo clippy --workspace --all-targets -- -D warnings]`.
- **`Point` gaining `Ord` is purely additive** — no manual `PartialEq`/`Hash` to
  conflict with, no `derive_ord_xor_partial_ord` trigger (both derived together)
  `[derived → cargo clippy … -D warnings]`.
- **Minted snapshot frozen blind (AC6).** The exact cell set cannot be computed at
  design time. Mitigation: the implementor mints it by running Ф1 once for the fixed
  seed, then cross-checks the captured skeleton satisfies AC2/AC3 invariants before
  freezing — not a blind paste `[derived → cargo test phase1 + reviewer check]`.

## Test Design

- **Task 1 — `Seeds` (`crates/core/src/rng.rs` `#[cfg(test)]`).**
  - Entry points: the four `*_rng()` accessors.
  - Scenarios: (AC7) same seed → identical `next_u64` stream per source; distinct
    seeds across the four fields → four independent streams (a shared seed value in
    two fields yields identical streams, confirming per-field seeding); (AC9)
    `ai_learning_rng`/`ai_inference_rng` are callable on a constructed `Seeds` (a
    `Default` value plus a field-set value) and produce a stream.
  - Fixtures: a `seeds(c,g,l,i)` builder.

- **Task 2 — collision re-point (`crates/core/src/sim/collision.rs` tests).**
  - Entry point: `resolve_collisions` (unchanged).
  - Scenarios: every existing determinism test (`ac7_repeated_calls_are_byte_identical`,
    `ac3_equidistant_seeded_pick_is_exact_and_stable`, etc.) still passes with the
    rng built via `Seeds { collision: N, ..Default::default() }.collision_rng()`
    instead of `ChaCha8Rng::seed_from_u64(N)`; the exact pin (seed 42 → `(2,3)`)
    is unchanged (AC11). No new behaviour — a mechanical seeding swap.

- **Task 3 — `GenParams` reconciliation (`crates/gen/src/lib.rs` tests).**
  - Entry point: `GenParams::generation_rng`.
  - Scenarios: (AC8) same generation seed → identical stream (port the existing
    `rng_same_seed_yields_identical_stream`); different generation seed → different
    stream; (AC10) `generation_rng` is the sole generation RNG path — a `GenParams`
    built with a given `Seeds.generation` reproduces the same stream regardless of
    the other three seeds.

- **Task 6 — `P` construction (`crates/gen/src/phase1.rs` tests).**
  - Entry points: the base-strip, growth, and hole-fill helpers (test-visible via a
    seeded call).
  - Scenarios: `P` is non-empty and 4-connected (`component_count == 1` on a
    `Corridor` built from `P`); `P` is simply connected **after** the pre-dilation
    hole-fill (`bounded_complement_components(P) == 0`); **no growth cell has
    `y ≤ 1`** (the `y ≥ 2` keep-out, Issue-2 guard) so the base strip's south edge is
    intact; **every `y == 0` cell's `Side::South` edge is on the boundary** (the
    enclosure-based AC3(b) guard from NOTE 1 — asserted, and the same predicate the
    production `debug_assert!` encodes); `P`'s own border has no length-1 run
    (even-sublattice guarantee, checked via `max_straight_runs` on a `Corridor` from
    `P`); the `l_min → l_eff` clamp maps an in-domain `l_min` to itself. Fixture: a
    small fixed seed + `corridor_from_cells(&BTreeSet<Point>)` and `max_straight_runs`
    test helpers.

- **Task 7 — ring / widen / check-retry-fallback / orientation
  (`crates/gen/src/phase1.rs` tests).**
  - Entry point: `phase1_coarse_ring`.
  - Scenarios: (AC2) `ring = dilate_moore(P)\P` disjoint from `hole`; on a `Corridor`
    built from `ring`, `component_count == 1` and `bounded_complement_components == 1`,
    `hole` non-empty; hole-fill precedes dilation (a `P` seeded with an enclosed hole
    is filled before the ring is built). (AC3a) `max_straight_runs(&ring)` on the
    returned ring has `min_run ≥ 2` — the run-length check is *enforced* on output.
    **Post-widen AC2 is CHECKED, not assumed (amendment d0f665e):** widening a concave
    ring's extremal arm outward can pinch off a second hole, so the returned ring is
    asserted `component_count == 1 && bounded_complement_components == 1` (a
    concave-arm fixture, or seed 48 / `l_min=3`, exercises the pinch → retry/fallback
    path). (AC4) `dir ∈ {Cw, Ccw}` and equal across two same-seed calls.

- **Task 8 — Ф1 replay + snapshot + container type (`crates/gen/src/phase1.rs`).**
  - Entry point: `phase1_coarse_ring` end-to-end.
  - Scenarios: (AC5) two same-generation-seed runs (seeded via
    `Seeds::generation_rng`) produce identical `{ring, hole, dir}`; two different
    seeds differ in ≥ 1 field. (AC6) **snapshot**: one known small seed → an exact
    `assert_eq!` of sorted `ring`/`hole` `Vec<Point>` and `dir`, values **minted by
    the implementor** and cross-checked against AC2/AC3 before freezing (a data
    snapshot, not an egui image golden — no pixel threshold applies). (AC12) the
    returned containers are `BTreeSet` (no `std::HashSet` reaches output — enforced by
    the type).
  - Fixtures: `corridor_from_cells`, a fixed `SNAPSHOT_SEED`.

- **Task 9 — multi-seed property test (`crates/gen/src/phase1.rs`).**
  - Entry point: `phase1_coarse_ring` over `SEED in 0..N` (N a `const`, e.g. 64).
  - Scenarios: for every seed, on a `Corridor` from the returned `ring`,
    `component_count == 1`, `bounded_complement_components == 1`, `|hole| ≥ 1`
    (AC2); `max_straight_runs(&ring)` has `min_run ≥ 2` **and** `max_run ≥ l_eff`
    (AC3a/AC3b) — the property backstop for the infallibility claim, since it rests on
    the by-construction + checked guarantees rather than one/two seeds. The sweep
    range **must include seed 48 at `l_min = 3`** — the widen-pinch regression witness
    (amendment d0f665e) that a narrower seed set missed; its post-widen ring must now
    pass AC2 via the step-6 connectivity check (retry or fallback).
  - **Fallback-rate assertion (recommendation).** Count how many of the N seeds hit
    the rectangular fallback and assert `fallback_count ≤ FALLBACK_RATE_MAX * N`
    (`FALLBACK_RATE_MAX` a `const`, e.g. `0.20`). Justification: outer-border length-1
    runs arise only where Moore dilation rounds a concave corner of an even-aligned
    `P`, which is uncommon, so a healthy construction falls back rarely; a bug that
    makes the step-6 check fail on most seeds (dull rectangle-heavy output) would
    still pass a weaker "not all fall back" check but blows past a 20% ceiling. The
    exact constant is tuned to the observed rate at mint time (start at 0.20; the
    implementor lowers it toward the measured rate if comfortably below)
    `[derived → cargo test phase1 multi-seed records the rate]`.
  - **Clamp-boundary test (NOTE 2).** `phase1_coarse_ring(i32::MAX, &mut rng)` and
    `phase1_coarse_ring(i32::MIN, &mut rng)` each return a valid skeleton (AC2/AC3
    hold) with a **bounded** cell count (`≤ ~(MAX_COARSE_STRAIGHT)²` order), proving
    the clamp caps work on both the primary and fallback paths — no multi-billion-cell
    allocation, no hang.
  - **Forced-exhaustion test.** Drive `MAX_ATTEMPTS` to 0 via a test-only entry (or a
    seed known to exhaust) and assert the rectangular fallback itself satisfies AC2 +
    AC3 (all runs ≥ 2, bottom side ≥ `l_eff`, one hole ≥ 1).
  - Fixtures: `corridor_from_cells`, `max_straight_runs`; a couple of representative
    in-domain `l_min` values plus the two clamp-boundary extremes.

- **Miri (Tasks 6–9).** Guard nothing — these are pure integer/`std`/`BTreeSet` tests
  with no FFI/GPU and no `egui::Context`
  `[measured: AGENTS.md § Rust Test Conventions — the Miri gate is for
  wgpu/egui/vello tests, none present here]`.

## Open questions

None. Q1 (r1) and Q2 (r2) are resolved in the spec's Key decisions; every remaining
choice (struct shape/naming, `GenParams` reconciliation, skeleton type, container =
`BTreeSet<Point>`, base-strip + even-sublattice draw scale, Moore dilation,
outward widening, orientation rule, and Ф1's total-with-defined-terminal failure
handling — AC3(b) by construction (base strip + `y ≥ 2` keep-out); **AC2 by
construction for the pre-widen Moore-dilated ring, then re-verified after widening**
(post-widen pinch is checked, per amendment d0f665e); AC3(a) checked; all with
bounded same-stream retry + guaranteed rectangular fallback) is decided above and is
an internal design choice, not a product question.
