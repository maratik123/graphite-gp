# Design: gp-core — finalize the TrackArtifact contract (s_field + start grid + gate segment + centerline sampling)

**Issue:** #6
**Date:** 2026-07-16

## Approach

The whole task lands in **one file, `crates/core/src/track.rs`** (currently 95 lines).
It adds the two missing artifact members (`s_field`, `start_grid`), a dedicated
timing-gate type on `StartFinish`, and implements `Centerline::at`. `sim.rs` is
**not** touched: `LapCounter::register_move` already takes `&StartFinish` and stays
`todo!` — this task only makes `StartFinish` *carry* the gate data that logic will
later read (AC6, spec Out-of-scope). No new module/file is created (see § Module
split); no new dependency (`enumflags2` already present, verified; and no BFS is run
here — see below).

### Key decision — the BFS is *not* run in this task

Spec scope item 3 describes the s-field scalar as "the BFS distance on `D \ gate`
seeded from the gate's forward face" — that is its **semantics**, not work performed
here. Populating the field (the generator, Ф7 `phase7_output`) is explicitly
out-of-scope. This task ships the **type + read accessors**, exercised on hand-filled
distances. So the #5 `geom` graph helpers (`CorridorScratch::geodesic_bfs`,
`geodesic_layers`) are **not** consumed now — the generator will reuse them later to
populate `SField` from `D \ gate`. `SField::new` therefore takes a caller-supplied
per-cell distance lookup (a hand-filled closure in tests today; the BFS result later).

### Resolved *Key decisions* (spec table)

| Question | Decision + rationale |
|---|---|
| Scalar / gradient numeric type | `s` is `u32` (BFS hop distance `0..=L`); `gradient_at` returns `(i32, i32)` integer neighbor differences. Matches design §2 N1 ("монотонно 0..L"). |
| Unit-direction numeric type | `(f32, f32)`, mirroring the existing `CenterlineSample.tangent`. `track.rs` is the render/AI-facing artifact module — the §3a integer-only determinism axiom binds only `sim`/`geom`, which this task does **not** touch. |
| Accessor shape | Three methods on `SField`: `scalar_at(p) -> Option<u32>` (AC2 scalar), `gradient_at(gate, p) -> Option<(i32,i32)>` (raw integer ∇s — observable for the fixture assertion, decision #3), `tangent_at(gate, p) -> Option<(f32,f32)>` (unit tangent for every band cell, AC2/AC3). `None` when `p ∉ band`. |
| s-field storage | Dense grid mirroring `Corridor`: `SField { rect: Rect, dist: Vec<Option<u32>> }`, row-major over the corridor's bounding box; `None` = ¬band. `Rect`/`Size` are pub-constructible, so `SField::new` rebuilds the box from `Corridor::{origin,width,height}()` — **no new `geom` API**, `track.rs` is the only file changed. |
| Gate representation | Dedicated `TimingGate { behind: Vec<Point>, forward: Side }`. `behind` = the drivable cross-section cells immediately behind the gate; the dual edges are implicitly `{cell: behind[i], side: forward}` — "one dual edge ahead of the front row spanning the cross-section" (§2 Ф3 [C2]). This is the "Vec of half-grid dual-edge unit segments" option, compacted (forward is uniform, stored once). **Doc-string requirement (item 3):** the `TimingGate` `///` states that `behind` holds the *drivable* cross-section cells **behind** the gate and each implied dual edge is `{cell, forward}` — so a future reader of `register_move` does **not** mistake it for a `Wall` (`D ↔ ¬D` boundary) set: a gate edge sits between two *drivable* cells, not on `D`'s boundary — distinct semantics. A `separates(a, b) -> bool` method exposes the cut as a barrier for the gradient; **it MUST be order-independent / symmetric: `separates(a, b) == separates(b, a)` for all `a, b`** — stated explicitly on `separates`'s `///`, because `tangent_at`/`gradient_at` walk 4-neighbors and may query the *reversed* stored pair (e.g. for gate cell `behind[i]` at `(1,1)` with `forward = East`, the reversed pair `separates((2,1),(1,1))` must also report the cut, not just the forward pair `separates((1,1),(2,1))`). An order-sensitive impl would silently drop the barrier for one traversal direction and leak the cross-cut jump into the gradient. `behind` gives the crossing test its line-coordinate + span, `forward` its sign — sufficient for `register_move`'s half-open test (AC6). |
| Forward-direction source | **Stored explicitly** as `forward: Side` on `TimingGate`. `orient` + `race_dir` alone are insufficient: whether `+race_dir` means `+x` or `−x` at the gate depends on the gate's winding position on the loop (a CW ring's bottom straight is `+x`, its top straight `−x`). Storing it is O(1) and robust; `forward` doubles as the crossing axis+sign and the gate-cell tangent (AC3, AC6). |
| Start-grid element type | `Point` (newtype `StartGrid { positions: Vec<Point> }`). `v = (0,0)` is a documented invariant (all start states at rest, §3), so `CarState` would store a redundant zero velocity on every element. The sim trivially lifts a `Point` to `CarState { x, y, vx: 0, vy: 0 }`. |
| Band-boundary gradient scheme | `∇s(p) = Σ over in-band, non-cut 4-neighbors q of (s(q) − s(p))·(q − p)`. Per axis this **unifies** central difference (both neighbors present: `s(E) − s(W)`) and one-sided difference (one neighbor ¬band: `s(E) − s(p)`) — no special casing. Gate cells (adjacent to a `separates` cut) short-circuit to `forward` (AC3). Degenerate `∇s = (0,0)` (a flat/saddle — shaping-only per §2 P1) → documented module-const fallback `(1.0, 0.0)`. Every band cell thus gets a defined unit tangent. |
| Module placement | **Single `track.rs`** (estimate ≈ 350 lines excl. tests, ≈ 600 incl. — under the soft 500/800). Counter-rule "don't over-split" applies. Contingency if the soft limit is crossed during implementation: split into `track/mod.rs` (RaceDir, StartFinish, TimingGate, StartGrid, TrackArtifact, TrackMetrics), `track/sfield.rs` (SField + accessors), `track/centerline.rs` (Centerline). |

