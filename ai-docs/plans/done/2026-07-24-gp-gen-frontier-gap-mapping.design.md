# Design: gp-gen N3 — `map_frontier_gap_to_edge` (reachability-deficit → dual-edge mapping)

**Issue:** [#30](https://github.com/maratik123/graphite-gp/issues/30)
**Spec:** `ai-docs/plans/2026-07-24-gp-gen-frontier-gap-mapping.spec.md`
**Branch:** `feat/2026-07-24-gp-gen-frontier-gap-mapping`
**Date:** 2026-07-24

---

## AC7 proof gate — EXECUTED during the design phase; AC8 takes **Branch A**

The spec (KD4, AC7, AC8) requires the outcome-shape arity to be fixed by an
**executed** proof, not a derivation. It was executed in this design phase.

**Method.** A throwaway probe module (`crates/gen/src/probe_tmp.rs` + a
`#[cfg(test)] mod probe_tmp;` line in `lib.rs`) was added, run, then removed;
`lib.rs` was restored from a `cp` backup and the tree re-verified clean
`[measured: git status --porcelain → only the untracked spec file]`, with the
pre-existing suite re-confirmed green
`[measured: cargo test -p gp-gen → test result: ok. 138 passed; 0 failed]`.
The probe file no longer exists — **subtask 2 re-creates its AC7 content as a
permanent in-tree test**, and every fixture body it used is reproduced verbatim
in § Fixture designs below.

**Deciding run** — `cargo test -p gp-gen probe_tmp -- --nocapture`, all 7 probe
tests green:

```
AC7 ring:          oracle_liveness_v1=true  full_oracle_lappable=true
AC7 broken_ring:   oracle_liveness_v1=false full_oracle_lappable=false
AC7 dead_end:      oracle_liveness_v1=false full_oracle_lappable=false
AC7 long_straight: oracle_liveness_v1=true  full_oracle_lappable=true
AC7 crash_pocket:  oracle_liveness_v1=false full_oracle_lappable=false
AC7 trap_ring:     oracle_liveness_v1=true  full_oracle_lappable=true
trap_ring: |R|=185 |B|=569 |R\B|=10 witness_in_R=true witness_in_B=false
```

The biconditional `oracle_liveness_v1(d, grid, sf, dir) ==
matches!(phase5_full_oracle(d, grid, sf, dir), OracleResult::Lappable(_))` held
on **every** fixture of the battery, **including the purpose-built candidate
counterexample** `trap_ring` — a corridor that IS V=1 lappable *and* carries a
hazard un-brakeable at higher speed (`|R \ B| = 10` at `V_ceil = 2`, with the
named witness `CarState { x: 6, y: 5, vx: 0, vy: 2 } ∈ R`, `∉ B`). A battery of
only pre-existing fixtures would not have discharged AC7; the counterexample
candidate is the load-bearing member and it failed to falsify.

**Independent reproduction + what the result actually proves.** `design-review`
rebuilt the deleted probe from the public API and reproduced every figure above
to the digit, and additionally established the stronger structural result: the
biconditional is **unfalsifiable, not merely unfalsified** — `live` is monotone
in `V_ceil`, so the `let Some(fastest) = fastest else` arm (`phase5b.rs:312`)
can only fire on the **first** iteration, at `V_ceil = 1`. No fixture can make
the assertion fail while the driver keeps its current shape. Two consequences
this design carries forward:

- The AC12 rustdoc must record the emptiness as *"the dynamic-only stall class
  is empty because `live` is monotone in `V_ceil`, so `NotLappable` can only be
  returned at `V_ceil = 1`; the AC7 test pins that structural property against
  future driver edits"* — **not** as "no counterexample was found". The former
  is what is true; the latter would misdescribe a structural theorem as an
  empirical survey. (Subtask 8.)
- The AC7 test is still **required and worth its cost**: it is a regression
  guard on the driver's monotonicity, and it is what a future edit to the
  `V_ceil` loop would trip.

**What `trap_ring` contributes, precisely.** Of its 10 `R \ B` states at
`V_ceil = 2`, **9 are ordinary corner-overshoot states** and **1 is the named
spur witness** `(6, 5, 0, 2)`. The spur is what makes the fixture a *deliberate*
un-brakeable-hazard construction rather than an incidental one, but the design
does not claim the spur produces the bulk of `R \ B`. `[measured: probe →
|R \ B| = 10; witness_in_R = true, witness_in_B = false]` `[measured:
design-review reproduction → 9 corner-overshoot + 1 spur witness]`

**Consequence — AC8 Branch A.** The *dynamic-only stall* class (V=1 lappable
but full-`Vmax` `NotLappable`) is **empty**. `RepairCandidate` therefore has
exactly **two** variants — `Edge(Wall) | NoCandidate`. **No `Declined` arm is
written**, the mapper does **not** call `oracle_liveness_v1` for classification
(KD5's "default" is dropped as dead work — every stall it can be handed is a
V=1 geometric sever), and the emptiness is recorded in the `phase6` module
rustdoc with a pointer to the AC7 test (AC12).

**Branch B stays fully specified anyway** (§ AC8 Branch B contingency) so that
if it ever triggers, resolution needs no fresh design cycle. But **the
implementor does not get to switch branches**: on any disagreement with the
expected AC7 result, subtask 2 **records the observed output in `.progress.md`
and STOPS, surfacing to the orchestrator** (see the subtask-2 row and § AC8
Branch B contingency). Rationale: the arity of a **public enum** is the exact
decision the product owner routed through "prove it first" (KD4) so that it
would be taken on executed evidence **at a design tier**, not improvised
mid-implementation by a `sonnet`/`medium` implementor with no Opus-tier review
in the loop. This is AGENTS.md § Patterns 1 applied to the delegate direction —
a delegate's design-blocking observation is a finding, not a licence to amend
the contract. It should not trigger regardless: the probe called the same `pub`
entry points on the same fixture bodies, and the biconditional is structurally
guaranteed.

**Deferred-table consequence.** The spec's Deferred row *"a quality (not
passability) repair path for poor `Vmax_attain` / run-out"* is **conditional on
branch A → raise one issue**. Branch A was taken, so `/task` Step 12 must file
it (or an `_inbox.jsonl` row) — recorded here so the obligation is not lost.

