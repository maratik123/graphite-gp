# Design: gp-gen Ф6 — local repair (one dual edge per edit + edit-type-scoped recheck `[C3]`)

**Issue:** [#31](https://github.com/maratik123/graphite-gp/issues/31)
**Spec:** `ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.spec.md`
**Date:** 2026-07-24

## Approach

Five new deliverables land as **five new sibling modules** plus targeted widening of
existing Ф4 helpers. `generate()` stays `todo!`; no `gp-core` change; no new dependency.

### Module layout (decision — spec § KD *Where the detector bodies live*, "exact module names are design's call")

| Module | Contents | Why not an existing file |
|---|---|---|
| `crates/gen/src/coarse.rs` | `pub(crate) block_origin` / `block_points` (coarse→fine `k×k` mapping) | Currently duplicated privately in `phase2.rs` **and** `phase4.rs`; this task adds two more consumers (`phase4_defects`, `phase6_arms`) → **4 sites** ⇒ the ≥3-site shared-crate/module rule fires. Lift once, delete both copies. `[measured: grep -n "fn block_points" crates/gen/src/phase{1,2}.rs crates/gen/src/phase4.rs → phase2.rs:53, phase4.rs:247]` |
| `crates/gen/src/phase4_defects.rs` | `ConcaveChordCut` + `ArmsMerging` detector bodies, their fixtures/tests, the AC8 Ф1→Ф2 pin, and `axis_width` (new code, sited here rather than in `phase4.rs` — § Risks R1 Mitigation 1) | `phase4.rs` is **754 lines**; inlining bodies + tests crosses the 800-line incl.-tests soft cap `[measured: wc -l crates/gen/src/phase4.rs → 754]` |
| `crates/gen/src/phase5_runout.rs` | The run-out model (`triangular`, `braking_cells`, `sink_indices`, `travel_dir`, `runout_room`, `WindowFlood`/`window_speed`, `corner_speed`, `deficit_at`) + `phase5_runout_checks` (the `NoBraking` detector body; the `Issue::NoBraking` **variant** is declared in `phase4.rs`'s one enum, subtask 2) | `phase5b.rs` is **1145 lines**, already past the soft cap `[measured: wc -l crates/gen/src/phase5b.rs → 1145]` |
| `crates/gen/src/phase6_arms.rs` | Edit plumbing (`in_box`, `add_edit_wall`, `remove_edit_wall`) + all five repair arms, `pub(crate)` | `phase6.rs` is **504 lines** and owns the settled `map_frontier_gap_to_edge` contract (#30) — not reopened `[measured: wc -l crates/gen/src/phase6.rs → 504]` |
| `crates/gen/src/phase6_repair.rs` | Public types (`RepairContext`, `RepairOutcome`, `CommittedEdit`, `RepairArm`, `RecheckScope`), `issue_sort_key`, the single-pass dispatch driver, `[C3]` recheck routing | As above |

Naming avoids `phase4b` / `phase6b`: `Ф4b`/`Ф6b` are **not** design-doc labels (unlike `Ф5b`),
so a `b`-suffix would invent a phase.
`[measured: grep -o "Ф[0-9][a-z]*" docs/design.md | sort -u → Ф1 Ф2 Ф3 Ф4 Ф5 Ф5a Ф5b Ф6 Ф7 — no Ф4b, no Ф6b]`

**Pre-decided split rule (do not re-derive mid-flight):** if `phase6_arms.rs` exceeds **800 lines
incl. tests** at subtask 11, mechanically move `trim_arm_wall` + `nudge_finger` + their tests into
`crates/gen/src/phase6_remove.rs` — a move, no logic change.

### Ф6's signature and failure-signalling shape (spec § KD *Ф6's own signature* / *Failure signalling shape*)

```rust
pub fn phase6_local_repair(ctx: &RepairContext<'_>, issues: &[Issue]) -> RepairOutcome
```

`RepairContext<'a>` (all `pub`, all documented — `missing_docs` is `deny`):
`d: &'a Corridor` · `skel: &'a CoarseSkeleton` · `k: i32` · `n: u32` · `m: u32` ·
`grid: &'a StartGrid` · `sf: &'a StartFinish` · `race_dir: RaceDir` · `v_target: i32` ·
`metrics: Option<&'a TrackMetrics>` · `stall_walls: Option<&'a [Wall]>`.

`metrics` and `stall_walls` are the two mutually-exclusive Ф5b outcomes (`Lappable` /
`NotLappable`); `None` on either simply declines that family's arms. A context struct (rather
than 11 parameters) also keeps `clippy::too_many_arguments` (pedantic, deny) satisfied.

```rust
pub enum RepairOutcome {
    /// ≥1 edit committed. `edits` is non-empty by construction, in pass order.
    Repaired { d: Corridor, edits: Vec<CommittedEdit> },
    /// Zero edits committed — the `[N4]` reseed signal.
    Failed,
}

pub struct CommittedEdit {
    pub arm: RepairArm,          // which arm produced it
    pub wall: Wall,              // the single dual edge naming the flip (AC1)
    pub cell: Point,             // the single cell whose drivability flipped (AC1)
    pub drivable: bool,          // true = add-edit, false = remove-edit
    pub recheck: RecheckScope,   // the scope actually taken (AC2)
}

pub enum RepairArm { PushOuterWallOut, FillInnerTooth, LengthenStraight,
                     WidenCorner, TrimArmWall, NudgeFinger, MapFrontierGap }
pub enum RecheckScope { Local, GlobalFloodFill, SinkToSink }
```

Rejected: `Option<Corridor>` / a sentinel `Corridor` — the same reasoning #30 applied to
`RepairCandidate`. Rejected: returning a bare `Corridor` — `Corridor` does not impl `PartialEq`
`[measured: sed -n 240,242p crates/core/src/geom/mod.rs → #[derive(Clone, Debug, Default)]]`, so
AC4's "never returns an unchanged `D` as a success" would be untestable without the `edits` list.

Per-issue dispatch is a separate `pub(crate)` function so AC5 totality is unit-testable:

```rust
pub(crate) enum ArmOutcome { Edit(CommittedEdit), NoEdit(DeclineReason) }
pub(crate) enum DeclineReason { NotRepairable, MissingOracleInput, StalePayload,
                                NoCandidate, MetricNotImproved, RecheckFailed }
pub(crate) fn dispatch(ctx, working: &Corridor, label: DispatchLabel) -> ArmOutcome
```

### AC2: the recheck scope is read off the output, never off timing

One `const fn recheck_scope(arm: RepairArm) -> RecheckScope` maps arm → scope; the driver passes
**that value** to `verify(scope, …)` and records **the same value** in `CommittedEdit.recheck`.
Label-reading alone is weak evidence, so AC2 is additionally pinned by two
**consequence discriminators** (§ Test Design), each a fixture where the two scopes disagree:

- *add → local only*: `push_outer_wall_out` on a corridor that is **globally disconnected
  elsewhere**. A global flood-fill recheck would reject (`component_count != 1`); the local
  width metric improves, so the edit **must** commit. Committing proves no global flood-fill ran.
- *remove → global*: `trim_arm_wall` on a removal that strictly decreases `|H ∩ D|` (local metric
  passes) but **disconnects** `D`. It must **not** commit. Rejection proves the flood-fill ran.

### `[C3]` recheck routing

| Arm | Scope | What the driver runs on the scratch copy |
|---|---|---|
| `PushOuterWallOut`, `FillInnerTooth`, `MapFrontierGap` | `Local` | the arm's own local metric only — **no** `component_count`, **no** `bounded_complement_components` |
| `LengthenStraight`, `WidenCorner` | `SinkToSink` | `deficit_at(scratch, c) < deficit_at(working, c)` — the same `deficit_at` the detector uses |
| `TrimArmWall`, `NudgeFinger` | `GlobalFloodFill` | `\|H ∩ D\|` strictly decreases **and** `component_count == 1` **and** `bounded_complement_components == 1` |

### Decision 1 — braking-distance form: **exact `v·(v+1)/2`** (pinned)

`triangular(v) = v·(v+1)/2`; `braking_cells(from, to) = triangular(from) − triangular(to)`, `0`
when `to ≥ from`. Reason: the comparison is a strict `>` on small integers where design §2 Ф3's
`V_target²/2` differs by half a step and flips the verdict; `v(v+1)` is always even so integer
division is exact; and `v + (v−1) + … + (to+1)` is the true cell count under cardinal `±1`-per-turn
deceleration `[measured: grep -n "pub enum Action" -A 12 crates/core/src/sim/mod.rs → 5 variants
Coast/East/West/North/South, one axis per turn]`. `phase3.rs`'s own AC2 test keeps its
`v_target²/2` form untouched (out of scope — it measures Ф3's accel zone, not Ф6's deficit)
`[measured: grep -n "v_target.saturating_mul" crates/gen/src/phase3.rs → 961: let threshold = v_target.saturating_mul(v_target) / 2; // integer division]`.

Totality + lint shape (both forced, not stylistic):

```rust
/// Largest v with v·(v+1) ≤ i32::MAX (46_340·46_341 = 2_147_441_940).
const MAX_BRAKE_SPEED: i32 = 46_340;

#[allow(clippy::arithmetic_side_effects, reason = "v is clamped to 0..=MAX_BRAKE_SPEED above, \
        so v*(v+1) <= i32::MAX; the divisor is the non-zero literal 2")]
const fn triangular(v: i32) -> i32 { /* if-chain clamp (Ord::clamp is not const-stable) */ }
```

`arithmetic_side_effects` **does** fire on `/` with a literal divisor in this workspace — both
in-tree divisions carry an explicit `#[allow]`
`[measured: sed -n 465,471p crates/gen/src/phase1.rs → #[allow(clippy::arithmetic_side_effects, reason = "… base_w / 2 …")]]`
`[measured: sed -n 120,131p crates/gen/src/phase2.rs → fn-level #[allow] covering const NEG_INF: i32 = i32::MIN / 2]`.
`missing_const_for_fn` (nursery, deny) forces `const fn` here — the body is integer arithmetic and
an if-chain, all const-callable on stable (the `wall_sort_key` / `vnorm` precedent, `phase5b.rs:79,225`).

### Decision 2 — the run-out model (corrective refinement; see § Risks R3)

`NoBraking` is a `v_target`-referenced run-out budget check, **never** derived from a lappability
verdict (#30 AC7). The spec's stated condition is

> `braking_distance(min(v_target, attainable(c))) > runout_room(c)`, `runout_room(c)` = *fastest_lap
> steps from `c` to the next downstream sink*

which, taken literally, makes **every add-arm vacuous** — proof in § Risks R3. The design pins the
minimal repair that keeps the inequality's meaning and makes it edit-sensitive:

| Term | Definition (measured on the corridor being evaluated — `D` to detect, the scratch to recheck) | Moves under an add-edit |
|---|---|---|
| `dir(c)` | dominant-axis sign of the frozen `fastest_lap` step at `c` (tie → x) | fixed |
| `end(D,c)` | last drivable cell from `c` along `dir(c)` — the wall the braking ray hits | **grows** (`lengthen_straight`) |
| `runout_room(D,c)` | `wall_run(D, c, dir(c)) − 1` (Ф4's own `wall_run`, widened to `pub(crate)`) | **grows** (`lengthen_straight`) |
| `attainable(D,c)` | `flood.peak.get(&c).copied().unwrap_or(0)` — max `vnorm` at `c` over the **sink-seeded** forward flood (below). **Never `flood.peak[c]`**: `HashMap`'s `Index` impl panics on a missing key, and `c` is not guaranteed present — a sink cell lying between `path[u]` and `c` is a barrier and truncates the flood before it reaches `c`. The `0` default under-reports the deficit, i.e. errs toward *not* emitting a `NoBraking` — the conservative direction | grows (adverse — bounded by `v_target`) |
| `v_entry(D,c)` | `min(v_target, attainable(D,c))` | grows (adverse) |
| `v_corner(D,c)` | `corner_speed(D, &flood, end(D,c))` — max `vnorm` over the flood's **arrival states** at `end(D,c)` that have **≥1 legal successor** in `D` | **grows** (`widen_corner`) |
| `deficit(D,c)` | `braking_cells(v_entry, v_corner) − runout_room` | may strictly decrease ⇒ non-vacuous |

`NoBraking { at: c }` fires iff `deficit(D,c) > 0`; one issue per **maximal run** of deficient
`fastest_lap` indices, anchored at the run's first point (deterministic).

**`v_entry`'s `min` never binds — keep it, and say so.** `window_speed` prunes by
`within_v_ceil(·, max(v_target, 1))`, so `attainable ≤ v_target` always holds and
`min(v_target, attainable) == attainable`. The `min` is retained as a defensive clamp that keeps
`deficit_at` correct if a future caller ever passes a `v_ceil` above `v_target`;
`deficit_at`'s rustdoc must state that it is currently a no-op, so a reader does not mistake it
for a live constraint.

Both arms now have a real lever (`lengthen_straight` → `runout_room`; `widen_corner` → `v_corner`),
so neither ships as dead vocabulary — the reason KD1 exists. The adverse `v_entry` term is exactly
the design-doc `[C3]` effect ("удлинение прямой … одновременно *повышает* скорость приезда")
`[measured: sed -n 64p docs/design.md → "Удлинение прямой чинит run-out поворота X, но одновременно
*повышает* скорость приезда в X — чек фиксированного радиуса возьмёт устаревшую
upstream-достижимость и объявит X починенным, пока он всё ещё нетормозим."]`; the greedy rejects any
candidate whose adverse term outweighs its lever.

**The sink-to-sink flood** (the one function detection and recheck share — this is what makes AC3
honest rather than a shape assertion):

```rust
/// One forward flood: the states it reached, plus the per-cell max `vnorm` over them.
pub(crate) struct WindowFlood {
    /// Every state reached, barrier arrivals included (recorded, never expanded).
    pub(crate) states: HashSet<CarState>,
    /// Per-cell max `vnorm` over `states` — what `attainable` reads.
    pub(crate) peak: HashMap<Point, i32>,
}

pub(crate) fn window_speed(d: &Corridor, seeds: &[CarState], barriers: &HashSet<Point>,
                           v_ceil: i32) -> WindowFlood;

/// Max `vnorm` over `flood.states` at `end` that have ≥1 legal successor in `d`.
pub(crate) fn corner_speed(d: &Corridor, flood: &WindowFlood, end: Point) -> i32;
```

Forward BFS over `legal_move`/`step` (never reimplemented — the Ф5a/Ф5b discipline), bounded by
`within_v_ceil(·, v_ceil)` with `v_ceil = max(v_target, 1)`; a successor whose `pos()` ∈ `barriers`
is **recorded but not expanded** (a sink is a cut in the state space).

**Seed exemption (load-bearing in both directions — state it in `window_speed`'s rustdoc).** Seed
states are **always expanded**, even when their own cell is in `barriers`; only *successors*
landing on a barrier cell are recorded-not-expanded. The sink-seeded detection call depends on
this (its seed cell **is** a barrier, so without the exemption the flood would be empty), and the
AC3 counter-scope depends on the barrier half (below).

`corner_speed` returns `0` when no qualifying arrival exists. It takes `flood` rather than
recomputing, so it is measured under **exactly** the seeding/barrier pair its caller used — the
per-state "≥1 legal successor" predicate cannot be recovered from `peak` alone, which is why
`window_speed` returns `states` too.

- *Detection / sink-to-sink recheck* — `window_speed(d, &{ |v| ≤ 1 states at path[u] }, &{ every
  sink cell }, v_ceil)`, where `u` = the nearest **upstream** sink index (`sink_indices`, below).

  **The seed set is a deliberate conservative superset — do not go looking for a `live` input.**
  AC3 words this as "the **live** states of the nearest upstream sink", but `RepairContext` carries
  `metrics`, **not** `live` (Ф5b returns `TrackMetrics`; `live` is internal to
  `phase5_full_oracle`), and recomputing `live` per candidate edit would be the global oracle run
  `[C3]` exists to avoid. So the seed set is **every `|v| ≤ 1` state at `path[u]` whose cell is in
  `D`** — a superset of the live states there, since every live state at a sink has `|v| ≤ 1` by
  the sink definition. Supersetting is safe in the only direction that matters: it can only
  *inflate* `attainable`, hence the deficit, hence over-report `NoBraking` — it can never mask a
  genuine one. This is a design choice, not a missing `RepairContext` field.
- *The AC3 counter-scope* (`#[cfg(test)]` only) — `window_speed(d, &{ at-rest states at the cells
  `N` path-steps upstream of `c` }, &{ those same cells }, v_ceil)`. **The barrier set is
  mandatory and is exactly the seed cells.** A fixed-radius window boundary is not a cut, and with
  `barriers = ∅` the counter-scope is **provably non-discriminating**: nothing spatially bounds the
  flood, so from an at-rest seed the car legally backs up one cell at a time (`West`→`East` pairs,
  every chord's supercover staying on the drivable straight) to the far end of the straight and
  re-accelerates, making the "radius-`N`" flood equal the global flood. On
  `brake_deficit_corridor` that raises `attainable((10,0))` from `2` to `3`, collapsing the
  radius deficit onto the sink-to-sink verdict and making subtask 7's discriminating test
  unpassable. Tuning the straight length, `v_target` or `N` cannot recover it — on any straight
  long enough to reach `v_target`, an unbarriered window flood **is** the global flood. With the
  barrier in place the excursion still happens but cannot help: reaching `(10,0)` at `v = 3`
  requires a predecessor state at cell `10 − 3 = 7`, which is recorded-not-expanded, while
  `v = 2` survives via `(6,0)v2 → (8,0)v2 → (10,0)v2`. `[derived → subtask 7 asserts
  `attainable == 2` under the barriered counter-scope and `== 3` under the sink scope]`

`sink_indices(metrics)` = `{ i : heatmap(path[i]) ≤ 1 } ∪ {0}`. **Index 0 is always included**;
the spec's "non-empty by construction" argument is unsound (§ Risks R4) and the unconditional
`0` restores totality with a *sound* justification: `path[0]` is a start-grid position and the
race-start seeds are at rest, so seeding there reproduces the global race-start flood — a
conservative superset, never a stale window.

### Decision 3 — `lengthen_straight` vs `widen_corner` selection (deterministic)

Both candidate sets are generated, evaluated under the **one** metric (`deficit_at` on a scratch
copy), and the winner is chosen by: **max deficit reduction → arm rank (`LengthenStraight` = 0
before `WidenCorner` = 1) → min `wall_sort_key`**. This is #30's verified-greedy pattern with an
explicit arm rank added so ties are total.

- `lengthen_straight` candidates: `Wall { cell: end(D,c), side: side_of(dir(c)) }` — extend the
  braking ray.
- `widen_corner` candidates: `Wall { cell: end(D,c), side: s }` for the two sides `s` perpendicular
  to `dir(c)` — widen the corner so a faster arrival keeps a legal successor.

Every candidate is filtered to *in-box, currently `¬D`* (an out-of-box target makes `Corridor::set`
a documented no-op — the treatment `map_frontier_gap_to_edge` already gives it).

Rationale for the tie-break order: `runout_room` is a purely geometric, monotone lever, whereas
`v_corner` is measured through the flood and can be dragged back by the adverse `v_entry` term —
prefer the monotone lever when the measured gains are equal.

### Decision 4 — issue ordering within the single pass: **fixed severity order** (pinned)

Not emitted order. Two reasons: (a) the spec's own argument — trim a topology bridge *before* a
width push widens it further; (b) `phase4_static_checks`' emission order is an implementation
detail (connectivity → topology → `narrow_issues` over `box_points` → `NarrowSf` → `infield_fingers`),
so emitted-order dispatch would let a future Ф4 refactor silently change Ф6's outcome.

`pub(crate) const fn issue_rank(i: Issue) -> u8`, then sort by `(rank, payload Point, axis rank, width)`:

| Rank | Label | Family | Arm |
|---|---|---|---|
| 0 | `Disconnected` | — | decline (`NotRepairable`) |
| 1 | `BadTopology` | — | decline (`NotRepairable`) |
| 2 | `LostHairpin { tip }` | remove | `nudge_finger` |
| 3 | `ArmsMerging { bridge }` | remove | `trim_arm_wall` |
| 4 | `ConcaveChordCut { tooth }` | add | `fill_inner_tooth` |
| 5 | `Narrow { .. }` | add | `push_outer_wall_out` |
| 6 | `NarrowSf { .. }` | add | `push_outer_wall_out` |
| 7 | `NoBraking { at }` | add (run-out) | `lengthen_straight` / `widen_corner` |
| 8 | *(dynamic stall — `ctx.stall_walls`)* | add | `map_frontier_gap_to_edge` |

`Issue` has no `Ord` derive, so the key function is crate-local (the same reason `wall_sort_key`
exists). `LostHairpin` outranking `ArmsMerging` **is** the spec's "`LostHairpin` takes precedence":
both arms share one metric (`|H ∩ D|` strictly decreases + global flood-fill), so running both is
monotone progress, not a conflict — no suppression rule is written (YAGNI). The dynamic arm runs
**last**: it is the most global add and benefits from every earlier repair.

**Driver invariant — re-validate every payload against the *working* corridor before acting.** An
earlier edit can stale a later issue's payload (a trimmed bridge cell is already `¬D`). Each arm
re-derives its own precondition on the working `D` and returns `NoEdit(StalePayload)` when it no
longer holds. This is the "never trust the diagnostic" discipline `map_frontier_gap_to_edge`
already applies, generalised to the whole pass — and it is what keeps the pass a *single* pass
without re-detection.

### The five arms (one wall, one cell flip — AC1)

Canonical wall for a flip (spec KD *What "one dual edge" means*): min `wall_sort_key` among the
walls that identify it.

- **add** on cell `q`: walls `w` with `w.cell ∈ D`, `wall_neighbor(w) == Some(q)`.
- **remove** on cell `c`: walls `Wall { cell: c, side: s }` whose neighbour is `¬D`/out-of-box.
  If `c` has none, `c` is `D`-interior — removing it would punch a new hole; return
  `NoEdit(NoCandidate)` rather than propose a non-boundary flip.

| Arm | Candidate derivation | Metric (on scratch) |
|---|---|---|
| `push_outer_wall_out` | The chord through `center` along `axis`; its two cap walls (`{center, −axis}` and `{far end, +axis}`) | `axis_width(scratch, center, axis) > axis_width(working, center, axis)`; max gain, tie → min `wall_sort_key` |
| `fill_inner_tooth` | `tooth` itself (re-validated degree-1, sole `¬D` neighbour **in-box**) | `tooth` drivable on the scratch and was not on the working `D`; hole preservation decided **locally** (below) |
| `lengthen_straight` / `widen_corner` | § Decision 3 | `deficit_at` strictly decreases (sink-to-sink) |
| `trim_arm_wall` | Cells of `bridge`'s 4-connected component of `H ∩ D` that are `D`-boundary cells, ascending `Point`; **first admissible wins** | `\|H ∩ D\|` strictly decreases **and** global flood-fill passes |
| `nudge_finger` | `tip`'s finger footprint (`infield_fingers` → `block_points`) ∩ `D`, restricted to `D`-boundary cells, ascending `Point`; **first admissible wins** | as above |
| `map_frontier_gap_to_edge` | #30, unchanged; `Edge(w)` → add `wall_neighbor(w)` | **already verified inside the mapper** (strict `\|P0\|` growth at `V_ceil = 1`); not re-verified — re-running it would double the cost and reopen a settled contract. `NoCandidate` → `NoEdit(NoCandidate)` (AC7) |

Remove arms use **first-admissible in a total order**, not max-gain, because every single-cell
removal reduces `|H ∩ D|` by exactly 1 — there is no gain to maximise, and the global flood-fill is
the real discriminator.

**`fill_inner_tooth`'s local hole-preservation guard** (spec KD *Add-edits and the single-hole
invariant*): the guard is decided from `tooth`'s 4-neighbourhood alone — exactly one neighbour is
`¬D` **and it is in-box**. That neighbour *is* the second cell of the component, so "the component
has ≥ 2 cells" holds without a flood; leaf removal from a ≥2-cell 4-connected component can neither
split it nor empty it. A tooth whose sole `¬D` neighbour is **out-of-box** lies on the box border,
hence in the *unbounded* complement component
`[measured: sed -n 141p crates/core/src/geom/graph.rs → "complement component is *unbounded* (the outfield) iff any of its cells lies on the box border"]`,
so it is excluded by the producing condition — filling it would be a pointless outfield edit. The
production code therefore **never calls `bounded_complement_components` on an add-edit** (AC2);
`bounded_complement_components(scratch) == 1` is asserted **test-side**, as the proof that the
local guard is sound.

**Scope of the guard (state this in `fill_inner_tooth`'s rustdoc).** It is a *topology* guard, not
an *infield-membership* guard: it excludes only a tooth whose sole `¬D` neighbour is out-of-box.
An interior tooth whose complement component still reaches the box border — an **outfield** nub,
not the "inner tooth" design §2 Ф6 names — is admitted. That is topologically harmless (leaf
removal from a ≥2-cell component leaves `bounded_complement_components` unchanged, which is the
property AC2 needs), but it means the arm can spend an edit on an outfield nub. Deciding infield
membership would require a flood, i.e. exactly the whole-corridor recheck AC2 forbids for an
add-edit; the guard is therefore correct as scoped, and the rustdoc records the gap rather than
letting a reader infer a stronger property from the arm's name.

### The two static detectors (Ф4 family — one entry point preserved)

`phase4_static_checks(d, skel, k, n, m, sf)` gains both checks with **no signature change** (it
already takes every input needed). Emission appended in fixed order:
`ConcaveChordCut` (ascending `Point`) → `ArmsMerging` (ascending `Point`).

- **`Issue::ConcaveChordCut { tooth: Point }`** — in-box `c` with `¬d.contains(c)`, exactly one of
  its four 4-neighbours not drivable (out-of-box counts as not drivable), **and that neighbour
  in-box**. `[derived → subtask 3's positive + two near-miss negatives]`
- **`Issue::ArmsMerging { bridge: Point }`** — one issue per 4-connected component of `H ∩ D`
  (`H = ⋃ block_points(c, k), c ∈ skel.hole`), anchored at the component's min `Point`. The
  component BFS is confined to `H ∩ D`, so it is bounded by `|skel.hole| · k²` — **not** a
  whole-corridor flood.

### Alternatives rejected

- **A `Defect` wrapper enum** (`Static(Issue) | DynamicStall`) as Ф6's input: rejected — it
  duplicates the vocabulary the spec deliberately keeps in one place, and `stall_walls` is already
  a context member per the spec's own signature note. The dynamic label rides `ctx.stall_walls`.
- **Re-running the oracle inside the recheck** (the design doc's own "проще" fallback, §2 `[C3]`):
  rejected — AC3 explicitly requires sink-to-sink, and a per-edit global V=1 liveness run would not
  discriminate the two scopes at all.
- **Re-deriving sinks on the scratch copy**: rejected — that is re-detection (KD2 forbids it) and
  is circular (the barrier set would depend on the answer it gates).
- **`ArmsMerging`/`LostHairpin` overlap suppression**: rejected as YAGNI — ordering precedence
  plus the shared metric already makes the overlap deterministic and idempotent.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Lift `block_origin`/`block_points` into `coarse.rs` (`pub(crate)`), delete both private copies; widen Ф4's `box_points`, `wall_run`, `wall_runs`, `walk_finger`, `infield_fingers`, `absorbed` to `pub(crate)`; add `pub(crate) fn axis_width(d, p, axis) -> u32` (the shared width routine `push_outer_wall_out`'s metric reuses) **in `phase4_defects.rs`, not `phase4.rs`** — it is new code and needs only `wall_runs`, which this subtask already widens, so siting it there costs no churn to existing Ф4 width code and keeps `phase4.rs` clear of the soft cap (§ Risks R1) | new `crates/gen/src/coarse.rs`, new `crates/gen/src/phase4_defects.rs` (stub module + `axis_width`), `crates/gen/src/phase2.rs`, `crates/gen/src/phase4.rs`, `crates/gen/src/lib.rs` | — |
| 2 | `Issue` gains **all three** new variants — `ConcaveChordCut { tooth }`, `ArmsMerging { bridge }`, `NoBraking { at }` — each with rustdoc naming its Ф6 arm. All three land in the **one** `Issue` enum (spec § KD *Where the `Issue` vocabulary lives*); `NoBraking` is emitted from the Ф5 family (subtask 6), not from `phase4_static_checks` | `crates/gen/src/phase4.rs` | 1 |
| 3 | `phase4_defects.rs`: both detector bodies + their fixtures and tests (positive + near-miss negatives per AC6) | new `crates/gen/src/phase4_defects.rs`, `crates/gen/src/lib.rs` | 2 |
| 4 | Wire both detectors into `phase4_static_checks`; update the **3** existing exact-set Ф4 tests the new issues change; add the AC8 Ф1→Ф2 pin | `crates/gen/src/phase4.rs`, `crates/gen/src/phase4_defects.rs` | 3 |
| 5 | `phase5_runout.rs` primitives: `MAX_BRAKE_SPEED`/`triangular`/`braking_cells`, `sink_indices`, `travel_dir`, `end_of_ray`, `runout_room`, `WindowFlood`/`window_speed` (incl. the **seed exemption**: seeds expand even when their own cell is a barrier), `corner_speed(d, &flood, end)`, `deficit_at` (its rustdoc records **both** caveats: `v_entry`'s `min` is currently a no-op, and the seed set is a conservative `\|v\| ≤ 1` superset of AC3's "live states" — chosen because `RepairContext` exposes `metrics`, not `live`, and safe because it can only inflate the deficit) + per-primitive unit tests | new `crates/gen/src/phase5_runout.rs`, `crates/gen/src/lib.rs` | 1 |
| 6 | `phase5_runout_checks(...) -> Vec<Issue>` — maximal-run anchoring, `NoBraking` emission; tests incl. the adequate-run-out near-miss (AC6) and the "heatmap at the start cell can exceed 1" pin (R4) | `crates/gen/src/phase5_runout.rs`, `crates/gen/src/phase4.rs` (consumes `Issue::NoBraking`) | 5, 2 |
| 7 | AC3: `brake_deficit_corridor` + its frozen path/sink fixtures in `testfix.rs`; the **discriminating** test (radius-N seeding says "fixed", sink seeding says "still deficient") and the **non-vacuity** test (a correct repair clears the deficit). The counter-scope helper **must** pass `barriers = the seed cells` — see § Decision 2; an unbarriered counter-scope makes the test unpassable, so assert `attainable == 2` under it explicitly | `crates/gen/src/testfix.rs`, `crates/gen/src/phase5_runout.rs` | 6 |
| 8 | `phase6_repair.rs` public types (`RepairContext`, `RepairOutcome`, `CommittedEdit`, `RepairArm`, `RecheckScope`, `ArmOutcome`, `DeclineReason`), `recheck_scope`, `issue_rank`/`issue_sort_key` + their tests | new `crates/gen/src/phase6_repair.rs`, `crates/gen/src/lib.rs` | 2 |
| 9 | `phase6_arms.rs` plumbing: `in_box`, `add_edit_wall`, `remove_edit_wall`, `apply_edit`; tests for canonical min-`wall_sort_key` selection, out-of-box decline, `D`-interior remove decline, AC1 single-cell-flip helper | new `crates/gen/src/phase6_arms.rs`, `crates/gen/src/testfix.rs`, `crates/gen/src/lib.rs` | 8 |
| 10 | Add arms `push_outer_wall_out` + `fill_inner_tooth` with their metrics; tests incl. the bounding-box-headroom decline | `crates/gen/src/phase6_arms.rs` | 9 |
| 11 | Run-out arms `lengthen_straight` + `widen_corner` (candidate sets + joint verified-greedy + arm-rank tie-break); tests incl. `widen_corner` non-vacuity. **Apply the pre-decided split rule if the file crosses 800 lines incl. tests** | `crates/gen/src/phase6_arms.rs` | 10, 7 |
| 12 | Remove arms `trim_arm_wall` + `nudge_finger`; tests incl. the global-flood-fill rejection of a disconnecting removal | `crates/gen/src/phase6_arms.rs` | 11 |
| 13 | The single-pass driver: `dispatch` (all 9 labels incl. the two declines and the dynamic arm), severity ordering, staleness re-validation, `[C3]` recheck routing, `RepairOutcome`; tests AC1/AC4/AC5/AC7 | `crates/gen/src/phase6_repair.rs` | 12 |
| 14 | AC2 consequence discriminators (add-on-disconnected commits; remove-that-disconnects rejected), AC9 totality/determinism battery, AC10 gate sweep (`build`/`test`/`clippy --workspace --all-targets -D warnings`/`fmt --check`/`RUSTDOCFLAGS="-D warnings" cargo doc`), `lib.rs` re-exports + module rustdoc | `crates/gen/src/phase6_repair.rs`, `crates/gen/src/phase6_arms.rs`, `crates/gen/src/lib.rs` | 13 |

## Handoff plan

Every subtask is a **code** change-type (`crates/gen/src/*.rs` only — no `.md`, no `.claude/**`),
so all groups route to the `code-writer` subagent. `M = 14`; the size cap is 10, so the minimum
group count is 2 — and 2 is what this plan uses (no avoidable split; the boundary at 7/7 falls on
the natural detectors→repair dependency seam, and subtask 11's dependency on 7 is respected because
7 precedes it).

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the `code-writer`
  subagent (frontmatter-pinned; no inline `model=`/effort override), 1M-token window — subtasks
  **1–7** (code change-type: `*.rs`). Entered via `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Handoff after Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). The parent `/task`
  resumes in Group B with fresh context.
- **Group B** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the `code-writer`
  subagent, 1M-token window — subtasks **8–14** (code change-type: `*.rs`). **Terminal group**
  (7 subtasks; within the `1..=10` range).

2 groups ≤ the default maximum of 4 — no user gate needed. The `design`, `design-review`,
`self-review` and `spec-writer` subagents stay on Opus regardless of these markers.

## Risks

- **R1 — `phase4.rs` line-count pressure.** Currently **754** lines; soft cap is 800 incl. tests.
  Projected delta: `−13` (block helpers move out) `+24` (**three** `Issue` variants + docs — the
  count rose from two after round-2 review Finding 3 put `NoBraking` in subtask 2) `+8` (visibility
  doc notes) `+6` (three updated assertions) `≈ 779`.
  **Mitigation 1 (binding, primary — chosen over the alternatives below).** `axis_width` is *new*
  code, so it is sited in `phase4_defects.rs` rather than `phase4.rs` (subtask 1). It needs only
  `wall_runs`, which subtask 1 already widens to `pub(crate)`. This removes `+18` from the
  projection at **zero** churn to existing Ф4 width code, and — the point that decides it — it
  retires a *likely-firing conditional* from a sonnet/medium implementor's mid-group path instead
  of leaving one to be evaluated under context pressure. 800 is a **soft** cap with no enforcing
  gate (`phase5b.rs` ships at 1145), so this was never a blocker; it is simply the cheaper fix.
  **Mitigation 2 (binding).** *All* new Ф4-family tests — including the AC8 Ф1→Ф2 pin — live in
  `phase4_defects.rs`, never in `phase4.rs`.
  **Backstop (retained, now unlikely to fire).** Check `wc -l crates/gen/src/phase4.rs` at the end
  of **both** subtask 2 and subtask 4. If either lands ≥ 800, extract `narrow_at` / `narrow_issues`
  + their tests into `phase4_defects.rs` as a mechanical move — the more invasive option, kept only
  as a backstop.
  `[measured: wc -l crates/gen/src/phase4.rs → 754]` · `[derived → subtask 4's wc -l check]`
- **R2 — `phase6_arms.rs` size.** Five arms + tests. **Mitigation (pre-decided, § Approach):** split
  the two remove arms into `phase6_remove.rs` at subtask 11 if the file crosses 800 incl. tests.
  `[derived → subtask 11's wc -l check]`
- **R3 — the spec's original run-out deficit formula was vacuous (design-blocking; RESOLVED — spec amended, round 3 A1).**
  With `runout_room` defined as *fastest_lap steps to the next downstream sink*, the frozen path
  makes it **constant** under an edit, while `attainable(c)` is **monotone non-decreasing** in `D`
  (`D ⊆ D'` ⇒ forward-reachable grows ⇒ max `vnorm` at `c` grows). Every repair arm for `NoBraking`
  is an *add*-edit, so `deficit = braking(min(v_target, attainable)) − runout_room` can never
  strictly decrease ⇒ **no run-out edit would ever be committed**, contradicting AC3's non-vacuity
  clause and KD1's "no arm ships as dead vocabulary". Pinned fix: geometric `runout_room` (the
  braking ray, which `lengthen_straight` grows) + a measured `v_corner` term (which `widen_corner`
  grows). Surfaced rather than silently applied: the orchestrator independently verified it, the
  owner approved, and **the spec now carries this formula** (§ Amendments (round 3) **A1**, amending
  § The three new detectors item 3 and the § Per-arm progress metrics `NoBraking` row). Spec and
  design agree; nothing here is open.
  `[derived → subtask 7's non-vacuity test, which fails if any lever is missing]`
- **R4 — the spec's original "sink set is non-empty by construction" was unsound (RESOLVED — spec amended, round 3 A2).**
  `speed_heatmap` is the
  **per-point max `vnorm` over all live states**, not a per-state value
  `[measured: sed -n 231,246p crates/gen/src/phase5b.rs → "Per-corridor-point max vnorm over live's states at that point"]`,
  so a start-grid cell traversed at speed later in the lap has heatmap `> 1` and is **not** a sink;
  the sink set can therefore be empty. Pinned fix: `sink_indices` always includes index `0`, with
  the sound justification that the race-start seeds are at rest.
  `[derived → subtask 6's assertion that heatmap(long_straight start cell) > 1]`
- **R5 — the spec's original `fill_inner_tooth` "tooth count strictly decreases" metric was unachievable (RESOLVED — spec amended, round 3 A3).**
  Counterexample: a 3-cell straight-line hole `{(2,2),(2,3),(2,4)}` has teeth `{(2,2),(2,4)}`
  (count 2); filling `(2,2)` leaves `{(2,3),(2,4)}`, whose teeth are `{(2,3),(2,4)}` (count 2) —
  **unchanged**. Pinned replacement: *`tooth` is drivable on the scratch and was not on the working
  `D`*, plus the local hole-preservation guard. Strictly local, strictly improving, and it is the
  arm's actual objective. The spec's § Per-arm progress metrics `fill_inner_tooth` row now reads
  the same. `[derived → subtask 10's fill_inner_tooth tests]`
- **R6 — `NarrowSf`'s metric measures corridor width, not `sf.chord.len()`.** Ф4 derives `NarrowSf`
  from `sf.chord.len()` `[measured: sed -n 176,187p crates/gen/src/phase4.rs → check_narrow_sf reads sf.chord.len()]`,
  and Ф6 cannot re-cut `sf.chord` (that is Ф3's `cross_section`, and `sf` is not Ф6's to rewrite).
  Ф6 therefore widens the **corridor** at the chord's `center` along `sf.orient`; re-cutting the
  chord is the `generate()` loop's job (out of scope). Documented in the arm's rustdoc so the
  integration item is visible. See § Open questions Q3.
- **R7 — three existing Ф4 exact-set tests change.** `ac7_filled_finger_yields_exactly_lost_hairpin`
  (the filled finger footprint is drivable inside `H` → `ArmsMerging { bridge: (15,9) }` also fires),
  `ac7_disk_merge_yields_exactly_bad_topology` (a fully-drivable 21×21 makes all of `H` drivable →
  `ArmsMerging { bridge: (6,6) }`), and `ac7_sharp_neck_yields_exactly_narrow` (the carved row
  `y=10` leaves degree-1 teeth at `(1,10)` and `(3,10)` → two `ConcaveChordCut`). Expected per spec
  § KD *API stability*, not a regression. `ac6_clean_ring_with_intact_finger_is_empty` and
  `ac8_degenerate_inputs_are_total_no_panic` are unaffected (both `¬D` regions have minimum
  complement-degree 2, and `H` is entirely `¬D` / out-of-box).
  `[derived → subtask 4: the exact post-change issue sets are asserted, and any fourth affected test surfaces there]`
- **R8 — a `-D warnings` gate aborts on the first failure.** Subtask 14's clippy sweep may reveal
  same-class sites behind the first. Budget a re-run after cleanup; any newly-revealed
  out-of-contract class is surfaced to the orchestrator as a blocker, not absorbed.
- **R9 — remove-arm cost.** Each remove candidate runs `component_count` +
  `bounded_complement_components` (two full-box floods). Candidates are bounded by
  `|H ∩ D| ≤ |skel.hole| · k²`, and first-admissible short-circuits. No cap is introduced (YAGNI);
  the bound is documented in the arm's rustdoc. `[derived → subtask 12's tests complete within the normal cargo test budget]`
- **R10 — zero production panics.** `ai-docs/panic-index.md` carries **five** rows, all in
  `gp-render`/`gp-game`; `gp-gen` and `gp-core` have none
  `[measured: read ai-docs/panic-index.md → 5 rows, files crates/render/*, crates/game/*]`. This
  task adds **no** row: no `unwrap`/`expect`/`panic!`/panicking index in production. Path indexing
  uses `.get(i)`; heatmap lookup uses `binary_search_by_key`; **every `WindowFlood::peak` lookup
  uses `.get(&p).copied().unwrap_or(0)`, never the panicking `peak[p]` `Index` form** (a barrier
  can truncate the flood short of the queried cell — § Decision 2); all arithmetic is
  `saturating_*`/`checked_*`/`try_from(..).unwrap_or(..)`, except `triangular`'s documented
  `#[allow(clippy::arithmetic_side_effects)]` over a clamped domain.
- **R11 — Miri.** `gp-gen` rides the sanctioned crate-level `--exclude` (#134) — no per-test
  `#[cfg_attr(miri, ignore)]` is added or needed.
  `[measured: grep -n "gp-gen" AGENTS.md → "gp-gen is excluded from the Miri gate … the one sanctioned crate-level --exclude"]`
- **R12 — determinism.** `HashSet`/`HashMap` appear only as membership/max structures; every value
  reaching the output is ordered by `issue_sort_key`, `wall_sort_key`, or ascending `Point`.
  `[derived → subtask 14's repeated-call and shuffled-input equality assertions]`
- **R13 — `window_speed`'s `barriers` argument is load-bearing in *both* directions; a plausible
  simplification silently destroys AC3.** Dropping `barriers` from the AC3 counter-scope (the
  natural reading of "a radius-`N` window", since nothing else in the call names a radius) makes
  the counter-scope equal the global flood — the back-up-and-re-accelerate excursion is legal on
  any straight — so the discriminating fixture reports the *same* deficit under both scopes and
  subtask 7 becomes unpassable. Dropping the **seed exemption** in the other direction empties the
  sink-seeded detection flood, since its seed cell is itself a barrier. Both are one-line changes a
  future refactor could make in good faith. **Mitigation:** `window_speed`'s rustdoc states both
  rules explicitly, and subtask 5 carries a direct unit test for each (`seeds ⊆ barriers` yields a
  non-empty flood; a successor landing on a barrier is recorded but not expanded), so neither can
  regress silently. `[derived → subtask 5's two barrier-semantics tests + subtask 7's `attainable == 2` assertion]`

## Test Design

Placement: `#[cfg(test)] mod tests` per implementing module; shared fixtures in
`crates/gen/src/testfix.rs` (already `#![cfg(test)]`).

### AC3 — the discriminating fixture (the flagged highest-risk piece): **it can be built**

Built at the `deficit_at` level with a **hand-supplied** frozen path and sink set, so it does not
depend on which lap the oracle happens to pick. Both AC3 halves ride one corridor.

`testfix::brake_deficit_corridor() -> (Corridor, Vec<Point>, BTreeSet<usize>)`

- Box `origin (0,0)`, `14 × 6`. Drivable: the straight `y = 0, x ∈ 0..=11`, plus the corner leg
  `x = 11, y ∈ 1..=4`. `(12,0)` and `(13,0)` are **in-box and `¬D`** — so the add-edit is a real
  flip, not a `Corridor::set` no-op.
- Frozen path: `[(0,0), (1,0), …, (11,0), (11,1), (11,2), (11,3), (11,4)]`. Sink index set: `{0}`.
- Probe point `c = (10,0)` (path index 10), `dir(c) = East`.
- The counter-scope is radius `N = 3`: `window_speed(d, &{at-rest states at (7,0)}, &{(7,0)}, 3)`.
  **The `{(7,0)}` barrier set is not optional** — with `barriers = ∅` this row reads `3`, not `2`,
  and the fixture stops discriminating (§ Decision 2).

Hand-derived numbers (from `Action`'s five cardinal accelerations and `legal_move`'s
`p1 ∈ D ∧ supercover ⊆ D` rule, `crates/core/src/sim/mod.rs:89-108`). Each `v_corner` row is
measured on **its own row's** `WindowFlood` — the two seedings give two floods, and
`corner_speed`'s per-state "≥1 legal successor" predicate is evaluated over that flood's `states`:

| Quantity | pre-edit | post-edit (`(12,0)` made drivable) |
|---|---|---|
| `runout_room((10,0))` | `wall_run = 2` → **1** | `wall_run = 3` → **2** |
| `attainable` (sink flood: seeds `{\|v\| ≤ 1 at (0,0)}`, barriers `{(0,0)}`, `v_ceil = v_target = 3`) | **3** (via `(7,0)v2 → East → (10,0)v3`) | **3** |
| `v_corner` at `end` (sink flood) | `end = (11,0)` → **1** (the `v = 2` and `v = 3` arrivals have *no* legal successor: every successor lands on `x ≥ 12`, all `¬D`) | `end = (12,0)` → **1** (the `v = 2` and `v = 3` arrivals at `(12,0)` still have none; the `v = 1` arrival brakes to `v = 0` in place) |
| **`deficit` (sink-to-sink)** | `(tri 3 − tri 1) − 1 = 5 − 1 =` **4** | `5 − 2 =` **3** → *committed* (strict decrease) **and still deficient** |
| `attainable` (radius flood: seeds `{at rest at (7,0)}`, barriers `{(7,0)}`, `v_ceil = 3`) | **2** — `v = 3` at `(10,0)` needs a predecessor state at cell `10 − 3 = 7`, which is recorded-not-expanded; `v = 2` survives via `(6,0)v2 → (8,0)v2 → (10,0)v2` | **2** (the added cell is downstream of `c`) |
| `v_corner` (radius flood) | **1** — the `v = 2` arrival at `(11,0)` (from `(9,0)v3`) has no successor; the `v = 1` arrival (from `(10,0)v2 → West`) brakes to `v = 0` in place | **1** — the `v = 3` and `v = 2` arrivals at `(12,0)` have no successor; the `v = 1` arrival does |
| **`deficit` (radius-3)** | `(tri 2 − tri 1) − 1 = 2 − 1 =` **1** (fires) | `2 − 2 =` **0** → **reports "fixed"** |

⇒ post-edit, the fixed-radius recheck reports *fixed* (`≤ 0`) while the sink-to-sink recheck
correctly reports *still deficient* (`3 > 0`). That is exactly AC3's discriminating requirement,
on a real corridor, with the **same** `window_speed` function under two seedings — so the spec's
honest fallback ("a directly unit-tested `attainable(c)` comparison between the two seedings") is
*subsumed*, not substituted.

**AC3 non-vacuity (b)** — same corridor, `v_target = 2`: `deficit` pre `= (tri 2 − tri 1) − 1 = 1`
(fires); post `= 2 − 2 = 0` ⇒ the repair **clears** it. One fixture, two `v_target` values.

The arithmetic above is hand-derived, not executed. Confirmation history, by the round each
landed in — a record of what happened, not a standing guarantee (AGENTS.md § Communication: *a
recorded result is a claim, not a completion*):

- **round 1** — `design-review` re-derived every cell of the original table and confirmed it.
- **round 2** — `design-review` and the orchestrator each independently re-derived the unbarriered
  back-up excursion (the R13 defect), and `design-review` re-derived the rows added in that round:
  the barriered `attainable = 2`, both post-edit `v_corner` rows, and the seed exemption's
  load-bearingness for the sink-seeded detection call.

Nothing below round 2 is asserted here; execution is what settles it.
`[derived → subtask 7 asserts every cell of the table]`

**If a number is off, the free parameters are the straight length, `v_target`, and `N` — but the
`barriers` argument is NOT a free parameter.** Tuning cannot rescue an unbarriered counter-scope:
on any straight long enough to reach `v_target`, an unbarriered window flood *is* the global flood,
so the two scopes agree by construction (§ Decision 2, § Risks R13). The discriminator needs only
`deficit_radius_post ≤ 0 < deficit_sink_post`, which this construction achieves with ≥ 2 units of
slack **once the barrier set is in place**.

### Per-subtask scenarios

| Subtask | Entry point | Scenarios |
|---|---|---|
| 1 | `coarse::block_points`, `phase4_defects::axis_width` | block expansion matches the deleted copies verbatim; `axis_width` equals Ф4's `wall_runs`-derived width on the existing neck fixtures |
| 3 | `ConcaveChordCut` / `ArmsMerging` detectors | **positive**: a ring with one degree-1 tooth → exactly one issue; a ring with one drivable infield cell → exactly one `ArmsMerging`. **near-miss negatives** (AC6): a degree-2 notch does not fire; a 1-cell hole (degree 0) does not fire; a border tooth (sole `¬D` neighbour out-of-box) does not fire; a clean infield yields no `ArmsMerging` |
| 4 | `phase4_static_checks` | the three changed exact-set assertions (R7); the AC8 pin on `phase1_coarse_ring(l_min, rng) → phase2_rasterize` at a fixed seed — the issue set is asserted *by value*, so a later Ф2 or detector change cannot alter it silently |
| 5 | `triangular`, `braking_cells`, `sink_indices`, `travel_dir`, `runout_room`, `window_speed`, `corner_speed` | `triangular` at `0/1/2/3` and at `i32::MAX` (clamped, no panic); `braking_cells(to ≥ from) == 0`; `sink_indices` always contains `0`; `travel_dir` on a diagonal step picks the dominant axis, ties → x; `window_speed` records but does not expand a successor landing on a barrier; **the seed exemption** — a seed whose own cell is a barrier is still expanded (assert the flood is non-empty when `seeds ⊆ barriers`, the exact shape the sink-seeded detection call uses); `corner_speed` excludes an arrival with no legal successor and returns `0` when none qualifies |
| 6 | `phase5_runout_checks` | one issue per maximal deficient run, anchored at the run's first point; **near-miss negative** (AC6): adequate run-out room ⇒ empty; `metrics.fastest_lap` empty ⇒ empty, no panic; the R4 pin (`speed_heatmap` at the `long_straight` start cell is `> 1`, so the sink set is *not* non-empty by construction) |
| 7 | `deficit_at` | the AC3 table above, both halves |
| 9 | `add_edit_wall` / `remove_edit_wall` | canonical wall = min `wall_sort_key` when several identify the same flip; out-of-box target ⇒ `None`; `D`-interior cell ⇒ `None` for a remove; `assert_single_cell_flip` helper (AC1) added to `testfix.rs` |
| 10 | `push_outer_wall_out`, `fill_inner_tooth` | width strictly grows on the `ac7_sharp_neck` geometry; a cap at the bounding-box edge yields `NoEdit` (the `BBOX_PAD = 1` headroom constraint); `fill_inner_tooth` commits and `bounded_complement_components(scratch) == 1` (test-side proof of the local guard); a border tooth is declined |
| 11 | `lengthen_straight`, `widen_corner` | `lengthen_straight` wins on `brake_deficit_corridor`; a `widen_corner` fixture where widening `end` gives the `v = 2` arrival a legal successor (found by direct `legal_move` enumeration in the test, not hand-derivation) strictly reduces the deficit; arm-rank tie-break asserted on a constructed tie |
| 12 | `trim_arm_wall`, `nudge_finger` | `trim_arm_wall` clears a one-cell infield intrusion and the post-edit flood-fill reports connected + exactly one bounded hole; `nudge_finger` re-opens the Ф4 `base_ring_d` + `hole_with_finger_skel` finger at its base cell `(15, 9..=11)`, which is `D`-boundary because the main hole survives at `x ≤ 14` `[measured: sed -n 593,613p crates/gen/src/phase4.rs → base_ring_d clears x,y ∈ 6..15 (i.e. 6..=14); notch_ring_d's finger footprint is x ∈ 15..18, y ∈ 9..12]`; a removal that would disconnect `D` is **rejected** |
| 13 | `phase6_local_repair`, `dispatch` | AC5: all 9 labels reach a non-panicking `ArmOutcome`; `Disconnected`/`BadTopology` ⇒ `NoEdit(NotRepairable)`. AC4: `Failed` iff zero commits; `Repaired.edits` non-empty and each edit's cell drivability in the returned `d` equals `edit.drivable` and differed in `ctx.d`. AC7: `map_frontier_gap_to_edge` wired with unchanged semantics on the `broken_ring` diagnostic; `NoCandidate` ⇒ `NoEdit(NoCandidate)`. Severity ordering: a mixed issue list is processed removes-before-adds regardless of input order; a stale payload ⇒ `NoEdit(StalePayload)` |
| 14 | `phase6_local_repair` | AC2 consequence discriminators (both directions, § Approach); AC9 totality battery — empty issue list, out-of-box walls, degenerate zero-area corridor, `metrics: None`, `stall_walls: None`; AC9 determinism — repeated calls and a shuffled issue list yield identical `RepairOutcome` |

Fixtures added to `testfix.rs`: `brake_deficit_corridor` (+ its frozen path/sinks),
`assert_single_cell_flip`. Everything else reuses `ring_*`, `long_straight_*`, `trap_ring`,
`dead_end_corridor`, `crash_pocket_fixture`, and Ф4's own `base_ring_d` / `notch_ring_d` /
`hole_with_finger_skel` geometry.

## Open questions

- **Q1 — the run-out deficit refinement (§ Risks R3). RESOLVED, not open.** The orchestrator
  independently verified the vacuity proof, the owner approved, and the spec was amended:
  § Amendments (round 3) **A1** now carries the geometric `runout_room` + measured `v_corner`
  formula verbatim, amending § The three new detectors item 3 and the § Per-arm progress metrics
  `NoBraking` row (AC4's referent). No AC was renumbered and no AC's intent changed. Spec and
  design agree; no owner action outstanding.
- **Q2 — `fill_inner_tooth`'s metric (§ Risks R5). RESOLVED, not open.** Same route: verified,
  approved, and amended into the spec as § Amendments (round 3) **A3** — the § Per-arm progress
  metrics `fill_inner_tooth` row now reads "the named `tooth` **became drivable** and exactly one
  bounded complement component survives". Spec and design agree. (§ Amendments **A2** likewise
  closes § Risks R4's sink-set defect.)
- **Q3 — `NarrowSf` after Ф6 (§ Risks R6). GENUINELY OPEN, and potentially spec-amending.** Ф6
  widens the corridor at the S/F but cannot re-cut `sf.chord`, so a `NarrowSf` issue survives Ф6
  until Ф3 re-runs. Is re-cutting `sf` in the `generate()` repair loop the intended integration, or
  should Ф6 decline `NarrowSf` outright? **Default taken by this design (implementable as-is):**
  repair the corridor at `center` along `sf.orient` — progress toward the floor — and record the
  gap in the arm's rustdoc. Resolving it the other way would change the § Per-arm progress metrics
  **`NarrowSf` row** and is therefore a **spec amendment**, not a design fold-in; it does not block
  implementation, since the default is well-defined and testable.
- **Label count.** `docs/design.md` §2 Ф6 lists **seven** labels across five dispatch rows
  (`NARROW`, `NARROW_SF`, `NO_BRAKING`, `CONCAVE_CHORD_CUT`, `ARMS_MERGING`, `LOST_HAIRPIN`,
  `DYNAMICALLY_DISCONNECTED`) `[measured: sed -n 174,183p docs/design.md]`; the spec's "six defect
  labels → five repair arms" appears to collapse `NARROW`/`NARROW_SF`. The design enumerates all
  seven plus the two decline labels — strictly stronger than AC5 asks. No action needed.
- **`v_target` promotion to `GenParams`** — deferred by the spec; unchanged here (`v_target` stays
  a call/context parameter, matching `phase3_start_finish(d, skel, m, v_target)`).
