# gp-gen N3: `map_frontier_gap_to_edge` — reachability-deficit → dual-edge mapping (prototype-first spike)

**Source:** issue #30
**Date:** 2026-07-24
**Tracked in:** #30

## Scope

Build `map_frontier_gap_to_edge` in `gp-gen` (crate dir `crates/gen`, package
`gp-gen`) — the design's single riskiest, explicitly-unproven step
(`docs/design.md` §2 `[N3]`, §2 Ф6 `DYNAMICALLY_DISCONNECTED`). Given the
oracle's "reachability stalled here" verdict, return the **concrete dual edge**
(`gp_core::geom::Wall` = `{ cell: Point, side: Side }`) whose one-edge shift is
the repair candidate — the `DYNAMICALLY_DISCONNECTED` arm of the Ф6 dispatch
table.

The design names this a **prototype-first spike**: validated on a hand-built
almost-valid track *before* the rest of the Ф6 repair loop, because this is where
the "almost-valid by construction + oracle certifies + local repair" scheme most
plausibly fails to converge and falls back to a full reseed.

Four deliverables:

1. **Amend the Ф5b stall diagnostic so it localizes the stall.** The currently
   committed diagnostic does not (proof in § Technical constraints), so
   `crates/gen/src/phase5b.rs`'s emitted diagnostic — `OracleResult::NotLappable`'s
   payload, the `frontier_gap` helper, and their rustdoc — is **in scope for
   redefinition**, together with the committed assertions that pin the old
   semantics (enumerated in § Contract changes to a merged module).
2. **The mapping function.** A deterministic, total, non-panicking function from
   that amended diagnostic (plus `D`, the start grid, and the S/F gate) to a
   result carried by a **dedicated enum**, never by `Option<Wall>` — an absent
   `Wall` alone cannot distinguish "not this arm's job" from "this arm's job, and
   it failed". Whether that enum carries **two** outcomes (repair `Wall` |
   no-candidate) or **three** (plus a *declined* arm) is settled during design by
   an executable proof gate that runs *before* the shape is locked — see
   § The monotonicity proof gate, AC7 and AC8.
3. **Hand-built fixture validation under a monotone-progress criterion.** At
   least one hand-built almost-valid track with a single known blocking edge; the
   returned edit must **strictly grow** the measured progress set (§ Progress
   metric), plus a full-closure assertion on the fixture where one edge provably
   suffices.