### Tangent / gradient mechanics (AC2/AC3 detail)

`tangent_at(gate, p)`:
1. `p ∉ band` → `None`.
2. `p` is a **gate cell** (some 4-neighbor `q` has `gate.separates(p, q)`) → `Some(forward_unit)` — exact forward `race_dir`, the cut not differenced across (AC3).
3. else compute `g = ∇s`; if `g == (0,0)` → `FLAT_FALLBACK`; else `normalize(g)`.

Because the gradient skips `separates` neighbors, only gate cells ever touch the cut, so non-gate cells never difference across it. `gradient_at` at a behind-gate cell (`s ≈ L`) drops its forward (cut) neighbor and differences only backward (`s ≈ L−1`) → a small `+forward` vector, **never** the spurious `L→0` jump — the AC3 assertion checks exactly this.

### `Centerline::at(s)` (AC4)

`n = samples.len()`. `n == 0` → `None`. `n == 1 || !(length > 0.0)` (guards div-by-zero / NaN) → `Some(samples[0])`. Else `sw = s.rem_euclid(length)` (wraps: `at(length) ≡ at(0)`, `at(length+x) ≡ at(x)`, negatives wrap positively); bracket by `rposition(|c| c.s <= sw)`, the last sample closing back to `samples[0]` with `s += length`; component-wise linear interpolation of `pos` and `tangent`. Samples already carry `race_dir`-oriented tangents (documented on `CenterlineSample`), so the blend is oriented along `race_dir` by construction — no re-orientation. Plain lerp (no re-normalization) is chosen to honor AC4's literal "linearly interpolates … tangent"; the doc notes the tangent is unit at sample points and a near-unit blend between them (dense render samples).

**Totality / precondition (item 2 — no panic path).** `rposition(|c| c.s <= sw)` yields `None` only when *no* sample sits at/below `sw`, i.e. `samples[0].s > sw ≥ 0`, which requires `samples[0].s > 0`. `samples[0].s == 0` is a genuine **closed-loop centerline invariant** (the arc-length resample seeds the first sample at `s = 0`); it is documented as a **precondition on `at`'s `///`**. To keep the accessor **total** regardless (AGENTS.md: no unjustified `unwrap`/panic in prod), the bracket lower index is taken with `rposition(|c| c.s <= sw).unwrap_or(n - 1)` — the `None` arm folds to `lo = n - 1`, placing `sw` on the **closing bracket `[samples[last], samples[first]]`** (with `first` taken as `s += length`), the same wrap segment such a below-`samples[0].s` `sw` conceptually lies on. Under the documented invariant the `unwrap_or` fallback is never taken; it exists solely so a future generator emitting a non-zero first `s` degrades to a defined interpolated value instead of panicking.

## Decomposition