---

## Approach

### 1. The amended Ф5b diagnostic: `NotLappable { stall_walls: Vec<Wall> }`

The committed `break_points: Vec<Point>` cannot name a non-drivable cell (spec
§ *Why the committed `break_points` is not a stall localizer*), so it cannot
localize a geometric sever. The replacement is the **P0 boundary-wall set**:

> for every cell `c ∈ P0` and every `Side` whose neighbour across it is not in
> `D`, one `Wall { cell: c, side }`.

This satisfies R1 **directly and in the exact shape R1 asks for** — "a boundary
dual edge and its off-`D` neighbour are derivable": the `Wall` *is* the dual
edge, and `c + side.delta()` *is* the off-`D` neighbour, i.e. precisely the
cell an add-edit would make drivable.

Measured on the broken ring (`ring_corridor()` with `(4, 2)` cleared)
`[measured: probe → broken_ring P0 = [(3,0), (4,0), (4,1)] (|P0| = 3); walls =
[(3,0)N, (3,0)S, (4,0)E, (4,0)S, (4,1)E, (4,1)W, (4,1)N]; off-D neighbours =
[(3,1), (3,-1), (5,0), (4,-1), (5,1), (3,1), (4,2)]]`:

- it **implicates the severed region** — `Wall { cell: (4,1), side: North }`'s
  off-`D` neighbour is exactly the severed cell `(4, 2)` (**AC1**);
- it contains **no** wall anchored at the behind-gate cell `(2, 0)`, because
  `(2, 0) ∉ P0` — the old expectation is not merely relaxed, it is *excluded*
  by construction (**AC1**).

**Naming.** `break_points` → `stall_walls`; the helper `frontier_gap(r_cells,
p0_cells)` → `p0_boundary_walls(d, p0)`. Free rename per AGENTS.md § *API
Stability* (`gp-gen` is a game-app crate; `phase5b.rs` is the sole referencing
site under `crates/`). The **mapper** keeps the name `map_frontier_gap_to_edge`
— fixed by `docs/design.md` §2 Ф6 (`docs/design.md:182`) and `[N3]`
(`docs/design.md:198`); "frontier gap" now denotes this diagnostic.

`proj(R)` disappears from the driver's `NotLappable` arm — the new diagnostic is
a function of `D` and `P0` only.

**R2 (non-empty exactly when `NotLappable`) — a documented two-tier fallback:**

| Tier | Emitted when | Value |
|---|---|---|
| 1 | `p0_boundary_walls(d, &p0)` is non-empty | that set |
| 2 | tier 1 is empty (`P0 == ∅`, or every `P0` cell is `D`-interior) | `gp_core::geom::walls_from_boundary(d)`, re-sorted with the same key |

Tier 2 is non-empty for every non-empty `D`: take the topmost drivable row —
its cells' `North` neighbours are outside `D` (or outside the box, which
`walls_from_boundary` already treats as `∉ D`, `geom/graph.rs:321-324`), so at
least one wall is always emitted. `[measured: probe no_crossing fixture →
|P0| = 0, tier-1 walls = [], tier-2 walls_from_boundary = 4 walls at (2,0)
E/W/N/S]`

**Edge case — an empty `D`, and the chosen resolution.** The retired
`grid.positions` fallback was *unconditionally* non-empty; tier 2 is not. On a
degenerate `D` (e.g. `Corridor::new(_, 0, 0)`) both tiers are empty and the
driver would return `NotLappable { stall_walls: [] }`, which reads as an AC2
violation. **Chosen resolution: a documented precondition, not a tier 3.**

- **What is written:** "`D` is non-empty" becomes an explicit *precondition*
  line in **both** the `OracleResult::NotLappable` rustdoc and the
  `p0_boundary_walls` rustdoc — sourced from the Ф2 generator contract (`D` is
  the rasterized corridor; an empty `D` never reaches Ф5b in the pipeline) — and
  AC2's test **names** it: one test asserts non-emptiness for non-empty `D`, and
  a second pins the degenerate-`D` behaviour (empty `stall_walls`) as the
  *documented* out-of-precondition outcome rather than leaving it unpinned.
- **Why not a tier 3 derived from `grid.positions`:** on an empty `D` those
  cells are by definition **not drivable**, so any `Wall` anchored at them would
  violate AC4's "the `cell` is drivable" property — the payload would carry
  edges that are not boundary edges of `D`. The mapper re-validates and would
  discard every one of them, so tier 3 buys a non-empty `Vec` at the cost of a
  *dishonest* payload and zero downstream effect. A precondition that says what
  is actually true is preferable to a fallback that manufactures invalid data.
- The mapper is unaffected either way: `Corridor::new(_, 0, 0)` is already an
  AC10 case and returns `NoCandidate` without panicking `[measured: probe →
  degenerate |P0| = 0, adversarial totality OK (no panic)]`.

