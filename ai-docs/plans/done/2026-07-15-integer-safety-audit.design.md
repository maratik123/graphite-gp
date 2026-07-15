# Design: Integer overflow- and signedness-safety audit

**Issue:** #48
**Date:** 2026-07-15

## Approach

### Ground truth (verified against source, not the spec's approximate inventory)

I enabled `arithmetic_side_effects = "deny"` in the root `[workspace.lints.clippy]`
table and ran the real gate (`cargo clippy --workspace --all-targets -- -D warnings`)
after a clean rebuild, then reverted the Cargo.toml. The lint flags **exactly 22
production sites, all in `gp-core`**, and **zero test-target sites** (the `gp-core
(lib test)` target reports the *same* 22 production sites — no test-only site). The
scaffold crates (`gen`/`render`/`ai`/`game`) produce **no** flagged sites. Full
enumeration (used as the binding contract below):

| Site (file:line:col) | Function | Op | Disposition |
|---|---|---|---|
| `geom/mod.rs:40:23`,`41:23`,`42:31`,`43:31` | `Point::neighbors4` | `self.x ± 1`, `self.y ± 1` (i32) | **(a)** `saturating_add(1)` / `saturating_sub(1)` |
| `geom/mod.rs:101:9` | `Size::area` | `width * height` (usize) | **(b)** retain + `#[allow]` |
| `geom/mod.rs:146:65` | `Rect::index` | `dy * width + dx` (usize) | **(b)** retain + `#[allow]` |
| `geom/mod.rs:176:52`,`176:67` | `Rect::on_border` | `dx + 1 == w`, `dy + 1 == h` (usize) | **(b)** retain + `#[allow]` |
| `geom/mod.rs:295:14`,`296:14`,`297:17`,`301:22`,`302:16` | `supercover` | i64 `-`,`+`,`*`,`-`,`*` | **(b)** retain + `#[allow]` |
| `sim.rs:85:23`,`85:34`,`86:25`,`86:36` | `legal_move` | `vx+ax`,`vy+ay`,`x+vx2`,`y+vy2` (i32) | **(a)** `checked_add` → `false` |
| `geom/graph.rs:283:40`,`283:53` | `walls_from_boundary` | `cell.x+dx`, `cell.y+dy` (i32) | **(a)** `checked_add` → off-grid ⇒ emit wall |
| `geom/graph.rs:111:13` | `component_count` | `count += 1` (usize) | **(b)** retain + `#[allow]` |
| `geom/graph.rs:134:13` | `bounded_complement_components` | `count += 1` (usize) | **(b)** retain + `#[allow]` |
| `geom/graph.rs:246:13` | `CorridorScratch::geodesic_bfs` | `distance += 1` (usize) | **(b)** retain + `#[allow]` |

