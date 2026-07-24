# Design: gp-gen `generate()` pipeline orchestration + `gp_core::geom::medial_axis` connected-ridge fix

**Issue:** [#34](https://github.com/maratik123/graphite-gp/issues/34)
**Date:** 2026-07-24

> **Amendment-2 reconciliation (revision 3).** The spec was amended twice. Amendment 1
> (a `MAX_BRIDGE_GAP` bump) is **superseded** — its root cause was refuted by
> measurement. Amendment 2 moves the fix to its true source: `gp_core::geom::medial_axis`
> (spec Scope 7–8, AC8–AC9). The product owner answered this design's **Q-Ф7 with
> Option A1 — fix `medial_axis` at the source in `gp-core`** (spec § Key decisions).
> **Q-Ф7 is CLOSED** (§ Open questions); the A1 algorithm is specified concretely in
> § *A1 — the `medial_axis` replacement* below, which is the load-bearing new work of
> this revision. The Scope 1–6 orchestration design is **unchanged** and already
> written (subtasks 1–4).

## Approach

Wire the already-landed phases Ф1→Ф7 into the outer generation loop of
`docs/design.md` §2 (`generate_track` pseudocode) — orchestration + `TrackArtifact`
assembly, no Ф1–Ф6 behaviour or signature changes — **plus** the amendment-2
`gp-core` `medial_axis` replacement and the bounded Ф7 follow-through it demands.
The stub `pub fn generate(_params: GenParams) -> TrackArtifact { todo!(...) }`
(`crates/gen/src/lib.rs`) becomes
`pub fn generate(params: GenParams) -> Result<TrackArtifact, GenerationError>`.

### Current committed state (reconcile — do NOT re-implement)

`[measured: git log --oneline -5 → ed282c2 subtask 1, 047400c subtask 2, a2bb3cc
subtask 3; git status --porcelain → M crates/gen/src/lib.rs, ?? crates/gen/src/generate.rs]`

- **Subtask 1 (`GenParams` `seed_budget`/`repair_budget` fields) — DONE** (`ed282c2`).
- **Subtask 2 (`corridor_min_width` helper + test) — DONE** (`047400c`).
- **Subtask 3 (`thiserror` workspace edge) — DONE** (`a2bb3cc`).
- **Subtask 4 (`generate.rs` orchestration + `lib.rs` wiring) — WRITTEN, UNCOMMITTED.**
  `crates/gen/src/generate.rs` holds the full loop, `GenerationError`,
  `should_run_oracle`, `build_artifact` and the AC1/AC3/AC6 unit tests, all correct
  and passing. Two finalization items remain before its commit: (i) **delete the
  `scratch_diagnose_e2e` `#[ignore]` test** (`generate.rs:165-195`, a Step-8
  debugging leftover — it must not ship) `[measured: sed -n '165,195p'
  crates/gen/src/generate.rs → #[ignore = "scratch diagnostic, not part of the
  suite"]]`; (ii) **move the non-empty-centerline assertion out of the e2e test**
  (`generate.rs:296` `assert!(!a1.centerline.samples.is_empty())`) — it fails until
  A1 lands, so it belongs to subtask 8, not to subtask 4's commit.

The rest of the Scope 1–6 design (control-flow shape, `GenerationError`, `width_min`
producer, artifact assembly, file layout) is retained verbatim below and matches the
written code.

### Landed-signature verification (unchanged, still valid)

`[measured: grep -n 'pub fn phase*' crates/gen/src/*.rs →`
- `phase1_coarse_ring(l_min: i32, rng: &mut Xoshiro256PlusPlus) -> CoarseSkeleton` (`phase1.rs:65`)
- `phase2_rasterize(skel: &CoarseSkeleton, k: i32, n: i32) -> Corridor` (`phase2.rs:38`)
- `phase3_start_finish(d: Corridor, skel: &CoarseSkeleton, m: u32, v_target: i32) -> Phase3Output { d, sf, grid }` (`phase3.rs:487`)
- `phase4_static_checks(d: &Corridor, skel: &CoarseSkeleton, k: i32, n: u32, m: u32, sf: &StartFinish) -> Vec<Issue>` (`phase4.rs:244`)
- `oracle_liveness_v1(d: &Corridor, grid: &StartGrid, sf: &StartFinish, race_dir: RaceDir) -> bool` (`phase5.rs:145`)
- `phase5_full_oracle(d: &Corridor, grid: &StartGrid, sf: &StartFinish, race_dir: RaceDir) -> OracleResult` (`phase5b.rs:346`)
- `phase5_runout_checks(d: &Corridor, metrics: &TrackMetrics, v_target: i32) -> Vec<Issue>` (`phase5_runout.rs:320`)
- `phase6_local_repair(ctx: &RepairContext<'_>, issues: &[Issue]) -> RepairOutcome` (`phase6_repair.rs:259`)`]

`phase2_rasterize`'s `n` is **`i32`**, so `generate` computes `n_u32 = params.min_width()`
(Ф4/ctx) and `n_i32 = i32::try_from(n_u32).unwrap_or(i32::MAX)` (Ф2, total/saturating)
— as written in `generate.rs:85-86`.

### Control-flow shape (unchanged — as written in `generate.rs`)

Matches §2 pseudocode wired to landed Ф6. Per **seed** iteration `skel`, `sf`, `grid`,
`race_dir` are fixed; only `d` evolves via `RepairOutcome::Repaired`. The cheap V=1
liveness (`oracle_liveness_v1`) carries no `stall_walls`; a stall diagnostic capable of
driving Ф6's dynamic arm exists only after the expensive oracle returns
`NotLappable { stall_walls }` `[measured: sed -n '42,67p' crates/gen/src/phase5b.rs →
enum OracleResult { Lappable(TrackMetrics), NotLappable { stall_walls: Vec<Wall> } }]`.
Single continuing RNG (`params.generation_rng()` built once, `&mut rng` threaded —
replay-determinism, #49). Fall-through → `Err(GenerationError::SeedBudgetExhausted)`.

### `GenerationError`, `width_min` producer, artifact assembly, file layout

Unchanged from the GO'd design and matching the written code: `GenerationError` is a
one-variant `thiserror` enum (`generate.rs:19-26`); the `thiserror` edge adds **no**
new `[[package]]` and bumps **no** crate — the only `Cargo.lock` delta is one
`"thiserror 2.0.19"` line in gp-gen's `dependencies` `[measured: git show a2bb3cc
--stat → Cargo.lock touched, gp-gen entry only]`. `corridor_min_width`
(`phase4_defects.rs`, subtask 2, committed) reuses the same-module `axis_width`
`pub(crate) fn`; `width_min ≥ n` is asserted at the e2e test, not proven by
construction (the `Narrow` gate is DT-consistency-filtered — see § Risks).
`build_artifact` (`generate.rs:43-66`) computes `walls`/`s_field`/`centerline`/
`width_min` before moving `d`/`sf`/`grid`/`metrics`. The loop lives in `generate.rs`
(`pub use`d from `lib.rs`) for file-size discipline.

---

## A1 — the `medial_axis` replacement (the load-bearing new work)

### The defect, mechanically

`medial_axis` (`crates/core/src/geom/distance.rs:112`) keeps `p` iff `p` is a **strict**
local maximum of `dt` along at least one axis (`dp > dt(E) && dp > dt(W)` ‖
`dp > dt(N) && dp > dt(S)`) `[measured: sed -n '112,131p'
crates/core/src/geom/distance.rs]`. On a wide corridor the DT is a **plateau** in the
along-flow direction and **ties** across an even-width cross-section, so *neither* axis
has a strict maximum and the ridge collapses to sporadic cells. The pathological limit
is exact and reproducible: the medial axis of an even×even filled rectangle is
**empty** `[measured: scratchpad reference impl → 6×6 filled: OLD = 0 cells]`, and of a
41×41 ring with a 14-cell band it is **20 cells in 20 singleton components**
`[measured: scratchpad reference impl → OLD=20c/20comp]`. That is the same failure the
orchestrator's public-API probe measured on real corridors (40–84 components, mostly
singletons, dt_peak 14–21). **Strictness cannot be relaxed to `≥`** — the along-flow
plateau would then admit every cell and collapse the ridge to the whole corridor
(`medial_axis`'s own rustdoc states this, `distance.rs:94-97`). The ridge *test* must
therefore be **replaced**, not loosened.

> **Reference-implementation provenance.** Every `[measured: scratchpad reference impl]`
> tag below comes from a throwaway Python port (scratchpad only, nothing written to the
> repo) of `DistanceTransform::compute`, the *old* `medial_axis`, the new algorithm, and
> `phase7`'s `prune_spurs`/`walk_cycle`. The port is **validated against in-tree ground
> truth**: it reproduces the three current in-tree expected sets exactly — 5×3 →
> `{(1,1),(2,1),(3,1)}`, 4×3 → `{(1,1),(2,1)}`, annulus → the 20-cell 4-strip set
> `[measured: scratchpad reference impl vs distance.rs:189-194 / 242-251 / 275-280]`.
> The predicted new outputs below are therefore high-confidence **predictions, not
> facts**: the implementer MUST re-derive each by running the Rust, and on ANY
> divergence **STOP and report** rather than silently re-pinning (a divergence means
> the implementation deviates from the algorithm specified here).

### Chosen algorithm — DT-ordered homotopic thinning with anchored end points

`medial_axis` becomes a **distance-ordered, connectivity-preserving morphological
thinning**: repeatedly delete the lowest-`dt` *simple* cell that is not an *anchored
end point*, until no cell is deletable. Signature and return type are unchanged
(`pub fn medial_axis(dt: &DistanceTransform) -> BTreeSet<Point>`, spec constraint 5).

**Inputs.** `D` is recovered from `dt` alone: `p ∈ D ⟺ dt.at(p) > 0` over `dt.rect()`
— `at` is `0` for every `¬D`/out-of-box point and `≥ 1` for every drivable cell
`[measured: sed -n '15,20p' crates/core/src/geom/distance.rs → DistanceTransform's
rustdoc: zero for any p outside D (out-of-box included), at least 1 for every drivable
cell]`.

**Connectivity convention.** Foreground **4-connected**, background **8-connected** —
the complementary (4,8) pair digital topology requires, and the one every consumer
already uses (`phase7::components`/`degree`/`walk_cycle` are all 4-conn
`[measured: sed -n '116,134p;217,222p;325,338p' crates/gen/src/phase7.rs]`).

**Predicates** (all over the 3×3 window `N8(p)`; `A = N8(p) ∩ S`, `B = N8(p) \ S`,
where `S` is the current cell set and out-of-box cells are in `B`):

- `is_simple(S, p)` — `p` is deletable without changing local topology iff
  **(i)** the number of **4-connected components of `A`** (adjacency computed *within
  the window*) that contain at least one 4-neighbour of `p` is exactly **1**, **and**
  **(ii)** `B` is non-empty and forms exactly **one 8-connected component** (within the
  window). Both counts are computed by an explicit ≤ 8-cell flood fill — not a
  remembered crossing-number formula. The flood fill runs over a **fixed 8-slot
  window** and MUST **allocate nothing per call** (fixed arrays / an 8-bit bitmask;
  no per-invocation `Vec`/`BTreeSet`/`VecDeque`). Both shapes produce identical
  results, and the allocating one is 3.6× more expensive under Miri
  `[measured: reviewer's Rust port, both shapes → |S| = 48 on the 21×21 fixture and
  |S| = 146 on the 61×61 AC9 fixture, identical; 170 s vs 47 s under
  MIRIFLAGS=-Zmiri-tree-borrows at 450 pops]`.
- `is_anchored_endpoint(S, dt, p)` — `p` has exactly one 4-neighbour in `S` **and**
  `dt(p) ≥ dt(q)` for all `q ∈ N8(p)`. Such a cell is a genuine medial branch tip and is
  **never** deleted; an unanchored degree-1 cell (a boundary artefact whose `dt` is
  dominated by a neighbour) **is** deletable, so corner arms peel back to the ridge.

**Algorithm.**

```
S     : BTreeSet<Point> = { p ∈ dt.rect().points() : dt.at(p) > 0 }
queue : BTreeSet<(u32, Point)> = { (dt.at(p), p) : p ∈ S }      // min-first
while let Some((_, p)) = queue.pop_first():
    if !S.contains(&p)                    { continue }          // stale entry
    if is_anchored_endpoint(&S, dt, p)    { continue }
    if !is_simple(&S, p)                  { continue }
    S.remove(&p)
    for q in neighbors8(p) where S.contains(&q):
        queue.insert((dt.at(q), q))       // re-examine; BTreeSet dedups
return S
```

`BTreeSet::pop_first` is available on the pinned toolchain `[measured: grep -n "pub fn
pop_first" ~/.rustup/.../alloc/src/collections/btree/set.rs → line 843, feature
"map_first_last" stable since 1.66.0; rustc --version → 1.97.1, workspace
rust-version = "1.97.1"]`. A `BTreeSet` queue (rather than a `BinaryHeap`) removes any
need to reason about duplicate-key pop order: keys are unique per point (`dt` is fixed
per cell), so the queue holds ≤ `|D|` live entries.

**Why each property holds:**

1. **Determinism (spec constraint 1).** The only ordered structures are
   `BTreeSet<Point>` and `BTreeSet<(u32, Point)>` — total integer orders, no hashing,
   no float, no address- or iteration-order dependence; `Rect::points()` is a fixed
   row-major walk `[measured: sed -n '197,207p' crates/core/src/geom/mod.rs]`. The
   deletion sequence is therefore a pure function of `dt`, identical on every run and
   platform. `compute_and_medial_axis_are_deterministic` (`distance.rs:307`) passes
   **unmodified** `[derived → subtask 6: cargo test -p gp-core distance]`.
2. **Integer-only (constraint 2).** Only `u32` comparisons, `Point` set membership and
   `saturating_add/sub` coordinate offsets — no float anywhere (`docs/design.md` §3a).
3. **Totality / no panics (constraint 3).** No indexing, no `unwrap`/`expect`, no raw
   arithmetic: coordinate offsets use `saturating_*`, counts come from
   `Iterator::count`. Empty corridor ⇒ `S` empty ⇒ loop body never runs ⇒ empty
   `BTreeSet`, so `empty_corridor_has_zero_dt_and_empty_medial_axis`
   (`distance.rs:284`) passes **unmodified**. `clippy::arithmetic_side_effects`,
   `pedantic` and `nursery` are all `deny` at the workspace root `[measured: sed -n
   '60,81p' Cargo.toml → pedantic/nursery deny priority -1, arithmetic_side_effects
   deny]`, so no new `#[allow]` is expected; `nursery`'s `missing_const_for_fn`
   **forces** `const fn` on the one const-eligible new helper,
   `const fn neighbors8(p: Point) -> [Point; 8]` (body = an array literal of
   `Point::new(x.saturating_add(1), …)`; `Point::new` is `const`
   `[measured: sed -n '37,56p' crates/core/src/geom/mod.rs]` and `i32::saturating_add`
   is const-stable `[measured: grep -n "const fn saturating_add"
   ~/.rustup/.../core/src/num/int_macros.rs:1959]`). `is_simple` /
   `is_anchored_endpoint` / `degree4` call `BTreeSet::contains` and
   `DistanceTransform::at`, neither const-callable, so the lint correctly does not fire
   on them.
4. **Termination + cost (constraint 4).** Each iteration pops one queue entry;
   entries are only re-inserted after a deletion (≤ 8 per deletion) and the queue
   dedups, so total pops ≤ `9·|D|`. Measured pops are ≈ `1.03·|D|`
   `[measured: scratchpad reference impl → |D|=1512 ⇒ 1580 pops; |D|=3552 ⇒ 3642;
   |D|=4816 ⇒ 4920]`, each pop costing an ≤ 8-cell flood fill. Native cost is
   negligible; **interpreted (Miri) cost is not**. Measured on the AC9 fixture
   (|D| = 3552, 3642 pops) under `MIRIFLAGS=-Zmiri-tree-borrows`: **8 m 33 s** with an
   **allocation-free** flood fill, and a per-call-allocating flood fill is **3.6×
   slower** (170 s vs 47 s at a 450-pop fixture) ⇒ ≈ **30 min**
   `[measured: reviewer's validated Rust port of this algorithm on this design's AC9
   fixture under MIRIFLAGS=-Zmiri-tree-borrows → 8m33s alloc-free; 170s vs 47s at 450
   pops for the allocating shape]`. Against the current gp-core Miri baseline of **140
   tests in 29.61 s** `[measured: CI run 30110922152 → gp-core Miri binary 140 tests in
   29.61 s, whole job 2m12s]`, one un-gated AC9 test would multiply that job by
   **17×–60×** on a CI runner slower than the measuring machine — the same wall-clock
   failure mode that forced the `gp-gen` crate-level carve-out (#134). Two consequences,
   both binding on subtasks 5–6: the flood fill **must allocate nothing per call**
   (§ *Predicates*), and the AC9 wide-corridor test **is** gated with a per-test cost
   carve-out (§ Test Design, § Risks). The smaller fixtures stay in the Miri run
   `[derived → subtask 6: MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace
   --exclude gp-gen]`.
5. **Topology (why the ridge is connected — the whole point).** Deleting a simple point
   preserves the number of foreground components and the number of background
   components, i.e. the (4,8)-homotopy type. Anchoring only *forbids* deletions, so the
   invariant survives. Hence for a corridor that is 4-connected with exactly one bounded
   hole — precisely what Ф4 certifies before `generate` accepts (`Issue::Disconnected`
   iff `component_count != 1`, `Issue::BadTopology` iff
   `bounded_complement_components != 1` `[measured: sed -n '94,107p'
   crates/gen/src/phase4.rs]`) — the resulting skeleton is **connected and carries
   exactly one cycle**. Measured on 15 single-hole ring fixtures (11×11 annulus, a
   41×41/band-14 ring, 12 staircase-jittered block rings at k=6/k=7 with |D| up to
   2895): **1 component, no 2×2 block, every one of them**
   `[measured: scratchpad reference impl thin5.py → comps=1, 2x2=False on 15/15]`.
6. **Thinness.** "No 2×2 block of skeleton cells" held on 15/15 fixtures (above). It is
   an **empirical property of thinning, not a theorem** — a pinwheel-attached 2×2 (four
   arms leaving diagonally opposite corners) makes all four cells non-simple and would
   survive. AC9 therefore asserts thinness **on its fixture**, and § Risks records the
   residual case and its graceful consequence. Do NOT write a doc sentence claiming a
   guarantee here.
7. **Neck property (constraint 6) — preserved and strengthened.** Connectivity is
   preserved by construction, so the skeleton contains a path between the two lobes of
   any constriction, i.e. it crosses every cut cross-section; at a 1-cell neck that
   cross-section *is* the neck cell, so the neck is on the skeleton. Measured: the
   in-tree neck fixture's skeleton is exactly `{(1,2),(2,2),(3,2),(4,2),(5,2)}` —
   identical to today's output, containing `(3,2)` and 4-connected across it
   `[measured: scratchpad reference impl → neck 7×5 NEW == OLD]`. The rustdoc keeps a
   neck claim, restated on the connectivity argument instead of the strict-max one.
   `phase4_defects.rs`'s `narrow_issues` reasoning is **unaffected**: it deliberately
   scans *all* `D` cells rather than the ridge, and that remains right regardless of
   which skeleton `medial_axis` returns (only its parenthetical rationale needs a
   reword — § Doc reconciliation).
8. **`docs/design.md` compatibility.** §D2 calls the medial axis "гребень
   distance-transform … **ветвящийся** геометрический объект, про *ширину*"
   `[measured: sed -n '72p' docs/design.md]` — a *branching* DT skeleton, which is
   exactly what anchored thinning returns. No `docs/design.md` change is needed or
   proposed.

### Rejected alternatives

- **Relax the strict test to `≥`.** Collapses the ridge to the whole corridor on any
  along-flow plateau — stated by the existing rustdoc and confirmed by the 6×6 result.
- **Zhang-Suen / Guo-Hall parallel thinning.** Both produce **8-connected** skeletons
  (diagonal single steps). Every downstream consumer walks 4-connected neighbours, so
  an 8-connected strand reads as a *disconnected point cloud* to `components`/`degree`/
  `walk_cycle` — it would reproduce the very defect being fixed.
- **No end-point protection (pure homotopic thinning).** Gives 0 leaves on rings and
  needs no Ф7 change at all, but collapses every simply-connected corridor to a **single
  cell** — the 5×3 band, the 4×3 band and the neck fixture all thin to one arbitrary
  cell, destroying the neck property and gutting three of the four exact-output tests
  `[measured: scratchpad reference impl mode='none' → 5×3 → {(3,1)}, 4×3 → {(2,1)},
  neck → {(5,2)}]`.
- **Unconditional end-point protection (protect every degree-1 cell).** Keeps arcs but
  leaves 3–10 spur tips per ring whose minimum pairwise gap is 40–47 cells — which makes
  `bridge_gaps` return `None` and the centerline empty again
  `[measured: scratchpad reference impl mode='all' → leaves 3–10, min leaf gap 40–47]`.
  Anchoring is what removes those artefact tips (0 leaves on the clean rings, 2–6 on
  jittered ones).
- **Anchor every DT local maximum (not just end points).** On a wide band the whole
  2-cell-wide `dt`-max strip is a non-strict local maximum, so the "skeleton" keeps a
  2-cell band ⇒ 2×2 blocks ⇒ AC9 thinness fails.
- **Options A2 / B / C from revision 2** (rebuild `racing_line` on `s_field`; in-`D` BFS
  bridging; defer Scope 7) — all closed by the owner's A1 decision.

### Ф7 follow-through (spec Scope 8) — bridge only a disconnected skeleton

**A1 alone does not fix the centerline.** `bridge_gaps` aborts with `None` whenever
≥ 2 leaves remain and the minimum non-adjacent leaf pair exceeds `MAX_BRIDGE_GAP = 6`
`[measured: sed -n '160,213p' crates/gen/src/phase7.rs → leaves.len() < 2 ⇒ Some;
dist > MAX_BRIDGE_GAP ⇒ None]`. The anchored skeleton of a jittered ring carries 2–6
genuine branch tips, typically tens of cells apart `[measured: scratchpad reference
impl thin5.py → leaves 2–6 on 12/12 jittered rings]` — so an unchanged `racing_line`
would still return `Centerline::default()` on many real corridors. This is a
**measured, load-bearing finding**, not a precaution.

Minimal fix inside Scope 8 ("minor `phase7.rs` adjustments … no wholesale redesign"):
**bridge only when the medial set is actually disconnected.** In `racing_line`
(`phase7.rs:507-524`), replace the unconditional bridge with

```rust
let bridged = if components(&medial).len() > 1 {
    bridge_gaps(d, medial)
} else {
    Some(medial)
};
let Some(bridged) = bridged else {
    return Centerline::default();
};
```

`?` is **not** usable here — `racing_line` returns `Centerline`, not `Option`/`Result`
`[measured: sed -n '507,524p' crates/gen/src/phase7.rs → fn racing_line(..) ->
Centerline, each stage uses `let Some(..) = .. else { return Centerline::default(); }`]`
— so the guard folds the non-bridging branch into `Some(..)` and keeps the surrounding
function's existing `let … else` fallback shape verbatim. Everything else —
`prune_spurs` → `walk_cycle` → `orient` → `resample` — is untouched, `MAX_BRIDGE_GAP`
keeps its value, and no `phase7` function is deleted, so every `bridge_gaps_*`,
`prune_spurs_*` and `walk_cycle_*` unit test keeps its exact semantics.

Measured outcome of the full pipeline on single-hole rings (thinning → `prune_spurs` →
`walk_cycle`, emulated faithfully from `phase7.rs`): the 2-core is **all-degree-2, one
component, and the walk closes over every core cell** on 15/15 fixtures — core sizes
32…214 `[measured: scratchpad reference impl thin3.py/thin5.py → maxdeg=2,
walk == |core| on 15/15]`.

*Rejected:* reordering to `prune → bridge → walk` — the 2-core is leafless by
definition, so `bridge_gaps` would become an unconditional no-op (dead in effect) while
still needing to exist for AC8's "no `phase7` test deleted"; the component guard keeps
bridging genuinely reachable for a disconnected input. *Rejected:* raising or removing
`MAX_BRIDGE_GAP` — it is not the binding constraint (owner-verified) and it would
regress `bridge_gaps_abandons_over_max_gap` `[measured: sed -n '46,53p'
crates/gen/src/phase7_tests.rs → expects is_none() for a 39-cell gap]`.

### Doc reconciliation (spec constraint 9)

| Surface | Current claim | Action |
|---|---|---|
| `distance.rs:1-9` module doc | "its **strict axis-wise ridge** ([`medial_axis`])" | Restate as the DT-ordered thinning skeleton. |
| `distance.rs:88-111` `medial_axis` rustdoc | strict-local-max definition; "strict inequality is load-bearing"; closing paragraph defers 2-cell thinning + corner bridging to Ф7 | Full rewrite: the thinning definition, the (4,8) convention, determinism, the topology guarantee, the neck claim restated on connectivity, and the honest thinness wording (no 2×2 *observed*, not *guaranteed*). Keep the first paragraph short (`pedantic::too_long_first_doc_paragraph`). |
| `phase7.rs:1-16` module doc | "`medial_axis` **deliberately leaves a thin but imperfect ridge cell set**… `racing_line` … bridge cross-component gaps → prune → walk" | Rewrite: the ridge now arrives connected and thin; bridging runs only for a disconnected set; the rest of the pipeline is unchanged. |
| `phase7.rs:23-28` `MAX_BRIDGE_GAP` doc | justified by "the annulus fixture's nearest cross-strip corner pair" | Reword to cite the hand-built 4-strip *unit-test* fixture (the annulus's real medial axis is no longer 4 strips). Constant value unchanged. |
| `phase7.rs:498-506` `racing_line` rustdoc | pipeline description | Add the conditional-bridge step. |
| `phase4_defects.rs:107-109` | "deliberately not restricted to `medial_axis`'s ridge, since **a neck is a DT valley a local-maximum ridge would miss**" | Reword the parenthetical: the `Narrow` scan covers all `D` cells independently of Ф7's skeleton. (The check itself is unaffected — Ф4 never calls `medial_axis` `[measured: sed -n '1,20p' crates/gen/src/phase4.rs; sed -n '95,120p' crates/gen/src/phase4_defects.rs → both mentions sit inside doc-comments; no call site]`.) |
| `phase4.rs:8`, `geom/mod.rs:13` | location/re-export listings only | **No change** — neither states a behavioural contract. |

The doc gate must stay green `[derived → subtask 7: RUSTDOCFLAGS="-D warnings" cargo
doc --no-deps --workspace]`.

---

## Decomposition

All subtasks change **code** (`*.rs`; subtask 3's `Cargo.toml` is committed). M = 8.
Subtasks 1–3 are committed; 4 is written; 5–8 are new.

| # | Task | Files | Depends on | Status |
|---|------|-------|------------|--------|
| 1 | `seed_budget`/`repair_budget` fields on `GenParams` | `crates/gen/src/lib.rs` | — | **DONE** `ed282c2` |
| 2 | `corridor_min_width` helper + unit test | `crates/gen/src/phase4_defects.rs` | — | **DONE** `047400c` |
| 3 | `thiserror = { workspace = true }` edge (one-line lock delta) | `crates/gen/Cargo.toml` | — | **DONE** `a2bb3cc` |
| 4 | Finalize + commit the orchestration: delete `scratch_diagnose_e2e` (`generate.rs:165-195`); drop the non-empty-centerline assertion from the e2e test (it returns in subtask 8); `lib.rs` `mod generate; pub use`. AC1/AC2/AC3/AC6 unit tests stay (AC2 = `zero_repair_budget_fails_promptly` + the three `should_run_oracle_*` tests — § Test Design). | `crates/gen/src/generate.rs`, `crates/gen/src/lib.rs` | 1,2,3 | **WRITTEN** — finalize + commit |
| 5 | **Replace `medial_axis` with DT-ordered anchored thinning** + private `const fn neighbors8`, `degree4`, `is_simple`, `is_anchored_endpoint`; rewrite its rustdoc + the `distance.rs` module doc. **Highest-difficulty subtask.** | `crates/core/src/geom/distance.rs` | 4 | **NEW** |
| 6 | `distance.rs` tests: re-derive + verify the 4 exact-output tests (3 expected sets are predicted unchanged; the annulus set changes — § Test Design); add the AC9 wide-corridor test **with its verbatim `#[cfg_attr(miri, ignore = "cost: …")]` per-test cost carve-out** (§ Test Design); confirm the 2 invariant tests pass unmodified; run the workspace Miri gate and report its wall time against the ~40 s expectation (§ Risks). | `crates/core/src/geom/distance.rs` | 5 | **NEW** |
| 7 | Ф7 follow-through: the `components(&medial).len() > 1` bridge guard in `racing_line`; `phase7.rs` module/`racing_line`/`MAX_BRIDGE_GAP` doc rewrites; `phase4_defects.rs` doc reword; rebuild `bridge_gaps_joins_annulus_corner_gaps_into_one_component` on a hand-built 4-strip set; confirm every other `phase7` test green. | `crates/gen/src/phase7.rs`, `crates/gen/src/phase7_tests.rs`, `crates/gen/src/phase4_defects.rs` | 5,6 | **NEW** |
| 8 | e2e tests in `generate.rs`: AC5(a) heavy `#[ignore]` full-invariant determinism test; AC5(b) cheap default-suite test (bs=6 seed=6, non-empty centerline); AC8 regression running a real `generate()` corridor through `racing_line`. | `crates/gen/src/generate.rs` | 7 | **NEW** |

Scope is 8 tasks (< 15) — no issue split needed.

## Handoff plan

Per `.claude/skills/task/SKILL.md` Step 8 + `.claude/skills/context-reset/SKILL.md`
§ Compaction recovery (re-entry), a `/context-reset` handoff is bound at the **start of
every** design-defined group, including the first. Subtasks 1–3 are committed; the live
work is subtasks 4–8 — all one change-type (**code**, `*.rs`).

- **Group A** — model `sonnet` (sonnet-5), effort `medium` (pinned in the `code-writer`
  frontmatter), 1M-token window, via the `code-writer` subagent — subtasks **4, 5, 6, 7,
  8** in that order. Change-type: **code** (`*.rs`), homogeneous. **Terminal group**
  (5 subtasks; within the `1..=10` range, ≤ the size cap of 10). Group-count is minimal
  — one homogeneous group cannot be reduced further, and rule (f) bounds splitting to
  the size cap / dependencies / change-type only, none of which forces a boundary here
  (1 group ≤ the default max of 4). This first-and-only live group is entered via, and
  completes Step 8 inside, its own `/context-reset` subagent. `design`, `design-review`
  and `self-review` stay on Opus regardless of this marker.
- **Within-group emphasis (not a group boundary):** subtask 5 is the highest-difficulty
  item of the task. The implementor must give it its own commit + full gate cycle
  (`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test -p gp-core`) before starting subtask 6, and must **STOP and report**
  rather than re-pin expectations if subtask 6's re-derived sets diverge from the
  predictions in § Test Design.

## Risks

- **A prediction in § Test Design turns out wrong on the real implementation.** The
  expected sets come from a validated reference port, not from the Rust. Mitigation:
  subtask 6 re-derives every set by running; any divergence is a **STOP-and-report**,
  never a silent re-pin — a divergence means the implementation is not the algorithm
  specified here `[measured: scratchpad reference impl reproduces all three in-tree
  expected sets exactly]` `[derived → subtask 6: cargo test -p gp-core distance]`.
- **A 2×2 block survives thinning on some real corridor** (the pinwheel case, § A1
  point 6). It would add a topologically trivial graph cycle, which can leave a degree-3
  cell in the 2-core and make `walk_cycle` fail → `Centerline::default()` (graceful, no
  panic — `racing_line`'s documented fallback). Not observed on 15/15 fixtures.
  Mitigation: subtask 8's AC8/AC5(b) tests run a **real** `generate()` corridor; if the
  centerline comes back empty, report the corridor rather than widening scope
  `[measured: scratchpad reference impl → 2x2=False on 15/15]` `[derived → subtask 8:
  cargo test -p gp-gen --release <e2e>]`.
- **`walk_cycle` still fails after A1 + the bridge guard.** Permitted remedy is bounded
  by Scope 8: a *minor* `phase7.rs` adjustment only (e.g. the anchor choice or the
  candidate ordering in `walk_cycle`). A wholesale Ф7 redesign, a `medial_axis` rewrite,
  or an `#[ignore]`d AC8 test are **out of scope** — surface to the orchestrator instead
  `[derived → subtask 8: the AC8 regression test]`.
- **Miri wall-clock — the AC9 fixture is a real cost problem and is gated.** `gp-core`
  **is** inside the Miri gate — only `gp-gen` rides the #134 carve-out
  `[measured: sed -n '185,193p' .github/workflows/ci.yml → "cargo miri test --workspace
  --exclude gp-gen"]`. The algorithm is pure integer/`BTreeSet` work, but interpreted
  cost is ≈ 0.1 s **per pop**: the AC9 fixture's 3642 pops measure **8 m 33 s**
  allocation-free against a **29.61 s / 140-test** gp-core Miri baseline
  `[measured: reviewer's Rust port under MIRIFLAGS=-Zmiri-tree-borrows → 8m33s; CI run
  30110922152 → 140 tests in 29.61 s]`. Mitigation: the AC9 test carries the per-test
  cost carve-out spelled out in § Test Design (**per-test, never crate-level**), and
  subtask 6 runs the **workspace** Miri command (never a narrower `-p`) and reports the
  job's wall time `[derived → subtask 6: MIRIFLAGS=-Zmiri-tree-borrows cargo miri test
  --workspace --exclude gp-gen]`.
- **The 11×11 annulus test's Miri cost rises from ~0 s to ~10 s.** It stays in the Miri
  run (101 pops — no carve-out), but it is no longer free: extrapolating the measured
  450-pop = 47 s point gives ≈ **10 s** `[measured: reviewer's Rust port under
  MIRIFLAGS=-Zmiri-tree-borrows → 47 s at 450 pops; 101 pops on the 11×11 annulus]`.
  This is the concrete number subtask 6's "report the job's wall time" step compares
  against: expect the gp-core Miri binary to land near **40 s** (29.61 s baseline +
  ~10 s), not near 30 s. A materially larger figure means a fixture escaped the carve-out
  or the flood fill allocates — surface it, do not widen the carve-out
  `[derived → subtask 6: MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace
  --exclude gp-gen]`.
- **AC5(b)'s cheap e2e is measured in *release*, but CI runs `cargo test` in debug.**
  The probe figure (bs=6 seed=6, `seed_budget=1`, ~3 s release) does not bound the debug
  wall time. Mitigation: subtask 8 **measures** the debug-mode runtime of the cheap test
  and records it in the progress file; if it exceeds ~120 s, surface to the orchestrator
  for an explicit decision — do **not** silently `#[ignore]` it (AC5(b) requires it in
  every CI run) `[derived → subtask 8: time cargo test -p gp-gen <cheap e2e>]`.
- **`corridor_min_width(d) ≥ n` is NOT guaranteed by construction** (AC4/AC5) — the
  `Narrow` gate only fires on a DT-consistent sub-`n` neck, so a DT-inconsistent
  staircase neck escapes it yet lowers the plain geometric minimum. An AC5 failure is a
  **real** signal — align `corridor_min_width` with the `Narrow` metric or prove the
  neck unreachable; never silence the assertion `[measured: sed -n '52,67p'
  crates/gen/src/phase4_defects.rs → emits iff w < n && (w == 2·dt−1 || w == 2·dt)]`
  `[derived → subtask 8: the AC5 `width_min >= min_width()` assertion]`.
- **Subtask 4 must not commit the failing assertion.** The written e2e test asserts a
  non-empty centerline, which fails until subtask 5–7 land; committing it reds CI.
  Mitigation: subtask 4's commit drops that one assertion and subtask 8 reinstates it in
  the AC5(b)/AC8 tests `[derived → subtask 4: cargo test -p gp-gen after the scratch-test
  removal and assertion move]`.
- **Determinism (AC5 twice-identical)** relies on order-deterministic phases; neither
  the orchestration nor the new `medial_axis` adds a non-deterministic source (single
  seeded RNG; `BTreeSet`-ordered integer control flow). A twice-run Debug-string
  mismatch is a phase bug — surface it, don't patch a phase `[derived → subtask 8:
  two-run `format!("{:?}", …)` equality]`.
- **Zero production panics (AC7).** Every conversion stays total
  (`i32::try_from(..).unwrap_or(i32::MAX)`, `.min().unwrap_or(0)`); the new gp-core code
  adds no `unwrap`/`expect`/index — `ai-docs/panic-index.md` must stay **empty**
  `[derived → subtask 5/6: cargo clippy --workspace --all-targets -- -D warnings +
  the gp-core panic-index discipline]`.

## Test Design

`gp-core` tests live in `crates/core/src/geom/distance.rs`'s `#[cfg(test)] mod tests`
(which already imports `component_count` `[measured: sed -n '133,138p'
crates/core/src/geom/distance.rs]`); `gp-gen` tests in `crates/gen/src/generate.rs` and
`crates/gen/src/phase7_tests.rs`. Exactly **one** `#[cfg_attr(miri, ignore)]` is added —
on the new AC9 wide-corridor test, as a **cost** carve-out (verbatim attribute + full
justification below). Every other gp-core test stays in the Miri run; `gp-gen` is
Miri-`--exclude`d already.

### Subtask 4 — AC2 (oracle stall routing + reseed condition)

AC2 has two halves; both are discharged here so neither is left silent.

- **Reseed ONLY on `RepairOutcome::Failed`** — covered by the existing
  `zero_repair_budget_fails_promptly` (`generate.rs:249`), which drives
  `repair_budget = 0` with `seed_budget = 8` and asserts
  `Err(GenerationError::SeedBudgetExhausted)`: reaching seed exhaustion proves the outer
  loop advanced a seed per repair-loop exit and never returned early or looped forever
  `[measured: sed -n '248,255p' crates/gen/src/generate.rs → params(1, 8, 0) ⇒
  Err(SeedBudgetExhausted)]`. The complementary `RepairOutcome::Repaired ⇒ no reseed`
  half is structural — `Repaired` assigns `d = nd` and continues the *inner* loop, and
  only `Failed` `break`s to the next seed `[measured: sed -n '135,136p'
  crates/gen/src/generate.rs → Repaired { d: nd, .. } => d = nd, Failed => break]`.
- **`NotLappable.stall_walls` → `RepairContext.stall_walls`** — covered **by
  construction, with NO new test**, and this is a deliberate call, not an oversight. A
  direct test would need a corridor that is simultaneously **static-clean** (so
  `should_run_oracle`'s `static_issues.is_empty()` holds `[measured: sed -n '32,34p'
  crates/gen/src/generate.rs → static_issues.is_empty() && liveness]`), **V=1-live**
  (same gate), and **`NotLappable`** — and every in-tree `NotLappable` fixture is a
  deliberately *broken* corridor that fails one of the first two: a severed ring
  (`ring_corridor()` with `d.set(Point::new(4, 2), false)`), a `no_crossing_corridor`,
  or an empty `D` `[measured: rg -U -n 'NotLappable' --type rust → 4 files; the
  producing fixtures are phase5b.rs:827/859 broken ring, :896 no_crossing_corridor, :930
  empty D, phase6.rs:292 broken_ring_diagnostic (same severed ring)]`. Worse for
  testability, `phase5_full_oracle`'s **first** `v_ceil` iteration is `v_ceil = 1`
  `[measured: sed -n '366,385p' crates/gen/src/phase5b.rs → let mut v_ceil: i32 = 1;
  loop { … fastest_lap_through_live(.., v_ceil) … None ⇒ NotLappable }]`, i.e. the same
  V=1-lap-existence property `oracle_liveness_v1` just gated on — so the conjunction is
  at minimum rare, and possibly unreachable. **That last is a plausibility argument
  across two independent implementations, NOT a proof**, so it is not written into any
  doc or assertion. Hand-building the triple is a generator-search problem outside this
  task's scope. What *is* asserted instead: the routing is a single straight-line move
  with no branch between producer and consumer —
  `OracleResult::NotLappable { stall_walls }` binds `oracle_stall = Some(stall_walls)`
  and `ctx.stall_walls = oracle_stall.as_deref()` with nothing in between
  `[measured: sed -n '114,132p' crates/gen/src/generate.rs → NotLappable { stall_walls }
  => oracle_stall = Some(stall_walls); … stall_walls: oracle_stall.as_deref()]`, and the
  `should_run_oracle` gate that guards it *is* directly unit-tested by
  `should_run_oracle_declines_on_outstanding_static_issue` / `_declines_on_dead_liveness`
  / `_runs_when_both_cheap_checks_are_clean` (`generate.rs:200/208/213`). If the
  implementor finds a cheap way to construct the triple, adding the test is welcome —
  but its absence is **not** an AC2 gap to work around by relaxing the assertion
  `[derived → subtask 4: cargo test -p gp-gen generate]`.

### Subtask 5/6 — `medial_axis` (AC9)

**Predicted exact outputs** (re-derive by running; STOP and report on divergence):

| Test (`distance.rs`) | Fixture | Current expected | **Predicted new** |
|---|---|---|---|
| `medial_axis_is_thin_centerline_on_straight_band` (`:183`) | 5×3 filled | `{(1,1),(2,1),(3,1)}` | **unchanged** — only the doc-comment rationale is rewritten (thinning, not strict max) |
| `medial_axis_even_width_band_is_two_cell` (`:267`) | 4×3 filled | `{(1,1),(2,1)}` | **unchanged** — rename/re-comment: it is the final 1-cell-thin skeleton, no longer "a 2-cell band Ф7 thins" |
| `medial_axis_includes_neck_and_is_connected_across_it` (`:198`) | 7×5 pinched at `x=3` | asserts `(2,2)`,`(3,2)`,`(4,2)` ∈ set | **passes unmodified**; strengthen to the exact set `{(1,2),(2,2),(3,2),(4,2),(5,2)}` |
| `medial_axis_forms_four_connected_strips_on_annulus` (`:222`) | 11×11 minus a centred 5×5 | 20 cells in 4 strips | **changes** to the 32-cell single closed loop below; rename to `medial_axis_on_annulus_is_one_closed_thin_loop` |

`[measured: scratchpad reference impl (validated against the three in-tree expected
sets) → 5×3 NEW==OLD, 4×3 NEW==OLD, neck NEW=={(1,2)..(5,2)}, annulus NEW = the 32-cell
loop]`

Predicted annulus set (32 cells, one 4-connected closed loop, all degree 2, no 2×2):

```
(1,2) (1,3) (1,4) (1,5) (1,6) (1,7) (1,8)
(2,1) (2,2) (2,8) (2,9)
(3,1) (4,1) (5,1) (6,1) (7,1) (8,1)
(3,9) (4,9) (5,9) (6,9) (7,9) (8,9)
(9,1) (9,2) (9,3) (9,4) (9,5) (9,6) (9,7) (9,8) (9,9)
```

The two corners cut at `(2,2)`/`(2,8)` while the east corners stay square — a
deterministic consequence of the pinned `(dt, Point)` deletion order, not a bug; say so
in the test comment. Replace the old "top strip is one connected run" sub-assertion with
`component_count(&corridor_of(medial)) == 1`.

**New AC9 test — `medial_axis_thins_a_wide_ring_to_one_connected_loop`.** Fixture: a
**61×61 filled box minus a centred 13×13 hole** (`Corridor::filled(Point::new(0,0), 61,
61)`, then `set(false)` over `x,y ∈ 24..37`; band 24 cells, `dt` peak **16** — inside
the 14–21 range the owner's probe measured on real corridors; |D| = 3552). Under the old
strict test this fixture shatters into **32 cells / 32 singleton components**; the new
one returns **146 cells in 1 component, no 2×2 block, 0 leaves**
`[measured: scratchpad reference impl → outer 61 hole 13: OLD=32c/32comp,
NEW=146c/1comp, 2x2=False, leaves=0, pops=3642]`. Assertions (structural, not an exact
146-cell set):
- `component_count` over a `Corridor` built on `dt.rect()` with the medial cells set == **1**;
- no 2×2 block: for every `p`, not all of `p+x̂`, `p+ŷ`, `p+x̂+ŷ` are members;
- every medial cell is drivable (`dt.at(p) > 0`);
- the set is non-empty and every cell has 4-degree ≥ 1.
Document the fixture's provenance (the measured old-vs-new numbers) in the test comment
so a future reader can see what it regression-guards.

**This test carries a per-test Miri cost carve-out** — verbatim, directly above `#[test]`:

```rust
#[cfg_attr(miri, ignore = "cost: 3642 thinning pops over 3552 cells ≈ 8.5 min under Tree Borrows; pure-integer, no UB signal the small distance.rs fixtures don't already cover")]
```

Why this is correct and sufficient:

- **Sanctioned category.** AGENTS.md § Rust Test Conventions gates a test that "**aborts
  (or costs)** under Miri" — explicitly including a test that "**is a
  zero-production-UB-signal cost test**". This is the cost arm, not the abort arm
  `[measured: grep -n "zero-production-UB-signal" AGENTS.md → § Rust Test Conventions
  Miri-gate bullet]`. Justified by **cost, not correctness**, exactly like the `gp-gen`
  #134 carve-out.
- **Per-test, never crate-level.** `gp-core` keeps every other test in the Miri run; a
  `--exclude gp-core` would also drop the 140 Miri-clean gp-core tests and is
  **forbidden** here.
- **The reason string describes THIS test's own cause** (3642 pops over its own 3552
  cells), per the AGENTS.md rule that a reason must never borrow a sibling's — a wrong
  reason is a false justification for a different failure.
- **AC9 is still satisfied as written.** AC9 requires `MIRIFLAGS=-Zmiri-tree-borrows
  cargo miri test --workspace --exclude gp-gen` to be **green**; a `cfg_attr(miri, …)`
  gate keeps it green *and* leaves the test running in the ordinary `cargo test` suite,
  where AC9's structural assertions (1 component, no 2×2) are actually discharged
  `[measured: grep -n "^| AC9" ai-docs/plans/2026-07-24-gp-gen-generate-pipeline.spec.md
  → "… and MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace --exclude gp-gen is
  green"]`. It is **not** a plain `#[ignore]`, so the spec's "no such test may be deleted
  or `#[ignore]`d" clause — which in any case binds the four *exact-output* tests, none
  of which is gated — is untouched `[measured: sed -n '119p'
  ai-docs/plans/2026-07-24-gp-gen-generate-pipeline.spec.md]`. **No spec amendment is
  needed.**
- **Nothing else is gated.** The four exact-output tests, the two invariant tests and
  the predicate unit tests all stay in the Miri run (the 11×11 annulus is the most
  expensive of them at ≈ 10 s — § Risks).

**Unmodified invariants (must stay green, untouched):**
`empty_corridor_has_zero_dt_and_empty_medial_axis` (`:284`) and
`compute_and_medial_axis_are_deterministic` (`:307`).

**Predicate unit tests** (small, hand-checkable, for `is_simple`/`is_anchored_endpoint`):
an isolated cell is not simple; an interior cell of a filled block is not simple (its
`B` is empty); a straight-line interior cell is not simple (two `A` components); an L
corner cell of a 2×2 block is simple; a 1-cell-wide finger tip is an anchored end point;
a low-`dt` degree-1 corner artefact is **not** anchored.

### Subtask 7 — Ф7 (AC8 half 1)

- `bridge_gaps_joins_annulus_corner_gaps_into_one_component` (`phase7_tests.rs:29`)
  currently derives its input from `medial_axis` and asserts `components(&medial).len() > 1`
  — that premise dies with A1 (the annulus medial is now one component). Rebuild it on a
  **hand-built** 4-strip set (the old 20-cell expectation: `x ∈ 3..8` at `y ∈ {1,9}`,
  `y ∈ 3..8` at `x ∈ {1,9}`), keeping the test's purpose and name. Sanctioned by spec
  Scope 8 / AC8; no `phase7` test is deleted or `#[ignore]`d.
- Every other `phase7` test is expected to stay green **unmodified**, including
  `racing_line_orients_resamples_and_tangents_a_clean_ring` and
  `ac2_wraps_around_the_closed_loop` (the 4×4 border ring's medial becomes the full
  12-cell ring instead of today's 8 cells — both tests assert properties, not counts
  `[measured: sed -n '274,301p;399,415p' crates/gen/src/phase7_tests.rs]`),
  `ac7_annulus_closes_monotone_and_race_dir_aligned`,
  `ac1_prunes_spur_to_a_single_non_branching_loop` (the `trap_ring` spur is a 1-wide
  finger at `x=6, y∈1..=5`; it survives thinning as a tree branch and is removed by
  `prune_spurs` `[measured: sed -n '203,214p' crates/gen/src/testfix.rs]`), and
  `ac6_racing_line_is_deterministic`. Run `cargo test -p gp-gen phase7`.

### Subtask 8 — e2e (AC4, AC5, AC8 half 2)

- **AC5(b) cheap, default suite:** `generate(params(bs=6, seed=6, seed_budget=1,
  repair_budget=8))` → `Ok(a)`; assert acceptance, two-run Debug-string determinism, and
  `!a.centerline.samples.is_empty()`. Record the debug-mode wall time (§ Risks).
- **AC5(a) heavy, `#[ignore]`:** a larger-budget config, run twice → identical Debug
  strings; full artifact invariants — exactly one bounded complement hole; S/F chord
  width ≥ `m`; `oracle_liveness_v1(&a.corridor, &a.start_grid, &a.sf, a.race_dir)`;
  `a.s_field.rect == a.corridor.rect()` and `scalar_at(cell) == Some(0)` for every
  `gate.forward_face()` cell; `a.width_min >= params.min_width()`; distinct start-grid
  positions; and a non-empty, well-formed centerline.
- **AC4 well-formedness helper** shared by both: `samples` non-empty,
  `samples[0].s.abs() < f32::EPSILON`, strictly increasing `s`, the loop wraps
  (`at(length) ≈ at(0)`), and `cl.length > 0.0` with `samples.len() >= 4` (the minimum
  4-connected grid cycle). Do **not** pin a tighter corridor-relative bound unless the
  implementer measures the accepted corridor's actual loop length first and records it.
- **AC8 regression:** take the accepted artifact's `corridor` / `sf.gate` / `race_dir`,
  call `racing_line(&a.corridor, &a.sf.gate, a.race_dir)` directly, and assert a
  non-empty, well-formed centerline — i.e. a **real `generate()`-produced corridor**,
  not the hand-built annulus.

## Open questions

- **Q-Ф7 — CLOSED (product-owner decision, spec amendment 2).** Resolution: **Option
  A1 — fix `gp_core::geom::medial_axis` at the source**; `racing_line` keeps its
  pipeline. The revision-2 alternatives (A2 `s_field` rebuild, B in-`D` BFS bridging,
  C defer) are rejected and recorded in § Rejected alternatives. The one bounded Ф7
  change A1 *demands* — bridging only a disconnected medial set — is specified in
  § Ф7 follow-through and is within spec Scope 8.
- **Default budget magnitudes** (spec § Open questions): no crate-level `Default` for
  `GenParams`; tests inject explicit budgets. Not design-blocking — none hard-coded.
- **Failure diagnostics on `GenerationError`:** deferred; the current contract is a bare
  sentinel.