**R3 (determinism):** `Wall` does **not** derive `Ord` and neither does `Side`
`[measured: crates/core/src/geom/graph.rs:20 → #[derive(Clone, Copy, PartialEq,
Eq, Hash, Debug, strum::EnumIter)]` — no `Ord`/`PartialOrd`]`, so a bare
`.sort()` will not compile. A crate-local key is required:

```rust
pub(crate) const fn wall_sort_key(w: Wall) -> (Point, u8)   // side rank: E=0, W=1, N=2, S=3
```
`Point` derives `Ord` (`geom/mod.rs:29`), so `(Point, u8)` is a total order.
Adding `Ord` to `gp-core`'s `Side`/`Wall` is explicitly **out of scope** (spec
§ Out of scope, "any `gp-core` change").

**R4:** `stall_walls` stays a gen-internal Ф6 input; nothing enters
`TrackArtifact`.

### 2. The mapper: verified-growth greedy over the diagnostic

`map_frontier_gap_to_edge` is a **total, deterministic, non-panicking** function
that only ever returns an edge it has *proved* grows the progress metric:

```rust
pub fn map_frontier_gap_to_edge(
    d: &Corridor, grid: &StartGrid, sf: &StartFinish,
    race_dir: RaceDir, stall_walls: &[Wall],
) -> RepairCandidate
```

1. `base = |p0_at_v1(d, grid, sf)|`.
2. For each `w ∈ stall_walls`, **re-validate** (never trust the diagnostic —
   AC10): `d.contains(w.cell)` and `wall_neighbor(w) == Some(q)` with
   `!d.contains(q)`; otherwise skip. This re-establishes **AC4** (returned
   `Wall` is a genuine boundary edge of `D`) independently of the producer.
3. Scratch-apply: `let mut d2 = d.clone(); d2.set(q, true);` then
   `grown = |p0_at_v1(&d2, grid, sf)|`. Keep the candidate only if
   `grown > base` (**strict** — § Progress metric).
4. Choose **max `grown`**, ties broken by **min `wall_sort_key(w)`**. The
   decision is a function of the *set* of candidates, not of the slice order,
   so **AC11** holds even for an unsorted input slice.
5. No surviving candidate → `RepairCandidate::NoCandidate` (**AC9**), never a
   sentinel `Wall`.

`race_dir` is accepted for signature fidelity and discarded with `let _ =
race_dir;` — the same convention `oracle_liveness_v1` (`phase5.rs:148`) and
`phase5_full_oracle` (`phase5b.rs:288`) already use.

Why greedy-with-verification rather than a geometric heuristic (rejected
alternative): a heuristic ("shift the wall nearest the medial axis", "shift the
wall at the P0 geodesic maximum") can return a *plausible* edge that does not
move the goal-aware metric at all, which is exactly the failure `|proj(R)|` was
rejected for. Verifying the metric inside the mapper makes **AC5 true by
construction** and makes **AC9** a genuine, non-arbitrary outcome. The cost is
`|stall_walls|` V=1 flood recomputations per call — bounded, integer-only, and
paid once per Ф6 repair-budget step. Measured end-to-end on the broken ring:
`[measured: probe → mapped = Some((Wall { cell: (4,1), side: North }, base 3,
grown 16)); closure OK: repaired by adding (4,2)]` — one edge, `|P0|` 3 → 16,
and `phase5_full_oracle` flips `NotLappable → Lappable` (**AC5 + AC6**).

**`NoCandidate` is reachable and correct, not a defect** `[measured: probe →
dead_end mapped = None; crash_pocket mapped = None]`: in both fixtures `D`
fills its whole bounding box, so every off-`D` neighbour lies outside the box
and `Corridor::set` is a documented no-op there (`geom/mod.rs:302-308`) — no
edit can grow `P0`. This must be stated in the rustdoc so a reader does not
read it as a swallowed error.

### 3. Result type (Branch A)

`missing_docs = "deny"` is workspace-wide (`Cargo.toml [workspace.lints.rust]`),
so **every variant carries its own `///`** — omitting them costs a needless
doc-gate cycle. The snippet is the contract, doc comments included:

```rust
/// The outcome of mapping a Ф5b stall diagnostic to a repair edit
/// (design `[N3]`, `docs/design.md` §2 Ф6 `DYNAMICALLY_DISCONNECTED`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepairCandidate {
    /// The dual edge to shift outward: making the cell across
    /// `side` drivable is verified to strictly grow `|P0|` at `V_ceil = 1`.
    Edge(Wall),
    /// No boundary edge derived from the diagnostic grows `P0` — the caller
    /// burns a repair-budget step (`[N4]` reseed fallback), never a sentinel
    /// `Wall`.
    NoCandidate,
}
```
A dedicated enum, never `Option<Wall>` (spec deliverable 2). Two variants only,
per the executed AC7 gate. No `Hash`/`Default` derive (YAGNI — no consumer).

### 4. Module placement and layering

New module `crates/gen/src/phase6.rs`, wired in `lib.rs` as `mod phase6;` +
`pub use phase6::*;` (matching every existing phase). `phase5b.rs` is 958 lines
`[measured: wc -l crates/gen/src/phase5b.rs → 958]`, already past the
incl.-tests soft limit, so the mapper does not extend it (spec KD *Module
placement*).

New `pub(crate)` items and their homes:

| Item | Home | Why |
|---|---|---|
| `p0_boundary_walls(d, p0) -> Vec<Wall>` | `phase5b.rs` | replaces `frontier_gap`; the driver is its only producer |
| `wall_sort_key(w) -> (Point, u8)` | `phase5b.rs` | the diagnostic defines the order; `phase6` reuses it for the tie-break |
| `wall_neighbor(w) -> Option<Point>` | `phase5b.rs` | shared by the diagnostic (∉ D test) and the mapper (edit target) |
| `live_at(d, seeds, sf, v_ceil) -> HashSet<CarState>` | `phase5b.rs` | the `R → G → B → R∩B` composition, extracted from the driver loop; 2 prod call sites + the test module |
| `p0_at_v1(d, grid, sf) -> HashSet<Point>` | `phase6.rs` | the **progress metric** primitive; Ф6's concern |

