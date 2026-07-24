# Design: gp-gen Ф7 — render-only racing centerline

**Issue:** [#33](https://github.com/maratik123/graphite-gp/issues/33)
**Date:** 2026-07-24

## Approach

Implement the `racing_line` producer (design doc `§2` line 191,
`centerline = racing_line(medial_axis(D))`) as a **new `gp-gen` module
`phase7.rs`**, consuming the existing `gp-core` primitives and populating the
existing `gp_core::track::Centerline`. Nothing new is added to `gp-core`; the
whole algorithm lives in `gp-gen`, matching the spec Key-decision
("`racing_line` lives in `gp-gen`").

**Public entry point (signature deviates from the pseudo-code, intentionally):**

```rust
pub fn racing_line(d: &Corridor, gate: &TimingGate, race_dir: RaceDir) -> Centerline
```

The design-doc pseudo-code writes `racing_line(medial_axis(D))`, but the producer
needs `gate` (the `s = 0` anchor, Key-decision) and `race_dir` (tangent
orientation, AC3) in addition to the medial cells, and it computes
`DistanceTransform::compute(d)` + `medial_axis(&dt)` internally (both are
`pub` — [measured: `rg -n 'pub use distance::\*' crates/core/src/geom/mod.rs` →
`18:pub use distance::*;`; `medial_axis` returns `BTreeSet<Point>` per
`crates/core/src/geom/distance.rs:112`]). Passing `d` directly keeps the seam
single-call and lets the fallback own the whole pipeline.

### The loop-trim / resample algorithm (chosen)

The medial axis of a generated ring is a *thin* (1–2 cell) cell set that is
almost the loop but not quite: `medial_axis` deliberately leaves **(i)** 2-cell
bands on even cross-sections unthinned, **(ii)** a diagonal gap at each
rectilinear corner, and **(iii)** spur branches on infield-finger / hairpin
tracks (all three documented as `racing_line`'s job —
[measured: `crates/core/src/geom/distance.rs:108-111`, `:228-232`, `:266-272`]).
The pipeline turns that set into one closed, arc-length-parameterised loop:

1. **Medial set.** `dt = DistanceTransform::compute(d)`;
   `m: BTreeSet<Point> = medial_axis(&dt)`. Empty ⇒ fallback (empty `Centerline`).
2. **Bridge diagonal / corner gaps (`bridge_gaps` + `bridge_path`, leaf-cell
   tracking).** Each round, collect the degree-`< 2` **"leaf"** cells of the
   4-connected graph of `m`; find the minimal-Manhattan-distance leaf pair
   `(a, b)` that is **not already 4-adjacent** (deterministic tie-break by
   `(a, b)` `Point`-`Ord`, `a <= b`), **preferring a cross-component pair over a
   same-component pair when both exist**; insert every `bridge_path(d, a, b)`
   cell that lies in `d` into `m`. `bridge_path(d, a, b)` is a **minimal
   4-connected rectilinear path** between the two cells (`manhattan(a, b) + 1`
   cells), trying both the `x`-then-`y` and `y`-then-`x` axis orders and
   preferring whichever lies **entirely within `d`** (`x`-then-`y` wins a tie) —
   a single fixed order can route through a `¬D` cell (e.g. the annulus's own
   centre hole) that the other order avoids. A minimal single-width path adds
   only **one** new neighbour to each interior cell, so — unlike `supercover`
   over a thick corridor (see § Rejected alternatives) — it **cannot** introduce
   a degree-3/4 junction that would defeat `prune_spurs` or dead-end
   `walk_cycle`.
   **Stopping conditions:** stop when fewer than 2 leaves remain, when no viable
   (non-4-adjacent) leaf pair remains, or when a chosen bridge inserts no new
   cell (stall guard for genuinely-open, non-ring inputs). Guard: if the minimal
   candidate gap ever exceeds `MAX_BRIDGE_GAP` (a module const, Manhattan units),
   abandon bridging ⇒ fallback. Empty `m` ⇒ fallback.
   **Why leaf tracking rather than "while >1 component":** a stop-at-one-component
   rule stops one gap too early on a single ring — three corner bridges can
   transitively merge all four strips into ONE component while the fourth corner
   gap's two leaf cells remain unconnected, so a component-count test declares the
   ring closed before it is, and `prune_spurs` then peels the two dangling ends.
   Tracking leaves (and only *preferring*, not *requiring*, cross-component pairs)
   closes that last gap. This closes the ring using only 4-connectivity; no
   8-neighbour helper is introduced ([measured: `rg -n 'neighbors8'
   crates/core/src/` → no matches — none exists to reuse].)
3. **Prune spurs.** Iterative degree-1 removal on the 4-connected graph of `m`
   until every surviving cell has degree ≥ 2 → core set `c`. A clean 1-wide loop
   is already all-degree-2 (no-op); infield fingers / hairpin tails are trees
   hanging off the ring and peel away. If `c` becomes empty ⇒ fallback.
4. **Order into a cycle (straightest-continuation walk).** Anchor `start` =
   the `c` cell of minimal Manhattan distance to the centroid of
   `gate.forward_face()` (min-`Point` tie-break) — this puts `s = 0` at the gate
   (Key-decision; `forward_face` exists — [measured:
   `crates/core/src/track.rs:70`]). Walk 4-connected: at each step pick the
   not-yet-visited neighbour in `c` that best continues the current heading
   (minimum turn; deterministic min-`Point` tie-break), closing when the next
   neighbour is `start`. Straightest-continuation traces a **single strand**
   through a 2-cell band (it prefers going straight along one rail over turning
   onto a rung), which is the band-thinning step. If the walk dead-ends or fails
   to return to `start` ⇒ fallback.
5. **Orient to `race_dir`.** Compute the ordered ring's integer shoelace signed
   area (i64) → winding sense (this grid is **x-east / y-north right-handed**, so
   the standard math convention holds: shoelace `> 0 ⇔ Ccw`, `< 0 ⇔ Cw` —
   [measured: `crates/core/src/geom/mod.rs:30-36` — "increasing eastward" /
   "increasing northward"]); if it disagrees with `race_dir`'s expected
   winding, reverse the ring. Decoupling walk-direction from `race_dir` via a
   post-hoc reversal is simpler and more robust than steering the walk, and
   makes AC3 a one-line consequence: forward finite differences then point along
   `race_dir` by construction.
6. **Resample by arc length.** Positions = cell centres `(p.x as f32, p.y as
   f32)`. Walk the closed polyline, emitting a `CenterlineSample` every
   `RESAMPLE_STEP` (`= 1.0`, ≈ one cell) of accumulated arc length, seeding
   `samples[0].s == 0`, `s` strictly increasing, `length` = total perimeter
   (positive, finite). Closed: last sample is one step before wrapping to
   `at(length) ≡ at(0)`.
7. **Tangents.** Per resampled sample, `tangent = normalize(next.pos −
   prev.pos)` with ring wraparound → unit vector; `race_dir`-aligned by step 5.
8. Return `Centerline { samples, length }`.

Every fallback yields `Centerline::default()` (empty `samples`, `length` 0.0);
`Centerline::at` already returns `None`/degrades gracefully on that
([measured: `crates/core/src/track.rs:303-310`]) — **never panics** (spec
Key-decision + `gp-core` zero-panic invariant).

**Determinism (AC6).** `medial_axis` returns a `BTreeSet` for stable iteration;
every subsequent choice (bridge pair, walk start, walk tie-break, reversal) is a
total order over `Point`/`(Point, Point)` — so identical `(d, gate, race_dir)`
⇒ byte-identical `Centerline` [derived → AC6 determinism test].

### AC5 enforcement mechanism (chosen)

A **source-scan guard test** in `phase7.rs`'s `#[cfg(test)]` module: recursively
read every `.rs` file under `concat!(env!("CARGO_MANIFEST_DIR"), "/../ai/src")`
and assert none contains the identifier `Centerline` (the render type) or the
field access `.centerline`. gp-ai references `TrackArtifact` (allowed) but not
the centerline type/field today — the doc-comment prose says lowercase
"centerline"/"centerline-frame", which the case-sensitive `Centerline` /
`.centerline` scan does not match [measured: `rg -Un 'Centerline|\.centerline'
crates/ai/src/` → NO MATCHES]. The test lives in `gp-gen` (not `gp-ai`)
deliberately: `gp-gen` is `--exclude`d from the Miri gate ([measured:
`.github/workflows/*.yml:192-193` → `cargo miri test --workspace --exclude
gp-gen`]), so the file-read incurs no Miri-isolation abort and needs no
`#[cfg_attr(miri, ignore)]`; the same test in `gp-ai` (Miri-included) would.

### Rejected alternatives

- **`supercover(a, b)` to bridge corner gaps (design's original choice —
  rejected in implementation).** `supercover`'s closed-square touch test is
  exactly right for a moving car's chord (design doc §3 C4), but over a corridor
  **thicker** than one cell (the realistic case — generated tracks are `>= n`
  cells wide, design doc §1) a diagonal corner gap's `supercover(a, b)` touches a
  small *blob* of cells, not a thin path, and that blob can reconnect the medial
  ridge at **more than one point** — creating degree-3/4 junctions that
  `prune_spurs` (degree-`< 2`-only) cannot remove and that dead-end
  `walk_cycle`'s non-backtracking search. Empirically it failed this design's own
  annulus reference fixture: `racing_line` produced **zero** samples and 12 cells
  around the ring's 4 corners had degree 3/4 [measured: module AC7 annulus test
  during subtask-2 implementation]. Replaced by `bridge_path` (§ algorithm step
  2), a minimal single-width rectilinear path that adds only one neighbour per
  interior cell, so it keeps the loop 4-connected and bridges gaps of any bounded
  size **without** introducing branching. (`supercover` is `pub` — [measured:
  `crates/core/src/geom/mod.rs:396`] — so it was reusable; it is the *shape* of
  its output on a thick corridor, not its availability, that disqualifies it.)
