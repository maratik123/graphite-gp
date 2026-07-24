# gp-gen Ф6: local repair — one dual edge per edit + edit-type-scoped recheck (C3)

**Source:** issue #31
**Date:** 2026-07-24
**Tracked in:** #31

## Scope

Build the Ф6 local-repair phase in `gp-gen` (crate dir `crates/gen`, package
`gp-gen`) — `docs/design.md` §2 `Ф6`, §2 `[C3]` (recheck scope), §2
`phase6_local_repair` — **together with the three missing defect detectors that
give its dispatch table a complete set of producers** (owner, round 1, Q1
"Detectors too").

Ф6 applies **one dual-edge shift per edit**, re-checks only what that edit's
*type* can have invalidated, and reports either a repaired corridor `D` or
`FAILED` — the signal that drives an Ф1 reseed (`[N4]`).

Five deliverables:

1. **Three new defect detectors**, each with a concrete producing condition and
   a deterministic fixture (§ The three new detectors): `NoBraking` (run-out),
   `ConcaveChordCut` (supercover-cut inner tooth), `ArmsMerging` (drivable
   intrusion into the infield). Without them, three of the design's six dispatch
   arms would ship as dead vocabulary and issue AC3 would have nothing to drive
   it (§ Technical constraints → *the AC7 result*).
2. **The Ф6 dispatch table** — six defect labels → five repair arms, each
   producing a **single dual-edge** edit.
3. **The `[C3]` edit-type-scoped recheck** — add → local, remove → global
   flood-fill, run-out → **speed-sink → speed-sink** (§ The `[C3]` recheck
   contract).
4. **The per-arm progress metric and the `FAILED` contract** (§ Per-arm progress
   metrics; owner, round 1, Q3 "Per-arm metric").
5. **Deterministic per-arm fixture tests**, including the AC3 discriminating
   fixture where a fixed-radius recheck wrongly reports "fixed" while the
   sink-to-sink recheck correctly reports "still broken".