`wall_neighbor` deliberately does **not** use `Point::neighbors4` — that method
saturates, and a saturated self-neighbour would test `d.contains(cell) == true`
and wrongly suppress a real boundary wall. This is the identical hazard
`walls_from_boundary` documents at `geom/graph.rs:316-320`; use `checked_add`.

### 5. Lint-forced shapes (binding constraints, not style preferences)

Workspace lints deny `clippy::nursery` and `clippy::pedantic`
`[measured: Cargo.toml [workspace.lints.clippy] → pedantic = deny, nursery =
deny, arithmetic_side_effects = deny]`, so `missing_const_for_fn` **forces**
`const fn` on every const-eligible pure fn:

- `wall_sort_key` — body is a field read + a `match` on a `Copy` enum → const-
  eligible → **must** be `const fn`.
- `wall_neighbor` — `Side::delta` is `pub const fn` (`graph.rs:35`),
  `i32::checked_add` and `Point::new` are const → const-eligible → **must** be
  `const fn`, and it **must not** use `?`: `[measured: rustc --crate-type lib
  --edition 2024 (const fn returning Some(a.checked_add(b)?)) → error[E0658]:
  '?' is not allowed on Option<i32> in constant functions; rustc 1.97.1]`. Use
  the `let Some(x) = … else { return None; };` form — the exact in-tree
  precedent is `predecessor` (`phase5.rs:30-45`).
- `p0_boundary_walls` / `live_at` / `p0_at_v1` / the mapper allocate or call
  non-const fns → not const-eligible; the lint will not fire.

No raw arithmetic is introduced anywhere (`checked_add` only), so no
`arithmetic_side_effects` `#[allow]` is needed. No `unwrap`/`expect`/`panic!`/
panicking index is added to production code, so this task **adds no new row to
`ai-docs/panic-index.md`**; the `gp-core` zero-production-panics invariant is
untouched (no `gp-core` change). The index is **not** empty — it carries 5 live
rows (`crates/render/src/screens/setup.rs:229`/`:230`, `race.rs:417`/`:418`,
`crates/game/src/main.rs:111`), all in the `gp-render`/`gp-game` layers this
task does not touch. `[measured: design-review + coordinator verification of
ai-docs/panic-index.md → 5 live rows]` `[derived → AC13 clippy + doc gates]`

### 6. Rejected alternatives

| Rejected | Why |
|---|---|
| Keep `break_points: Vec<Point>`, add a second Wall-valued field | Two diagnostics for one stall; KD1 chose one signal. A `Vec<Point>` of *drivable* cells cannot name the repair target at all. |
| Emit `Vec<Point>` of off-`D` candidate cells instead of `Vec<Wall>` | Loses the anchor cell, so AC4's "is a boundary edge of `D`" is no longer decidable from the payload alone, and Ф6's other arms all speak `Wall`. |
| Progress metric `|proj(R)|` | Explicitly rejected by the spec — goal-blind; grows for a dead-end pocket pointing away from the goal. |
| Geometric heuristic instead of verified growth | Can return a non-progressing edge; would make AC5 a coin flip and AC9 unreachable. |
| Add `Issue::DynamicallyDisconnected` to Ф4's enum | **No.** `Issue` is Ф4's *static*-check vocabulary (`phase4.rs:20-25`), and after AC7 the dynamic verdict is carried by `OracleResult::NotLappable` itself. A variant with no producer is dead vocabulary that `-D warnings` and the next reviewer would both question. (Spec § Open questions, resolved.) |
| Adding `Ord` to `gp-core`'s `Side`/`Wall` for sorting | A `gp-core` change — out of scope. A crate-local sort key costs 6 lines. |

---

## Scope-delta vs the spec's § Contract changes table

Everything below is **inside** `crates/gen/`, semantics-preserving, and named
here so the reviewer can reject any item without re-reading the diff. Nothing
outside `crates/gen/src/` and the two `ai-docs/plans/` files is touched;
`ai-docs/plans/done/` is **not** edited.

1. **`live_at` extraction in `phase5b.rs`** — the driver's inline
   `forward_reachable → lap_close_goals → backward_reachable → intersection`
   block (`phase5b.rs:305-308`) becomes one `pub(crate) fn`, called by the
   driver and by `phase6`. Pure extraction, identical behaviour. Without it the
   same 4-line composition exists at 2 production sites in 2 files (and a 3rd
   copy already exists as the test-local `live_for`, `phase5b.rs:529-539`) —
   the ≥2-call-site consolidation rule that created `testfix.rs`.
2. **`live_for` (test helper, `phase5b.rs:529`) is deleted** and its 4 call
   sites point at `live_at`. Identical body; no assertion semantics change.
3. **`ORACLE_V1_CEIL` (`phase5.rs:124`) widens from private to `pub(crate)`**
   and `phase6` reuses it, rather than re-declaring the literal `1` for the
   V=1 progress metric. One-token visibility change, no behaviour change.
4. **Two new `testfix.rs` fixtures** — `trap_ring` (AC7's purpose-built
   counterexample candidate) and `no_crossing_corridor` (AC2's tier-2 fallback
   witness). Additions only.

If the reviewer judges any of 1–3 to be drift, dropping it costs only local
duplication and no AC coverage.

---

## Fixture designs

### `trap_ring` — the purpose-built AC7 counterexample candidate (load-bearing)

Requirements from AC7: **(a)** IS V=1 lappable, **(b)** carries a hazard
un-brakeable at higher speed, i.e. non-empty `R \ B` at some `V_ceil > 1`. A
closed ring alone gives (a); the *braking-trap spur* gives (b) deliberately
rather than incidentally.