- **8-connectivity graph to bridge corner gaps.** The annulus fixture's corner
  gap is 2 diagonal cells (strip endpoints `(3,1)`↔`(1,3)`, Chebyshev 2 /
  Manhattan 4 — [measured: `crates/core/src/geom/distance.rs:242-250` expected
  set]), so Chebyshev-1 (8-conn) does **not** bridge it. A `bridge_path`
  rectilinear route between the nearest leaf endpoints bridges gaps of any
  bounded size and keeps the loop 4-connected, so the walk stays 4-conn and no
  `neighbors8` helper is needed.
- **Angle-sort around the centroid.** Robust to gaps/bands without adjacency,
  but inward-pointing spurs land mid-sector and inject radial in/out jags — it
  cannot satisfy AC1's "spurs pruned". Rejected in favour of graph pruning.
- **Morphological (Zhang-Suen) thinning of 2-cell bands.** A full skeletoniser
  is far more code than the render-only fallback-tolerant scope warrants;
  straightest-continuation thins the band as a side effect of the single walk.
- **Moore boundary tracing.** Degenerates on a ~1-cell-wide skeleton (double-
  covers each cell) — wrong tool for a thin input.
- **AC5 as a documented-invariant only (no test).** A negative that no gate
  discharges; a source-scan makes it executable. Rejected.