All subtasks edit only `crates/core/src/track.rs` (change-type: **code**, `*.rs`). TDD:
each type's `#[cfg(test)]` tests are written with it; the `code-writer` runs
fmt/clippy(`-D warnings`)/doc/test gates per subtask.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `TimingGate { behind, forward }` (+ `forward_unit`, `separates`, private `side_unit_f32` helper); finalize `StartFinish` — remove the `TODO(1)` comment, add `pub gate: TimingGate`, refresh docs. Add `Rect, Size, Side` to the `geom` import. Tests: `forward_unit` per side; `separates` true/false pairs **incl. a dedicated symmetry assertion (reversed gate pair → true)**. | `crates/core/src/track.rs` | — |
| 2 | Add `StartGrid { positions: Vec<Point> }` (derive `…, Default`); document the distinct + in-`D` + front-to-back-along-`−race_dir` invariant. Test: fixture ordering + distinctness. | `crates/core/src/track.rs` | — |
| 3 | Add `SField { rect, dist }` + `new`, `scalar_at`, `gradient_at`, `tangent_at`, private `is_gate_cell`, `normalize`, `FLAT_FALLBACK`. Tests: AC2 (scalar monotone `0..=L`; gradient direction `(+,0)`; tangent `(1,0)` on `+x` fixture), AC3 (gate tangent = forward; gradient not differenced across the cut). | `crates/core/src/track.rs` | 1 |
| 4 | Implement `Centerline::at` (remove `todo!` + `TODO(1)`; private `lerp` helper). Tests: AC4 (empty→`None`; single-sample/`length=0` guards; `at(length)≡at(0)`; `at(length+x)≡at(x)`; interior + closing-segment interpolation of `pos`/`tangent`). | `crates/core/src/track.rs` | — |
| 5 | Extend `TrackArtifact` with `pub s_field: SField` and `pub start_grid: StartGrid`; refresh the struct doc to enumerate all 8 members (AC1). Final full-gate pass (fmt / clippy `-D warnings` / `RUSTDOCFLAGS=-D warnings` doc / test) green (AC7). | `crates/core/src/track.rs` | 2, 3, 4 |

Scope: 5 subtasks (< 15). No split into multiple issues.

## Handoff plan

Per § Rules → handoff-grouping. **(a)** Grouping is required for every `M ≥ 1`; here
`M = 5`. **(e) Change-type homogeneity:** all 5 subtasks change **code** (`*.rs`,
`crates/core/src/track.rs` only) — one homogeneous group, no instructions/harness
subtasks, so no forced change-type boundary. **(f) Group-minimization:** the whole
dependency chain (1→3, {2,4} free, 5→{2,3,4}) is one change-type, so it collapses to
the **fewest possible = 1 group**; a valid execution order is 1, 2, 3, 4, 5.
**(b) Size cap:** 5 ≤ 10. **(d) Terminal sizing:** the sole (terminal) group holds 5,
within `1..=10`. **(h) Max groups:** 1 ≤ 4 (no user gate needed).

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent (its `model: sonnet` + `effort: medium` are frontmatter-pinned;
  no inline override), 1M-token window — subtasks 1–5 (code change-type: `*.rs`).
  **Terminal group** (5 subtasks; within `1..=10`).
- **(c) Handoff into Group A:** at the start of Group A, spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Being the
  single, terminal group, it completes Step 8 in its own `/context-reset` subagent;
  there is no later group and thus no inter-group handoff.

The `design`, `design-review`, `self-review`, and `spec-writer` subagents stay on Opus;
only the per-group implementor (here `code-writer`/sonnet/medium) varies.

## Risks

- **`pedantic` + `nursery` clippy are `-D` deny** (verified in workspace `Cargo.toml`),
  so `as` casts (`cast_precision_loss`, `cast_possible_wrap`, `cast_possible_truncation`)
  and raw integer arithmetic (`arithmetic_side_effects`) all fire. Mitigations, matching
  house style (`geom`'s documented domain-bounded allows):
  - Side→`(f32,f32)` via a literal `match` (`side_unit_f32`) — **no cast at all**.
  - The only `i32→f32` cast is in `normalize`; guard with one documented
    `#[allow(clippy::cast_precision_loss, reason = "…")]` (gradient components are `±2`
    in a valid BFS field — exact in `f32`).
  - `gradient_at` uses `u32→i32` via `i32::try_from(..).unwrap_or(i32::MAX)` (not `as`)
    and one documented `#[allow(clippy::arithmetic_side_effects, reason = "…")]` for the
    bounded accumulation; point steps use `checked_add` (like `walls_from_boundary`).
  - `Centerline::at`'s index math is `usize::checked_add`; all interpolation is `f32`
    (floats are not flagged by `arithmetic_side_effects`).
- **`normalize` div-by-zero:** only reachable with a non-zero gradient — gate cells and
  the `∇s == (0,0)` fallback both return *before* calling it. Documented precondition.
- **Differencing across the gate cut (correctness, AC3/N1):** `separates` acts as the
  gradient barrier; gate cells short-circuit to `forward`. Directly tested (a huge
  `L→0` fixture jump must not leak into the gradient).
- **`rem_euclid` on `length ≤ 0` / NaN:** guarded by the `n == 1 || !(length > 0.0)`
  early return (returns `samples[0]`).
- **`Centerline::at` `rposition` returns `None` (latent panic, item 2):** reachable only
  if the `samples[0].s == 0` closed-loop invariant is violated (a future generator emits a
  non-zero first `s`). Mitigated by folding the `None` arm to `lo = n - 1` via
  `unwrap_or(n - 1)` (closing bracket `[last, first]`), keeping `at` total; the invariant is
  documented as a precondition on `at`'s `///`. Directly tested (invariant-violated fixture
  returns a defined value, not a panic).