```rust
/// A closed 12×8 ring (V=1 lappable, same border construction as
/// `ring_corridor`) plus a 5-cell dead-end **braking-trap spur** hanging north
/// off the bottom straight at `x = 6`, separated from the top straight by the
/// single wall row `y = 6`. A car that enters the spur and builds |v| = 2 has
/// **no legal move at all** at `(6, 5)` …
pub(crate) fn trap_ring() -> (Corridor, StartFinish, StartGrid) {
    let mut d = Corridor::new(Point::new(0, 0), 12, 8);
    for y in 0..8 {
        for x in 0..12 {
            if x == 0 || x == 11 || y == 0 || y == 7 {
                d.set(Point::new(x, y), true);
            }
        }
    }
    for y in 1..=5 {
        d.set(Point::new(6, y), true);   // the spur
    }
    // gate/grid: behind (2, 0), forward East, one car at rest on (2, 0)
    //            — same shape as `ring_sf` / `ring_grid`.
}
```

Why `(6, 5)` with `v = (0, 2)` is a genuine dead state: every action leaves
`vy ∈ {1, 2, 3}`, so `y' ≥ 6`; `(6, 6)`, `(5, 6)`, `(7, 6)` are all walls, and
`y' = 7` requires a chord whose `supercover` passes through the wall row
`y = 6`, which `legal_move` rejects. Reached from rest by
`(2,0)→…→(6,0)` at `v=(1,0)`, then NorthWest → `(6,1)`, North → `(6,3)`,
Coast → `(6,5)`. `[measured: probe → trap_ring |R|=185 |B|=569 |R\B|=10,
witness (6,5,0,2) ∈ R, ∉ B; oracle_liveness_v1 = true]`

### `no_crossing_corridor` — the AC2 tier-2 fallback witness

`Corridor::filled(Point::new(2, 0), 1, 1)` with the `ring_sf`-shaped gate at
`(2, 0)`: no cell ahead of the gate exists, so no forward crossing and no
lap-close goal exists, `live = ∅`, and hence `P0 = ∅`. `[measured: probe →
|P0| = 0, tier-1 walls = [], tier-2 = 4 walls]`

### Reused fixtures

`ring_corridor` / `ring_sf` / `ring_grid` / `dead_end_corridor` (already in
`testfix.rs`); `crash_pocket_fixture` (`phase5b.rs:623`) and
`long_straight_corridor` / `long_straight_sf` / `long_straight_grid`
(`phase5b.rs:812`/`:827`/`:841`) **move verbatim** to `testfix.rs` — a move,
not a rewrite (spec § Contract changes, last row).

---

## Decomposition

M = 9. All subtasks change **code** (`*.rs`) only. The `ai-docs/plans/INDEX.md`
row and any `ai-docs/learnings.md` entry are `/task` Steps 11–12 obligations,
not implementor subtasks — so no instructions/harness group exists.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Fixture consolidation.** Move `crash_pocket_fixture`, `long_straight_corridor`, `long_straight_sf`, `long_straight_grid` verbatim from `phase5b.rs`'s test module to `testfix.rs` as `pub(crate)`; add `trap_ring` and `no_crossing_corridor` (§ Fixture designs). The bodies move verbatim, but the **move is not import-free**: `testfix.rs`'s current `use` list is `Corridor, Orient, Point` + `CarState` + `StartFinish, StartGrid, TimingGate`, and it reaches `Side` only via the fully-qualified `gp_core::geom::Side::East`. The moved `crash_pocket_fixture` / `long_straight_sf` need `Orient`, `TimingGate` (already imported) and `Side` — add `Side` to the `gp_core::geom` import (and optionally normalise the two existing fully-qualified uses). `phase5b.rs` already does `use crate::testfix::*`, so its call sites need no edit beyond deleting the local defs. Suite must stay green. | `crates/gen/src/testfix.rs`, `crates/gen/src/phase5b.rs` | — |
| 2 | **AC7 proof gate, in-tree.** Create `crates/gen/src/phase6.rs` with module rustdoc + a `#[cfg(test)] mod tests` holding (a) the `trap_ring` two-property test and (b) the biconditional battery over all 6 fixtures; `mod phase6;` in `lib.rs` (no `pub use` yet — nothing public yet). **Record the run's output in `.progress.md`; it is the AC8 gate.** Expected: Branch A (§ AC7 proof gate), which is already reproduced twice and structurally guaranteed. **On ANY disagreement: record the observed output in `.progress.md` and STOP — surface to the orchestrator. Do NOT switch to Branch B in-group, and do NOT alter `RepairCandidate`'s arity.** The public enum's shape is a design-tier decision (KD4); an implementor observation is a finding, not a licence to amend the contract (AGENTS.md § Patterns 1). Branch B's full specification exists so that resolution, if ever needed, costs no design cycle — it is not an in-group escape hatch. | `crates/gen/src/phase6.rs`, `crates/gen/src/lib.rs` | 1 |
| 3 | **Amend the Ф5b diagnostic.** Replace `frontier_gap` with `p0_boundary_walls(d, p0)`; add `wall_sort_key`, `wall_neighbor` (both `const fn`, `let-else` not `?`), `live_at`; change `OracleResult::NotLappable`'s payload to `stall_walls: Vec<Wall>`; rewire the driver's `NotLappable` arm to the two-tier fallback; rewrite the affected rustdoc (variant, helper, module header) to describe localization + the fallback tiers. | `crates/gen/src/phase5b.rs` | 2 |
| 4 | **Rewrite the pinned assertions** listed in the spec's § Contract changes: `ac3_broken_ring_…` → asserts the diagnostic implicates `(4,2)` via some wall's off-`D` neighbour **and** contains no wall anchored at `(2,0)` (**AC1**); the three `frontier_gap_*` tests → `p0_boundary_walls_*` equivalents; `oracle_result_variants_are_constructible_and_clonable` → new payload type. Add **AC2** tests: tier-1 non-empty on the broken ring, tier-2 non-empty on `no_crossing_corridor`, sorted-order and repeat-run determinism. | `crates/gen/src/phase5b.rs` | 3 |
| 5 | **Mapper skeleton, tests first.** In `phase6.rs`: `pub(crate) fn p0_at_v1` (reusing `live_at` + `ORACLE_V1_CEIL`, widened in `phase5.rs`), `pub enum RepairCandidate { Edge(Wall), NoCandidate }`, `pub fn map_frontier_gap_to_edge(...)` per § Approach (2); `pub use phase6::*;` in `lib.rs`. | `crates/gen/src/phase6.rs`, `crates/gen/src/phase5.rs`, `crates/gen/src/lib.rs` | 3, 4 |
| 6 | **AC3/AC4/AC5/AC6 tests** on the broken ring — returns `Edge(Wall { cell: (4,1), side: North })`; the returned wall is a boundary edge (`cell ∈ D`, neighbour ∉ D); `\|P0_after\| > \|P0_before\|` recomputed **independently** of the mapper; `phase5_full_oracle` flips `NotLappable → Lappable` after the edit. **Also assert the max-growth selection is non-vacuous** (see § Test Design → *selection non-vacuity*): `Wall { cell: (3,0), side: North }`'s off-`D` neighbour `(3,1)` is in-box and non-drivable, i.e. a second admissible candidate, so measure both growths and pin that `(3,1)`'s is strictly smaller than `(4,2)`'s — without it, "max growth" is untested on a single-candidate fixture. | `crates/gen/src/phase6.rs` | 5 |
| 7 | **AC9/AC10/AC11 tests** — `NoCandidate` on `dead_end_corridor` and `crash_pocket_fixture`; totality on an empty slice, on walls at `(9999, 9999)` / `(i32::MAX, i32::MAX)` / `(i32::MIN, i32::MIN)`, and on a `Corridor::new(_, 0, 0)`; two identical calls yield an identical outcome, and a shuffled input slice yields the same outcome. | `crates/gen/src/phase6.rs` | 5 |
| 8 | **AC12 module rustdoc** on `phase6.rs`: the `[N3]` convergence risk (`docs/design.md:198`); what **each** of the two outcomes means for the caller; the reseed fallback (`NoCandidate` → burn a repair-budget step → budget exhaustion → Ф1 with a new seed, `[N4]`); the AC8 **Branch A** record naming the AC7 test — worded as *"the dynamic-only stall class is empty because `live` is monotone in `V_ceil`, so `NotLappable` can only be returned at `V_ceil = 1` (`phase5b.rs`'s `let Some(fastest) … else` arm); the AC7 test pins that structural property against future driver edits"*, **NOT** "no counterexample was found" (§ AC7 proof gate → *what the result actually proves*); the `NoCandidate`-is-not-an-error note (box-filling corridors); the one-edge-vs-multi-edge finding (§ Open questions, resolved); and the medial-axis tie-break refinement left un-adopted. Also update `phase5b.rs`'s module header line that still says `break_points`, and add the **`D` non-empty precondition** line to the `NotLappable` and `p0_boundary_walls` rustdoc (§ Approach (1) → *edge case*). | `crates/gen/src/phase6.rs`, `crates/gen/src/phase5b.rs` | 6, 7 |
| 9 | **AC13 gates.** `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`, `cargo test` (whole workspace). Re-run clippy **after** the first clean pass — it aborts on the first failure and can mask later sites. | all touched | 1–8 |