The `DYNAMICALLY_DISCONNECTED` arm is **already built** (#30, CLOSED):
`crates/gen/src/phase6.rs::map_frontier_gap_to_edge(...) -> RepairCandidate`.
This task **wires it in**; it does not re-derive or re-litigate it.

### Resolving the round-1 tension: wide detectors, narrow driver

Q1 ("Detectors too") widens scope; Q2 ("Single pass") keeps the driver minimal.
These are **orthogonal, and both hold**:

- The three new detectors are **producers**. They run *before* Ф6, in the Ф4 /
  Ф5 families, exactly where the design's pipeline puts defect detection. They
  are not part of Ф6's driver.
- `phase6_local_repair` remains **one pass over the issue list**: for each
  issue, dispatch → derive one edge → scratch-apply → run the type-scoped
  recheck → commit the edit iff its own metric improved. It does **not** loop to
  validity, does **not** re-detect, and does **not** own a repair budget.
- The `[C3]` recheck — including AC3's sink-to-sink re-verification — is a
  **per-edit verification step inside that single pass**, not a loop. It is what
  decides whether an edit is committed, which is precisely how the per-arm
  progress metric (Q3) is enforced.

## Out of scope

- **`generate()` pipeline wiring.** `generate()` stays `todo!` — the same
  deferral Ф4, Ф5a, Ф5b and #30 each took. The `repeat seed_budget` /
  `repeat repair_budget` loops of design §2's `generate_track` pseudocode are the
  integration item.
- **Ф7 output assembly** (`s_field`, `centerline`, `TrackArtifact` population).
- **Any `gp-core` change.** `Corridor`, `Wall`, `Side`, `flood_fill`,
  `component_count`, `bounded_complement_components`, `walls_from_boundary`,
  `geodesic_layers`, `DistanceTransform`, `medial_axis`, `legal_move`,
  `supercover` are consumed unchanged. A genuinely missing core primitive is a
  Design Amendment, not a silent widening.
- **Re-running Ф1/Ф2.** Design §2 Ф4 allows a static failure to fall back "to Ф2
  locally or Ф1 (seed)". Ф6's failure route here is `FAILED` **returned to the
  caller**; who reseeds is the `generate()` integration item.
- **Re-opening #30's contract.** `map_frontier_gap_to_edge`'s signature,
  `RepairCandidate`'s two-variant shape, and the AC7 monotonicity result are
  settled. A different call shape at the call site is a widening, not a
  redefinition of the mapper's semantics.
- **Making a lappable track *faster*.** `NoBraking` as specified here is a
  **bounded, `v_target`-referenced run-out check**, not a general lap-time
  optimiser. Tuning `tempo` / `vmax_attain` is a separate quality concern.
- **Rewriting `ai-docs/plans/done/`** — history surfaces stay untouched.

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| `generate()` wiring: seed budget, repair budget, Ф4/Ф5/Ф6 loop assembly | Integration item; deferred identically by every Ф-phase issue so far | No — the design build order covers it |
| Ф7 output (`s_field`, `centerline`, artifact population) | Distinct build-order item | No |
| A general lap-time/quality repair path (poor `vmax_attain` / `tempo` beyond the `v_target` run-out check) | Quality, not validity; #30's spec already flagged it on the branch that was taken | Yes — raise separately if not already raised |
| `v_target` promotion to a `GenParams` field | `phase3_start_finish` already takes `v_target: i32` as a call parameter; matching that convention keeps this task free of a `GenParams` change | No — a `generate()`-wiring decision |
| The medial-axis tie-break quality refinement | Left un-adopted by #30 for want of evidence; the same deferral applies to Ф6's arms | No |

## Key decisions

| Question | Decision |
|---|---|
| **KD1** — scope of the three producerless dispatch arms | **Build the detectors too** (owner, round 1). All six design labels get a producer in this task; no arm ships as dead vocabulary. Producing conditions pinned in § The three new detectors. |
| **KD2** — Ф6's loop shape | **Single pass** (owner, round 1). `phase6_local_repair` iterates the issue list once, at most one edit per issue, with the type-scoped recheck inside the pass. The repair-budget loop stays with `generate()` (`todo!`). |
| **KD3** — progress definition driving `FAILED` | **Per-arm metric** (owner, round 1). Each arm verifies its **own** metric on a scratch copy *before* committing its edit; an edit whose metric does not strictly improve is **not committed**. Ф6 returns the repaired `D` iff **≥ 1** edit was committed, otherwise `FAILED`. Metrics enumerated in § Per-arm progress metrics. |
| What "one dual edge" means operationally | **One identified `gp_core::geom::Wall`, one cell drivability flip** — the semantics #30 already committed (`RepairCandidate::Edge(w)` names one wall; the edit sets `wall_neighbor(w)` drivable). A cell flip may make several boundary walls appear or disappear at once; that is not a multi-edge edit. Where more than one wall identifies the same flip, the canonical one is the min `wall_sort_key` (`phase5b.rs`, `pub(crate)`). |
| `push_outer_wall_out(until width >= target)` vs AC1 | One edge **per edit**, unconditionally. The design pseudocode's "until width ≥ target" is realised as repeated single-edge edits across `generate()`'s repair-budget iterations, never as a multi-cell sweep inside one edit. |
| Add-edits and the single-hole invariant | `[C3]`'s "add-edits are monotonically safe" is about **lap existence** (`R`, `B`, `live` only grow) — it does **not** cover topology: filling a cell can empty or split the infield hole. Each add-arm therefore carries a **locally decidable** hole-preservation guard (see `ConcaveChordCut`'s degree-1 condition), not a whole-corridor flood-fill, so AC2's add→local rule stays honest. |
| Bounding-box growth | **Ф6 never re-boxes `D`.** `Corridor::set` is a documented **no-op outside the bounding box**, and Ф2 pads `D0` by exactly `BBOX_PAD = 1` fine cell per side (`crates/gen/src/phase2.rs`), so an outward add-edit has at most one cell of headroom at the outer perimeter. An add-edit whose target cell is out of box is **not a candidate** (it would be a silent no-op) — the same treatment `map_frontier_gap_to_edge` already gives it. |
| `Issue::Disconnected` / `Issue::BadTopology` | **Not repairable by a single dual-edge shift** — whole-topology verdicts with no locality payload (both are unit variants). Ф6 dispatches them to a defined *decline* outcome (no edit, no metric), contributing nothing to progress; if they are the only issues, Ф6 returns `FAILED`. |
| Where the `Issue` vocabulary lives | `Issue` (`crates/gen/src/phase4.rs`) becomes **Ф6's dispatch vocabulary**, not solely Ф4's static vocabulary, and gains `NoBraking`, `ConcaveChordCut`, `ArmsMerging`. One enum, one place. (`DynamicallyDisconnected` is still **not** added — #30 settled that the dynamic verdict rides `OracleResult::NotLappable`.) |
| Where the detector *bodies* live | The two **static** detectors extend `phase4_static_checks`' output (Ф4 is the single static-validation entry point; a second entry point would fragment its "empty ⟺ statically valid" contract), with their bodies + tests in a **new sibling module** — `phase4.rs` is **754 lines**, and inlining them would cross the 800-line incl.-tests soft limit. The **run-out** detector is a new **dynamic** check in its **own new module**: `phase5b.rs` is **1145 lines**, already past that soft limit, so it is not extended. Exact module names are design's call. |
| Ф6's own signature | Design's call. Ф6 needs materially more than the pseudocode's `(D, issues, n, m)`: also `skel` + `k` (infield mask), `grid` + `sf` + `race_dir` (dynamic arms), `v_target` (run-out), and the Ф5b `stall_walls`. Bundling into a context struct is acceptable and probably preferable. |
| Failure signalling shape | A **dedicated enum**, never a bare `Option<Corridor>` / sentinel `Corridor` — the reasoning #30 applied to `RepairCandidate`. Exact variants are a design call. |
| Determinism | Required: identical inputs → identical outcome, including which edit is chosen and in what order. `HashSet`/`HashMap` allowed internally; every decision reaching the output is sorted or aggregated (`wall_sort_key`, `Point` order — the Ф5a/Ф5b/#30 discipline). |
| Integer-only, total, panic-free | Required. `gp-gen` follows the integer-only determinism rule (`docs/design.md` §3a) and the Ф1–Ф5b house style: no production `panic!`/`unwrap`, `saturating_*` / `checked_*` / `try_from(..).unwrap_or(..)` throughout. |
| New dependency | None. `gp-gen`'s set (`gp-core`, `rand`, `rand_xoshiro`, `strum` — verified in `crates/gen/Cargo.toml`) is sufficient. |
| Test placement / fixtures | `#[cfg(test)] mod tests` per implementing module; shared fixtures in the existing `crates/gen/src/testfix.rs` (already holds `ring_corridor` / `ring_sf` / `ring_grid` / `dead_end_corridor` / `crash_pocket_fixture` / `long_straight_*` / `trap_ring`). |
| Miri | `gp-gen` rides the sanctioned crate-level Miri `--exclude` (#134, OPEN — verified) — no per-test `#[cfg_attr(miri, ignore)]` needed. |
| API stability | Free rein. AGENTS.md § *API Stability*: `gp-gen` is a game-app crate, never published, no downstream consumer — rename/restructure cleanly, add no aliasing layer. Existing Ф4 tests that assert an exact issue list may need updating; that is expected, not a regression. |

## The three new detectors

Each is stated as: **producing condition** (exact, integer-only, deterministic) ·
**payload** · **fixture**.

### 1. `Issue::ConcaveChordCut { tooth: Point }` — static, Ф4 family

Design §2 Ф6: *"supercover cuts a concave corner → remove the inner tooth"*. A
one-cell non-drivable protrusion into the corridor is exactly the concavity the
strict supercover predicate (`docs/design.md` §3 C4) refuses to graze past,
blocking an otherwise-legal fast chord.

- **Producing condition.** An in-box cell `c` with `¬d.contains(c)` such that
  **exactly 3** of its four 4-neighbours are drivable (equivalently: exactly one
  is not, counting out-of-box as not drivable) — a degree-1 tooth. Plus the
  hole-preservation guard: if `c` belongs to the bounded complement component
  (the infield hole), that component must have **≥ 2** cells. Both clauses are
  decidable from `c`'s 4-neighbourhood plus one component-size query, so the
  fill's local recheck stays local (KD *Add-edits and the single-hole
  invariant*).
- **Why degree-1 and not "≥ 3 drivable".** Degree 1 means `c` is a leaf of its
  complement component, so filling it can neither split that component nor
  (given the `≥ 2` guard) empty it. Topology preservation is then provable
  locally rather than by a global flood-fill.
- **Payload.** `tooth: Point` — the cell the repair makes drivable. One issue
  per tooth, emitted in ascending `Point` order.
- **Fixture.** A hand-built ring with one infield cell poked out into the
  corridor as a degree-1 tooth; assert exactly one `ConcaveChordCut`, that the
  fill makes it drivable, and that exactly one bounded complement component
  survives. Negative fixtures: a degree-2 notch (two non-drivable neighbours)
  must **not** fire, and a 1-cell hole must **not** fire.

### 2. `Issue::ArmsMerging { bridge: Point }` — static, Ф4 family

Design §2 Ф6: *"arms merge / the hole dies → trim the arm wall or nudge the
finger"*; design §2 names the failure mode *"the perturbation merges the
corridor through the infield (the hole is lost)"*.

- **Producing condition.** A drivable fine cell `p ∈ D` lying inside the
  **expanded coarse-hole mask** `H = ⋃ { block_points(c, k) : c ∈ skel.hole }` —
  a drivable intrusion into the infield, i.e. the bridge across which two arms
  merge. `block_points(c, k)` already exists (privately) in both `phase2.rs` and
  `phase4.rs`; Ф2 Stage 2 protects `H` by construction, so a cell in `H ∩ D` is
  by definition a post-Ф2 perturbation.
- **Relation to `LostHairpin`.** `LostHairpin` (already committed) fires when a
  whole coarse **finger**'s footprint is drivable — a strict special case.
  `ArmsMerging` is the general, one-cell-granularity signal. Where both fire,
  `LostHairpin` takes precedence (it carries the coarse `tip` anchor the
  `nudge_finger` arm wants); the dispatch must be deterministic and idempotent
  across the overlap.
- **Payload.** `bridge: Point` — the drivable intrusion cell the repair makes
  non-drivable. One issue per 4-connected intrusion component, anchored at that
  component's min `Point`, emitted in ascending `Point` order.
- **Fixture.** A ring whose infield has one cell set drivable; assert exactly
  one `ArmsMerging { bridge }`, that the remove-edit clears it, and that the
  global flood-fill afterwards reports connected + exactly one bounded hole.

### 3. `Issue::NoBraking { at: Point }` — dynamic, Ф5 family, new module

This is the arm the AC7 result makes subtle, so its producing condition is
stated in full.

**It is a `v_target`-referenced run-out check, not a lap-existence check.** Per
#30's executed AC7 proof, no track that is V=1 lappable can be un-lappable at a
higher `V_ceil`, so "the corner cannot be taken at all" is never a producible
verdict — a driver can always crawl. What design §2 Ф6 means by *"the corner is
not braked"* is the **Ф3 run-out budget applied per corner**: design §2 Ф3
requires an accel zone `≥ ~V_target²/2` points, and the same budget must hold in
reverse ahead of every corner.

- **Inputs.** A `Lappable` oracle result (`phase5_full_oracle` →
  `OracleResult::Lappable(TrackMetrics)`), which supplies the along-track
  ordering `metrics.fastest_lap: Vec<Point>` and `metrics.speed_heatmap:
  Vec<(Point, i32)>`; plus `d`, `grid`, `sf`, `race_dir`, and `v_target: i32`
  (already an established call-parameter convention —
  `phase3_start_finish(d, skel, m, v_target)`).
- **Speed sink.** A path point `p` on `fastest_lap` whose heatmap value is
  `≤ 1` — *every live state there is already slow*, design §2 `[C3]`'s own
  definition (a hairpin, or the start of the accel zone). The sink index set
  **unconditionally includes path index `0`**, which is what makes "nearest
  upstream sink" total. Justification: `path[0]` is a start-grid position and the
  race-start seeds are at rest, so a flood seeded there reproduces the **global**
  race-start flood — a conservative superset, never a stale window. (It is *not*
  sound to argue the heatmap makes start cells sinks by construction:
  `speed_heatmap` is a **per-point max over all live states**
  (`crates/gen/src/phase5b.rs:236-246`, pinned by
  `speed_heatmap_is_per_point_max_and_sorted_by_point`), so a start cell
  traversed fast on a later lap has heatmap `> 1` — amendment 2.)
- **Travel direction.** `dir(c)` = dominant-axis sign of the frozen `fastest_lap`
  step at `c` (tie → x). `end(D,c)` = the last drivable cell from `c` along
  `dir(c)` — the wall the braking ray hits.
- **Run-out room — geometric, not path-indexed.** `runout_room(D,c) =
  wall_run(D, c, dir(c)) − 1`, measured on the corridor being evaluated (`D` to
  detect, the scratch copy to re-check) with Ф4's own ray-length helper
  `wall_run` (`crates/gen/src/phase4.rs:96`, private today — widening its
  visibility is design's call). This term is the `lengthen_straight` arm's lever:
  it **grows** under that add-edit. A path-indexed run-out room would be constant
  under any edit (AC3 freezes the path), which is what made the arm vacuous —
  amendment 1.
- **Attainable entry speed.** `attainable(D,c)` = max `vnorm` over
  forward-reachable states at `c`, flooded **from the live states at the nearest
  upstream sink** (all with `|v| ≤ 1` by the sink definition) — i.e. the
  sink-to-sink computation, not a global flood and not a fixed-radius window.
  **Unchanged by amendment 1**; AC3's sink-to-sink requirement holds verbatim.
  This term grows *adversely* under an add-edit, which is exactly design §2
  `[C3]`'s "lengthening the straight also raises the arrival speed" effect.
- **Corner speed — the `widen_corner` lever.** `v_corner(D,c)` = max `vnorm` over
  arrival states at `end(D,c)` that have **≥ 1 legal successor** in `D`. It
  **grows** under the `widen_corner` add-edit (a wider corner lets a faster
  arrival keep a legal successor), giving that arm its own real lever.
- **Entry speed.** `v_entry(D,c) = min(v_target, attainable(D,c))`.
- **Braking cells.** `braking_cells(from, to)` = cells needed to decelerate from
  `from` to `to` under `±1`-per-turn deceleration, `0` when `to ≥ from`; to rest
  this is `v·(v+1)/2` (design §2 Ф3 writes the same budget approximately as
  `V_target²/2`, the form `phase3.rs`'s own AC2 test uses — design picks one and
  pins it).
- **Producing condition.** `NoBraking { at: c }` fires at path point `c` iff
  `deficit(D,c) = braking_cells(v_entry(D,c), v_corner(D,c)) − runout_room(D,c)`
  is `> 0`.
- **Payload.** `at: Point` — the corner-entry path point. One issue per maximal
  run of deficient points, anchored at the run's first point along
  `fastest_lap`, so emission is deterministic.
- **Repair arm.** `lengthen_straight` or `widen_corner`, both **add**-edits: one
  cell made drivable, extending the straight upstream of `c` or widening `c`'s
  outer wall. Selection between them is design's call, but must be
  deterministic.
- **Fixture.** A ring with a long straight feeding a tight corner, sized so the
  condition fires. Required in two directions: (a) a repair that genuinely fixes
  it (deficit → `≤ 0` under the sink-to-sink recheck), so the arm is not
  vacuous; (b) the **AC3 discriminating** case below.

## The `[C3]` recheck contract

Selected by edit type, never by a fixed radius:

| Edit type | Arms | Recheck |
|---|---|---|
| **add** | `push_outer_wall_out`, `fill_inner_tooth`, `map_frontier_gap_to_edge` | **Local only.** The arm's own local measurement (§ Per-arm progress metrics), plus the local hole-preservation guard for fills. **No** whole-corridor flood-fill: add-edits only grow `R`, `B`, `live`, so lap existence cannot regress. |
| **remove** | `trim_arm_wall`, `nudge_finger` | **Global flood-fill.** `component_count(d) == 1` **and** `bounded_complement_components(d) == 1` on the scratch-edited `D`. A removal failing either check is **not committed**. |
| **run-out** | `lengthen_straight`, `widen_corner` | **Speed-sink → speed-sink.** Re-derive `attainable(c)` by flooding forward from the live states at the nearest **upstream** sink, through the edited region, to the next **downstream** sink, and recompute the deficit on the scratch-edited `D`. |

**Why sink-to-sink is sound and a fixed radius is not** (design §2 `[C3]`,
restated so the AC is testable): a speed sink is a **cut** in the state space —
every live state there has `|v| ≤ 1`, so downstream reachability is independent
of everything upstream of it, and a flood seeded there reproduces the global
answer on the window. A fixed-radius disc boundary is **not** a cut: states
crossing it can be fast, so a radius-`N` recheck reads **stale upstream
reachability**. Lengthening a straight fixes the local geometry *and* raises the
attainable arrival speed at the corner; the radius check sees only the former
and declares the corner fixed while it is still deficient. That is precisely the
failure design §2 `[C3]` names, and precisely what AC3's discriminating fixture
must exhibit.

## Per-arm progress metrics

Each metric is measured on a **scratch copy** before the edit is committed;
`FAILED` ⟺ **zero** edits were committed across the whole pass (KD3).

| Defect | Arm | Edit type | Metric — commit iff |
|---|---|---|---|
| `Narrow` / `NarrowSf` | `push_outer_wall_out` | add | The measured cross-section width at the issue's `center` along its `axis` **strictly increases** (recomputed with Ф4's own width routine on the scratch `D`). |
| `ConcaveChordCut` | `fill_inner_tooth` | add | The named `tooth` **became drivable** (it was not drivable on the working `D` and is on the scratch) **and** exactly **one** bounded complement component survives (the local hole guard). |
| `NoBraking` | `lengthen_straight` / `widen_corner` | add (run-out) | The deficit `braking_cells(v_entry(D,c), v_corner(D,c)) − runout_room(D,c)` (§ The three new detectors, 3) **strictly decreases** under the **sink-to-sink** re-verification, every term re-measured on the scratch `D`. |
| `ArmsMerging` / `LostHairpin` | `trim_arm_wall` / `nudge_finger` | remove | `\|H ∩ D\|` (drivable cells inside the coarse-hole mask) **strictly decreases** **and** the global flood-fill still reports connected + exactly one bounded hole. |
| `NotLappable { stall_walls }` | `map_frontier_gap_to_edge` | add | `\|P0\|` at `V_ceil = 1` **strictly grows** — already implemented and verified inside #30's mapper; `RepairCandidate::NoCandidate` means "no edit", contributing nothing to progress. |
| `Disconnected` / `BadTopology` | *(decline)* | — | Never commits an edit; contributes nothing to progress. |

## Technical constraints

### State of the art (verified against `crates/gen/src/`, not the issue body)

- **Ф4 — `crates/gen/src/phase4.rs`** (#27, CLOSED; **754 lines**). `pub fn
  phase4_static_checks(d, skel, k, n, m, sf) -> Vec<Issue>`. `Issue` has exactly
  **five** variants: `Disconnected` · `BadTopology` ·
  `Narrow { center, axis, width }` · `NarrowSf { center, axis, width }` ·
  `LostHairpin { tip }`. The width payloads already carry the locality Ф6 needs
  ("Ф6 pushes the two capping outer walls apart along this axis"); `LostHairpin`
  carries the coarse `tip`, "the anchor Ф6's `nudge_finger` acts near". Private
  helpers `block_points`, `infield_fingers`, `absorbed`, `wall_runs` live here.
- **Ф5a — `crates/gen/src/phase5.rs`** (#28, CLOSED). `pub fn
  oracle_liveness_v1(d, grid, sf, race_dir) -> bool` (bare `bool`, no payload);
  `forward_reachable` / `backward_reachable` are `pub`; `ORACLE_V1_CEIL: i32 = 1`
  and `within_v_ceil` are `pub(crate)`.
- **Ф5b — `crates/gen/src/phase5b.rs`** (#29, CLOSED; **1145 lines**). `pub fn
  phase5_full_oracle(d, grid, sf, race_dir) -> OracleResult`, with
  `Lappable(TrackMetrics)` | `NotLappable { stall_walls: Vec<Wall> }`.
  `pub(crate)` helpers available for reuse: `wall_sort_key`, `wall_neighbor`,
  `p0_boundary_walls`, `live_at`, `lap_close_goals`, `crosses_sf_forward`,
  `vnorm`, `speed_heatmap`, `fastest_lap_through_live`.
- **Ф6 (partial) — `crates/gen/src/phase6.rs`** (#30, CLOSED; **504 lines**).
  `pub fn map_frontier_gap_to_edge(d, grid, sf, race_dir, stall_walls: &[Wall])
  -> RepairCandidate`, `RepairCandidate` = `Edge(Wall)` | `NoCandidate`. A
  verified-growth greedy: re-validates each input wall against `d`,
  scratch-applies, keeps only strict `|P0|` growth at `V_ceil = 1`, picks max
  growth with a `wall_sort_key` tie-break. `p0_at_v1(d, grid, sf) ->
  HashSet<Point>` is `pub(crate)` and directly reusable.
- **`TrackMetrics`** (`gp-core`, `pub`): `vmax_attain: Option<i32>`,
  `tempo: Option<f32>`, `fastest_lap: Vec<Point>`,
  `speed_heatmap: Vec<(Point, i32)>` — the along-track ordering and per-point
  peak speed the run-out detector needs are both already exported.
- **`v_target`** is an established call parameter: `pub fn
  phase3_start_finish(d, skel, m, v_target: i32)`. `GenParams` has no `v_target`
  field (`cars`, `min_straight`, `v_ceiling`, `block_size`, `seeds`);
  `v_ceiling` is the *oracle scaffold*, explicitly not `V_target` (#30 spec).
- **`gp-core::geom`** offers `flood_fill`, `component_count`,
  `bounded_complement_components`, `walls_from_boundary`, `geodesic_layers`,
  `CorridorScratch`, `DistanceTransform`, `medial_axis`, `supercover` — all
  `pub`.
- **`generate()`** in `crates/gen/src/lib.rs` is `todo!`.

### The AC7 result and what it forces on `NoBraking`

#30 executed an **executable proof gate** establishing
`oracle_liveness_v1(..) == matches!(phase5_full_oracle(..), Lappable(_))`, for a
structural reason: `live` is monotone in `V_ceil`, so `phase5_full_oracle`'s
"no fastest lap" arm can only fire on its **first** iteration at `V_ceil = 1`.

Consequence: the oracle **never** reports a dynamic-only stall, so `NoBraking`
cannot be defined as "the oracle says this corner is impassable". It is defined
instead against the `v_target` run-out budget (§ The three new detectors, 3).
Any design that re-derives `NoBraking` from a lappability verdict contradicts a
proven, test-pinned theorem.

### Bounding-box headroom

`Corridor` has a **fixed** bounding box and `Corridor::set` is a documented
**no-op** outside it. Ф2 allocates `D`'s box with `BBOX_PAD = 1` fine cell of
padding per side. An outward add-edit therefore has at most one cell of headroom
against the outer perimeter — a real constraint on `push_outer_wall_out` and
`lengthen_straight` that fixtures must respect.

### Risk: the AC3 discriminating fixture

Constructing a fixture where the two recheck scopes **disagree** is the hardest
single piece of this task. It needs: a straight feeding a corner; a repair edit
that lengthens the straight; a radius-`N` window that (with `N` small enough to
be a plausible "local" check) contains the edit and the corner but not the
upstream sink; and speeds tuned so the extra cell of straight raises
`attainable(c)` by enough to keep the deficit positive. If no such fixture can be
built, AC3 must **not** be silently downgraded to a shape assertion — that is a
Design Amendment trigger, and the honest fallback is a directly unit-tested
`attainable(c)` comparison between the two seedings (sink-seeded vs
window-boundary-seeded) on the same `D`.

### Regression risk: new detectors firing on well-formed Ф2 output

Adding checks to `phase4_static_checks` changes its output for every existing
caller and test. Ф2 is *expected* to be clean for both new static detectors by
construction (`stage2b_absorb_pockets` fills every complement pocket touching
neither the box border nor the infield mask `H`; Stage 2's taper protects `H`),
but "expected" is not "verified" — AC8 pins the observed behaviour so a future
change to Ф2 or to a detector cannot silently alter it.

### Amendments (round 3)

Three defects in this spec's own formulas were surfaced by the `design`
subagent, **independently verified against live code** by the orchestrator, and
**approved by the owner**. The design document
(`2026-07-24-gp-gen-phase6-local-repair.design.md` §§ Decision 2, Risks R3/R4/R5)
already encodes them; the sections above are amended to agree with it. No AC was
renumbered and no AC's intent changed — AC4's referent is the amended
§ Per-arm progress metrics table.

| # | What changed | Why |
|---|---|---|
| **A1** (design R3) | § The three new detectors, 3 + § Per-arm progress metrics `NoBraking` row: `runout_room` is now **geometric** (`wall_run(D, c, dir(c)) − 1`, grown by `lengthen_straight`) and the deficit gains a measured `v_corner` term (grown by `widen_corner`): `deficit = braking_cells(v_entry, v_corner) − runout_room`, `v_entry = min(v_target, attainable)`. The **sink-seeded flood for `attainable` is unchanged** — AC3 holds verbatim. | The previous path-indexed `runout_room` made **every** `NoBraking` add-arm provably vacuous: AC3 freezes the path, so that term is constant under an edit, while `attainable` is monotone non-decreasing under an add-edit ⇒ the deficit could never strictly decrease ⇒ no run-out edit would ever be committed, contradicting AC3's non-vacuity clause and KD1. |
| **A2** (design R4) | § The three new detectors, 3, *Speed sink*: the "sink set is non-empty by construction — every start-grid position is at rest" justification is replaced by unconditionally including path index `0`, justified by the race-start seeds being at rest. | The old argument was unsound: `speed_heatmap` is a per-point max over **all** live states (`crates/gen/src/phase5b.rs:236-246`, pinned by `speed_heatmap_is_per_point_max_and_sorted_by_point`), so a start cell traversed fast on a later lap has heatmap `> 1` and is not a sink — the sink set can be empty. The unconditional `0` restores totality as a conservative superset, never a stale window. |
| **A3** (design R5) | § Per-arm progress metrics, `fill_inner_tooth` row: "the tooth count **strictly decreases**" → "the named `tooth` **became drivable** and exactly one bounded complement component survives". | The old metric is unachievable. Counterexample: a 3-cell line hole `A–B–C` has leaves `A` and `C` (count 2); filling leaf `A` leaves `B–C`, both leaves ⇒ count still 2. The replacement is strictly local, strictly improving, and is the arm's actual objective. |

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Every repair edit shifts **exactly one** dual edge: applying it flips the drivability of **exactly one** cell of `D`, and the edit is identified by exactly one `gp_core::geom::Wall`. Asserted per arm by cell-count diff on fixtures. |
| AC2 | **Add-edits** (`push_outer_wall_out`, `fill_inner_tooth`, `map_frontier_gap_to_edge`) trigger **only** a local recheck — no whole-corridor flood-fill; **remove-edits** (`trim_arm_wall`, `nudge_finger`) trigger a **global flood-fill** asserting `component_count == 1` and `bounded_complement_components == 1`. The scope actually taken is observable from the function's own output or an equivalently direct signal, never inferred from timing. |
| AC3 | Run-out repairs re-verify **speed-sink → speed-sink**: the re-verification is seeded at the live states of the nearest **upstream** sink and runs forward through the edit to the next **downstream** sink. A fixture exists on which a fixed-`N`-cell-radius recheck reports "fixed" while the sink-to-sink recheck correctly reports "still deficient", **and** a second fixture on which a correct repair does clear the deficit (non-vacuity). |
| AC4 | Ф6 returns `FAILED` iff **no** edit was committed in the pass; otherwise it returns the edited `D`. An edit is committed **only** when its own arm's metric (§ Per-arm progress metrics) strictly improved on a scratch copy. Never returns an unchanged `D` as a success. |
| AC5 | Dispatch is **total** over the whole defect set: each of the six design labels reaches its arm, and `Disconnected` / `BadTopology` reach a defined *decline* outcome — no panic, no silent skip. |
| AC6 | Each of the three new detectors emits its issue **exactly** on its producing condition: one positive fixture and at least one near-miss negative fixture per detector (degree-2 notch and 1-cell hole for `ConcaveChordCut`; clean infield for `ArmsMerging`; adequate run-out room for `NoBraking`). |
| AC7 | The committed `map_frontier_gap_to_edge` is wired in as the `DYNAMICALLY_DISCONNECTED` arm with **unchanged semantics**, and its `NoCandidate` result routes to "no edit / no progress" rather than being swallowed. |
| AC8 | The new detectors' behaviour on a deterministic Ф1→Ф2 output (`phase1_coarse_ring` → `phase2_rasterize`, fixed seed) is **pinned by assertion**, so a later change to Ф2 or to a detector cannot silently alter it. |
| AC9 | **Deterministic and total**: identical inputs yield an identical outcome (same edits, same order); no production panic, no `unwrap`, integer-only. Asserted against adversarial inputs too — empty issue list, out-of-box walls, degenerate zero-area corridor — the totality battery `map_frontier_gap_to_edge` already carries. |
| AC10 | Gates green: `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`. Every new public item carries a `///`. |
| AC11 | No `gp-core` change; no new dependency; `generate()` still `todo!`; no `Issue::DynamicallyDisconnected` variant added. |

## Open questions

- **Braking-distance form.** Exact `v·(v+1)/2` vs design §2 Ф3's approximate
  `V_target²/2` (the form `phase3.rs`'s own AC2 test uses). Design picks one and
  pins it in rustdoc; either is defensible and the two differ by half a step.
- **`lengthen_straight` vs `widen_corner` selection.** Which of the two
  `NoBraking` add-edits to try, and in what order, when both are admissible.
  Must be deterministic; the natural default is "try both on scratch copies,
  take the larger deficit reduction, tie-break by `wall_sort_key`" — the same
  verified-greedy pattern #30 established.
- **Issue ordering within the single pass.** Whether Ф6 processes the issue list
  in emitted order or in a fixed severity order (e.g. removes before adds, so a
  topology bridge is trimmed before a width push widens it further). Design's
  call; both are deterministic.
- **`v_target` promotion to `GenParams`.** Deferred (§ Deferred) — a
  `generate()`-wiring decision, not this task's.