**Split:** convert (a) = 10 sites / 3 functions; retain (b) = 12 sites / 7 functions
→ **7 `#[allow(clippy::arithmetic_side_effects, reason = "…")]`** attributes (one per
retained function; `supercover`'s single fn-level allow covers all 5 of its sites,
`on_border`'s covers 2).

### Spec-inventory corrections (verified against real code)

- The method is `Point::neighbors4`, not `neighbours`; it is a `pub const fn`.
- `Size` fields are **`usize`**, not `u32` — `area` is a `usize` multiply.
- Spec row "`Rect::far_corner` (~L161–162)" is actually **`Rect::points()`** (L159–164);
  it already uses `i32::try_from` + `saturating_add` and is **not** among the 22 flagged
  sites — no action, already compliant. (A method call, `saturating_add`, is not a raw
  operator, so the lint does not fire.)
- `Rect::index`'s conversion (`offset`: `checked_sub` + `usize::try_from`, L131–136) is
  **not** flagged — only the residual `dy * width + dx` mul/add is (L146).
- `GenParams::min_width`'s `div_ceil` (gen/lib.rs) is a method call, not a raw op — not
  flagged, no change. Confirms AC5.
- **No `as` numeric casts** and **no pre-existing `#[allow(clippy::cast_*/arithmetic_side_effects)]`**
  exist anywhere in the workspace (verified `rg` over `crates/` incl. tests). So AC1/AC2's
  cast clause is **vacuously satisfied** — there is nothing to migrate from `as` to `try_from`.

### Per-site method choice (rule intent → method)

- **`Point::neighbors4` → `saturating_*` (a).** A public `const fn` that must be
  panic-free on adversarial coords (AC4). At `i32::MAX` the east neighbour and at
  `i32::MIN` the west/south neighbour have no representable "+1" — `saturating_add(1)`/
  `saturating_sub(1)` clamp to the grid-representable range, yielding a degenerate
  *self*-neighbour. Every caller (`flood_component`, `geodesic_bfs`) filters neighbours
  through `Corridor::index`/`stamp`/`visited`, so a self-neighbour is harmlessly skipped
  — no infinite loop, no incorrect flood/BFS (the true off-grid neighbour is `∉ D`
  anyway, so flooding correctly does not expand across it). `saturating_add`/`_sub` are
  const-callable on the pinned 1.97.0 toolchain (verified), so `neighbors4` **stays
  `const fn`**. In-domain (all existing coords ≪ `i32::MAX`) saturating is bit-identical
  to raw `±1` — behaviour-preserving.
- **`sim::legal_move` → `checked_add` → `false` (a).** A target position that overflows
  `i32` is outside *any* corridor's bounding box (`∉ D`), so the move is illegal.
  `checked_add` on `(vx+ax, vy+ay)` then `(x+vx2, y+vy2)`; on any `None`, `return false`.
  This is the intent-correct mapping (`checked_*` → out-of-range ⇒ the `None` branch),
  panic-free (AC4), and physics-preserving (an in-`D`-reachable move never overflows).
- **`walls_from_boundary` → `checked_add`, off-grid ⇒ emit wall (a).** At a pathological
  box origin of `i32::MIN`, a drivable cell's west/south neighbour underflows `i32`. The
  neighbour is then off the representable grid ⇒ `∉ D` ⇒ the cell borders "outside" on
  that side ⇒ a boundary wall **must** be emitted. So the checked form maps `None` to
  *emit wall*, not *skip*. **This is why `walls_from_boundary` must NOT reuse the
  saturating `neighbors4`:** a saturated self-neighbour would test `d.contains(self)` =
  `true` and *suppress* the wall — an incorrect result. The distinction (saturating is
  safe for flood/BFS, but wall-derivation needs the checked off-grid⇒wall mapping) is the
  key correctness point of this task.
- **`supercover` → retain (b).** The reference bounded-chord pattern (design doc §3 C4,
  §3a). Operands are widened `i32→i64` (2³² of headroom) and the doc already states the
  precondition (`|v| ≪ 1.5×10⁹`, `|cr| ≤ 2·|dx|·|dy|`). One fn-level `#[allow]` + `reason`.
- **`Size::area`, `Rect::index` → retain (b).** Both products are bounded by the box cell
  count (`area`), which is itself bounded by allocatability (`Corridor` could not allocate
  its `Vec<bool>` for a larger count). `Rect::index`'s product is additionally guarded by
  the immediately-preceding `dx < width && dy < height`, so `dy*width + dx < area`.
  Converting `area` to `checked_mul`/`Option` would ripple the signature through
  `Corridor::new`/`CorridorScratch::new` for no in-domain benefit — retain matches the
  already-documented pattern and the spec's "confirm the bound" guidance.
- **`component_count`, `bounded_complement_components`, `geodesic_bfs` counters → retain
  (b).** `count`/`distance ≤ cell count ≤ area` (allocated), so `+= 1` cannot overflow in
  domain. Rejected alternative: rewriting the two `component_count`-family loops as
  `.filter(…).count()` (which would drop the `#[allow]`); rejected because it restructures
  working, tested code for marginal gain, has **no** analogue for the imperative
  `distance += 1` BFS-layer counter, and the audit's principle is "harden *how*, not
  *what*" with minimal diff.

### Enforcement

Add `arithmetic_side_effects = "deny"` to the root `[workspace.lints.clippy]` table
(AC7). A specific restriction lint at default priority sits above the `priority = -1`
group denies, so it activates without conflict, matching the existing `deny` posture
(`missing_docs`, `large_stack_frames`) and the CI `-D warnings` floor.

### Ordering rationale

The lint is enabled **last** (final task) so no intermediate committed state is red.
Adding a `#[allow]` for a not-yet-active lint is a silent no-op (`#[allow]`, unlike
`#[expect]`, does not warn when unfulfilled; `clippy::allow_attributes` is not in the
denied pedantic/nursery groups), so the retain-(b) allows can land before enablement.
The final task re-runs the full gate as the completeness certifier.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | `Point::neighbors4`: convert 4 raw `±1` ops to `saturating_add(1)`/`saturating_sub(1)` (keep `const fn`). TDD: first add adversarial test (`neighbors4` at `i32::MAX`/`i32::MIN` does not panic and returns the saturated set), which panics red under raw `±1`, then apply the fix → green. | `crates/core/src/geom/mod.rs` | — |
| 2 | `sim::legal_move`: convert 4 raw i32 adds to `checked_add`; on any `None`, `return false`. TDD: adversarial test first (`CarState` at `i32::MAX`/`i32::MIN` coord/velocity with an overflowing accel → `legal_move`/`legal_mask` return `false`, no panic) — panics red today → green after fix. | `crates/core/src/sim.rs` | — |
| 3 | `walls_from_boundary`: convert 2 raw i32 adds to `checked_add`; map off-grid neighbour (`None`) to *emit wall*. TDD: adversarial test first (drivable cell at `i32::MIN` origin emits its west/south boundary walls without panic) — panics red today → green after fix. Do NOT reuse the saturating `neighbors4` here. | `crates/core/src/geom/graph.rs` | — |
| 4 | Retain-(b) in `geom/mod.rs`: add `#[allow(clippy::arithmetic_side_effects, reason = "…")]` to `Size::area`, `Rect::index`, `Rect::on_border`. **Doc-comments (NOTE 2):** extend `Rect::index`'s doc with the `dx<width && dy<height ⇒ product < area` bound, and — because `Size`/`Rect` are public, standalone-constructible types (tests build them via struct literal, so a `Size { width: usize::MAX, .. }` / oversized `Rect` is representable without a `Corridor`) — give **both** `Size::area` and `Rect::index` explicit supercover-style domain wording: the bound holds for grid-realistic (`Corridor`-backed / allocatable) dimensions, and adversarially-large, unallocatable dimensions near `usize::MAX` lie **outside the documented domain and are unsupported**. Refine `Size::area`'s / `Rect::on_border`'s existing bound wording to match. **Tests (NOTE 1 — mandatory, not conditional):** in addition to the existing `size_area_*` / `rect_index_in_box_is_row_major` / `rect_on_border_*`, add at least one **large-but-in-domain** `area` case and one large `index` case (see Test Design for concrete portable dims + precomputed-literal expected values). | `crates/core/src/geom/mod.rs` | — |
| 5 | Retain-(b) for `supercover`: add one fn-level `#[allow(clippy::arithmetic_side_effects, reason = "…")]`; confirm the bounded-chord precondition doc (already present, §3 C4) and that the existing case-table tests exercise the integer cross-product path. Record that the i64-overflow bound is a documented precondition, not a feasibly test-drivable panic (see Risks). | `crates/core/src/geom/mod.rs` | — |
| 6 | Retain-(b) in `geom/graph.rs`: add `#[allow(clippy::arithmetic_side_effects, reason = "count/distance ≤ cell count ≤ area")]` to `component_count`, `bounded_complement_components`, and `CorridorScratch::geodesic_bfs`; add a one-line `// bound:` comment at each `+= 1`. Confirm existing tests cover the increments. | `crates/core/src/geom/graph.rs` | — |
| 7 | Enable `arithmetic_side_effects = "deny"` in the root `[workspace.lints.clippy]` table. Run the full gate: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`. Confirm 0 residual arithmetic sites and that scaffolds (`gen`/`render`/`ai`/`game`) stay clean. If any new same-class site surfaces (e.g. a fix or a new test introduced variable-operand arithmetic), STOP and surface it to the orchestrator rather than absorbing it. | `Cargo.toml` | 1,2,3,4,5,6 |

## Handoff plan

- **(a) Grouping required:** `M = 7 ≥ 1`, so this `## Handoff plan` is mandatory; the one
  group below is also terminal.
- All 7 subtasks are the **code** change-type (Rust `*.rs` plus the workspace `Cargo.toml`
  build manifest — edited by the code implementer and verified by the same
  clippy/test/doc gate; not instructions/harness `*.md`/`.claude/**`/`AGENTS.md`/
  `ai-docs/**`). One homogeneous change-type ⇒ **one group** (group-minimization: the
  fewest groups possible, bounded by the `≤ 10` size cap and dependency order — here all 7
  fit one group).

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, 1M-token window —
  subtasks **1–7** (code change-type). **Terminal group** (7 subtasks; within the `1..=10`
  range, ≤ the size cap of 10). **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). There is no
  second group, so no inter-group handoff; the single group completes /task Step 8 in its
  own `/context-reset` subagent.