---

## Handoff plan

M = 9, single change-type (**code**, `*.rs` only). Group-minimisation
(AGENTS.md (f)) with a size cap of 10 (b) yields **one** group; 9 ∈ `1..=10`
satisfies the terminal-group rule (d). One group is ≤ the 4-group default (h),
so no user gate is needed.

- **Handoff into Group A:** run the `/context-reset` **skill** per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) —
  required at the start of **every** design-defined group, including the first
  (`.claude/skills/task/SKILL.md` Step 8). The skill launches one
  `Agent(subagent_type="code-writer")` call for Group A; `/context-reset` is a
  skill and `code-writer` is a subagent — the two are not interchangeable
  (AGENTS.md § Propagation Rule).
- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent (`model` + `effort` are frontmatter-pinned; no inline
  override), 1M-token window — subtasks **1–9** (code change-type: `*.rs`).
  **Terminal group** (9 subtasks; within `1..=10`). There is no inter-group
  handoff; Group A is the single group, entered through the `/context-reset`
  run described above and executed inside the one `code-writer` subagent it
  launches.

The Opus quality gates (`design`, `design-review`, `self-review`) are unaffected
by the group marker — only the implementor model/effort varies.

---

## AC8 Branch B contingency (specified, not expected)

Trigger: subtask 2's in-tree biconditional test **fails** on any fixture — and
then **only after the orchestrator has been surfaced to and has authorised the
switch**. The implementor's obligation on failure is to record the observed
output in `.progress.md` and STOP (subtask 2); it never selects this branch
itself. This section exists so that the authorised switch costs no design
cycle, not so that it can happen unreviewed.

