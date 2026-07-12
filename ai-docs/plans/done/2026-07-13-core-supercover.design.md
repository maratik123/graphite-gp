# Design: gp-core exact integer supercover predicate

**Issue:** #4
**Date:** 2026-07-13

## Approach

Replace the `todo!()` stub in `crates/core/src/geom.rs::supercover` with an exact
integer predicate built on the **Separating-Axis Theorem (SAT) for a segment vs an
axis-aligned unit square**, evaluated per candidate cell over the segment's bounding
box. The whole computation is a single uniform inequality — no per-orientation
special cases — which is the decisive property for the codebase's most
correctness-critical predicate (`docs/design.md` §3 C4: "off-by-one here silently
allows illegal diagonal slips OR forbids legal moves, breaking validation and the
game identically").

### Algorithm

Endpoints `a`, `b` are integer cell centers (`Point`, `Coord = i32`). Each cell `c`
covers the **closed** unit square `[c.x ± ½] × [c.y ± ½]` (spec Key decisions;
`docs/design.md` §1). Scan every cell in the inclusive bounding box
`[min(a.x,b.x), max(a.x,b.x)] × [min(a.y,b.y), max(a.y,b.y)]` and include cell `c`
iff its closed square meets the closed segment.

Let `dx = b.x − a.x`, `dy = b.y − a.y`, and for a cell center `c` let the integer
cross product `cr = dx·(c.y − a.y) − dy·(c.x − a.x)` (= `(b−a) × (c−a)`). Then:

```
supercover(a, b):
    dx = i64(b.x) - i64(a.x)          # widen BEFORE subtracting (see Overflow)
    dy = i64(b.y) - i64(a.y)
    result = Vec::new()
    for cx in min(a.x,b.x) ..= max(a.x,b.x):
        for cy in min(a.y,b.y) ..= max(a.y,b.y):
            cr = dx*(i64(cy) - i64(a.y)) - dy*(i64(cx) - i64(a.x))
            if 2*cr.abs() <= dx.abs() + dy.abs():
                result.push(Point::new(cx, cy))
    return result
```

### Why this inequality is exact (SAT derivation)

For two closed convex sets, non-intersection requires a separating axis among the
faces' normals. The candidate axes for *segment vs AABB* are the box's `x`- and
`y`-axes plus the segment normal `n = (−dy, dx)`.

- **`x` / `y` axes are auto-satisfied by the bbox restriction.** A scanned cell has
  `min(a.x,b.x) ≤ c.x ≤ max(a.x,b.x)`; since `c.x` is an integer and the square's
  half-width is ½, this is exactly the condition that the segment's `x`-projection
  overlaps the cell square's `x`-projection (same for `y`). No cell outside the bbox
  can be touched (a touched cell's center lies within ½ of a segment point, and
  integrality pulls it back inside `[min,max]`), so the bbox is both necessary and
  sufficient for these two axes.
- **Segment-normal axis.** The entire segment projects onto `n` to the single scalar
  `n·a` (because `n ⊥ (b−a)`). The cell square projects to
  `[n·c − r, n·c + r]` with radius `r = ½(|dx| + |dy|)`. They are **separated** iff
  `|n·(c − a)| > r`. Since `n·(c − a) = dx·(c.y−a.y) − dy·(c.x−a.x) = cr`, separation
  is `|cr| > ½(|dx|+|dy|)`, i.e. **intersection ⟺ `2·|cr| ≤ |dx| + |dy|`**. Equality
  is a boundary/corner touch → included (closed square). The degenerate `dx=dy=0`
  gives `0 ≤ 0` for the single bbox cell `{a}`.

This inequality was validated against the full §3 C4 case table and a 625-pair
symmetry sweep before adoption (see Test Design). It uniformly yields: both
endpoints (each has `cr = 0`), the 4-cell dual-vertex tie (all four corners give
`cr` straddling the bound), corner grazes (`cr` hits the bound exactly), axial runs
(single-row/column bbox, no diagonal neighbours), and the degenerate case.

### Rejected alternatives

- **DDA / Amanatides–Woo supercover walk** (`O(|dx|+|dy|)`, linear in the touched
  set). Rejected for round 1: getting the dual-vertex 4-cell inclusion and
  corner-graze ties exactly right requires explicit "line hits a lattice corner"
  special-casing — precisely the off-by-one surface §3 C4 warns is
  correctness-fatal. The SAT scan has zero special cases. The exact-set test table
  pins the observable contract, so a DDA variant can replace the body later **behind
  the same signature and tests** if profiling ever demands it (see Risks).
- **`BTreeSet`/`HashSet` return type.** Rejected — see Key decisions (container).
- **`gcd`-based dual-vertex enumeration** (the spec's illustrative hint). The
  "doubled coords / cross-products / gcd" hint is an `e.g.`, not a mandate; SAT
  already satisfies its intent (integer-exact via a cross product). `gcd` would only
  enumerate dual-vertex crossings and still needs separate handling for
  between-crossing and edge-graze cells — more moving parts, no correctness gain.

### Key decisions (resolving both spec Open questions)

| Question | Decision | Rationale |
|---|---|---|
| **Return container & duplicate guarantee** (spec OQ1) | Keep `Vec<Point>` (unchanged stub signature); each bbox cell is pushed exactly once. | The bbox double-loop visits every `(cx,cy)` once → structurally duplicate-free (AC6) with no dedup pass. The caller `sim.rs::legal_move` does `supercover(...).iter().all(...)` — works on `Vec` verbatim, **zero caller edits**. A `HashSet` would add allocation/hashing for a small collection consumed by one `.all()` scan (YAGNI). The guarantee lives in the algorithm, not the type. |
| **Internal integer width** (spec OQ2) | `i64`, widening each `Coord` with `i64::from(...)` **before** any subtraction/product. | `b.x − a.x` can itself overflow `i32` for far-apart points, so widen first. Both cross-product operands are *relative to `a`* and bounded by the chord's per-axis speed (`|c.x−a.x| ≤ |dx|`, `|c.y−a.y| ≤ |dy|`), so `|cr| ≤ 2·|dx|·|dy|` and `2·|cr| ≤ 4·|v|²`; `i64` stays safe up to `|v| ≈ 1.5×10⁹` — a velocity component unreachable in this game (a move is one velocity vector; oracle `V_ceil` is bounded by track geometry, `docs/design.md` §3). No `i128` needed. `i64::from` is a lossless **integer** widening (not the float cast AC4 forbids), and avoids clippy's `cast_lossless`/`cast_possible_truncation` lints. |

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `#[cfg(test)] mod tests` to `geom.rs` with the full §3 C4 case table (see Test Design), a `HashSet<Point>` set-compare helper, symmetry checks, an explicit duplicate-free assertion, and the degenerate case. Tests fail (RED) against the current `todo!()`. | `crates/core/src/geom.rs` | — |
| 2 | Implement `supercover`: replace `todo!()` with the bbox-scan + `2·\|cr\| ≤ \|dx\|+\|dy\|` inequality in `i64` (via `i64::from`), pushing each cell once into `Vec<Point>`. Rewrite the `///` doc — drop the `TODO(3a)` line; state, as the doc-comment bullet plan, the closed-square contract, dual-vertex tie, integer-exactness, order-independence, duplicate-freeness, **and the endpoint-separation overflow precondition** — the doc states `supercover` assumes endpoints separated by a bounded chord (one move's velocity, `\|v\| ≪ 1.5×10⁹`), under which the `i64` cross product `dx·(c.y−a.y) − dy·(c.x−a.x)` never overflows; adversarial full-range `i32` endpoints lie outside the documented domain, so the i64 no-overflow contract is visible at the call site; cite `docs/design.md` §3 C4 (pointer-only, no inlined rules). Tests pass (GREEN). | `crates/core/src/geom.rs` | 1 |
| 3 | Run the gate suite — `cargo test -p gp-core`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` — and fix any lint/format/doc issues on the new code. Confirms AC7. | `crates/core/src/geom.rs` | 2 |

Scope = 3 subtasks in one file; no issue split needed (well under the 7-task ceiling).

## Handoff plan

Grouping is mandatory for **every M ≥ 1** (this design has M = 3). Non-terminal
groups MUST be exactly **3 consecutive subtasks**; the terminal group may hold
**1..=3**. Every group boundary — including entry into the first group — spawns a
`/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery
(re-entry).

- **Group A:** subtasks 1–3 — terminal group (3 subtasks; within the 1..=3 range).
  Entry into Group A runs under its own `/context-reset` subagent. There is only one
  group, so there is **no inter-group handoff**; the single group completes /task
  Step 8 inside that `/context-reset` subagent.

## Risks

- **Overflow on large chords.** *Mitigation:* `i64` with `i64::from` widening before
  arithmetic; operands are relative to `a` (bounded by speed), safe to `|v| ≈ 1.5×10⁹`
  (see Key decisions). No realistic move or oracle edge approaches this.
- **AC4 "no casts" read literally.** AC4 bans *floating-point* operations/casts.
  `i64::from` is integer widening, not `as f64`; there is no `f32`/`f64`/`as`-float
  anywhere. *Mitigation:* documented in the `///` and this design so review doesn't
  misread the widening as a prohibited cast.
- **Performance: bbox scan is `O(|dx|·|dy|)`** while the touched set is `O(|dx|+|dy|)`
  — quadratic in speed. *Mitigation:* per-call chord length is one velocity vector,
  bounded by achievable speed (single/low-double digits at runtime; oracle `V_ceil`
  bounded by geometry). The exact-set test table pins the contract, so a linear DDA
  walk can replace the body later with no observable change if profiling flags it.
- **Clippy `-D warnings` on new code.** The `cx/cy` loop vars are used as coordinates
  (not slice indices) → `needless_range_loop` does not fire; `i64::from` avoids
  `cast_*` lints. Subtask 3 re-runs the full gate to catch any residual lint
  (the gate aborts on first failure, so re-run after each fix until clean).
- **Caller compatibility.** Keeping `Vec<Point>` leaves `sim.rs::legal_move`
  (`supercover(...).iter().all(...)`) unchanged — verified against `sim.rs:84`. No
  edits outside `geom.rs`.

## Test Design

- **Location:** `crates/core/src/geom.rs`, `#[cfg(test)] mod tests` (unit tests, same
  file — AGENTS.md Rust Test Conventions).
- **Entry point:** `geom::supercover`.
- **Fixtures / helpers:**
  - `fn cover_set(a: Point, b: Point) -> std::collections::HashSet<Point>` =
    `supercover(a, b).into_iter().collect()`. `Point` is `Eq + Hash`, so
    `HashSet<Point>` compares with `assert_eq!` and normalizes away iteration order
    (spec: compare as sets). No `Ord`/`BTreeSet` needed — **do not** add derives to
    `Point` (out of scope).
  - Small `Point::new` literals; expected sets via `HashSet::from([...])`.
- **Scenarios (each asserts the exact expected set, not just its size):**

  | Test | Segment | Expected cell set | Covers |
  |---|---|---|---|
  | `axial_horizontal_no_diagonal` | `(0,0)→(3,0)` | `{(0,0),(1,0),(2,0),(3,0)}` | AC3 |
  | `axial_vertical_no_diagonal` | `(0,0)→(0,3)` | `{(0,0),(0,1),(0,2),(0,3)}` | AC3 |
  | `dual_vertex_diagonal_all_four` | `(0,0)→(1,1)` | `{(0,0),(1,0),(0,1),(1,1)}` | AC2 |
  | `dual_vertex_symmetric` | `(1,1)→(0,0)` | same four | AC1/AC2 symmetry |
  | `primitive_slope_gcd1` | `(0,0)→(2,1)` | `{(0,0),(1,0),(1,1),(2,1)}` (excludes `(2,0),(0,1)`) | AC1 |
  | `collinear_dual_vertices_gcd2` | `(0,0)→(2,2)` | `{(0,0),(1,0),(0,1),(1,1),(2,1),(1,2),(2,2)}` (7; excludes `(2,0),(0,2)`) | AC1/AC2 (two collinear ties) |
  | `single_corner_graze` | `(1,0)→(0,1)` | `{(0,0),(1,0),(0,1),(1,1)}` (four around `(½,½)`) | AC1 corner graze |
  | `long_diagonal_three_vertices` (recommended extra) | `(0,0)→(3,3)` | `{(0,0),(0,1),(1,0),(1,1),(1,2),(2,1),(2,2),(2,3),(3,2),(3,3)}` (10) | AC1 reinforcement |
  | `degenerate_single_cell` | `(2,2)→(2,2)` | `{(2,2)}` | AC5 |

  - **Spec "two collinear dual vertices" case (AC2):** the existing
    `collinear_dual_vertices_gcd2` row (`(0,0)→(2,2)`, through `(½,½)` and
    `(1½,1½)`) already satisfies the spec's required "chord through two collinear
    dual vertices" case — both ties' 4-cell sets appear in the 7-cell expected set —
    so no additional test is added.
  - **Duplicate-freeness (AC6):** in at least the dual-vertex case, assert
    `let v = supercover(a,b); v.len() == v.iter().collect::<HashSet<_>>().len()` so a
    double-push would fail (the `HashSet` compare alone would hide it).
  - **Endpoints present (AC5):** the degenerate case plus every set above already
    contains both endpoint cells; optionally add a direct
    `assert!(cover_set(a,b).contains(&a) && ...contains(&b))` on one long chord.
  - **Order-independence / symmetry (AC1):** assert `cover_set(a,b) == cover_set(b,a)`
    for a handful of the pairs above (the `(1,1)` reverse row already does one).
  - **AC4 (integer-only)** is a static/inspection criterion (no `f32`/`f64`/float
    `as` in the body), enforced by review + clippy — **not** a runtime assertion.

## Open questions

None remaining. Both spec Open questions are resolved in Key decisions above —
container = `Vec<Point>` (duplicate-free by construction, caller unchanged), internal
width = `i64` via `i64::from`. Neither was design-blocking.