- **(g) marking:** code group → `sonnet` / effort `medium` (pinned) / 1M window. The
  `design`, `design-review`, `self-review`, and `spec-writer` subagents stay on Opus
  regardless — only the implementor model/effort is pinned here.
- **(h) max-groups:** 1 group ≤ the default cap of 4 — no user gating needed.

## Risks

- **New test-code raw arithmetic trips the lint.** *Verified:* variable-operand arithmetic
  (e.g. `some_var + 0`) is flagged in **test** targets too, but **const-operand** arithmetic
  (`i32::MAX - 1`) is constant-folded and **not** flagged. *Mitigation:* the new adversarial
  tests must express expected saturated/checked values with const-operand arithmetic
  (`i32::MAX - 1` is safe) or precomputed literals — never `var ± n`. If a test genuinely
  needs a variable-operand computation, add a justified `#[allow(clippy::arithmetic_side_effects,
  reason = "…")]` on that test (AC7 permits test-target allows). This is the single most
  likely way to inadvertently introduce a 23rd flagged site.
- **Saturating `neighbors4` self-neighbour at extremes.** *Mitigation:* every caller filters
  through `index`/`stamp`/`visited`, so a self-neighbour is skipped — covered by an
  adversarial test asserting the saturated set plus the existing flood/BFS tests (unchanged).