Once authorised: `RepairCandidate` gains a third variant
`Declined` (documented as "V=1 lappable, so this arm's V=1 geometric-sever
classifier does not apply — a `Vmax`-quality problem, not a reachability
stall"); `map_frontier_gap_to_edge` calls
`oracle_liveness_v1(d, grid, sf, race_dir)` **first** and returns `Declined`
when it is `true` (KD3's "V=1 + classify"); the falsifying fixture becomes
`Declined`'s regression test in subtask 7; the AC12 rustdoc documents three
outcomes instead of two, and drops the Branch-A emptiness claim; and the spec's
Deferred row *"quality repair path"* stays live inside the declined arm and
needs **no** new issue (branch-B semantics). Nothing else in this design
changes — the diagnostic, the metric, the greedy, the fixtures, the group plan
and the file layout are branch-independent.

---

## Risks

- **The AC8 branch was decided by a run whose artefact was deleted** — *risk
  discharged.* The probe module was removed to keep the tree clean, so the
  branch initially rested on a run nobody else could re-execute. `design-review`
  has since rebuilt the probe from the public API and reproduced every figure to
  the digit, and separately proved the biconditional structurally (`live`
  monotone in `V_ceil` ⇒ the `NotLappable` arm at `phase5b.rs:312` can only fire
  at `V_ceil = 1`). Residual mitigation: subtask 2 re-creates the AC7 content as
  a **permanent** test whose output goes in `.progress.md`, and every fixture
  body is reproduced above. `[measured: probe deciding run, quoted in full in
  § AC7 proof gate]` `[measured: design-review independent reproduction → every
  figure identical]`
- **An implementor could silently re-shape a public enum.** Subtask 2 sits one
  failing assertion away from a `sonnet`/`medium` delegate rewriting
  `RepairCandidate`'s arity with no Opus-tier review — inverting KD4's whole
  point. Mitigation: subtask 2 mandates *record-and-STOP*, § AC8 Branch B
  contingency requires orchestrator authorisation before the switch, and Branch
  B stays fully specified so the authorised path is still cheap. `[derived →
  subtask 2's row + § AC8 Branch B contingency trigger wording]`
- **Multi-edge severs may not close in one edit** (spec § Open questions). The
  broken-ring fixture closes with one edge, but a 2-cell sever plausibly needs
  one edge per iteration. Mitigation: AC5 (strict growth) is the contract and is
  satisfiable either way; AC6 (closure) is asserted only on the fixture where
  one edge provably suffices; the finding is recorded in the AC12 rustdoc rather
  than papered over. `[measured: probe → broken ring closes with the single add
  of (4,2); |P0| 3 → 16]`
- **Mapper cost scales with the diagnostic's size** — one V=1 `live`+`P0`
  recomputation per candidate wall, and the diagnostic can be wide on a broad
  corridor. `[measured: probe → crash_pocket walls = 15 for |P0| = 7]`
  Mitigation: bounded by Ф6's repair budget (out of scope here), integer-only,
  no allocation growth beyond one `Corridor::clone` per candidate; documented in
  the rustdoc.
- **AC5's test is partially guaranteed by construction** (the mapper only
  returns verified-growing edges), so the assertion cannot fail while the mapper
  is correct. Mitigation: keep the test's recompute **independent** of the
  mapper's internals — it then still catches a regression where the mapper's
  metric drifts from `|P0| at V_ceil = 1` (e.g. someone swaps in `|proj(R)|`).
  The non-vacuous evidence is AC1 (localization) and AC6 (closure).
  `[derived → subtask 6 + self-review]`
- **`Corridor::set` silently no-ops outside the bounding box**, so a candidate
  whose off-`D` neighbour is out-of-box can never grow `P0`. This is *correct*
  but reads like a swallowed error. Mitigation: documented in the rustdoc and
  pinned by AC9's two fixtures. `[measured: probe → dead_end mapped = None,
  crash_pocket mapped = None; both are box-filling corridors]`
- **`phase5b.rs` grows further past the incl.-tests soft limit** (800).
  `[measured: wc -l → 958 today]` Mitigation: subtask 1 moves ~50 lines of
  fixtures out, and the AC7 battery lands in `phase6.rs`, so the net is roughly
  +80 → ≈ 1040, well under the 1000/**1500** hard limit for a file with a
  `#[cfg(test)]` block. No new violation class is introduced.
  `[derived → subtask 9 wc check]`
- **A `-D warnings` gate aborts on the first failure**, masking later sites.
  Mitigation: subtask 9 re-runs clippy after the first clean pass; a newly
  revealed out-of-contract class is surfaced to the orchestrator, not absorbed.
  `[derived → AC13]`
- **No new dependency, no Miri gate work.** `gp-gen`'s deps are unchanged
  `[measured: crates/gen/Cargo.toml → gp-core, rand, rand_xoshiro, strum]`, and
  `gp-gen` is the sanctioned crate-level Miri exclude `[measured:
  .github/workflows/ci.yml:193 → cargo miri test --workspace --exclude gp-gen]`,
  so no `#[cfg_attr(miri, ignore)]` is needed on any new test.

---

## Test Design

All tests are unit tests in `#[cfg(test)] mod tests` (fixtures in
`testfix.rs`, which is `#[cfg(test)]`-gated and crate-private — an integration
test in `crates/gen/tests/` **cannot** reach them, so no integration test is
proposed). No `rstest`/`mockall`/`pretty_assertions` need is identified; the
existing modules use plain `assert!`/`assert_eq!` and these follow.

### `crates/gen/src/phase6.rs` — AC7 proof gate (subtask 2)

- Entry points: `gp_gen::oracle_liveness_v1`, `gp_gen::phase5_full_oracle`.
- `trap_ring_is_v1_lappable_and_has_an_unbrakeable_hazard` — asserts (a)
  `oracle_liveness_v1 == true`; (b) with `v_ceil = 2`, the named witness
  `car(6, 5, 0, 2)` is in `forward_reachable` and **not** in
  `backward_reachable(lap_close_goals(...))`, and `R \ B` is non-empty.
  Non-vacuity: assert `r.contains(&witness)` *before* asserting `!b.contains`.
- `ac7_v1_liveness_is_equivalent_to_full_oracle_lappability` — the biconditional
  over `[ring, broken ring, dead_end, long_straight, crash_pocket, trap_ring]`,
  each case named in the assertion message so a failure identifies the
  falsifying fixture (which Branch B then adopts as a regression test).

### `crates/gen/src/phase5b.rs` — diagnostic (subtask 4)

