# Design: gp-gen Ф2 — rasterize coarse ring to points `D` with width taper

**Issue:** #25
**Date:** 2026-07-22

## Approach

Ф2 turns Ф1's coarse-block `CoarseSkeleton { ring, hole, dir }` into the fine
lattice corridor `D` (`gp_core::geom::Corridor`). A standalone
`pub fn phase2_rasterize(skel: &CoarseSkeleton, k: i32, n: i32) -> Corridor` in a
new `crates/gen/src/phase2.rs`, re-exported from `lib.rs`, mirroring Ф1's
signature/re-export pattern (`pub use phase1::*;` at `crates/gen/src/lib.rs:15`
`[measured: rg -n "pub use phase1" crates/gen/src/lib.rs]`). Ф2 consumes
`skel.ring` and `skel.hole` only — **not** `skel.dir` (traversal orientation is a
Ф3 concern). No RNG (AC6). Integer-only, total, panic-free — mirroring gp-core's
zero-production-panic posture and Ф1's `saturating_*` / `try_from(..).unwrap_or(..)`
integer-safety discipline (#48).

### Pipeline (three deterministic stages)

**Stage 1 — baseline `k×k` expansion (`D0`).** Map each coarse ring cell
`(cx, cy)` → the fine block `[cx·k, cx·k+k) × [cy·k, cy·k+k)`; the union of all
blocks is `D0`. Build the backing `Corridor` over the bounding box of every fine
point, padded by a small constant `BBOX_PAD = 1` cell on each side (ordinary
bounding-box padding so every `D0` boundary cell has an in-box `¬D` neighbor for
`walls_from_boundary` / complement-flood, exactly as Ф1's `corridor_from_cells(.., 1)`
does). The taper needs **no** taper-specific margin: `max(env(x)) = max(top(x))`
is the wide-side extent, already inside `D0`'s bounding box, so the additive fill
never grows beyond it (Note 4 — the earlier `M = WIDEN_MAX·k` "taper headroom" was
over-provisioned and mis-rationalized). `WIDEN_MAX = 3` is Ф1's per-side widen
ceiling `[measured: rg -n "WIDEN_MAX" crates/gen/src/phase1.rs -> const WIDEN_MAX:
u32 = 3]`, cited only for the jog-depth `Δ = w·k` in Stage 2.
Also compute the **expanded hole mask** `H` = the union of `k×k` blocks of
`skel.hole`, used by Stage 2 to protect the infield. Satisfies **AC1** (each ring
cell → a solid `k×k` drivable block) and **AC2** (a ring ≥2 coarse cells thick →
≥`2k` fine points, by union of adjacent blocks) *by construction* — no extra work.

**Stage 2 — outer-wall taper.** The only abrupt wall steps Ф1 can emit are its
*outward* `widen` jogs: a widened side (thickness `(1+w)·k`, `w ∈ 1..=WIDEN_MAX`)
abutting a nominal side (`k`) produces a concave outfield notch of depth
`Δ = w·k ∈ {k, 2k, 3k}` `[derived from phase1.rs widen(): amount ∈ 0..=WIDEN_MAX
extends the extremal run outward → gate: AC4 fixture test]`. All such jogs face the
**outfield** (widening is strictly away from the hole), so tapering is confined to
outfield-facing walls; hole-facing walls (the normal annulus turns, whose concave
inner corner faces `H`) are never touched.

Taper is a **two-substep** operation — an additive envelope fill (2a) followed by
an additive pocket absorption (2b) that repairs the topology hazard 2a can create.
Both substeps only add outfield cells, never remove, and never fill a cell of `H`.

**Stage 2a — 1-Lipschitz envelope fill.** Make each outfield wall **1-Lipschitz**
(advances ≤1 fine point per column). Four directional passes (outward = East, West,
North, South, in `Side::iter()` order for determinism), each a per-scanline
**1-Lipschitz upper envelope** by two linear passes:

- For outward direction `+y` (North), for each column `x` let `top(x)` = the
  greatest `y` with `(x,y) ∈ D` and the cell beyond (`(x, y+1)`) in the
  **outfield** (`∉ D`, `∉ H`); columns whose outward neighbor is in `H` face the
  hole and are **skipped**.
- Forward pass `env[x] = max(top(x), env[x-1] − 1)`, backward pass
  `env[x] = max(env[x], env[x+1] − 1)` → the minimal 1-Lipschitz field `≥ top`.
- Fill every column `x` outward up to `env[x]` (cells `∉ H` only; if `H` blocks,
  the ramp stops — additive and hole-safe either way).

On a `k→2k` jog this yields exactly a 45° ramp of horizontal extent `k`, capped by
the nominal wall so it **does not flatten** the whole arm (the nominal columns far
from the jog stay at `top = k−1`). Satisfies **AC4** (≤1 pt/column; a `Δ=w·k`
change spans `w·k` columns).

**The 2a topology hazard (confirmed defect).** Applied to real Ф1 geometry, 2a's
additive fill can **bridge a narrow genuine outfield gap between two corridor arms**
and seal a strip of previously-unbounded outfield into a spurious *second* bounded
complement component — a fake extra hole. Deterministic witness:
`phase1_coarse_ring(3, &mut rng(0))`, `k=6, n=3` → `D0` is clean
(`bounded_complement_components == 1`) but post-2a `== 2` (a 12-cell pocket at bbox
`x=[51,53] y=[60,65]`) `[measured: orchestrator repro; confirmed independent of
pass order — the defect survives when all 4 passes read extents from a pristine
`D0`]`. `fixture_jog`'s single simple jog never exercises this, so 2a alone is
**not** topology-safe. Hence 2b.

**Stage 2b — pocket absorption (topology-safe closure).** After 2a, enumerate the
bounded 4-connected components of the complement `¬D` (an internal complement-flood
over the padded bbox, mirroring Ф1's `fill_holes`, `crates/gen/src/phase1.rs:402`).
Exactly one legitimate bounded component exists — `H`, the infield hole (2a never
fills `H`, so it survives intact). **Absorb** (set drivable) every *other* bounded
complement component — i.e. every bounded `¬D` component that does **not** intersect
`H`. These are precisely 2a's spurious pockets. Absorption is still **additive**
(only adds cells) and never touches `H`.

Satisfies **AC5** (after 2a every outfield-facing wall is 1-Lipschitz; 2b only fills
enclosed pockets, which *removes* concavity and cannot introduce a `>1`-pt/column
outfield step), verified by Stage 3's **test-only** structural + supercover
post-check — not a production self-check.

**Stage 3 — no narrow carve; correctness by construction (production has no
self-check).** Ф2 is a pure `-> Corridor` fn with **no error channel** — unlike
Ф1's step-6, which is a production runtime check driving retry/fallback. Ф2
therefore carries **no production `assert!` / `panic!` / self-check**: topology is
guaranteed *by construction* by the Stage-2 mechanism itself — **2b is part of the
construction, not a check.** The argument (corrected — the earlier "2a additive fill
alone can never seal a pocket" claim was **false**, see the 2a hazard above):

1. **`component_count(D) == 1`.** `D0` is one connected component (Ф1 ring). 2a adds
   only cells vertically chained to a `D0` wall cell (`top(x)`); 2b adds only cells
   of a pocket fully enclosed by `D`. Neither can create a detached component nor
   split an existing one — adding cells never disconnects.
2. **`bounded_complement_components(D) == 1`.** After 2a the bounded components of
   `¬D` are `H` plus zero-or-more spurious pockets. 2b absorbs *every* bounded
   component disjoint from `H`, leaving exactly `H` (untouched by both substeps).
3. **Additive + hole-safe.** Every added cell is outfield (`∉ H`); nothing is
   removed. So AC1/AC2/AC3 (widths only grow) and the single hole `H` survive.

The topology *certification* (`component_count(D) == 1`,
`bounded_complement_components(D) == 1`), the supercover concave-cut post-check
(AC5), and the cross-section-width measurement (AC3/AC7) are **all test-only** —
they verify the by-construction argument, they do not gate production output.

**Narrow carve: none in round-1 (see Key Decisions).**

### Key Decisions

| Decision | Rationale |
|---|---|
| **Taper mechanism = 1-Lipschitz upper-envelope of outfield walls, 4 directional passes, additive.** | Direction-agnostic, `O(cells)`, non-iterative (no flattening — the nominal wall caps the envelope), provably additive (env ≥ top) and hole-safe (fill skips `H`). Rejected: iterative concave-corner stencil fill (flattens the whole arm because each fill spawns a new concave corner); coarse-jog analysis (Ф1's `widen` extends only *extremal* cells, so the post-widen outer boundary is irregular — a dense-grid pass is more robust than pairing coarse collinear runs). |
| **Topology-safe taper = envelope fill (2a) + pocket absorption (2b).** (Amendment — repairs a confirmed defect: 2a alone can bridge a narrow inter-arm outfield gap and seal a spurious 2nd bounded complement component; witness `rng(0)`, `k=6,n=3`.) 2b = enumerate bounded `¬D` components, absorb (fill drivable) every one disjoint from `H`. | **Chosen: (b) absorb-the-pocket.** Provably restores one-hole (after 2b the only bounded `¬D` component is `H`), stays additive (only adds outfield cells, never touches `H`), deterministic (a fixed-order complement flood, no RNG), and `O(cells)` (one complement flood + one H-membership scan + fill — no per-cell recompute). Reuses Ф1's in-crate `fill_holes` flood pattern (`phase1.rs:402`). **Rejected (a) per-cell topology-guarded fill:** naive `O(cells²)`, and a partial guard that stops mid-ramp would leave a `>1`-pt step → violate AC4. **Rejected (c) capped/gap-aware ramp extent:** cannot simultaneously fully taper a legit `Δ=3k` jog (needs a `3k`-column ramp — `18` cols at `k=6`) and never bridge a `<3k`-cell gap; the two requirements conflict, so no single cap satisfies AC4 *and* no-bridge. |
| **Taper slope = 1 fine point / column (45°); `MIN_TAPER_RUN = 3` recorded as the "several" floor.** (Open-question resolution.) | AC4's hard invariant *is* "≤1 point per column" — 45° meets it exactly and is the standard supercover-safe wall. A `Δ = w·k` jog then spans `w·k ≥ k` columns; for playable `k ≥ 3` this is ≥ `MIN_TAPER_RUN` ("several"). Gentler empirical slopes (1 pt / `t` cols, `t>1`) risk over-extending across the outfield and merging arms, and are deferred to oracle/playtest tuning (spec Deferred + design §"width profile"). |
| **No narrow carve in round-1.** (Open-question resolution — forced-pinch mechanics.) | Uniform `k×k` expansion gives every cross-section ≥ `k ≥ n`, and every S-hairpin cross-gabarit ≥ `3k ≥ 2n+1` with each arm ≥ `k ≥ n` (`k ≥ n` per Key Decisions; `n = ⌈m/2⌉` so `3n ≥ 2n+1` for `n ≥ 1`). The forced-pinch predicate ("a cross-section whose available gabarit forces an arm below `k`") is therefore **vacuously false on all Ф1 output**, so no carve fires and AC3's floor holds *by construction*. Writing a carve path (a *removal* — hole-facing, needing the topology-guarded machinery Ф6 owns) that Ф1 can never trigger is speculative (YAGNI) and out of scope. Ф2 asserts the vacuity (min cross-section == `k`, never < `n`). Revisit trigger recorded in Open questions: a future Ф1 that emits sub-`k` arms. |
| **`n` retained in the signature (spec-fixed) but referenced only in `debug_assert!(n <= k)`.** | Round-1 production geometry never narrows, so `n` guards the `k ≥ n` invariant and documents the width floor the (future) carve will consume. |
| **Ф2 ignores `skel.dir`.** | Traversal orientation is Ф3 (start/finish). Ф2 emits only `D`. |
| **Determinism: pure fn of `(skel, k, n)`, fixed `Side::iter()` pass order, no RNG.** | AC6 — byte-identical `D` for identical input; pinned by an exact snapshot. `[derived → AC6 snapshot test]` |

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Scaffold `phase2.rs`; `phase2_rasterize` signature + `debug_assert!(n <= k)`; Stage-1 baseline `k×k` expansion (`D0`) with `BBOX_PAD = 1` bbox padding; expanded-hole mask `H`; block-origin + corridor-sizing helpers (`saturating_*` / `try_from`, integer-safe per #48); `mod phase2; pub use phase2::*;` in `lib.rs`. Tests: AC1 (each ring cell → solid `k×k`), AC2-baseline (adjacent-block union ≥`2k`), AC6-baseline determinism. | `crates/gen/src/phase2.rs` (new), `crates/gen/src/lib.rs` | — |
| 2a | Stage-2a envelope fill (**committed**, `180c203`): `TAPER_SLOPE`/`MIN_TAPER_RUN` consts; per-scanline 1-Lipschitz upper-envelope helper (2 linear passes); 4 directional outfield passes (`Side::iter()` order) with the `H`-mask outfield-facing restriction; additive fill. Tests: AC4 (outer wall ≤1 pt/col; `Δ` spread ≥ `MIN_TAPER_RUN` cols on `fixture_jog`), taper determinism, additive (`D0 ⊆ D`), touches no `H` cell. | `crates/gen/src/phase2.rs` | 1 |
| 2b | **Stage-2b pocket absorption (NEW — amendment).** Internal complement-flood helper enumerating bounded `¬D` components (mirroring `fill_holes`, `phase1.rs:402`); absorb (set drivable) every bounded component disjoint from `H`; call after 2a inside `phase2_rasterize`. Integer-only, no panic, no RNG. Tests: on the `rng(0) k=6 n=3` witness, post-2b `bounded_complement_components == 1`; absorption is additive (`D_2a ⊆ D_2b`); `H` untouched. | `crates/gen/src/phase2.rs` | 2a |
| 3 | Stage-3 certification + test scaffolding (**tests drafted but uncommitted & currently RED**): post-taper `component_count == 1` + `bounded_complement_components == 1` assertions; internal test-only `cross_section_width` scan helper; AC5 supercover concave-cut post-check; AC3 (≥`n`) + narrow-carve **vacuity** assertion (min cross-section == `k`); AC7 profile (`n`/`k`/`≥2k`) on the fixture; multi-seed property test (Ф1 seeds → Ф2 → all ACs, **the primary `bounded_complement_components==1` regression gate for this defect**); exact AC6 snapshot (re-minted post-2b + cross-checked before freezing). | `crates/gen/src/phase2.rs` | 2b |

`M = 4` (all code, `crates/gen/src/*.rs`). **Amendment note:** subtasks 1 and 2a are
already committed (`c46b1f2` baseline, `180c203` taper). The amendment adds **2b** —
the committed 2a taper code is *not rewritten*; `phase2_rasterize` gains a call to
the new 2b absorption helper after the 2a passes.

**Subtask-3 state (Note 2 — drafted-and-RED, not future work).** Subtask 3's tests
are physically present in the working tree but **uncommitted and currently red** —
`property_sweep`, `ac5_part_b`, and `ac6_snapshot`. The AC6 snapshot literals are a
**placeholder** (`width = 0`, `origin = (-19, -1)`); a live post-2b mint gives
`origin = (-37, -13)` `[measured: cargo test -p gp-gen --lib ac6_snapshot →
left=(-37,-13)]`, so the placeholder is stale and must be re-minted. **Completing
subtask 3 therefore = (i) implement 2b, (ii) fix the Part-B H-clearance scoping
(Issue 1) so `ac5_part_b` is a non-vacuous green gate, and (iii) re-mint the AC6
snapshot from the final post-2b `D` with a topology (`bounded_complement_components
== 1`) cross-check before freezing the literals.** The implementer resumes at 2b and
carries these three through to green.

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping, mandatory for every
`M ≥ 1`. All four subtasks are **code** change-type (`*.rs`), form a linear
dependency chain (2a→1, 2b→2a, 3→2b), and fit within the size cap — so they cluster
into the **fewest possible groups: one**.

- **Group A** — model `sonnet` (sonnet-5), effort `medium` (pinned in the
  `code-writer` frontmatter), 1M-token window, via the `code-writer` subagent —
  subtasks 1, 2a, 2b, 3 (code change-type: `crates/gen/src/phase2.rs`,
  `crates/gen/src/lib.rs`). Terminal group (4 subtasks; within the `1..=10` range).
  Homogeneous (code-only). Subtasks 1 and 2a are already committed (`c46b1f2`,
  `180c203`); the resumed group implements the amendment (**2b**) and completes 3.
  Entry into Group A spawns `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry); the
  single group completes /task Step 8 in its own `/context-reset` subagent. No
  inter-group handoff (one group). Group count 1 ≤ 4 (max-groups default), no user
  gate needed.

The `design`, `design-review`, `self-review` subagents stay on Opus — only the
per-group implementor model varies.

## Risks

- **Envelope flattening (whole arm raised to `2k` instead of a compact ramp).** The
  2-pass envelope is seeded with the *actual* `top(x)` field, so nominal columns
  (`top = k−1`) cap the envelope and the ramp is confined to `Δ = w·k` columns — no
  flattening. `[derived → AC4 fixture test: assert nominal columns far from the jog
  keep width k, and the ramp spans exactly Δ columns at slope 1]`
- **Taper seals a spurious pocket → 2nd bounded hole (CONFIRMED, now real — not
  hypothetical).** Stage-2a's additive envelope fill *can* bridge a narrow genuine
  outfield gap between two arms and seal a pocket, creating a spurious second
  bounded complement component (witness `rng(0)`, `k=6,n=3`, 12-cell pocket
  `[measured: orchestrator repro — post-2a `bounded_complement_components == 2`]`).
  **Mitigation = Stage-2b pocket absorption**: after 2a, absorb every bounded `¬D`
  component disjoint from `H`, restoring one hole. Still additive; still no
  production panic (2b is construction, not a check). The **N=64 multi-seed property
  sweep asserting `bounded_complement_components(D) == 1` is the primary regression
  gate** for this defect (`fixture_jog`'s single jog does *not* exercise it).
  `[derived → gate: property sweep, all 64 seeds]`
- **2b absorption merges two arms / introduces a new concave niche (AC5).** Filling
  an enclosed pocket only *removes* concavity (a filled hole cannot add a `>1`-pt
  outfield step) and never touches `H`, so AC5 Part A (outfield walls ≤1 pt/col) and
  the single hole survive. A merged pair of arms is a wider blob but still a valid
  one-hole annulus with width `≥ n` — acceptable for round-1 (Ф4/Ф6 refine).
  `[derived → gate: AC5 Part A + AC3 across the property sweep]`
- **`arithmetic_side_effects` (deny) on coordinate math (`cx·k`, `+k`, envelope
  `−1`).** Mirror Ф1: `saturating_mul`/`saturating_add`, `try_from(..).unwrap_or(..)`,
  or a documented `#[allow(clippy::arithmetic_side_effects, reason = "<bounded
  domain>")]` where a bound is proven — never a bare op. `[measured: rg -n
  "arithmetic_side_effects" Cargo.toml → line 71 `= "deny"`; gate: cargo clippy
  --workspace --all-targets -- -D warnings]`
- **`missing_const_for_fn` (nursery deny) FORCES `const fn` on const-eligible pure
  integer helpers** (e.g. a `block_origin(cx, cy, k) -> Point` of pure saturating
  arithmetic — every call const-callable on stable). Declare such helpers `const
  fn`. Helpers that allocate (`Corridor`) or call non-const gp-core APIs
  (`flood_fill`, `walls_from_boundary`) are **not** const-eligible. `[measured: rg
  -n "nursery" Cargo.toml → line 63 `= "deny"`; gate: cargo clippy]`
- **Zero-production-panic invariant.** No `unwrap`/`expect`/`panic!`/panicking index
  in Ф2 production paths; `Corridor::new` is infallible, coordinate conversions use
  `try_from(..).unwrap_or(sentinel)`. `[derived → cargo clippy + the panic-index
  stays empty; Miri is N/A — no FFI/GPU, and gp-gen carries no golden]`
- **File size.** phase2.rs plus its `#[cfg(test)]` block will be sizeable; keep
  under the soft 500/800 (excl./incl. tests) cap — extract helpers, don't inline the
  fixture builders. `[derived → wc -l gate at review]`

## Test Design

All tests live in `crates/gen/src/phase2.rs` `#[cfg(test)] mod tests` (unit
convention; Ф1's tests are in-file — `crates/gen/src/phase1.rs:484`
`[measured: rg -n "mod tests" crates/gen/src/phase1.rs → 484]`).

**Fixtures.**
- `fixture_jog()` — a hand-built `CoarseSkeleton` with a **known widen-jog**: a
  rectangular coarse annulus (hole = a small block, ring = `dilate\hole`) with one
  side extended by 1 coarse cell over part of its run, giving a deterministic
  `k→2k` transition. Small `k = 3`, `n = 2` (`m = 4`). Concrete, hand-verifiable
  cross-sections for AC2/AC4/AC7.
- Ф1 replay fixtures — `phase1_coarse_ring(l_min, &mut rng(seed))` fed to Ф2 for
  the multi-seed property sweep and the AC6 snapshot (seed reused from Ф1's own
  pinned `SNAPSHOT_SEED = 999`, `l_min = 2`, or a fresh small seed).

**Helpers (test-only).**
- `cross_section_width(d: &Corridor, through: Point, axis) -> usize` — the maximal
  contiguous drivable run through `through` along `axis` (vertical scan measures a
  horizontal arm, horizontal scan a vertical arm). No such helper exists in gp-core
  (spec technical-constraint, confirmed `[measured: rg -n "width" crates/core/src/geom
  → only Rect/Size field accessors, no cross-section helper]`); it is internal to
  Ф2's tests.

**Per task.**

- **Task 1 (baseline).** Entry: `phase2_rasterize`.
  - AC1: for `fixture_jog`, every fine point of each ring cell's `k×k` block is
    `contains == true`; no ring cell dropped (count drivable == `|ring|·k²` before
    taper — assert on a *pre-taper* helper or on a widen-free fixture where taper is
    a no-op).
  - AC2-baseline: a 2-coarse-cell-thick section measures `cross_section_width ≥ 2k`.
  - AC6-baseline: two calls on identical input yield byte-identical `D` (compare via
    `walls_from_boundary` or a drivable-point `BTreeSet`).
- **Task 2a (envelope taper — committed).** Entry: `phase2_rasterize` + the envelope
  helper.
  - AC4: on `fixture_jog`, walk the tapered outer wall along the jog; assert the
    wall coordinate changes by **≤1 per column** and the full `Δ = k` change spans
    `≥ MIN_TAPER_RUN (= 3)` columns. Nominal columns far from the jog keep width
    `k` (no flattening).
  - Additivity: `D0 ⊆ D_2a` (every baseline drivable point stays drivable).
  - Hole-safety: no cell of `H` is drivable in `D_2a`.
  - Determinism: two taper runs byte-identical.
- **Task 2b (pocket absorption — NEW).** Entry: `phase2_rasterize` (full) + the
  absorption helper.
  - Defect regression (the reason 2b exists): `phase1_coarse_ring(3, &mut rng(0))`
    with `k=6, n=3` → assert `bounded_complement_components(D) == 1` on the final
    `D`. On the pre-2b build this is `2`; 2b restores `1`. This exact witness is a
    named test, mirroring Ф1's `seed_48_lmin3_widen_pinch_stays_one_hole` regression
    pin.
  - Additivity: `D_2a ⊆ D_2b` (absorption only adds cells).
  - Hole-safety: `H` is still entirely non-drivable in `D_2b` (the legitimate hole
    is never absorbed).
  - Determinism: byte-identical across two runs.
- **Task 3 (certification + profile).** Entry: `phase2_rasterize`.
  - Topology: `component_count(D) == 1`, `bounded_complement_components(D) == 1`
    on `fixture_jog` and — **the primary regression gate for the pocket-seal
    defect** — across the full N=64 property sweep.
  - AC3 + carve-vacuity: **every** cross-section `≥ n`, and the **minimum**
    cross-section `== k` (never `< k`) — documenting that no forced pinch arises and
    the narrow-carve path is unreached.
    - **`local_width` proxy slack (Note 4 — pre-existing, not amendment-introduced).**
      The AC3 sweep gate measures `local_width(p) = min(horizontal-run, vertical-run)`
      through each drivable `p` as a proxy for the true cross-section. This proxy is
      an **axis-aligned lower bound** valid on straight nominal/wide runs, but at a
      45° ramp's **outer tip** a lone protruding cell has small `H`-run *and* small
      `V`-run, so `local_width` can dip **below** the true perpendicular cross-section
      (and below `n`) even though no cross-section actually narrowed. The by-construction
      AC3 guarantee holds regardless: `D ⊇ D0` (taper + 2b are **additive**), and `D0`
      is uniform `k×k` blocks with true min cross-section `= k ≥ n`, so **adding**
      ramp/absorption cells can only *grow* every true cross-section — the "min
      cross-section `== k`" claim is about the true perpendicular width, which the
      additive property protects. **The sweep therefore must not assert
      `local_width ≥ n` at ramp-tip cells** (it would false-red on a correct `D`);
      assert `local_width ≥ n` on straight-run cells (where the proxy is exact) and
      rely on `D0 ⊆ D` + `D0`'s uniform `k` for the ramp/tip region — or measure the
      true perpendicular cross-section there. Slack is one-sided (proxy ≤ true), so it
      never *masks* a genuine `< n` narrowing; it only risks a false failure, which the
      straight-run restriction removes.
  - **AC5 — two parts (Note 1: a blanket "no chord cuts any concave corner" either
    falsely fails on normal turns or is vacuous, so it is NOT used).** First
    classify every concave corner of `D` (2×2 stencil: exactly one non-drivable
    cell) by its missing cell: **outfield-facing** (missing cell `∉ H`) = a tapered
    outer-wall corner; **hole-facing** (missing cell `∈ H`) = a normal annulus 90°
    turn, navigable and **exempt**.
    - **Part A — structural gate (the primary AC5 assertion).** Along every
      outfield-facing wall run, assert the wall's per-column advance is **≤1 point**
      — i.e. no `>1`-point outfield concave step survived the taper. This is the
      exact criterion separating a tapered-clean outfield wall (defect-free) from a
      normal hole-facing turn (exempt), and is precisely the supercover-cut
      precondition ("no reentrant outfield niche a fast chord can jump"). Reuses the
      AC4 wall-profile walk, restricted to outfield-facing runs via the `H` mask.
    - **Part B — supercover confirmation on `fixture_jog` (`k = 3`, known
      geometry).** The sharpest surviving outfield concave corner is a **unit step**
      of the 45° ramp. `V_ENTRY_CHECK = k = 3`: a plausible entry chord spans at most
      the ramp extent `Δ = k`, so this bound is *derived from the fixture's `k`*, not
      hand-tuned.

      **Corrected rationale (Issue 1 — my earlier "`(9,1) ∉ D`" claim was FALSE).**
      The reviewer live-probed the fixture: `(8,-2) ∈ D` **and `(9,1) ∈ D`** (both
      drivable), while the cut cell `(8,0) ∉ D` **and `(8,0) ∈ H`**. So
      `supercover((8,-2),(9,1))` clips a **hole cell** — it is a *hole-facing
      inner-corner* cut (a normal annulus turn), which Part A already classifies as
      **exempt**. It must be excluded for that reason, **not** because an endpoint is
      `¬D`. An endpoint-drivability filter alone leaves it in the family, so
      `ac5_part_b` would stay RED — and since `fixture_jog` has
      `bounded_complement_components == 1` (no pocket → 2b provably never alters it),
      no mechanism change can rescue it. The correct filter is an **H-clearance**
      test that mirrors Part A's outfield-vs-hole-facing split.

      **Chord family — H-clearance scoped:**
      `{ (a, b) : a ∈ D ∧ b ∈ D, both a,b in the ramp bbox,
      1 ≤ max(|dx|,|dy|) ≤ V_ENTRY_CHECK, |run-axis Δ| ≥ |perp-axis Δ|,
      supercover(a,b) ∩ H = ∅ }`. The added conjunct `supercover(a,b) ∩ H = ∅`
      **skips any chord whose supercover touches a hole cell** — removing the
      `(8,-2)→(9,1)` chord on principle (its supercover hits `(8,0) ∈ H`) and
      restricting the test to purely **outfield-facing** grazes, exactly matching
      Part A's classification. (Both-endpoints-drivable and along-ramp-axis are kept;
      they are necessary but, as this case proves, not sufficient.) Assert
      `supercover(a, b) ⊆ D` for every member.

      **Non-vacuity (the gate criterion).** The corrected family is non-empty and a
      genuine gate: take the East ramp's outfield-facing columns (`x ≥` the nominal
      east wall), whose grazing chords run parallel to the ramp with supercover
      entirely in the outfield (`∩ H = ∅`) — these are in-family. On the correct
      45°-tapered-then-absorbed `D` every such chord's supercover is `⊆ D` (ramp
      filled solid below the hypotenuse). **A regression that left an untapered
      `k`-step on that outfield-facing wall** produces an in-family chord (both
      endpoints drivable, along-axis, `supercover ∩ H = ∅`) whose supercover hits the
      step's non-drivable **outfield** cell → `⊄ D` → the test goes RED. So the
      family tests only outfield niches, cannot be satisfied vacuously, and fails iff
      a real outfield niche survives. ("Plausible entry speeds" beyond `V_ENTRY_CHECK`
      is empirical — deferred, Open questions.) `[derived → gate: this test]`
  - AC7 (Rec B — vacuity kept explicit): on `fixture_jog`, cross-sections match the
    intended profile — `k` nominal, `≥2k` wide; and the assertion **`min
    cross-section == k` / "no `n`-narrow section exists"** is retained verbatim so a
    future reviewer sees the "`n` in narrow" case is *intentionally vacuous* under
    the round-1 no-carve decision, not a missing test.
  - AC6 snapshot: **re-mint** `D` (the **final, post-2b** corridor) for a fixed small
    skeleton by running Ф2 once, **cross-check AC1–AC5 + topology
    (`bounded_complement_components == 1`) before freezing** (mirroring Ф1's
    `snapshot_pins_exact_cells_and_dir_for_a_known_seed`,
    `crates/gen/src/phase1.rs:751`), then pin an exact drivable-point set +
    dimensions in `assert_eq!`. The currently-drafted literals are a **stale
    placeholder** (`width = 0`, `origin = (-19, -1)`); the live post-2b mint gives
    `origin = (-37, -13)` `[measured: cargo test -p gp-gen --lib ac6_snapshot →
    left=(-37,-13)]`, so the placeholder MUST be replaced with the post-2b
    values — freezing the placeholder would pin a wrong (pre-2b/empty) `D`.
  - **Property sweep — `N = 64` seeds (Note 2).** `PROPERTY_SEED_COUNT = 64`,
    matching Ф1's own sweep count (the value at which Ф1's widen-pinch hazard
    surfaced at seed 48 — `crates/gen/src/phase1.rs`
    `[measured: rg -n "PROPERTY_SEED_COUNT|seed_48" crates/gen/src/phase1.rs]`).
    Real Ф1 output draws `widen ∈ 0..=3` **independently on all four sides**, so
    widened-meets-nominal corner geometry that `fixture_jog` (one jog) never
    exercises appears across the sweep. For `seed in 0..N`,
    `phase1_coarse_ring(3, rng(seed)) → Ф2 →` assert AC1 (`D0 ⊆ D`), AC2, AC3
    (≥`n`), **AC5 Part A (every outfield-facing wall run advances ≤1 pt/column)**,
    and topology — **`component_count(D) == 1` AND `bounded_complement_components(D)
    == 1`, the primary regression gate for the Stage-2a pocket-seal defect** (the
    `rng(0)` witness is one member; the sweep guards against other seeds hitting the
    same inter-arm-bridge hazard, just as Ф1's sweep surfaced its widen-pinch at seed
    48). AC5 and topology are thus swept on real four-sided-widen geometry, not only
    the single hand-built jog. Confirms Ф2 produces a Ф4-passable, supercover-clean
    `D` across Ф1's real output.

No golden-image / rendering tests in Ф2 (pure geometry), so the text-golden
threshold rule does not apply here.

## Open questions

- **Exact taper slope (empirical).** Round-1 fixes 45° (1 pt/col), meeting AC4's
  hard ≤1-pt/col invariant with `MIN_TAPER_RUN = 3` as the "several" floor. Whether
  a gentler slope (1 pt / `t` cols) reads better / passes the oracle at higher entry
  speeds is deferred to oracle/playtest data (spec Deferred; design §"width
  profile"). No separate issue yet.
- **Forced-pinch revisit trigger.** Round-1 carves nothing because Ф1's uniform
  `k×k` expansion never forces a sub-`k` arm (min cross-section `k ≥ n`;
  cross-gabarit `≥ 3k ≥ 2n+1`). If a later Ф1 revision emits sub-`k` arms, Ф2 will
  need the (hole-facing, topology-guarded) narrow-carve path — at which point the
  forced-pinch predicate and the `n`-carve taper become live. Tracked here; no code
  written for it now.