- **`walls_from_boundary` must not adopt saturating `neighbors4`.** A saturated self-neighbour
  would suppress a real boundary wall. *Mitigation:* task 3 uses `checked_add` with
  off-grid⇒emit-wall mapping and an explicit adversarial test at an `i32::MIN` origin.
- **`supercover`'s i64-overflow bound is not feasibly test-drivable.** Reaching i64 overflow
  needs `|dx|,|dy| ~ 10⁹`, and `supercover` scans the full `min..=max` bounding box
  (~10¹⁸ cells) — impossible to run. *Mitigation:* keep the documented precondition (§3 C4)
  as the safety basis; the existing case-table tests exercise the exact
  `2·|cr| ≤ |dx|+|dy|` integer arithmetic for correctness. Called out as an Open question.
- **A fix accidentally alters physics semantics.** *Mitigation:* every convert-(a) method
  is bit-identical to raw arithmetic for all in-`D` inputs (overflow occurs only for
  out-of-`i32`-range targets that are `∉ D` anyway); the spec's behaviour-preservation
  constraint means all existing tests must pass unchanged — task 7's `cargo test` is the check.
- **Hard-error gate could mask later same-class sites.** The 22-site enumeration was taken
  with `-W` (non-aborting), so it is complete; task 7 re-runs with `-D` and STOPs+surfaces
  any newly revealed site instead of silently absorbing it.

## Test Design