- **File soft-size limit (500 excl. / 800 incl.):** estimate under; contingency split
  plan recorded in § Module split if implementation crosses it.
- **`forward` underivable from `orient`+`race_dir` alone:** mitigated by storing it
  explicitly on `TimingGate`.
- **`chord`/`orient` retained on `StartFinish`:** no external field access exists
  (verified: only `sim.rs` uses `&StartFinish` as a param, no field reads), so adding
  `gate` beside them is non-breaking; they keep their S/F-highlight role.

## Test Design

All in `crates/core/src/track.rs` `#[cfg(test)] mod tests`. Fixtures use small
hand-built corridors + distance closures (mirroring the `geom` test-helper style).

- **`TimingGate`** (task 1) — entry: `forward_unit`, `separates`.
  - `forward_unit` returns `(1,0)/(−1,0)/(0,1)/(0,−1)` for East/West/North/South.
  - `separates(behind, behind+forward)` → true; a non-adjacent pair, a lateral pair,
    and a reversed *non-gate* pair → false.
  - **Symmetry (item 1):** with gate `behind: [(1,1)], forward: East`, assert **both**
    the forward gate pair `separates((1,1),(2,1))` **and** the reversed gate pair
    `separates((2,1),(1,1))` → **true** — a dedicated symmetry assertion at `separates`'s
    own boundary, so an asymmetric impl fails here rather than only transitively in
    task-3's AC3 gradient test.
- **`StartGrid`** (task 2) — entry: constructed `positions`.
  - A 3-element front-to-back fixture (e.g. forward = East → decreasing `x`): assert the
    `Vec` order equals the expected front→back list, and all elements are distinct.
- **`SField`** (task 3) — entries: `scalar_at`, `gradient_at`, `tangent_at`.
  - *AC2*: corridor cells `(0..4, 1)` with `s(x,1) = x`, empty gate
    (`behind: []`, `forward: East`). `scalar_at` returns `0..=3` monotone;
    `scalar_at` off-band → `None`; `gradient_at((1,1)) == (2,0)` (central) and
    `((0,1)) == (1,0)` (one-sided at the ¬band west edge); `tangent_at((1,1)) == (1.0,0.0)`.
  - *AC3*: cells `(0..4, 1)`, gate `behind: [(1,1)]`, `forward: East`; hand-filled
    `s = {0:3, 1:4, 2:0, 3:1, 4:2}` (a wrap at the cut, `L=4`). `tangent_at((1,1))` and
    `tangent_at((2,1))` (both gate cells) `== (1.0,0.0)`; `gradient_at((1,1)) == (1,0)`
    (backward-only diff `+forward`, **not** the cross-cut `(0−4)` term) — proves the cut
    is a barrier.
  - Fallback: an all-equal-`s` interior fixture → `tangent_at` returns `FLAT_FALLBACK`.
- **`Centerline::at`** (task 4) — entry: `at`.
  - Empty samples → `None`; single sample & `length = 0.0` → that sample.
  - Fixture: samples `s=0 pos(0,0) tan(1,0)`, `s=1 pos(1,0) tan(1,0)`,
    `s=2 pos(1,1) tan(0,1)`, `length = 4`. Assert `at(0) == sample0`;
    `at(4) == at(0)`; `at(4.5) == at(0.5)`; interior `at(1.5)` → `pos(1,0.5) tan(0.5,0.5)`;
    closing-segment `at(3)` → `pos(0.5,0.5) tan(0.5,0.5)`. f32 compares use exact halves
    (representable) or a small epsilon.
  - **Totality (item 2):** a fixture *violating* the `samples[0].s == 0` precondition
    (e.g. samples at `s = 0.5, 1.5, 2.5`, `length = 4`) with `at(0.2)` (`sw < samples[0].s`)
    returns a **defined** value via the closing-bracket `[last, first]` interpolation —
    **not** a panic — exercising the `unwrap_or(n - 1)` fold that upholds AGENTS.md's
    no-unjustified-panic rule.
- **`TrackArtifact`** (task 5, AC1) — a hand-built artifact with all 8 members
  constructs and the workspace builds; a smoke test reads `s_field`/`start_grid` back.

## Open questions

None blocking. The two sub-decisions the spec left to design (tangent re-normalization
in `at`; the `∇s = 0` fallback value) are resolved above (plain lerp; module-const
`(1.0, 0.0)`), both defensible under the stated ACs and design §2 P1.