- Entry points: `phase5_full_oracle`, `p0_boundary_walls`.
- **AC1** `ac1_broken_ring_diagnostic_implicates_the_severed_region` — from the
  `NotLappable` payload: `stall_walls.iter().filter_map(wall_neighbor)` contains
  `(4, 2)`; and `!stall_walls.iter().any(|w| w.cell == Point::new(2, 0))`.
  Both directions matter: the first is R1, the second is the retired
  expectation.
- **AC2** `…_diagnostic_is_non_empty_on_the_broken_ring` (tier 1),
  `…_diagnostic_falls_back_to_boundary_walls_when_p0_is_empty`
  (`no_crossing_corridor`; also asserts `P0 == ∅` so the test is non-vacuous
  about *which* tier fired), `…_diagnostic_is_sorted_and_deterministic` (two
  runs equal; equal to its own sorted copy).
- **AC2 precondition** `…_diagnostic_is_empty_only_outside_the_d_non_empty_precondition`
  — on a degenerate `Corridor::new(_, 0, 0)` the payload is `[]`, pinned as the
  **documented** out-of-precondition outcome (§ Approach (1) → *edge case*).
  The test's name and its comment must both say that this is the precondition
  boundary, not an AC2 violation — otherwise a future reader reads an empty
  diagnostic as a bug and "fixes" it with a dishonest tier 3.
- Pure-helper tests replacing the three `frontier_gap_*`:
  `p0_boundary_walls_lists_one_wall_per_off_d_side`,
  `p0_boundary_walls_is_empty_when_every_p0_cell_is_interior`,
  `p0_boundary_walls_is_empty_when_p0_is_empty` — hand-built `Corridor` +
  `HashSet<Point>`, no oracle involved.
- `oracle_result_variants_are_constructible_and_clonable` — updated to the
  `stall_walls: Vec<Wall>` payload; keeps the existing "no `PartialEq` on
  `OracleResult`" field-wise comparison note.

### `crates/gen/src/phase6.rs` — mapper (subtasks 6–7)

- Entry point: `map_frontier_gap_to_edge`; fixture: broken ring
  (`ring_corridor()` with `(4, 2)` cleared), diagnostic obtained from a real
  `phase5_full_oracle` call — **not** hand-built, so the test covers the
  producer/consumer contract end to end.
- **AC3/AC4** `maps_a_v1_sever_to_a_boundary_edge` — result is
  `RepairCandidate::Edge(Wall { cell: Point::new(4, 1), side: Side::North })`;
  separately assert `d.contains(w.cell)` and `!d.contains(wall_neighbor(w))`.
- **AC5** `returned_edit_strictly_grows_p0` — `p0_at_v1` before, scratch-clone
  `D`, `set(neighbour, true)`, `p0_at_v1` after; `assert!(after > before)` plus
  the measured values `3 → 16` as a comment, not a hard-coded assertion.
- **Selection non-vacuity** (subtask 6, recommendation from `design-review`)
  `max_growth_selects_the_severed_edge_over_a_lesser_candidate` — on the broken
  ring, `Wall { cell: (3,0), side: North }`'s off-`D` neighbour `(3, 1)` is
  **in-box and non-drivable**, so it passes the mapper's admissibility filter
  and is a second candidate; `design-review` reports it also *grows* `P0`, which
  is what makes `max grown` (rather than the `wall_sort_key` tie-break, which
  would pick `(3,0) < (4,1)`) the rule that selects `(4,1)N`. The test measures
  **both** growths and asserts `growth((3,1)) < growth((4,2))` — a form that
  holds whether `(3,1)` grows a little or not at all, so it cannot become a
  false red, while pinning that a single-candidate reading of this fixture is
  wrong. `[derived → subtask 6 measures both growths; the design-phase probe
  measured only the winner (`grown = 16`, `base = 3`) and did not print
  per-candidate growth, so the "second growing candidate" claim is
  `design-review`'s and is discharged by this test's own run]`
- **AC6** `returned_edit_closes_the_lap` — `phase5_full_oracle` is `NotLappable`
  before and `Lappable` after (both directions asserted, so the test cannot pass
  vacuously on an already-lappable fixture).
- **AC9** `no_candidate_when_no_boundary_edge_grows_p0` — `dead_end_corridor`
  and `crash_pocket_fixture`, both `== RepairCandidate::NoCandidate`; assert the
  diagnostic handed in was **non-empty** first, so "no candidate" is a real
  decision rather than an empty input.
- **AC10** `is_total_on_adversarial_input` — empty slice; walls at
  `(9999, 9999)`, `(i32::MAX, i32::MAX)` (East, so `checked_add` overflows),
  `(i32::MIN, i32::MIN)` (West); `Corridor::new(origin, 0, 0)`. Each returns
  `NoCandidate` without panic or overflow.
- **AC11** `is_deterministic_and_input_order_independent` — two identical calls
  agree; a reversed input slice yields the same outcome.
- Helper needed: none beyond `p0_at_v1` and the `testfix.rs` fixtures.

### AC13

Workspace gates as listed in subtask 9. Expected baseline to beat:
`[measured: cargo test -p gp-gen → 138 passed]` today; the workspace total
grows by the new tests.

---

## Open questions (resolved here; none left for the product owner)

- **Does one edge ever suffice in general?** On the one-edge-repairable fixture,
  yes and it is asserted (AC6). In general the honest answer is "one edge per
  iteration, N iterations"; recorded in the AC12 rustdoc as the `[N3]` risk
  rather than papered over, per the spec's instruction.
- **Should `Issue` gain `DynamicallyDisconnected`?** No — see § Approach (6).
- **Tie-breaking among equally-good candidates.** Max `|P0_after|`, ties broken
  by `wall_sort_key` ascending (min `Point`, then `Side` declaration order
  E/W/N/S). The "prefer the edge nearest the medial axis" quality refinement is
  **not** adopted: it needs a `DistanceTransform` per candidate for a benefit
  this spike has no evidence for. Left as a note in the rustdoc.