## Decomposition

All subtasks touch only Rust (`crates/gen/src/phase7.rs`, `crates/gen/src/lib.rs`,
and — for the shared test fixtures, GO-note 1 below — `crates/gen/src/testfix.rs`)
— a single **code** change-type. Each helper subtask carries its own
`#[cfg(test)]` unit tests (TDD, per AGENTS.md).

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Scaffold `phase7.rs`: `mod phase7;` + `pub use phase7::*;` in `lib.rs`; module consts `MAX_BRIDGE_GAP`, `RESAMPLE_STEP` (`SCREAMING_SNAKE_CASE`); `pub fn racing_line(d, gate, race_dir) -> Centerline` returning `Centerline::default()`; empty-`D`/empty-medial fallback test. | `crates/gen/src/phase7.rs`, `crates/gen/src/lib.rs` | — |
| 2 | Bridge helpers: `components` (4-conn components of `m`); `bridge_path` (minimal 4-conn rectilinear path, both axis orders, prefer entirely-in-`d`); `bridge_gaps` (per-round leaf tracking: minimal-Manhattan non-4-adjacent leaf pair, prefer cross-component, insert `bridge_path` ∩ `d`; stop on <2 leaves / no viable pair / stall; `MAX_BRIDGE_GAP` guard → fallback signal). Unit tests: annulus corner gaps bridged into one closed ring (no dangling leaves); over-`MAX_BRIDGE_GAP` gap ⇒ fallback. | `crates/gen/src/phase7.rs` | 1 |
| 3 | Prune helper: iterative degree-1 removal (4-conn) → core `c`; all-degree-2 loop is a no-op; empty-core ⇒ fallback signal. Unit tests: infield-finger fixture → spur removed, loop survives all-degree-2. | `crates/gen/src/phase7.rs` | 1 |
| 4 | Cycle-walk helper: gate-anchored `start`, straightest-continuation 4-conn walk, closure detection, dead-end ⇒ fallback. Unit tests: rectangular ring → ordered cycle covering the loop; 2-cell band → single strand (thinned). | `crates/gen/src/phase7.rs` | 2, 3 |
| 5 | Orientation + resample + tangents: i64 shoelace winding vs `race_dir` (reverse if needed); arc-length resample (`s[0]==0`, monotone, `length` = perimeter, closed); wraparound unit tangents. Assemble `racing_line`. Unit tests: winding sign→`RaceDir` mapping asserted explicitly (`>0⇔Ccw`, `<0⇔Cw`, x-east/y-north handedness) for both variants; resample `s[0]==0`/monotone/≈even; tangents unit + `race_dir` sign. | `crates/gen/src/phase7.rs` | 4 |
| 6 | AC5 guard test: recursive source-scan of `../ai/src` for `Centerline` / `.centerline` (asserts absence). | `crates/gen/src/phase7.rs` | 1 |
| 7 | Integration fixtures: AC1 (branching medial → single non-branching loop), AC2 (`at(length) ≡ at(0)`, first/last adjacent), AC6 (byte-identical repeat), AC7 (rectangular ring: closes, `s` monotone + ≈even, tangents follow `race_dir`). | `crates/gen/src/phase7.rs`, `crates/gen/src/testfix.rs` | 5 |

