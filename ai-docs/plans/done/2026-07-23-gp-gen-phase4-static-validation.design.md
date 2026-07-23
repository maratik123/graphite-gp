# Design: gp-gen Ф4 — static validation (connectivity · single hole · width · finger liveness)

**Issue:** [#27](https://github.com/maratik123/graphite-gp/issues/27)
**Date:** 2026-07-23

## Approach

Ф4 is the cheap static-validation pass. It consumes the fine corridor `D` plus
the width floors and the S/F line, runs four checks in a fixed order, and returns
a `Vec<Issue>` (empty ⟺ statically valid). Two of the four checks reuse the
merged #5 gp-core helpers verbatim; the other two are new, built on two new
integer geometry primitives (distance transform + medial axis).

### Resolution of the three design-owned Open questions

**Open Q1 — home crate for the DT / medial-axis / cross-section primitives.**

- **Distance transform + medial axis → `gp-core` `geom`** (new file
  `crates/core/src/geom/distance.rs`, flat re-exported like `graph.rs`). They are
  pure `Corridor` geometry, integer-only and deterministic — the exact profile of
  the `geom` module and siblings of the #5 flood-fill / component / geodesic-BFS
  family, which they reuse the same box-scan + visited-buffer idioms as
  `[measured: crates/core/src/geom/graph.rs:106-164 → flood_component / component_count / bounded_complement_components share the vec![false; d.area()] + d.box_points() scan]`.
  Living in `geom` lets them reuse `Corridor`'s **private** `index` / `box_points`
  / `area` / `on_border` directly, exactly as `component_count` does — instead of
  the external re-implementation gp-gen was forced into
  `[measured: crates/gen/src/phase3.rs:289-302 → gp-gen re-derives a private box_points from origin/width/height because Corridor's is private]`.
  This directly satisfies AC5 ("reusable … positioned so the future centerline
  (Ф7) can consume them") — Ф7's `racing_line(medial_axis(D))` (`docs/design.md`
  §2 line 191) is the second consumer, and a gp-core home needs no re-export.
- **§D2 ("medial axis живёт в Ф4") does not dictate crate placement** — it
  contrasts the *medial axis* (a width object) against the *`s`-parameterized
  centerline* (`docs/design.md` §"Два разных «центра»"/D2, lines 71-78); it is
  conceptual ownership, not module ownership. The primitive stays pure geometry.
- **Cross-section width helper → stays gp-gen-local in `phase4`.** It encodes
  Ф4 *policy* (what "width" means for a check: the DT-consistent min axis-run at a
  candidate cell), has exactly **one** consumer today (the NARROW check), and is
  not consumed by the centerline. Per the ≥3-site-duplication rule it is a clear single-site,
  keep-local case; lift it to `geom` only if Ф7 later needs it (YAGNI).
  `[measured: only phase4's NARROW check consumes it — one call site by design]`.

**Open Q2 — medial-axis extraction algorithm + output shape.**

- **The NARROW check does NOT sample the medial axis** (see § "The four checks"
  → Width). A narrow neck is a DT **valley** *along* the corridor axis — its
  along-flow neighbors in the wider flanks have strictly-greater DT — so a
  DT-local-maximum ("ridge") set **excludes every neck** and can never sample
  it. `medial_axis` is shipped here only to satisfy **AC5** (a reusable primitive
  the future Ф7 centerline consumes); the width check reads the DT field
  directly, not the ridge (see Width below).
- **Algorithm (`medial_axis`):** the **strict axis-wise DT ridge** — a `D` cell
  `p` is a medial cell iff it is a **strict** local maximum of the DT along at
  least one axis: `dt(p) > dt(p±x̂)` (both horizontal neighbors) **or**
  `dt(p) > dt(p±ŷ)` (both vertical neighbors), reading `dt = 0` for the `¬D` /
  out-of-box neighbor. **Strict** (`>`, not `≥`) is load-bearing: the along-flow
  axis of a straight corridor is a DT **plateau** (constant `dt`), so `≥` would
  admit every cell and collapse the ridge to the whole corridor; `>` keeps the
  thin cross-flow centerline. Critically, a **neck IS on this axis-wise ridge**:
  at the neck-center the two perpendicular walls are close, so `dt` is a strict
  local max *across* the neck — the neck is included and the ridge stays
  **4-connected across the constriction** (the exact defect Issue #1 flagged in
  the old local-maximum definition). This is the "гребень distance-transform" of
  `docs/design.md` §D2 (line 72), integer-only and deterministic.
- **Output shape:** a `BTreeSet<Point>` of medial cells (a *set*, not a graph).
  Deterministic iteration order (`Point`'s derived `Ord`, `x`-then-`y`
  `[measured: crates/core/src/geom/mod.rs:25 → #[derive(… PartialOrd, Ord …)] on Point]`).
- **AC5 Ф7-consumer reconciliation.** Ф7's `racing_line(medial_axis(D))`
  (`docs/design.md` §2 line 191) needs a ridge it can **trim to a loop** and
  arc-length-resample. The strict axis-wise ridge has **no neck gaps** (necks
  included, connected across), so the loop is never severed at a constriction —
  the property that makes it trimmable. Two documented consumer responsibilities
  remain Ф7's, per the spec's out-of-scope split ("this task ships only the DT +
  medial-axis primitives centerline will later consume, not the curve
  construction"): (i) **thinning** an even-width 2-cell ridge band to a single
  strand; (ii) **bridging residual corner gaps** — at a rectilinear corner the DT
  can plateau diagonally, so the axis-wise ridge may leave a 1-cell diagonal step
  that `racing_line`'s resample step closes (a corner bridge, not a neck gap).
  Unit tests assert the *primitive's* half of the contract: the ridge is a thin
  centerline on a straight corridor, **includes the neck and is 4-connected
  across it** on a necked corridor, and on a rectilinear annulus resolves to **4
  disjoint straight strips** (one per side, each internally 4-connected) with a
  **diagonal gap at each corner** — the corner region's 2×2 DT-plateau ties on
  both axes, so the strict axis-wise ridge admits neither diagonal cell. Closing
  those corner gaps into a single loop is Ф7's `racing_line` responsibility (the
  (ii) corner-bridge above), not the primitive's.
  `[derived → the medial_axis unit tests in Task 1]`.

**Open Q3 — `Issue` payload shape (locality for the future Ф6 repair phase).**

Ф6 maps each issue to the dual edge / wall it must move (`docs/design.md` §2 Ф6,
lines 174-184: `NARROW → push_outer_wall_out`, `LOST_HAIRPIN → trim_arm_wall /
nudge_finger`). Payloads carry the minimum locality Ф6 needs to re-derive that
wall, and every variant is `Eq + Hash` so tests assert an order-independent
`HashSet<Issue>`:

```rust
pub enum Issue {
    Disconnected,                                        // global, nullary
    BadTopology,                                         // global, nullary
    Narrow    { center: Point, axis: Orient, width: u32 },
    NarrowSf  { center: Point, axis: Orient, width: u32 },
    LostHairpin { tip: Point },                          // coarse finger tip
}
```

- `Narrow` / `NarrowSf` carry the narrow chord's **canonical cell** (`center` =
  the shorter run's min-`Point`, its bottom/left cap cell), the narrow chord
  **`axis`** (its orientation; Ф6 pushes the two capping outer walls apart along
  it), and the measured `width`. From `(center, axis, width)` Ф6 re-derives the
  cross-section and its outward walls at repair time — no need to serialize the
  whole cell run. The canonical cell also makes the ≤2 DT-centered cells of one
  chord collapse to a single `HashSet` entry (see Width check).
- `LostHairpin` carries the coarse finger **`tip`** — the anchor Ф6's
  `nudge_finger` acts near. A single `Point` is canonical and order-independent.
- `Disconnected` / `BadTopology` are nullary (global repair scope).
- `#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]` — `Point` and `Orient`
  are both `Eq + Hash`
  `[measured: crates/core/src/geom/mod.rs:25 (Point), :59 (Orient) → both derive PartialEq, Eq, Hash]`,
  so the enum derives cleanly; `Debug` satisfies `assert_matches!`'s `{:?}` bound
  (AGENTS.md § Rust Test Conventions).

### Orchestrator signature

The pseudocode signature (`phase4_static_checks(D, n, m, sf)`, `docs/design.md`
§2 line 141) is schematic; the Key-decisions table sanctions passing the extra
section/finger context. Concrete signature:

```rust
pub fn phase4_static_checks(
    d: &Corridor,
    skel: &CoarseSkeleton,   // finger reference: skel.hole (P) protrusions
    k: i32,                  // block size — maps coarse fingers → fine blocks
    n: u32,                  // global width floor  = GenParams::min_width()
    m: u32,                  // S/F width floor      = GenParams::start_finish_width()
    sf: &StartFinish,        // the S/F chord (its width = chord.len())
) -> Vec<Issue>
```

`n` / `m` are `u32` to match their sources
`[measured: crates/gen/src/lib.rs:38,43 → min_width()/start_finish_width() return u32]`;
`k` is `i32` to match Ф2's `k`
`[measured: crates/gen/src/phase2.rs:37 → phase2_rasterize(skel, k: i32, n: i32)]`.
Checks run in the fixed order: connectivity → topology → width (NARROW +
NARROW_SF) → finger liveness. Deterministic and total (no `Result`, no production
panic), mirroring Ф1/Ф2/Ф3 `[measured: crates/gen/src/phase3.rs:487 → pub fn phase3_start_finish(...) -> Phase3Output, no Result]`.

### The four checks

1. **Connectivity → `Disconnected`** iff `component_count(d) != 1`. Reuses the #5
   helper verbatim `[measured: graph.rs:122 → pub fn component_count(d: &Corridor) -> usize]`.
2. **Topology → `BadTopology`** iff `bounded_complement_components(d) != 1`. The
   #5 helper already counts only bounded holes of ≥1 cell, excluding empty and
   border-touching components `[measured: graph.rs:137-164 → doc + impl count only non-border-touching non-empty complement components]`,
   so `!= 1` encodes "not exactly one bounded hole of ≥1 point" (AC2) directly.
3. **Width → `Narrow` / `NarrowSf`:** DT pre-filter over **all** `D` cells, then
   an **exact perpendicular cross-section** confirmation (`docs/design.md` §2 Ф4:
   "distance-transform как пре-фильтр + точный подсчёт по поперечным сечениям").
   The candidate set is **all** `D` cells passing the pre-filter — **not** a ridge
   set — because a neck is a DT valley excluded from any local-maximum ridge (the
   Issue-#1 defect); every sub-`n` cross-section's center cell is a candidate (the
   completeness argument, § Risks).
   - Compute `dt = DistanceTransform::compute(d)` once.
   - For each `D` cell `p` (row-major over `d.box_points()`): **DT pre-filter** —
     skip if `2·dt.at(p) − 1 ≥ n` (provably wide, `w(p) ≥ n`; § Risks soundness
     note). Otherwise walk the four in-`D` wall-distances from `p`
     (`up/down/left/right`, each the step count to the first `¬D`/box-edge cell,
     bounded by the box) to get the two maximal runs `hrun = left+right−1`,
     `vrun = up+down−1` and `w(p) = min(hrun, vrun)`.
   - **Emit iff `w(p) < n` AND `w(p) ∈ {2·dt.at(p) − 1, 2·dt.at(p)}`** — the
     **DT-consistency test**. It fires exactly when `p` is *centered* in its
     shorter run (both of that run's caps are `p`'s nearest walls), i.e. the run
     is a **genuine perpendicular cross-section**, not an along-flow slice near a
     wall. This is what rejects the staircase / taper-edge **false positive** that
     `min(hrun, vrun)` alone produces (a short along-flow run at a near-wall cell
     has `dt` too small to be consistent — § Risks soundness note). The old
     ridge-restriction was introduced to kill that false positive but severed
     necks; the DT-consistency test kills it **without** severing necks.
   - **Canonical-chord dedup.** Emit `Narrow { center, axis, width }` keyed on the
     **shorter run's min-`Point`** (its bottom/left cap cell), so the ≤2 centered
     cells of one narrow chord collapse to **one** issue in the `HashSet`. If
     `vrun ≤ hrun`: `axis = Vertical`, `center = Point::new(p.x, p.y − down + 1)`,
     `width = vrun`; else `axis = Horizontal`,
     `center = Point::new(p.x − left + 1, p.y)`, `width = hrun`. **`width` is
     `usize → u32` via `u32::try_from(w).unwrap_or(u32::MAX)`** (total-conversion
     discipline; the run is bounded by box area so the fallback is unreachable in
     domain).
   - **NARROW_SF** is a dedicated check on the S/F chord (the one S/F section):
     if `sf.chord.len() < m`, emit `NarrowSf { center, axis: sf.orient, width:
     u32::try_from(sf.chord.len()).unwrap_or(u32::MAX) }`, where `center` is the
     chord's **min-`Point`** (its bottom/left cap cell) via
     `if let Some(center) = sf.chord.iter().copied().min()` — the **same**
     canonical-cell convention as `Narrow`'s shorter-run min-`Point` (Rec A:
     uniform Ф6 anchor convention). The `if let Some` guard keeps the emit total
     — a degenerate empty chord emits nothing rather than panicking on an index;
     the S/F chord is nonempty in any real generation (`width() == chord.len() ≥
     1`), so `min` is `Some` in domain. S/F emits at most once, so the min-`Point`
     choice is dedup-neutral and purely for anchor uniformity with `Narrow`.
   - **`axis = sf.orient` (NOT its perpendicular).** `sf.orient` **IS** the
     chord's own orientation "across the corridor," and `chord.len()` (==
     `StartFinish::width()`) is measured **along** `sf.orient`
     `[measured: crates/core/src/track.rs:84-85 → orient doc "Chord orientation
     across the corridor (H or V)"; :94 → pub const fn width() { self.chord.len() },
     the chord length along orient]` `[measured: crates/gen/src/phase3.rs:509 →
     sf.orient = perp_orient(axis) where axis is the along-flow travel direction,
     so orient is the cross-corridor chord's own run direction]`. So when the
     chord is too short, the narrow dimension runs **along** `sf.orient`, not
     perpendicular to it — exactly the `Narrow` convention (narrow `axis` = the
     shorter cross-corridor run's **own** orientation; for the S/F chord that run
     is the chord itself, whose orientation is `sf.orient`). Ф6 then pushes the
     two chord-cap outer walls apart along `sf.orient`, consistent with the
     payload's `axis` semantics above.
   - The chord's width is `chord.len()` directly
     `[measured: crates/core/src/track.rs:94 → StartFinish::width() == chord.len()]`,
     `usize`, so it converts to the `u32` `Issue.width` field with the same
     `u32::try_from(..).unwrap_or(u32::MAX)` form as NARROW — no DT sampling is
     needed to identify "section ∈ sf".
   - A sub-`n` S/F section fires **both** `Narrow` and `NarrowSf` (per the §2
     pseudocode's two independent `if`s); the AC7 too-thin-S/F fixture is built
     with width in `[n, m)` to isolate `{NarrowSf}` (see § Test Design).
4. **Finger liveness → `LostHairpin`:**
   - **`infield_fingers(skel)`** — peninsulas of the coarse hole `P` (`skel.hole`,
     a `BTreeSet<Point>` `[measured: crates/gen/src/phase1.rs:52-54 → pub ring/hole: BTreeSet<Point>]`):
     each coarse hole cell with exactly **one** 4-connected neighbor in `P` is a
     finger **tip**; walk the chain of degree-≤2 hole cells from the tip until a
     degree-≥3 branch cell — that chain is the finger. Keyed by its tip `Point`.
   - **`absorbed(finger, d, k)`** — the finger's fine footprint is the ×`k` block
     expansion of its coarse cells (`block_origin(c,k) = (c.x·k, c.y·k)`, a `k×k`
     patch, mirroring Ф2 `[measured: crates/gen/src/phase2.rs:48-49 → block_origin(c,k) = (c.x·k, c.y·k)]`).
     The finger is **absorbed** iff **every** fine cell of that footprint is
     drivable in `d` — the separating infield strip is entirely filled, so the two
     flanking arms have merged (`docs/design.md` §1 line 24). Emit `LostHairpin {
     tip }`. A finger that pinches into a *separate island* (tip stays `¬D`) is
     **not** absorbed here — that case surfaces as `BadTopology` (≥2 holes), the
     correct division of labor (§ Risks). Rejected alternative: reconstructing the
     two arms and testing their 4-connectivity through the finger — strictly more
     code for an equivalent verdict on the fill case (footprint-all-`D` ⟹ arms
     connected across).

### New gp-core primitives (`geom/distance.rs`)

```rust
pub struct DistanceTransform { rect: Rect, dist: Vec<u32> }   // 0 = ¬D/out-of-box; ≥1 = D
impl DistanceTransform {
    pub fn compute(d: &Corridor) -> Self;   // multi-source 4-conn BFS from the ¬D frontier
    pub fn at(&self, p: Point) -> u32;       // 0 if ¬D / outside box (const-ness: see below)
    pub const fn rect(&self) -> Rect;        // const-eligible trivial accessor (see below)
}
pub fn medial_axis(dt: &DistanceTransform) -> BTreeSet<Point>;  // strict axis-wise DT ridge
```

- **DT algorithm:** multi-source BFS. Seed = every `D` cell with a `¬D`
  4-neighbor (distance 1); BFS outward, +1 per layer; `dist = Manhattan distance
  to the nearest ¬D cell`. Out-of-box neighbors are `¬D` by construction
  (`Corridor::contains` is `false` outside the box
  `[measured: crates/core/src/geom/mod.rs:292-296 → contains delegates to rect.index, None outside box]`),
  so a `D` cell on the box border has `dt = 1` — consistent with
  `walls_from_boundary` treating out-of-box as `¬D` (`graph.rs:308-331`). This is
  a **new** function; the existing `geodesic_bfs` is single-source within `D`
  `[measured: graph.rs:241 → geodesic_bfs takes one seed: Point]`, not a
  multi-source wall-distance transform.
- **`at` const-ness is decided by clippy at implementation, not pinned here.**
  `missing_const_for_fn` (nursery = `deny`) **forces** `const fn` iff the body is
  const-eligible on stable rustc (every call const-callable). Whether `at`'s body
  qualifies depends on the exact form (a slice-index/`Vec::get` path routed
  through a not-yet-const-stable callee stays non-const and the lint declines;
  a fully const-eligible bounds+index body is forced `const`). The implementor
  writes `at`, runs the lint, and takes whichever verdict it returns — the design
  does **not** assert non-const. `[derived → cargo clippy --workspace
  --all-targets -- -D warnings decides at/rect const-ness]`.
- `rect()` returns a `Copy` field with no non-const call → **is** const-eligible
  → `missing_const_for_fn` **forces** `pub const fn rect`, mirroring
  `Corridor::origin`
  `[measured: crates/core/src/geom/mod.rs:276 → pub const fn origin(&self) -> Point]`.
- `compute` / `medial_axis` allocate/BFS/insert into heap collections → not
  const; no float anywhere (`u32` distances, `Point` sets) — satisfies §3a
  integer-only.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | `DistanceTransform` (multi-source BFS `compute`, `at`, `rect` — let clippy decide `at`/`rect` const-ness) + `medial_axis` (**strict axis-wise DT ridge**) in a new `geom/distance.rs`; wire `mod distance; pub use distance::*` into `geom/mod.rs`; `#[cfg(test)] mod tests` (DT values on straight/L/ring fixtures; medial axis = thin centerline on a straight corridor, **includes the neck & 4-connected across it** on a necked corridor, **4 disjoint straight strips (one per side, diagonal corner gaps)** on a rectilinear annulus; determinism; empty/filled corridors) | `crates/core/src/geom/distance.rs`, `crates/core/src/geom/mod.rs` | — |
| 2 | `Issue` enum + payloads (`Disconnected`, `BadTopology`, `Narrow`, `NarrowSf`, `LostHairpin`) with `#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]` and `///` docs; new `phase4.rs`; wire `mod phase4; pub use phase4::*` into `lib.rs` | `crates/gen/src/phase4.rs`, `crates/gen/src/lib.rs` | — |
| 3 | Connectivity + topology checks (delegate to `component_count` / `bounded_complement_components`) as private helpers returning `Option<Issue>` / pushing into the issue vec; tests (AC1/AC2: single-component ↔ no `Disconnected`; annulus→disk merge and ≥2 holes trip `BadTopology`, valid annulus does not) | `crates/gen/src/phase4.rs` | 2 |
| 4 | Cross-section helper (four in-`D` wall-distance walks → `hrun`/`vrun`, narrow-chord `Orient`, canonical min-`Point`) + NARROW check over **all `D` cells** (DT pre-filter → exact runs → `w<n` **and** DT-consistency `w ∈ {2·dt−1, 2·dt}` → canonical-chord-keyed `Narrow`, `width` via `u32::try_from(..).unwrap_or(u32::MAX)`) + NARROW_SF on `sf.chord` (same `u32` conversion); tests (AC3: sub-`n` neck-center → one `Narrow`; `[n,m)` S/F chord → `NarrowSf`; sections ≥ floor → neither; **staircase/taper edge → no false `Narrow`**; a doorway-neck DT-valley IS caught) | `crates/gen/src/phase4.rs` | 1, 2 |
| 5 | `infield_fingers(skel)` (coarse peninsula extraction) + `absorbed(finger, d, k)` (footprint-all-`D`) + LOST_HAIRPIN check; tests (AC4: filled finger → `LostHairpin`; surviving finger → none; a finger keyed by its coarse tip) | `crates/gen/src/phase4.rs` | 2 |
| 6 | `phase4_static_checks` orchestrator — the four checks in fixed order, returning `Vec<Issue>`; integration tests (AC6 clean ring → empty; AC7 four adversarial fixtures each → exactly its intended `HashSet<Issue>`, no spurious extra; AC8 determinism: repeated calls set-identical) | `crates/gen/src/phase4.rs` | 3, 4, 5 |

All six subtasks change **code** (`*.rs`) only — no `*.md` / `.claude/**` /
`AGENTS.md` / `ai-docs/**` edits. Scope is 6 tasks (≤ 15). If `phase4.rs` grows
past the 500/800 (excl./incl. tests) soft line
`[derived → wc -l crates/gen/src/phase4.rs at implementation]`, split into
`phase4/mod.rs` (Issue + orchestrator) + `phase4/fingers.rs` (finger extraction +
absorbed), per the technical-constraints note.

## Handoff plan

`M = 6`. One homogeneous **code** group; grouping is required for every M ≥ 1.

- **Group A** — model `sonnet` (sonnet-5), effort `medium` (pinned), 1M-token
  window, via the `code-writer` subagent — subtasks **1–6** (code change-type:
  `*.rs` only, so change-type-homogeneous; `≤ 10`; dependency order 1/2 → 3/4/5 →
  6 is preserved within the group). Terminal group (6 subtasks; within the
  `1..=10` range). Grouping is minimized: all six are the same change-type and
  same implementor model, so they cluster into the **fewest possible** groups
  (one) rather than interleaving. Entry into Group A spawns `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry); the
  single group completes /task Step 8 in its own `/context-reset` subagent. No
  inter-group handoff (only one group). Group count (1) is within the default
  max of 4.

## Risks

- **NARROW completeness (no sub-`n` cross-section is missed).** The candidate set
  is **all** `D` cells passing the pre-filter, so — unlike the retired ridge set,
  which excluded every neck (a DT valley along-flow: the neck-center's along-axis
  neighbour in the wider flank has strictly-greater DT, so no neck cell is a DT
  local max — the Issue-#1 defect) — every cell is reachable. Argument (scoped to a
  genuine **neck** — a perpendicular cross-section where the two perpendicular
  walls are the nearest `¬D`, i.e. the constriction case of the axis-aligned,
  no-taper domain; the axis-aligned/45° limitation is the separate risk below):
  any narrow perpendicular cross-section **at a neck** of width `w < n` has a
  **center cell** `p*` whose shorter run is that chord (`w(p*) = w`), with the two
  chord caps as its nearest walls, so `dt(p*) = ⌈w/2⌉` and
  `w ∈ {2·dt(p*)−1, 2·dt(p*)}` (odd `w`: `2·dt−1=w`; even `w`: `2·dt=w`). `p*`
  therefore (i) passes the pre-filter (`2·dt−1 ≤ w < n`), (ii) passes the
  DT-consistency test, and (iii) emits with `w < n`. Hence **no sub-`n`
  cross-section at a neck can be missed.** `[derived → the
  AC3/AC7 pinch fixtures + a doorway-neck (DT-valley) unit test must surface
  Narrow]`.
- **DT pre-filter soundness (never skip a truly narrow section).** For any `D`
  cell `p`, `w(p) = min(hrun, vrun)` satisfies `2·dt(p) − 1 ≤ w(p)`:
  `dt(p) = min` over the four wall-distances `≤ ⌈w(p)/2⌉`, hence
  `2·dt(p) − 1 ≤ w(p)`. Therefore `2·dt(p) − 1 ≥ n ⟹ w(p) ≥ n` — skipping is
  sound. `[derived → the completeness unit tests above]`.
- **NARROW soundness — DT-consistency rejects the staircase / taper-edge false
  positive.** `min(hrun, vrun) < n` at *every* `D` cell over-fires: at a
  near-wall cell on a sloped/staircase boundary (e.g. a taper edge) the **shorter
  run is an along-flow slice**, short only because the cell sits near the end of
  that row/column, though the true perpendicular width is `≥ n`. Such a cell has
  `dt` small (it is *at* a wall) so its shorter run `w > 2·dt`, i.e.
  `w ∉ {2·dt−1, 2·dt}` — the DT-consistency test rejects it. The test passes
  **only** when both caps of the shorter run are `p`'s nearest walls (`p` centered
  in a genuine cross-section). This solves the inner-corner false positive with
  the *true perpendicular* cross-section (DT-centered run), not raw
  `min(hrun, vrun)` — the reason ridge-restriction had been introduced, now
  achieved without severing necks (the axis-aligned-domain assumption this shares
  with `min(hrun, vrun)` is the next risk). `[derived → an AC7-adjacent
  staircase/taper-edge fixture must yield no Narrow while its center stays ≥ n]`.
- **Axis-aligned cross-section is a documented domain limitation.**
  `min(hrun, vrun)` approximates the perpendicular width only for axis-aligned
  corridors; a 45° corridor would over-count. Ф1→Ф3 output is axis-aligned
  (coarse-block rings, L-corners) `[measured: crates/gen/src/phase3.rs:50-55 →
  axis_for_inward maps every inward Side to a H/V Orient; no diagonal runs]`, and
  every AC7 fixture is axis-aligned. Note it in the primitive/check docs.
- **LostHairpin vs BadTopology division of labor.** A filled finger →
  `LostHairpin` (topology stays one hole). A finger pinched into a separate island
  → `BadTopology` (≥2 holes), not `LostHairpin`. The two are complementary, not
  overlapping, so AC7's absorbed-finger fixture yields exactly `{LostHairpin}`
  (build it as a *fill*, not a pinch-off). `[derived → AC7 absorbed-finger fixture
  asserts the singleton set]`.
- **Multiplicity / canonical-chord dedup.** The ≤2 DT-centered cells of one
  narrow chord all emit `Narrow` keyed on the **same** shorter-run min-`Point`, so
  they collapse to **one** issue in the `HashSet`. A pinch spanning several
  *distinct* narrow cross-sections (columns/rows) genuinely emits one `Narrow`
  each — those are real, not spurious. AC7's pinch fixture is a **sharp
  single-cell neck** (one narrow cross-section, flanked by ≥ `n` sections) →
  exactly one `Narrow`; Ф6 treats duplicate-locality issues idempotently.
  `[derived → AC7 sharp-neck fixture asserts a singleton Narrow set]`.
- **`arithmetic_side_effects` (denied workspace-wide** `[measured: Cargo.toml:71
  → arithmetic_side_effects = "deny"]`**).** DT `distance + 1`, the four
  directional wall-distance counters, `hrun/vrun = a+b−1`, the medial-axis
  neighbour compares, and the pre-filter/consistency `2·dt − 1` / `2·dt` all need
  bounded-domain `#[allow(…, reason = …)]` or `saturating_*`, mirroring the
  existing `graph.rs` pattern (count ≤ area)
  `[measured: graph.rs:117-121,235-240 → #[allow(arithmetic_side_effects, reason
  = "… bounded by area")] on component_count / geodesic_bfs]`. Use `saturating_*`
  for `2·dt − 1` / the run-length subtractions and a bound-documented `#[allow]`
  for BFS `distance + 1` and the directional counters (each ≤ box extent).
- **Zero production panic / no `Result` (gp-core invariant, empty panic-index).**
  DT/medial/checks are total: out-of-box → `dt.at` returns 0; empty `D` → no
  seeds, all `dt = 0`, `medial_axis` empty, checks return their global issues;
  width/len `usize→u32` conversions use `u32::try_from(..).unwrap_or(u32::MAX)`,
  never `as`. `[derived → cargo test + the AC8 totality assertions on degenerate
  fixtures]`.
- **Miri.** Pure integer BFS/set code, no FFI/GPU — expected Miri-clean; no
  `#[cfg_attr(miri, ignore)]` anticipated (spec technical-constraints).
  `[derived → MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace]`.

## Test Design

**Task 1 — `geom/distance.rs`** (`#[cfg(test)] mod tests`)
- Entry points: `DistanceTransform::compute`, `at`, `rect`, `medial_axis`.
- Fixtures: straight 1×W and W×1 corridors (DT = distance-to-wall band; medial =
  center row/band); 3×3 ring (`ring_3x3`-style annulus, medial = 4 disjoint
  straight strips, one per side, with diagonal corner gaps); an
  L-corner; a **necked corridor** (a wide horizontal corridor pinched to a
  narrower cross-section at one column — the medial-axis connectivity test); empty
  and filled corridors; an even-width corridor (2-cell medial band).
- Scenarios: exact DT values on the straight corridor (`1,2,…,2,1`); `at`
  returns 0 for `¬D` / out-of-box; **`medial_axis` is a thin cross-flow centerline
  on a straight corridor, INCLUDES the neck cell and is 4-connected across it on
  the necked corridor (the Issue-#1 property — a local-maximum ridge would leave a
  gap there), and on a rectilinear annulus resolves to 4 disjoint straight strips
  (one per side, each internally 4-connected) with a diagonal gap at each corner
  where the 2×2 DT-plateau ties on both axes (closing those gaps into a loop is
  Ф7's `racing_line` job, not the primitive's)**; determinism
  (repeated `compute`/`medial_axis` byte-identical, incl. `BTreeSet` order); `rect`
  round-trips the box (const-ness per clippy's verdict, not asserted).

**Task 3 — connectivity + topology** (AC1/AC2)
- Fixtures: single-component clean ring (→ no `Disconnected`); two disjoint
  blocks (→ `Disconnected`); valid annulus (→ no `BadTopology`); annulus→disk
  merge and a two-hole shape (→ `BadTopology`).

**Task 4 — width** (AC3)
- Fixtures: a clean ring, all sections ≥ `n` (→ no `Narrow`); a ring with one
  **sharp single-cross-section neck** `< n` (→ exactly one
  `Narrow { center, axis, width }`, `center` = the neck chord's min-`Point`,
  `width` the `u32`-converted run); a **doorway neck** — two wide rooms joined by
  a sub-`n` width-3 corridor, whose neck is a DT valley (→ `Narrow` still
  surfaces, the completeness case a ridge set would miss); a **staircase / taper
  edge** whose center column stays ≥ `n` (→ **no** `Narrow` — DT-consistency
  rejects the near-wall along-flow short run, the soundness case); an S/F chord of
  width in `[n, m)` (→ `NarrowSf`, no `Narrow`); a sub-`n` S/F chord (→ both).
- Fixtures hand-built via `Corridor::new` + `set`, `StartFinish` via struct
  literal (pub `chord`/`orient`/`gate`; `gate = TimingGate { behind, forward }`)
  `[measured: crates/core/src/track.rs:81-88,28-33 → StartFinish/TimingGate pub fields]`.

**Task 5 — finger liveness** (AC4)
- Fixtures: a `CoarseSkeleton` (struct literal: pub `ring`/`hole`/`dir`) whose
  `hole` `P` has a one-cell-wide peninsula; a matching fine `D` where (a) the
  finger footprint is `¬D` (→ alive, no issue) and (b) the footprint is fully
  drivable (→ `LostHairpin { tip }`). Assert the finger is keyed by its coarse tip.

**Task 6 — orchestrator** (AC6/AC7/AC8)
- Entry point: `phase4_static_checks`.
- AC6: a clean, valid hand-built ring (single component, one hole, all sections
  `≥ n`, S/F `≥ m`, finger intact) → **empty** `Vec<Issue>`.
- AC7: four adversarial fixtures — a **sharp single-cross-section neck** `< n`
  (one narrow chord flanked by ≥ `n` sections, so exactly one chord fires) →
  `{Narrow}`; annulus→disk merge → `{BadTopology}`; S/F width in `[n, m)` →
  `{NarrowSf}`; filled finger → `{LostHairpin}` — each asserted as an
  order-independent `HashSet<Issue>` (using the derived `Eq + Hash`), with **no**
  spurious extra issue. Each fixture is otherwise clean (uniform width ≥ `n`, no
  taper edges) so only its intended issue fires.
- AC8: repeated calls on identical input return set-identical results; the whole
  path is total (no panic) on degenerate inputs (empty `D`, 1×1 corridor).
- `[derived → cargo test -p gp-gen phase4; cargo clippy --workspace
  --all-targets -- -D warnings; RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
  --workspace]`.

## Open questions

- **Whether NARROW / LOST_HAIRPIN fire on real Ф1→Ф3 output in round 1** (spec
  Open-Q4) — deferred, does not block: the checks are proven correct on the
  hand-built adversarial fixtures regardless, mirroring Ф2's round-1 carve-
  vacuity. If round-1 `P` never produces a peninsula, LOST_HAIRPIN is a purely
  defensive check exercised only by fixtures — acceptable per the spec.
- **Cross-section width promotion to gp-core** — left in gp-gen `phase4` (single
  consumer). If Ф7's centerline turns out to need exact per-section width (not
  just DT + medial), lift the helper to `geom` then, per the ≥3-site rule.