- **`Point::neighbors4`** (`crates/core/src/geom/mod.rs` `#[cfg(test)]`)
  - Entry point: `Point::neighbors4`.
  - Scenarios: (happy, existing/implicit) small coords unchanged vs raw `±1`; (edge, new,
    AC4) `Point::new(i32::MAX, i32::MAX).neighbors4()` and `Point::new(i32::MIN, i32::MIN)
    .neighbors4()` do not panic and return the saturated neighbour set (east/north clamp
    to `i32::MAX`, west/south to `i32::MIN`; the non-saturating axis uses `i32::MAX - 1` /
    `i32::MIN + 1` — const-operand expected values, not flagged).
  - Fixtures: none.
- **`sim::legal_move` / `legal_mask`** (`crates/core/src/sim.rs` `#[cfg(test)]`)
  - Entry point: `legal_move` (and `legal_mask` map).
  - Scenarios: (edge, new, AC4) a `CarState` at `x/y = i32::MAX` (or `i32::MIN`) with a
    velocity+accel that would overflow the target coordinate → `legal_move` returns `false`
    for every `Action` without panic; `legal_mask` returns `[false; 5]`. Use a minimal
    `Corridor` (the overflowing target is `∉ D` regardless).
  - Fixtures: a small `Corridor` built via `Corridor::new` + `set`.
- **`walls_from_boundary`** (`crates/core/src/geom/graph.rs` `#[cfg(test)]`)
  - Entry point: `walls_from_boundary`.
  - Scenarios: (edge, new, AC4) a `Corridor` whose origin is `i32::MIN` with one drivable
    cell at `(i32::MIN, i32::MIN)` → the west and south walls are emitted (off-grid neighbour
    ⇒ boundary) without panic; (happy, existing) `walls_of_solid_2x2_block_*` and
    `walls_of_ring_*` remain byte-identical.
  - Fixtures: reuse the existing `corridor(origin, w, h, drivable)` helper (it already
    accepts an arbitrary origin `Point`).
- **Retain-(b) `Size::area` / `Rect::index` large-in-domain cases (NOTE 1 — MANDATORY)**
  (`crates/core/src/geom/mod.rs` `#[cfg(test)]`). The existing `size_area_*` /
  `rect_index_in_box_is_row_major` cases use only tiny 3×4-class dims, which do not
  "exercise the bound" AC3 asks for. Add, unconditionally:
  - `Size::new(50_000, 50_000).area()` → `2_500_000_000` (a genuinely large product,
    ~2×10⁸× the 3×4 fixture, yet safely below `usize::MAX` on **32-bit targets too**
    — `usize::MAX ≥ 4_294_967_295 > 2.5×10⁹` — so the case is portable and never itself
    overflows).
  - `Rect { origin: (0,0), size: Size::new(50_000, 50_000) }.index(Point::new(12_345, 49_999))`
    → `Some(2_499_962_345)` (drives a large `dy*width` product: `49_999*50_000 = 2_499_950_000`,
    `+ 12_345`), plus a small in-box sanity cell (e.g. `index(Point::new(1, 1)) == Some(50_001)`).
  - **Expected values are precomputed integer literals** (`2_500_000_000`, `2_499_962_345`,
    `50_001`) — never `var ± n` — so the test body introduces no new flagged
    variable-operand arithmetic site (per the verified test-target caveat). The implementer
    MAY choose different dims, but MUST keep the product `< ~4×10⁹` (32-bit-portable) while
    remaining ≫ the existing tiny fixtures, and MUST express expected values as literals.
- **Other retain-(b) sites** (`Rect::on_border`, `supercover`, `component_count`,
  `bounded_complement_components`, `geodesic_bfs`): AC3 is satisfied by the already-present
  doc bounds + existing tests (`rect_on_border_*`, the full `supercover` case table,
  `component_count_*`, `*_bounded_hole*`, `geodesic_*`). No adversarial/overflow test is
  added for these (their safety is the documented bound, not a runtime panic).

## Open questions

- **`supercover` AC3/AC4 coverage.** A test that actually drives the i64 cross-product to
  its overflow boundary is computationally infeasible (~10¹⁸-cell scan). Proposed
  resolution: the documented bounded-chord precondition (design doc §3 C4) + the existing
  integer case-table tests satisfy AC3/AC4 for this site; no giant-chord test is added.
  Flag for the product owner / reviewer only if stronger evidence than the precondition doc
  is required.
