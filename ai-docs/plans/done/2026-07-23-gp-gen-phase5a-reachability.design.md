# Design: gp-gen Ф5a — reachability substrate + V=1 liveness (+ core S/F bounded-chord fix)

**Issue:** [#28](https://github.com/maratik123/graphite-gp/issues/28)
**Date:** 2026-07-23
**Spec:** `ai-docs/plans/2026-07-23-gp-gen-phase5a-reachability.spec.md`

## Status of prior work (round-2 reconciliation)

Subtasks **1–5 are implemented and committed** on
`feat/2026-07-23-gp-gen-phase5a-reachability`
`[measured: git log --oneline → 82dd775 (skeleton) · f89aabc (forward) · d3ff23c
(backward) · 6802fb1 (oracle) · 03a851f (cross-cutting tests)]`. The substrate
(`forward_reachable`, `backward_reachable`, `oracle_liveness_v1`) and its tests
exist. This round **widens scope** to add the **`gp-core` S/F bounded-chord fix**
(new AC6, updated AC4/AC7) that the oracle's AC4 depends on, and to **correct the
oracle tests that encoded the pre-fix behaviour** — those are the only edits to
the already-committed substrate.

Concretely, the committed `oracle_liveness_v1_ac4_valid_ring_finding_raw_never_exceeds_zero`
asserts the oracle returns **`false`** on a valid ring (with a long "DISCOVERED
FINDING" comment), and `ac3_oracle_crossing_matches_direct_lap_counter_register_move`
also asserts `!oracle(...)` `[measured: read crates/gen/src/phase5.rs:372–417,
467–484 → both `assert!(!oracle_liveness_v1(...))`]`. Both encode the bug; both
must flip to `true` once the core fix lands (AC4 restored).

## Approach

The Ф5a substrate is unchanged from round 1 (recap below, for a complete
artifact). The new work is the **core fix** and the **test reconciliation**.

Everything is a finite, integer-only, deterministic, total computation, using
**core's own** `legal_move` as the graph edge and **core's own**
`LapCounter::register_move` for the S/F crossing test — no reimplementation
(spec AC3; design.md §3 *Оракул проходимости*, *Счётчик кругов*).

### Substrate (committed subtasks 1–5) — recap

`crates/gen/src/phase5.rs` (wired via `mod phase5; pub use phase5::*;` in
`lib.rs` `[measured: cat crates/gen/src/lib.rs → "mod phase5;" … "pub use
phase5::*;"]`) exposes:

```rust
pub fn forward_reachable(d: &Corridor, seeds: &[CarState], v_ceil: i32) -> HashSet<CarState>; // AC1
pub fn backward_reachable(d: &Corridor, goals: &[CarState], v_ceil: i32) -> HashSet<CarState>; // AC2
pub fn oracle_liveness_v1(d: &Corridor, grid: &StartGrid, sf: &StartFinish, race_dir: RaceDir) -> bool; // AC4
```

- Return container = `std::collections::HashSet<CarState>` (`CarState` derives
  `Copy + Eq + Hash` `[measured: sed -n 18p crates/core/src/sim/mod.rs →
  "#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]"]`). Membership is
  a deterministic function of inputs (AC5); iteration order is never relied on.
- **Forward edge** = `legal_move` over `Action::iter()` order, `within_v_ceil`
  bound, `step` on the assumed-legal domain.
- **Backward edge** = `predecessor(s2, a)` via `checked_sub` (total, `None` on
  overflow), gated on the **same** `legal_move` (AC3).
- **Oracle** = augmented `(CarState, LapCounter)` flood; each transition clones
  the counter and calls `register_move(sf, from, to)`; `raw() >= 1`
  short-circuits `true`; visited key `(CarState, raw().clamp(-1, 0))` keeps the
  key domain finite. `race_dir` is accepted but unused (crossing sign comes from
  `sf.gate.forward`); neutralised by `let _ = race_dir;`.

This is exactly the design.md §2 `oracle_liveness_V1` pseudocode with the counter
threaded through so "lap-closing" is decidable. **Because the oracle consumes
core's `register_move` unchanged, the core fix below is what makes AC4 reachable.**

### S/F bounded-chord core fix (AC6) — NEW

**Root cause (confirmed against source).** `register_move`
(`crates/core/src/sim/mod.rs:210`) reads only `sf.gate.behind.first()`, projects
each endpoint with `gate_coord` — the **doubled perpendicular** coordinate
*along `forward`'s axis only*: `2·((p.x−r.x)·dx + (p.y−r.y)·dy)`
`[measured: read crates/core/src/sim/mod.rs:247–250]` — and tests
`crossing_event` against the **global** half-grid line `GATE_LINE = 1`
`[measured: read :224, :259–267]`. The **along-chord (perpendicular-to-`forward`)
coordinate is never consulted**, so the test scores against the chord's *infinite
supporting line*, not the bounded chord. On any closed ring the supporting line
is re-crossed on the **far wall** (a reverse crossing), so `raw()` nets zero per
lap and `raw() >= 1` is topologically unreachable via continuous driving — the
finding in the spec. design.md §3 mandates a **bounded** chord: «S/F — полный
хорд, разрезающий аннулюс в односвязную полосу … Событие фиксируется по факту
пересечения отрезком хода, ≤1 за ход» `[measured: sed -n 238,244p docs/design.md]`.

**Fix — bound the crossing to the chord's along-chord extent.** `register_move`
carries the bound; `gate_coord` and `crossing_event` stay **unchanged** (they
remain the perpendicular projection + half-open test). Two new `const fn` leaf
helpers, then a guard inside `register_move`:

1. **`lat_coord(p, r, forward) -> i32`** — the *along-chord* (perpendicular-to-
   `forward`) coordinate of `p` relative to `r`, using the in-plane perpendicular
   of `forward.delta() = (dx, dy)`:
   `lat_coord = (p.x − r.x)·(−dy) + (p.y − r.y)·dx`.
   For `forward = East (1,0)` this is `p.y − r.y`; for `North (0,1)` it is
   `−(p.x − r.x)`. Sign is irrelevant (only used for a min/max span + a
   symmetric containment test). Const-eligible pure integer accessor exactly like
   `gate_coord` → **`const fn`** with the identical documented
   `#[allow(clippy::arithmetic_side_effects, reason = "…grid-realistic domain…")]`
   `gate_coord` already carries `[measured: read :240–250]`. *(Not doubled — see
   the interpolation note; mixing the doubled perp with the undoubled lat is
   homogeneous.)*

2. **`crossing_within_span(b_perp, b_lat, a_perp, a_lat, lo, hi) -> bool`** — the
   integer along-chord containment test for the crossing point. Given the
   **behind** endpoint `B = (b_perp, b_lat)` and the **ahead** endpoint
   `A = (a_perp, a_lat)` (both `perp` = doubled `gate_coord`, both `lat` =
   `lat_coord`), the segment meets the gate line `perp = GATE_LINE` at parameter
   `t* = (GATE_LINE − b_perp) / d`, `d = a_perp − b_perp > 0`. The crossing's
   along-chord coordinate is `lat* = b_lat + t*·(a_lat − b_lat)`; the crossing is
   in-extent iff `lo ≤ lat* ≤ hi`. Cleared of the division (multiply by `d > 0`,
   direction preserved), in **`i64`**:
   ```text
   d   = i64(a_perp) − i64(b_perp)                     // ≥ 2 > 0 (both even, straddling odd GATE_LINE)
   num = d·i64(b_lat) + (GATE_LINE − i64(b_perp))·(i64(a_lat) − i64(b_lat))   // = d·lat*
   in-span ⇔  d·i64(lo) ≤ num  ∧  num ≤ d·i64(hi)
   ```
   Pure integer, no float, no division, `d > 0` (so no zero-divide and no
   sign-flip). Const-eligible → **`const fn`**; the `i64` products stay in range
   on the grid-realistic domain (`|coord| ≪ 1.5×10⁹`, same domain `gate_coord`
   feeding it already assumes — and `gate_coord` overflows first if that domain is
   violated), so it carries the same documented `#[allow(clippy::arithmetic_side_effects,
   reason = "…")]` as the sibling `respawn_cell`'s `i64` projection `[measured:
   read :391–414]`. *(Equivalent total alternative: `checked_mul`/`checked_sub`
   returning `false` on overflow; the `i64` + documented-allow form is chosen for
   consistency with `gate_coord`/`respawn_cell` in the same module.)*

3. **`register_move` guard.** After the existing perpendicular event
   classification:
   ```text
   let Some(&r) = sf.gate.behind.first() else { return };   // unchanged no-op guard
   let fwd = sf.gate.forward;
   let (fp, tp) = (gate_coord(from, r, fwd), gate_coord(to, r, fwd));
   let ev = crossing_event(fp, tp);
   if ev == 0 { return; }
   // along-chord span over the WHOLE behind cross-section (fold, no unwrap/panic):
   let (lo, hi) = sf.gate.behind.iter().fold((0, 0), |(lo, hi), &b| {
       let l = lat_coord(b, r, fwd); (lo.min(l), hi.max(l))
   });                                                       // seed 0 = lat_coord(r,r,fwd)
   // orient endpoints: behind is the < GATE_LINE side, ahead the ≥ side
   let ((bp, bl), (ap, al)) = if ev > 0 {
       ((fp, lat_coord(from, r, fwd)), (tp, lat_coord(to, r, fwd)))
   } else {
       ((tp, lat_coord(to, r, fwd)), (fp, lat_coord(from, r, fwd)))
   };
   if crossing_within_span(bp, bl, ap, al, lo, hi) {
       self.counter = self.counter.saturating_add(ev);
   }
   ```
   The span folds over the **whole** `sf.gate.behind` cross-section (not just
   `first()`) — the reference cell `r = behind[0]` seeds `lat 0`, and the fold's
   `(0,0)` seed makes it total with no `unwrap`/`min().unwrap()` panic path
   (gp-core zero-panic posture). `saturating_add` is retained (unchanged). The
   empty-gate no-op (`behind.first()` returns `None`) is unchanged.

**Correctness re-derivation for the bounded case.**
- *At most one event per straight chord (preserved).* A straight segment's
  perpendicular coordinate is monotone in `t`, so it meets `perp = GATE_LINE` at
  most once (unchanged from the pre-fix argument, design.md §3 «две прямые
  пересекаются раз»). The new guard only **removes** an event when the crossing
  is out-of-extent; it never adds one. So "≤1 event/chord" holds.
- *Direction symmetry.* For the same physical segment, forward (`from=P,to=Q`)
  and reverse (`from=Q,to=P`) orient to the identical `(B=P, A=Q)`, hence the
  identical `num` — the containment verdict is orientation-independent (a segment
  either passes through the extent or it does not).
- *Soundness of the ring lap.* On the width-1 ring, the gate-cell forward step
  (behind `(2,0)` → ahead `(3,0)`) has `lat* = 0 ∈ [0,0]` → counts; every
  far-wall reverse crossing (top straight, `lat = 4 ∉ [0,0]`) is **excluded**, so
  a full CCW loop nets `+1` and `raw()` advances `−1 → 0 → 1` over two laps'
  gate passes. `[derived → subtask 6 physical-lap test reaches raw() >= 1]`
- *No regression on existing tests.* Every committed `register_move` test that
  scores a **crossing** uses endpoints on the chord line (`sf_east_gate` has
  `behind = [(1,1)]`; the crossing endpoints are all at `y = 1`), so `lat* = 0 ∈
  [0,0]` and every asserted delta is unchanged. The only off-`y=1` endpoints
  (`ac6`'s parallel `(2,0) → (2,3)`) form a **tangent** move with constant
  `gate_coord` (`ev = 0`), which short-circuits **before** the span guard — so it
  too is unaffected. `[measured: read crates/core/src/sim/mod.rs:692–847 → every
  crossing endpoint at y=1; the (2,0)→(2,3) move is ev=0 tangent]` `[derived →
  cargo test -p gp-core keeps register_move_ac1..ac7 green]`.

### Integer-safety / lint posture (binding constraints, measured)

- **`arithmetic_side_effects = "deny"`** (workspace #48) `[measured: sed -n
  '/\[workspace.lints/,/return_self_not_must_use/p' Cargo.toml →
  "arithmetic_side_effects = \"deny\""]`. New helpers: `lat_coord` mirrors
  `gate_coord`'s documented `#[allow]` (i32, grid-realistic domain);
  `crossing_within_span` mirrors `respawn_cell`'s documented `#[allow]` (i64);
  `register_move`'s fold uses `i32::min`/`max` (no raw op) and the unchanged
  `saturating_add`.
- **`missing_const_for_fn` (nursery) = "deny"** forces `const fn` on the two pure
  helpers — both call only const-stable operations (`Side::delta` is `const`
  `[measured: grep -n "fn delta" crates/core/src/geom/graph.rs:35 → "pub const fn
  delta"]`, `i64::from`, integer `*`/`-`/comparison), so `const` is both mandatory
  and viable. `register_move` mutates `self` (not const-eligible) — stays
  non-const, as today. If clippy declines a `const` for a callee that is not
  const-stable on stable (the `bool::then`/`Rect::index` `E0658` class), drop
  `const` for that helper and let the gate decide.
- **No production panic** (gp-core zero-panic; `ai-docs/panic-index.md` is
  intentionally empty): no `unwrap`/`expect`/`panic!`/indexing added. The span
  fold seeds `(0,0)` instead of `min().unwrap()`; `crossing_within_span` is pure
  comparison; `behind.first()` guard unchanged.
- **Snake-case / unused param** posture of the substrate is unchanged
  (`oracle_liveness_v1`, `let _ = race_dir;`) — already committed and green.

### Rejected alternatives

**For the core fix:**
- **Endpoint-only lateral bound** (require both `from`/`to` `lat` within span,
  skip interpolation). Rejected: correct for the V=1 single-step crossing but
  *wrong for the general runtime chord* — `register_move` is the runtime lap
  counter for arbitrary velocities (design.md line 15: chords may be large and
  diagonal, e.g. `(2,3)`); a long chord can cross the gate line with the crossing
  point in-extent while both endpoints are laterally outside, or vice-versa. The
  interpolation test `lo ≤ lat* ≤ hi` is the principled general answer and adds
  no cost.
- **Walk `supercover(from,to)` and test each adjacent-cell step against
  `TimingGate::separates`.** Rejected: heavier (allocates/iterates the whole
  cover), and `separates`' corner (dual-vertex) semantics reintroduce the exact
  tie-break ambiguity the perpendicular test cleanly avoids. The endpoint
  interpolation is `O(1)`, integer, and already keyed on the same `gate.behind`
  cut cells.
- **Bound via `sf.chord` instead of `sf.gate.behind`.** Rejected: `register_move`
  already keys on `gate.behind` (its reference cell `r = behind[0]`), and `behind`
  *is* the drivable cross-section directly behind the gate — the along-chord
  extent. Using `chord` would introduce a second geometry source for the same
  quantity. Stay on `behind`.

**For the substrate (unchanged from round 1):**
- **Post-scan a plain 4D reachable set for a lap-close crossing.** Rejected: the
  pure set discards the counter, so it cannot distinguish race-start (`−1 → 0`)
  from lap-close (`0 → 1`) — fails AC4. The augmented `(CarState, LapCounter)`
  flood is the minimal decidable state.
- **Cardinal-only V=1 domain / `BTreeSet` for ordered iteration.** Rejected in
  favour of the L∞ box (superset, sound + complete for binary liveness) and
  `HashSet` membership + a deterministic `VecDeque` worklist.
  `[derived → superset ⇒ every detected lap is a real V≤1 lap (sound); any
  cardinal lap still found (complete)]`

## Decomposition

Subtasks **1–5 are committed** (shown for provenance; no re-work). The remaining
work is subtasks **6–7** — all Rust (`.rs`), TDD (new/updated tests precede the
prod change within the subtask).

| # | Task | Files | Depends on | Status |
|---|------|-------|------------|--------|
| 1 | `phase5.rs` skeleton + `lib.rs` wiring + `predecessor`/`within_v_ceil` helpers | `crates/gen/src/phase5.rs`, `crates/gen/src/lib.rs` | — | ✅ committed 82dd775 |
| 2 | `forward_reachable` + tests | `crates/gen/src/phase5.rs` | 1 | ✅ committed f89aabc |
| 3 | `backward_reachable` + tests | `crates/gen/src/phase5.rs` | 1 | ✅ committed d3ff23c |
| 4 | `oracle_liveness_v1` + tests (valid/broken/dead-end) | `crates/gen/src/phase5.rs` | 1 | ✅ committed 6802fb1 |
| 5 | Cross-cutting AC tests (AC3 one-code-path, AC5 determinism, R∩B membership) | `crates/gen/src/phase5.rs` | 2,3,4 | ✅ committed 03a851f |
| **6** | **Core S/F bounded-chord fix (AC6):** add `const fn lat_coord` + `const fn crossing_within_span`; add the along-chord guard to `register_move` (`gate_coord`/`crossing_event` unchanged, `legal_move` untouched). **TDD tests (AC7 core):** (a) NEW off-chord supporting-line crossing is a no-op; (b) NEW physically-continuous closed-loop drive on a ring reaches `raw() >= 1`; (c) reconcile the two scripted-duplicate tests' framing (`register_move_ac4_init_and_laps`, `register_move_ac6_scripted_telescoping_and_parallel_move`). | `crates/core/src/sim/mod.rs` | — | remaining |
| **7** | **gp-gen oracle-test correction (AC4):** flip `..._ac4_valid_ring_finding_raw_never_exceeds_zero` to assert `true` (rename → `..._ac4_valid_ring_is_lappable`, replace the "FINDING" comment with the bounded-chord rationale); flip the trailing oracle assertion in `ac3_oracle_crossing_matches_direct_lap_counter_register_move` to `true` and update its comment. Broken-ring, dead-end, determinism, and R∩B tests stay unchanged (still correct). | `crates/gen/src/phase5.rs` | 6 | remaining |

## Handoff plan

Grouping is required for **every M ≥ 1**; boundaries are pre-computed here so
`/task` Step 8 reads them rather than re-deriving per turn. The **remaining**
work is subtasks 6–7 (subtasks 1–5 are already committed). Both remaining
subtasks change **code** (`*.rs` only — `crates/core/**` and `crates/gen/**`) —
one homogeneous change-type — and the dependency `6 → 7` fits inside a single
`≤ 10`-subtask group in listed order, so the minimized grouping is **one** group
(fewest possible; well under the default max of 4).

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, via the
  `code-writer` subagent (frontmatter-pinned `model`/`effort`; no inline
  override), 1M-token window — **subtasks 6–7** (code change-type: `*.rs`).
  Terminal group (2 subtasks; within the `1..=10` range). No inter-group handoff —
  the single group completes Step 8 in its own `/context-reset` subagent, spawned
  per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) at
  the **entry** into Group A.