**Test-fixture placement (GO-note 1 — `corridor` builder duplication).** The
4-arg `corridor(origin, w, h, &[(x,y)])` drivable-list builder is already
duplicated **within the gp-gen test binary at 2 sites** — `phase4.rs:279` and
`phase4_defects.rs:196` — so a fresh local copy in `phase7.rs` would be the **3rd**,
crossing the ≥3-site threshold *inside one crate/test binary* ([measured:
`rg -Un 'fn corridor' crates/` → gp-gen: `phase4.rs:279`, `phase4_defects.rs:196`;
gp-core: `distance.rs:141`, `track.rs:611`, `graph.rs:400`; gp-render:
`render/src/track/test_support.rs:32` (already a `pub(super)` helper); the 3-arg
shape at `transform.rs:95` is a different builder, excluded — 6 copies of this
shape across 3 crates]). **Disposition: reuse gp-gen's existing intra-crate
shared-fixture module `crates/gen/src/testfix.rs`** (`#![cfg(test)]`, `pub(crate)`
fixtures) — add the `corridor` builder and phase7's ring fixtures there as
`pub(crate)` and have `phase7.rs` consume them, so **phase7 adds zero new copy**.
This is that module's stated purpose: its doc says fixtures were "lifted … so both
modules share one definition rather than duplicating it" and that "**Ф6/Ф7 are
expected to need the same ring fixture too**" ([measured: `crates/gen/src/testfix.rs:1-8`]).
**No new workspace crate is needed** — all gp-gen tests link into one test binary,
where a `#![cfg(test)] pub(crate)` module shares fine; a shared workspace crate is
only forced when sharing must cross *binaries/crates* (which is exactly why
gp-render's copy is an intra-crate `pub(super)` helper, not a crate). Rationale:
the ≥3-site shared-helper rule + the in-tree `testfix.rs` precedent — **not**
"minimal surface / mirroring" (the reviewer flagged that alone is not an accepted
justification). *Scope boundary:* migrating the pre-existing `phase4.rs` /
`phase4_defects.rs` copies onto `testfix.rs` for a single definition is the ideal
end-state per the ≥3-site rule but widens scope beyond phase7 — flagged as an
**orchestrator-gated** follow-up (§ Open questions), never silently folded in.
*Cross-crate* unification (gp-core + gp-render also copy the shape) would need a
`test-support` feature/crate and is explicitly **out of scope** for this
render-only task.

## Handoff plan

- **(a)/(c)** A `## Handoff plan` is present (M = 7 ≥ 1); the sole group is
  entered via a `/context-reset` subagent per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Group A** — **code** change-type (`*.rs` only) → routed to the `code-writer`
  subagent, model **`sonnet`** (sonnet-5), effort **`medium` (pinned in
  `code-writer` frontmatter — no inline override)**, 1M-token window — subtasks
  **1–7**. Terminal group. Size 7 ∈ `1..=10` (b)/(d); homogeneous single
  change-type (e); one group is already the minimum (f); 1 group ≤ 4 (h).
- No inter-group handoff (single group); Group A runs Step 8 in its own
  `/context-reset` subagent.

## Risks

- **Straightest-continuation walk fails to thin/close on a pathological medial
  set** (e.g. a 3-cell-wide band, a figure-8 self-touch): the walk dead-ends or
  short-circuits. Mitigation: the walk detects non-closure and returns the empty
  `Centerline` fallback; render-only + graceful `at → None`, and the spec
  *defers* robustness beyond the fixtures. — [derived → subtask 4 unit tests +
  subtask 7 AC7 fixture; Deferred clause in spec].
- **`MAX_BRIDGE_GAP` too small (real corner unbridged) or too large (unrelated
  blobs joined).** Mitigation: set it from the observed corner-gap size in
  **Manhattan** units (`(3,1)`↔`(1,3)` on the annulus fixture is Manhattan 4)
  with a small margin — implemented value `MAX_BRIDGE_GAP = 6` (4 + 2). A bad
  value degrades to the empty fallback, never a panic or a wrong-but-plausible
  loop that passes silently. — [measured: annulus corner gap = Manhattan 4
  (Chebyshev 2), `distance.rs:242-250`; `MAX_BRIDGE_GAP = 6` at `phase7.rs:28`,
  pinned by subtask 2's fallback test].
- **`arithmetic_side_effects = "deny"`** on all integer geometry (Manhattan
  distance, shoelace, coord diffs) — [measured: `Cargo.toml:72
  arithmetic_side_effects = "deny"`, with `nursery`/`pedantic` = `deny` at
  `:63-64`]. Mitigation: use `saturating_*` / `checked_*` / i64-widening, or a
  justified `#[allow(clippy::arithmetic_side_effects, reason = …)]` — the
  established in-tree pattern ([measured: `crates/core/src/geom/mod.rs:389-395`
  supercover; `crates/core/src/track.rs:200-208` gradient_at]). — [derived →
  AC8 `cargo clippy --workspace --all-targets -D warnings`].
- **`missing_const_for_fn` (nursery, deny) FORCES `const fn` on const-eligible
  pure integer helpers — but const-*eligibility* turns on what the body CALLS.**
  The implemented `manhattan(a, b)` body is
  `i64::from(a.x.abs_diff(b.x)).saturating_add(i64::from(a.y.abs_diff(b.y)))`;
  `abs_diff` yields `u32` and **`i64::from(u32)` is not const-stable on the
  toolchain**, so `manhattan` is const-*in*eligible and correctly stays a plain
  `fn` — `missing_const_for_fn` declines to fire (the same const-stability
  blocker as `Rect::index`'s `bool::then` in the agent instructions, not a YAGNI
  call). Helpers containing `f32` math (hypot/normalize), loops with
  `Vec`/`BTreeSet` mutation, or `atan2` are likewise not const-eligible and stay
  non-const. — [measured: `manhattan` at `phase7.rs:32` is a plain `fn`; AC8
  clippy green].
- **`.centerline` field is exercised elsewhere; the AC5 scan must not
  over/under-match.** `TrackArtifact` HAS a `.centerline` field and gp-ai uses
  `TrackArtifact` — but does not read the field/type. Case-sensitive
  `Centerline` / `.centerline` avoids the lowercase-"centerline" comment
  false-positive. — [measured: `rg -Un 'Centerline|\.centerline' crates/ai/src/`
  → NO MATCHES].
- **`phase7.rs` file-size soft cap (GO-note 3).** phase7.rs carries the *whole
  algorithm* plus all 7 subtasks' tests, so it must watch the soft **500/800**
  (excl./incl. `#[cfg(test)]`) file-size limit (AGENTS.md § Code Style → File
  size). Offloading the shared fixtures to `testfix.rs` (GO-note 1) removes the
  fixture bulk; the algorithm module still landed at **529 lines** (29 over the
  soft-500 exclusive cap), so the `#[cfg(test)]` tests were split into a sibling
  `crates/gen/src/phase7_tests.rs` pulled in via `include!` — keeping the
  algorithm file under the hard limit. — [measured: `phase7.rs` 529 lines,
  `phase7_tests.rs` 446 lines].
- **No render golden / image test in scope.** Renderer consumption is out of
  scope (spec § Out of scope); the `Centerline` is validated numerically. The
  text-golden-threshold, shared-boundary-fill/stroke, and golden-fidelity design
  rules do not apply here — [measured: spec § Out of scope, "Renderer
  consumption of the centerline (block 2)"].

## Test Design

Tests live in a sibling `crates/gen/src/phase7_tests.rs` pulled into `phase7.rs`'s
`#[cfg(test)] mod tests` via `include!` (file-size split — see § Risks file-size
bullet); shared fixtures (the `corridor` builder + ring fixtures) live in
`crates/gen/src/testfix.rs` per **GO-note 1** above.
`gp-gen` is Miri-`--exclude`d ([measured: `.github/workflows/*.yml:192-193`]),
so no `#[cfg_attr(miri, ignore)]` gating is required for the file-reading AC5
test or any other.

- **Subtask 1 — fallback skeleton.** Entry `racing_line`. Scenario: empty
  `Corridor` (empty medial) ⇒ `Centerline::default()` (`samples` empty, `length`
  0.0, `at(0.0)` is `None`). Fixture: `Corridor::new(origin, w, h)` with no cells
  set.
- **Subtask 2 — bridge.** Entry: `bridge_gaps` (over the medial set) plus
  `bridge_path`. Scenarios: (happy) odd-thickness rectangular-ring medial (4
  strips + corner gaps) → one closed 4-connected ring after bridging, with **no
  dangling leaves** (the leaf-tracking stop closes the last corner gap, not just
  a component count); (edge) two components separated by > `MAX_BRIDGE_GAP`
  (Manhattan) → fallback signal (`None`). The `bridge_path` axis-order /
  `¬D`-avoidance case — an `(a, b)` whose `x`-then-`y` route would cross a `¬D`
  hole but whose `y`-then-`x` route stays in `d`, so the in-`d` order must be
  chosen — is **not** a standalone unit test; it is exercised via the annulus
  bridge test `bridge_gaps_joins_annulus_corner_gaps_into_one_component`
  ([measured: `crates/gen/src/phase7_tests.rs:29`]), which is precisely where
  the `¬D`-routing case was found (the annulus's own centre hole is the `¬D`
  region a fixed axis order would route through — [measured: `phase7.rs:94-98`
  doc-comment: "also found via the AC7 annulus test"]). Fixtures:
  `Corridor::filled` minus a
  centred hole (reuse the `crates/core/src/geom/distance.rs:233-238` annulus
  shape as a local fixture); a two-blob corridor for the gap-guard case.
- **Subtask 3 — prune.** Entry: prune helper. Scenario: a ring with one inward
  finger (spur) → after prune, finger cells gone, every surviving cell has
  4-conn degree ≥ 2. Fixture: a small rectangular-ring cell set with a 2–3 cell
  finger appended.
- **Subtask 4 — cycle walk.** Entry: walk helper. Scenarios: (happy)
  rectangular-ring core → ordered `Vec<Point>` that closes (last neighbour is
  `start`) and covers the loop; (thinning) even-width 2-cell band ring → single
  strand (each returned cell distinct, one rail). Edge: a broken (open) core →
  fallback signal.
- **Subtask 5 — orient / resample / tangents.** Entry: `racing_line` end-to-end
  on a hand-built ring. Scenarios:
  - **Winding → `RaceDir` sign (pin the convention by assertion, GO-note 2).**
    This grid is right-handed — `Point.x` increases eastward, `Point.y` increases
    *northward* ([measured: `crates/core/src/geom/mod.rs:30-36` — "increasing
    eastward" / "increasing northward"]) — so the standard math sense holds: the
    integer shoelace signed area `Σ(xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ)` is **> 0 for a CCW ring,
    < 0 for a CW ring** (unit check: `(0,0)→(1,0)→(1,1)→(0,1)` sums to `+2`, and
    that vertex order *is* CCW in x-east/y-north). The test builds a ring whose
    cell order is known and asserts its raw shoelace **sign** against this table
    (`> 0 ⇔ Ccw`, `< 0 ⇔ Cw`) *before* orientation, then asserts
    `racing_line(.., RaceDir::Ccw)` yields a positive-shoelace sample ring and
    `racing_line(.., RaceDir::Cw)` yields the reversed, negative-shoelace ring —
    pinning the sign→`RaceDir` mapping **explicitly**, not merely "the two variants
    are reversed". This nails the convention (shoelace sense vs `Cw`/`Ccw` under
    this handedness) by assertion rather than incidentally. — [derived → subtask 5
    unit test].
  - `samples[0].s == 0`, `s` strictly increasing, successive `Δs ≈ RESAMPLE_STEP`
    (within a tolerance).
  - each `tangent` is unit-length (`hypot ≈ 1.0`) and, walking samples, advances
    along `race_dir` (dot with the forward direction > 0). Fixture: a small closed
    rectangular ring where cell positions are known.
- **Subtask 6 — AC5 guard.** Entry: the scan test itself. Scenario: recursive
  read of `concat!(env!("CARGO_MANIFEST_DIR"), "/../ai/src")` `.rs` files asserts
  no `Centerline` / `.centerline` substring. Robust to CWD via
  `CARGO_MANIFEST_DIR`. — [measured baseline: NO MATCHES today].
- **Subtask 7 — integration (AC1/AC2/AC6/AC7).** Entry `racing_line`.
  - **AC7 (primary fixture):** an odd-thickness rectangular ring
    (`Corridor::filled(11×11)` minus a centred `5×5` hole — the annulus shape),
    a `TimingGate` on one straight, a `RaceDir`. Assert: loop closes
    (`cl.at(cl.length)` ≈ `cl.at(0.0)`, first/last resampled positions adjacent
    around the ring); `s` monotone strictly increasing with `samples[0].s == 0`;
    successive spacing ≈ `RESAMPLE_STEP`; tangents unit + `race_dir`-signed.
  - **AC1:** a branching fixture (ring + infield finger) → the returned
    `samples` trace one non-branching loop (spur absent — assert no sample sits
    on the finger cells; consecutive samples are ≤ ~1 step apart, no branch).
  - **AC2:** `cl.at(cl.length)` ≈ `cl.at(0.0)`; `cl.at(cl.length + x)` ≈
    `cl.at(x)` (delegates to `Centerline::at` wrap — already tested in `gp-core`,
    here asserted on a produced curve).
  - **AC6:** `racing_line(&d, &gate, dir)` called twice ⇒ field-by-field
    identical `Centerline` (positions/tangents/`s`/`length` bit-identical).
  - Helpers: the `corridor(origin, w, h, &[(x,y)])` builder + phase7's ring
    fixtures live in the shared `crates/gen/src/testfix.rs` (`pub(crate)`) and are
    consumed here rather than copied locally — see **Test-fixture placement
    (GO-note 1)** in § Decomposition for the verified 3-site count, the
    `testfix.rs` precedent, and the no-workspace-crate rationale.

## Open questions

- **Orchestrator-gated (GO-note 1 scope boundary).** Should the pre-existing
  `phase4.rs:279` / `phase4_defects.rs:196` `corridor` copies *also* migrate onto
  the shared `testfix.rs` builder for a single gp-gen definition? Recommended by
  the ≥3-site shared-helper rule, but it widens scope beyond phase7's feature — the
  orchestrator owns the include-now / defer-to-follow-up call. In-scope default:
  phase7 consumes the shared helper (adding no new copy) and the two legacy copies
  are left untouched for a follow-up dedup ticket.

Otherwise none blocking. `MAX_BRIDGE_GAP` and `RESAMPLE_STEP` concrete values, and the
exact straightest-continuation tie-break order, are implementation choices
pinned by their subtask tests (2, 4, 5); all defensible defaults are recorded
above and the empty-`Centerline` fallback bounds the blast radius of any wrong
default (render-only, `Centerline::at → None` degrades gracefully).