4. **The reseed fallback + convergence-risk documentation.** Module-level rustdoc
   records the `[N3]` convergence risk, states what each of the three outcomes
   means for the caller, and names the reseed fallback: a *no candidate* result
   burns a repair-budget step, and budget exhaustion returns to Ф1 with a new
   seed (design §2 `generate()`'s `if D == FAILED: break`; `[N4]` seed budget).

## Out of scope

- **The Ф6 repair loop itself** (`phase6_local_repair`). Applying the returned
  edit to `D`, iterating the issue list, the `[C3]` add-vs-remove recheck
  scoping, and the repair-budget loop are a separate build-order item. This issue
  produces the mapping, not its driver. In particular the mapper does **not**
  measure its own progress at runtime — progress is measured by the *tests*
  (§ Progress metric); the runtime `progressed ? D : FAILED` decision belongs to
  the Ф6 loop.
- **The other four Ф6 dispatches** — `NARROW`/`NARROW_SF` →
  `push_outer_wall_out`, `NO_BRAKING` → `lengthen_straight`/`widen_corner`,
  `CONCAVE_CHORD_CUT` → `fill_inner_tooth`, `ARMS_MERGING`/`LOST_HAIRPIN` →
  `trim_arm_wall`/`nudge_finger`.
- **`generate()` pipeline wiring.** `generate()` stays `todo!` — the same
  deferral Ф5a and Ф5b each took for their own entry points.
- **Ф7 output assembly** (`s_field`, `centerline`, `TrackArtifact` population).
- **Any `gp-core` change.** `Wall`, `Side`, `Corridor`, `walls_from_boundary`,
  `legal_move`, `supercover`, and `LapCounter::register_move` are consumed
  unchanged. A genuinely missing core primitive is a Design Amendment, not a
  silent widening.
- **Rewriting `ai-docs/plans/done/`.** Those are history surfaces and stay
  untouched even though this task supersedes part of what they describe. The live
  surfaces to keep truthful are the `phase5b.rs` rustdoc and the new `INDEX.md`
  row this task writes.
- **`V_target` / design-input sizing.** `V_ceil` remains the oracle's sliding
  search scaffold, never the design input `V_target` (design §2 `[D3]`).

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| Ф6 repair loop + the other four dispatch arms | Distinct build-order item; this is the prototype-first spike the design asks for | No — the Ф6 build-order issue covers it |
| `generate()` / Ф7 wiring | Integration item, deferred by Ф5a and Ф5b alike | No — design build order covers it |
| A *quality* (not passability) repair path for poor `Vmax_attain` / run-out | A track can be lappable yet slow; that is a metrics-quality concern, not a reachability stall, and has no producer in the design pseudocode today | **Conditional on AC8's branch.** On **branch A** (proof holds → two outcomes) yes — raise one, so the concern is not lost when the declined arm is dropped. On **branch B** it stays live inside the declined arm and needs no issue. |

## Key decisions

| Question | Decision |
|---|---|
| **KD1** — where the stall-localizing signal comes from | **Amend Ф5b** (owner, round 1). `phase5_full_oracle`'s emitted diagnostic is redefined to localize the stall, and the mapper consumes it directly. One signal, not two. Accepts reopening a merged module's contract and its tests. |
| **KD2** — strength of the hand-built-fixture assertion | **Monotone progress** (owner, round 1): the returned edit must strictly grow the measured progress set, with full closure asserted only where one edge provably suffices. Metric pinned in § Progress metric. |
| **KD3** — deficit classes handled | **V=1 geometric sever handled fully; anything else is declined, not guessed** (owner, round 1). |
| **KD4** — outcome-shape arity (2 vs 3 variants) | **Prove it first** (owner, round 2). The design phase must first pin the monotonicity claim with an **executable** test (AC7); the enum's arity is then fixed by that test's result under a **pre-authorised two-branch rule** (AC8). Deliberate ordering — the owner declined to lock the shape on an unexecuted derivation, so the proof is a gate, not a footnote. Neither branch needs a further interview round. |
| Shape of the amended diagnostic (`Vec<Point>` vs `Vec<Wall>` vs a struct) | Design call. This spec fixes the *requirements* (§ Requirements on the amended diagnostic), not the type. |
| Renaming `break_points` / restructuring `OracleResult` | Free. AGENTS.md § *API Stability*: `gp-gen` is a game-app crate, never published, with no downstream consumer — rename and restructure cleanly, add no aliasing layer. Verified: `crates/gen/src/phase5b.rs` is the **sole** site referencing `OracleResult` / `break_points` / `frontier_gap` anywhere under `crates/`. |
| Edit polarity emitted by this arm | **Add-edits only** — a sever is closed by making a currently non-drivable cell drivable, i.e. shifting a boundary wall outward. Design `[C3]` classes add-edits as monotonically safe for lap existence, so they need only a local recheck. A remove-edit repair, if ever needed, is a different Ф6 arm. |
| How the mapper learns V=1 lappability for classification | Default: the mapper calls the already-committed `oracle_liveness_v1(d, grid, sf, race_dir)` itself — cheap (flood-fill cost, design §3) and already in the crate. Design may instead take it as a parameter. |
| Determinism | Required: identical inputs → identical outcome. Same discipline as Ф5a/Ф5b — `HashSet`s allowed internally, every *output* order-independent (sorted or aggregated). |
| New dependency | None expected — `gp-gen` already has `gp-core`, `rand`, `rand_xoshiro`, `strum`. |
| Module placement | A new module. `phase5b.rs` is already 958 lines including tests, against the workspace soft limit, so the mapper does not extend it. |
| Test placement / fixtures | `#[cfg(test)] mod tests` in the new module; fixtures shared via the existing `crates/gen/src/testfix.rs`, which already holds `ring_corridor` / `ring_sf` / `ring_grid` / `dead_end_corridor` and was lifted there in anticipation of Ф6. |
| Miri | `gp-gen` is the sanctioned crate-level Miri `--exclude` (#134 cost carve-out) — no per-test `#[cfg_attr(miri, ignore)]` needed. |

## Technical constraints

### State of the art (verified against `crates/gen/src/`, not the issue body)

- `crates/gen/src/phase5.rs` (Ф5a, #28 CLOSED) — `forward_reachable(d, seeds,
  v_ceil)`, `backward_reachable(d, goals, v_ceil)`, and
  **`oracle_liveness_v1(d, grid, sf, race_dir) -> bool`**. The cheap V=1 liveness
  returns a **bare `bool`**, so the design's `issues = [DYNAMICALLY_DISCONNECTED]`
  arm carries **no location payload at all** today.
- `crates/gen/src/phase5b.rs` (Ф5b, #29) — `pub fn phase5_full_oracle(...) ->
  OracleResult`, with `OracleResult::NotLappable { break_points: Vec<Point> }`.
  `break_points` is the goal-aware **P0-frontier**: cells of `proj(R)` outside
  the phase-0 region `P0` having a 4-neighbour in `P0`, with an unconditional
  `grid.positions` fallback when that frontier is empty. `frontier_gap` and
  `fastest_lap_through_live` are `pub(crate)`; `OracleResult` is `pub`
  (re-exported by `pub use phase5b::*`).
- `crates/gen/src/phase4.rs` — the `Issue` enum (`Disconnected`, `BadTopology`,
  `Narrow`, `NarrowSf`, `LostHairpin`). It has **no** `DynamicallyDisconnected`
  variant; the design's Ф6 dispatch label has no representation in code yet.
- No `phase6.rs` exists; `generate()` is `todo!`.

### Why the committed `break_points` is not a stall localizer

This is the spike's central obstacle and the reason KD1 resolved to *amend*:

1. `break_points` is always a subset of `D`. Every member comes from `proj(R)`
   (states in `forward_reachable`, whose non-seed members all satisfy
   `legal_move`'s `p1 ∈ D`) or from the `grid.positions` fallback. It can
   therefore **never name a non-drivable cell** — and a geometric sever is
   repaired by making a currently non-drivable cell drivable.
2. On the repo's own broken-ring fixture (`ring_corridor()` with `(4, 2)` set
   non-drivable), the committed test asserts
   `break_points.contains(Point::new(2, 0))` — the **behind-gate cell**, adjacent
   to the S/F gate, at the opposite end of the track from the break. The severed
   cell `(4, 2)` is provably absent by (1). Shifting a dual edge anchored at
   `(2, 0)` does nothing to repair the break.
3. The cell where phase-0 reachability actually terminates (the dead end of the
   `P0` arc) lies *inside* `P0`, so the P0-frontier definition excludes it from
   `break_points` entirely.

The `phase5b` design already records that the design doc's *original*
`frontier_gap(d, &R)` was proven always-empty and was replaced by this goal-aware
P0 form. That amendment fixed non-emptiness; it did not fix localization.
Localization is this spike's job.

### Requirements on the amended diagnostic

The redefined Ф5b diagnostic must:

- **R1 — localize.** Name the place where phase-0 reachability terminates, in a
  form from which a **boundary dual edge and its off-`D` neighbour** are
  derivable. On the broken-ring fixture it must implicate the neighbourhood of
  the severed cell `(4, 2)`, not the behind-gate cell `(2, 0)`.
- **R2 — stay non-empty exactly when `NotLappable`.** The current code guarantees
  this with an unconditional `grid.positions` fallback; the amended form needs
  its own documented fallback preserving the invariant.
- **R3 — stay deterministic.** Order-independent output (sorted), as today.
- **R4 — stay gen-internal.** It is a Ф6 input, not part of the exported
  `gp-core` `TrackArtifact` contract.

### Contract changes to a merged module

Amending Ф5b invalidates committed assertions. These are **expected** to change,
and are listed so the design phase and the reviewer are not surprised (in
`crates/gen/src/phase5b.rs` unless noted):

| Item | Why it changes |
|---|---|
| `OracleResult::NotLappable`'s payload + its rustdoc (currently documents the P0-frontier + seed-cell-fallback rationale) | The diagnostic is redefined (R1) |
| `ac3_broken_ring_is_not_lappable_with_non_empty_break_points` — asserts `break_points.contains(Point::new(2, 0))` | `(2, 0)` is precisely the wrong-localization expectation this task removes; the replacement must implicate the severed region instead |
| `frontier_gap` + its rustdoc | The helper's definition is what changes |
| `frontier_gap_lists_r_cells_adjacent_to_a_proper_p0_but_not_in_p0`, `frontier_gap_is_empty_when_p0_equals_r`, `frontier_gap_is_empty_when_p0_is_empty` | Pin the old helper's semantics directly |
| `oracle_result_variants_are_constructible_and_clonable` | Constructs `NotLappable { break_points: vec![...] }`; changes only if the payload *type* changes |
| `fastest_lap_through_live_*` (`p0` assertions) | Change only if `P0`'s own contract changes; `P0` itself is expected to survive |
| `crash_pocket_fixture` (`:623`) and `long_straight_corridor` / `long_straight_sf` / `long_straight_grid` (`:812`/`:827`/`:841`) **move** from `phase5b.rs`'s private test module to `crates/gen/src/testfix.rs` | AC7's proof battery is a third call site; same ≥2-call-site consolidation that created `testfix.rs`. A move, not a rewrite — the fixture bodies are unchanged |

Anything **not** in this table that a design proposes to change is scope drift
and needs an explicit ask.

### Progress metric (the mechanically checkable core of AC5)

- **Measured set:** `P0` — the phase-0 reachable **cell** set at `V_ceil = 1`
  (post-race-start, pre-lap-close), i.e. the set `fastest_lap_through_live`
  already returns alongside the fastest path.
- **Metric:** `|P0|`, an integer set cardinality. Order-independent, hence
  deterministic despite `HashSet` intermediates; no fractional arithmetic.
- **Why not `|proj(R)|`:** it grows whenever *any* drivable cell is added
  anywhere, including a dead-end pocket pointing away from the goal — it is
  goal-blind and would pass a useless edit. `P0` is goal-aware by construction.
- **Assertion:** apply the returned edit to a scratch copy of `D`, recompute, and
  require `|P0_after| > |P0_before|` — **strict** growth.
- **Ties / no growth:** `|P0_after| == |P0_before|` is a **test failure** for the
  fixtures in this spike (the mapper returned an edge that does not help). It is
  not a runtime concern for the mapper — at runtime, a non-progressing edit is
  the Ф6 loop's `progressed ? D : FAILED` decision, which is out of scope.
- **Closure, where one edge suffices:** on the fixture whose single blocking edge
  is known by construction, additionally assert `phase5_full_oracle` flips from
  `NotLappable` to `Lappable` after the edit. This is the strongest available
  evidence on the `[N3]` convergence question and is asserted wherever the
  fixture supports it.

### The monotonicity proof gate

KD3's declined outcome was chosen under the framing "a **dynamic-only stall**:
V=1 lappable, but the full `Vmax` oracle is not". Tracing the committed driver to
size that class suggests it is **empty**:

`phase5_full_oracle` loops `V_ceil = 1, 2, 4, …`, and each ingredient grows
monotonically with `V_ceil` — `within_v_ceil` admits strictly more states, so
`R` grows; `lap_close_goals` iterates a larger `R` under a looser bound, so the
goal set grows; `backward_reachable` from a larger goal set grows; hence
`live = R ∩ B` grows. `fastest_lap_through_live` is confined to `live` under the
same bound, so a lap path found at `V_ceil = 1` is still present at every higher
ceiling. `NotLappable` is returned only via the `let Some(fastest) = fastest
else` arm — which therefore can only fire on the **first** iteration, at
`V_ceil = 1`. So `NotLappable` would imply no lap at `|v| ≤ 1`, which is exactly
what `oracle_liveness_v1` reports. A track with a genuinely un-brakeable corner
would still be *lappable* at low speed, reporting poor `Vmax_attain` — a
metrics-quality problem, not a reachability stall.

**Confirmed structurally, unconfirmed end-to-end.** The *structural* half was
independently re-checked against the committed sources: `within_v_ceil` is
monotone in `v_ceil` (`crates/gen/src/phase5.rs:55-58`);
`fastest_lap_through_live` gates successors on `within_v_ceil(s2, v_ceil) &&
live.contains(&s2)` (`crates/gen/src/phase5b.rs:224`); `NotLappable` is reachable
only from the single `else` arm, and `v_ceil` doubles only after that arm is
passed (`crates/gen/src/phase5b.rs:312-323` and `:347`). What remains
**unexecuted** is the end-to-end claim.

That structural confirmation is **not** a substitute for AC7's executable test.
The owner declined to lock the outcome shape on an unexecuted derivation, so the
ordering is deliberate: **AC7 runs first and gates AC8.** The design phase
executes the proof, then the enum's arity follows mechanically from the result
via AC8's two pre-authorised branches — no further interview round either way.

**Fixture availability note.** The proof battery needs fixtures that currently
have mixed visibility: `ring_corridor` / `ring_sf` / `ring_grid` /
`dead_end_corridor` are already shared in `crates/gen/src/testfix.rs`, but
`crash_pocket_fixture` (`phase5b.rs:623`) and `long_straight_corridor` /
`long_straight_sf` / `long_straight_grid` (`phase5b.rs:812`/`:827`/`:841`) are
private to `phase5b.rs`'s own test module. Reusing them requires lifting them to
`testfix.rs` — the same ≥2-call-site consolidation that created `testfix.rs`.
This lift is **in scope** and is an addition to the § Contract changes table.

### Workspace constraints applied silently

- Deterministic, integer reasoning over the corridor; no OS entropy.
- No `unwrap()` in production code without justification; `expect("reason")`
  preferred; total on adversarial input (`checked_*` / `saturating_*`), matching
  the Ф5a/Ф5b discipline.
- Strict clippy (`-D warnings`), every public item documented, magic numbers as
  module-level `const`.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The amended Ф5b diagnostic **localizes the stall** (R1): on the broken-ring fixture (`ring_corridor()` with `(4, 2)` non-drivable) it implicates the severed region, and a test pins that it no longer implicates only the behind-gate cell `(2, 0)`. |
| AC2 | The amended diagnostic is **non-empty exactly when** `phase5_full_oracle` returns `NotLappable` (R2), preserved through a documented fallback, and is deterministic across repeated runs (R3). |
| AC3 | Given a stall diagnostic for a V=1 geometric sever, `map_frontier_gap_to_edge` returns a concrete `gp_core::geom::Wall`. |
| AC4 | The returned `Wall` is a **boundary** edge of `D` — its `cell` is drivable and the neighbour across its `side` is not — so "shift it outward" is a well-defined add-edit rather than an interior no-op. |
| AC5 | **Monotone progress (KD2)** on ≥1 hand-built almost-valid track with a single known blocking edge: applying the returned edit to a scratch copy of `D` yields `\|P0_after\| > \|P0_before\|`, strictly, with `P0` measured at `V_ceil = 1`. |
| AC6 | **Closure where one edge suffices:** on that fixture, `phase5_full_oracle` returns `NotLappable` before the edit and `Lappable` after it. |
| AC7 | **Monotonicity proof gate (executable; runs BEFORE AC8 is decided).** A test asserts the biconditional `oracle_liveness_v1(d, grid, sf, dir) == matches!(phase5_full_oracle(d, grid, sf, dir), OracleResult::Lappable(_))` — equivalently, `NotLappable` implies no lap at `\|v\| ≤ 1`. It is evaluated over the **whole existing fixture battery** (`ring_corridor`; the broken ring with `(4, 2)` non-drivable; `dead_end_corridor`; `long_straight_corridor`; `crash_pocket_fixture` — lifting the private ones to `testfix.rs`) **plus ≥1 purpose-built candidate counterexample**: a corridor that *is* V=1 lappable and additionally contains a hazard un-brakeable at higher speed (a non-empty `R \ B` at some `V_ceil > 1`), i.e. the exact shape that would falsify the claim if it were falsifiable. A battery of only pre-existing fixtures does **not** discharge AC7. |
| AC8 | **Outcome-shape arity — a two-branch conditional keyed on AC7, both branches pre-authorised (KD4).** **Branch A — AC7's assertion holds on every fixture incl. the purpose-built counterexample:** the result enum has exactly **two** variants (repair `Wall` \| no-candidate); no declined arm is written, and the emptiness of the dynamic-only class is recorded in the module rustdoc with a pointer to the AC7 test. **Branch B — AC7's assertion fails on any fixture:** the result enum has **three** variants per round-1's "V=1 + classify" (repair `Wall` \| declined \| no-candidate), and the falsifying fixture becomes the declined arm's regression test. In both branches the outcome is a dedicated enum, never `Option<Wall>`. The design document must state which branch was taken and cite the AC7 run that decided it. |
| AC9 | The **no-candidate** outcome is produced, non-panicking and without a sentinel `Wall`, on a fixture where a stall is diagnosed but no boundary edge grows `P0`. (Present in both AC8 branches.) |
| AC10 | The mapper is **total** on adversarial input: an empty diagnostic, a diagnostic naming a cell outside `D`'s bounding box, and a degenerate corridor each yield an explicit outcome rather than panicking or overflowing. |
| AC11 | The mapping is **deterministic** — identical inputs yield an identical outcome across repeated runs, despite `HashSet` intermediates. |
| AC12 | Module-level rustdoc documents the `[N3]` convergence risk, states what **each** outcome of the shape chosen by AC8 means for the caller, and names the reseed fallback (no-candidate → burn a repair-budget step → budget exhaustion → new seed, design `[N4]`). |
| AC13 | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` are clean; the whole workspace test suite is green, including the rewritten `phase5b` assertions listed in § Contract changes. |

## Open questions

- **Does one edge ever suffice in general?** The design asserts each Ф6 repair is
  a single-dual-edge shift, but for a multi-cell sever the honest answer may be
  "one edge per iteration, N iterations". AC5 (strict growth) is satisfiable
  either way; AC6 (closure) is only assertable on a one-edge-repairable fixture.
  If the spike shows multi-edge severs are common, that finding belongs in the
  convergence-risk rustdoc (AC12) rather than being papered over — it is exactly
  the `[N3]` risk the design flags.
- **Should the `Issue` enum gain a `DynamicallyDisconnected` variant?** Design's
  Ф6 dispatches on that label, but Ф4's `Issue` is currently a *static*-check
  vocabulary. Whether the dynamic verdict joins that enum or stays a separate
  type is a design-phase call, recorded here so the design phase does not
  discover it cold.
- **Tie-breaking among equally-good candidate edges.** Determinism (AC11) forces
  *a* rule (e.g. min-`Point` then `Side` declaration order); whether the rule
  should also prefer, say, the edge nearest the medial axis is a quality question
  the spike may answer empirically.