## Risks

- **Core fix must be behaviour-preserving for the 6 committed `register_move`
  tests.** Every scored crossing uses endpoints on the chord line (`y = 1`,
  `behind = [(1,1)]`), so `lat* = 0 ∈ [0,0]` and the delta is unchanged; the one
  off-`y=1` move (`ac6`'s `(2,0)→(2,3)`) is an `ev = 0` tangent that
  short-circuits before the span guard. The guard only excludes off-chord
  crossings none of them exercise. — `[measured: read
  crates/core/src/sim/mod.rs:692–847 → crossing endpoints at y=1; (2,0)→(2,3) is
  ev=0 tangent]` · `[derived → cargo test -p gp-core keeps register_move_ac1..ac7
  green]`
- **Third committed `register_move` call-site survives unchanged — gp-gen
  `phase3.rs:970`.** A workspace-wide sweep for `register_move` finds a third
  committed call-site beyond `sim/mod.rs` and `phase5.rs`: the test
  `ac6_start_positions_read_negative_one_and_front_row_registers_a_forward_crossing`
  (`crates/gen/src/phase3.rs:970`), which calls `register_move` in two loops
  (lines 991, 1005) and asserts `raw() == 0`. It survives the bounded-chord fix
  with **no edit**: `start_grid` (`phase3.rs:407–429`) shifts each front-`chord`
  cell purely along `−forward` (`p.x.saturating_sub(dx·row)`,
  `p.y.saturating_sub(dy·row)`, lines 417–420), and shifting along `±forward`
  leaves the along-chord `lat_coord` invariant (its `(−dy, dx)` basis is
  orthogonal to `forward = (dx, dy)`), so every grid position inherits the **same
  along-chord `lat`** as its chord column. Each loop's move is purely along
  `forward` (`to = p + forward·n`, constant `lat`), so the crossing point's
  `lat*` equals that inherited chord-column `lat`, which lies within the behind
  cross-section span `[lo, hi]` — airtight because in the phase3 `StartFinish`
  `behind == chord` (`gate.behind` is built directly from `chord`), so every chord
  column's `lat` is a span member by construction — hence the crossing still counts
  and `raw() == 0` is unchanged. Subtask 7 consequently needs **no phase3 change**.
  — `[measured: read crates/gen/src/phase3.rs:970–1012 → two register_move loops
  asserting raw()==0; :407–429 → start_grid shifts chord along −forward via
  saturating_sub of forward.delta()]` · `[measured: read
  crates/gen/src/phase3.rs:507–514 → the StartFinish literal sets `behind: chord`
  (:511); asserted at :775 `assert_eq!(out.sf.gate.behind, out.sf.chord)` in
  ac4_gate_behind_equals_chord_and_forward_matches_segment]` · `[derived → cargo
  test -p gp-gen keeps ac6_start_positions_...forward_crossing green after
  subtask 6]`
- **`arithmetic_side_effects` (deny) on the new integer ops.** `lat_coord`
  mirrors `gate_coord`'s i32 documented `#[allow]`; `crossing_within_span`
  mirrors `respawn_cell`'s i64 documented `#[allow]`; the span fold uses
  `i32::min`/`max` and `saturating_add` (no raw op). — `[measured: sed -n
  '/\[workspace.lints/,/return_self_not_must_use/p' Cargo.toml →
  arithmetic_side_effects = "deny"]` · `[measured: read :240–250 (gate_coord
  allow), :391–414 (respawn_cell i64 allow)]` · `[derived → cargo clippy
  --workspace --all-targets -- -D warnings]`
- **`missing_const_for_fn` (deny) forces `const` on the two helpers, or `E0658`
  if a callee is not const-stable.** Both call only const-stable ops
  (`Side::delta` const, `i64::from`, integer arithmetic/comparison), so `const fn`
  is mandatory and viable; if the gate rejects a `const` (the `bool::then`/
  `Rect::index` class), drop it for that helper. — `[measured: grep -n "fn delta"
  crates/core/src/geom/graph.rs:35 → "pub const fn delta"]` · `[derived → cargo
  clippy --workspace --all-targets -- -D warnings]`
- **i64 overflow domain for `crossing_within_span`.** Products are `~d·lat ≈
  2·gridspan²`; on the grid-realistic domain (`|coord| ≪ 1.5×10⁹`, the domain
  `gate_coord` already assumes and overflows first on violation) `2·gridspan² <
  i64::MAX`, so the widened arithmetic stays in range under the documented
  `#[allow]`. — `[derived → cargo clippy + cargo test -p gp-core (in-domain
  fixtures) + Miri gp-core]`
- **`-D warnings` aborts on first failure.** After the lint classes above clear,
  re-run `cargo clippy --workspace --all-targets -- -D warnings` once more to
  surface any same-class site the first abort masked; surface any new
  out-of-contract class to the orchestrator. — `[derived → cargo clippy re-run]`
- **Miri (gp-core IS gated).** `register_move`, `lat_coord`,
  `crossing_within_span`, and the new core tests are pure integer (no FFI/GPU, no
  UB signal), so they are Miri-clean and need **no** per-test
  `#[cfg_attr(miri, ignore)]`; run the workspace gate command
  (`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace --exclude
  gp-gen`) to confirm gp-core stays green. — `[measured: AGENTS.md § Rust Test
  Conventions → "gp-core IS in the Miri gate … only gp-gen is the sanctioned
  crate-level --exclude"]`
- **Physical-lap fixture must actually be lappable at V=1.** A width-1
  4-connected border ring is lappable at V=1 (design.md §3: «на v=1 тормозной
  путь нулевой, разворот почти в любой клетке») via a crawl (accelerate one cell,
  brake to rest, turn at corners) — every move is a 1-cell chord with trivial
  `supercover`, so each is `legal_move`. — `[measured: sed -n 226p docs/design.md
  → "на v=1 тормозной путь нулевой (разворот почти в любой клетке)"]` · `[derived
  → subtask 6 physical-lap test drives the loop and reaches raw() >= 1]`

## Test Design

### Core tests — `crates/core/src/sim/mod.rs` `#[cfg(test)] mod tests` (subtask 6)

Reuse the existing `sf_east_gate()` (`behind = [(1,1)]`, `forward = East`) and
`car(...)` helpers already in the module.

- **NEW `register_move_ac6_off_chord_crossing_is_a_no_op`** — Entry:
  `register_move`. A crossing of the supporting line **outside** the chord extent
  leaves the counter unchanged. With `sf_east_gate()` (span along-chord = `y = 1`
  only), drive `(0,3) → (2,3)`: the perpendicular test classifies a forward
  crossing (`fp = −2 < GATE_LINE ≤ tp = 2`) but `lat* = 2 ∉ [0,0]`, so
  `register_move` must **not** change `raw()` (stays `−1`). Contrast row: the
  on-chord `(0,1) → (2,1)` on the same gate **does** count (`raw() −1 → 0`),
  pinning that the guard excludes only the off-chord case. — `[derived → subtask
  6 off-chord test: raw() unchanged]`
- **NEW `register_move_ac6_physical_lap_reaches_raw_ge_one`** — Entry:
  `register_move` (driven from a scripted physically-continuous path). Build the
  width-1 ring (border of a 5×5 box; drivable iff `x ∈ {0,4}` or `y ∈ {0,4}`) with
  gate `behind = [(2,0)]`, `forward = East` (ahead `(3,0)`). Script a continuous
  V≤1 crawl CCW from `(2,0)`: for each border step accelerate one cell then brake
  to rest (`East` then `West` advances `+x` and returns to `v=0`; `North`/`South`
  analogously), turning at the four corners from rest. **Assert every move is
  `legal_move` before `register_move`** (physical continuity + one-code-path), and
  after the loop closes past the gate a second time assert `raw() >= 1` and
  `laps() >= 1`. The far-wall (top, `y = 4`) reverse crossings must **not**
  decrement — assert `raw()` never drops below `0` after the race-start crossing.
  The exact move list is the implementor's (verified by the test passing); the
  crawl recipe above guarantees a legal V≤1 loop exists (see § Risks). — `[derived
  → subtask 6 physical-lap test: raw() >= 1, laps() >= 1, no far-wall decrement]`
- **UPDATE `register_move_ac4_init_and_laps` + `register_move_ac6_scripted_telescoping_and_parallel_move`**
  — these reach `raw() == 1` via a **repeated identical** `(1,1) → (2,1)` forward
  pair. Under the bounded-chord semantics the numbers are **unchanged** (both
  points are on the chord, `lat* = 0 ∈ [0,0]`), so the assertions stay green.
  Reconcile their **framing**: update the comments to state they exercise the
  signed-crossing **arithmetic** with scripted on-chord inputs (a valid unit test
  of the counter mechanism), **not** a physically-continuous lap — cross-reference
  the new `..._physical_lap_reaches_raw_ge_one` test for real-lap coverage. Do
  **not** delete or renumber them. — `[derived → both tests stay green; comments
  reconciled]`

### gp-gen oracle-test correction — `crates/gen/src/phase5.rs` (subtask 7)

**Scope note — subtask 7 touches `phase5.rs` only; `phase3.rs` needs no edit.**
The third committed `register_move` call-site, `phase3.rs:970`'s
`ac6_start_positions_read_negative_one_and_front_row_registers_a_forward_crossing`,
survives the bounded-chord fix unchanged (grid positions inherit their chord
column's along-chord `lat` — `start_grid` shifts along `−forward`, which leaves
`lat` invariant — and the test's moves are purely along `forward`, so
`lat* ∈ [lo, hi]` and every `raw() == 0` assertion holds; see § Risks →
*Third committed `register_move` call-site survives unchanged*). Do **not** edit
it. — `[measured: read crates/gen/src/phase3.rs:970–1012, :407–429]` · `[derived
→ cargo test -p gp-gen keeps this test green after subtask 6]`

- **`oracle_liveness_v1_ac4_valid_ring_finding_raw_never_exceeds_zero` →
  `oracle_liveness_v1_ac4_valid_ring_is_lappable`** — flip the assertion to
  `assert!(oracle_liveness_v1(&d, &grid, &sf, RaceDir::Ccw))`, delete the
  "DISCOVERED FINDING" block, and replace it with the bounded-chord rationale (the
  fix excludes far-wall reverse crossings, so a full CCW loop nets `+1` and the
  augmented flood reaches `raw() >= 1`). — `[derived → subtask 7 valid-ring test:
  oracle == true, after subtask 6]`
- **`ac3_oracle_crossing_matches_direct_lap_counter_register_move`** — keep the
  direct `register_move((2,0) → (3,0))` race-start assertion (`raw() == 0`, still
  correct — a lone on-chord forward crossing is `−1 → 0`), and flip the trailing
  `assert!(!oracle_liveness_v1(...))` to `assert!(oracle_liveness_v1(...))`,
  updating the comment: the shared `register_move` path scores the single crossing
  as race-start (`raw() 0`) **and**, threaded through the full flood, reaches a
  lap on the valid ring. — `[derived → subtask 7: oracle == true]`
- **Unchanged (still correct, do not touch):**
  `oracle_liveness_v1_ac6_broken_ring_is_not_lappable` (broken ring ⇒ `false`),
  `oracle_liveness_v1_ac4_distinguishes_race_start_from_lap_close` (dead-end ⇒
  `false`), `ac5_all_three_functions_deterministic_on_ring_fixture`,
  `ac6_forward_and_backward_reachable_intersect_on_known_state`. The dead-end and
  broken-ring fixtures remain valid witnesses that the oracle is not trivially
  `true`. — `[measured: read crates/gen/src/phase5.rs:419–435, 486–532 → these
  four are value-agnostic or assert false on non-lappable fixtures]`

## Open questions

None blocking. The round-1 spec Open-questions (velocity domain, container) were
resolved for the substrate. The round-2 Open-question — **which function carries
the chord-extent bound and how the along-chord span is derived** — is resolved
here: `register_move` carries the bound; `lat_coord`/`crossing_within_span`
(`const fn`) implement it via the perpendicular-of-`forward` projection of
`sf.gate.behind`, with the crossing point tested by integer interpolation against
the `behind` cross-section's along-chord span. `gate_coord`, `crossing_event`, and
`legal_move` are unchanged.
