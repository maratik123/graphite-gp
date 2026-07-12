# gp-core exact integer supercover predicate

**Source:** issue #4
**Date:** 2026-07-13
**Tracked in:** #4

## Scope

Implement the strict, corner-aware integer `supercover(a, b)` in `crates/core/src/geom.rs`
(crate `gp-core`), replacing the existing `todo!()` stub, and ship the full design-doc §3
C4 case table as unit tests.

1. **Definition.** `supercover(a, b)` returns the set of every cell whose **closed** unit
   square (the cell plus its boundary) the closed segment `a → b` touches. Cell `c` (an
   integer point / cell center) covers the region `x ∈ [c.x − ½, c.x + ½]`,
   `y ∈ [c.y − ½, c.y + ½]`. Corner-clipped cells (segment touching only a cell's corner or
   edge) are included.
2. **Dual-vertex rule.** When the segment passes through a dual vertex — a half-integer
   point `(i + ½, j + ½)` shared by 4 cells — **all 4** cells are included. This is the
   correctness-critical tie case: it forbids the diagonal "needle's-eye" slip between two
   diagonally placed walls and stops a fast chord from jumping a wall. Canonical instance:
   `(0,0) → (1,1)` passes through `(½,½)` and must return all of `{(0,0),(1,0),(0,1),(1,1)}`.
3. **Endpoints.** Both `a` and `b` are always in the result (their cells are trivially
   touched). A degenerate `a == b` returns exactly `{a}`.
4. **Symmetry / order-independence.** As a set, `supercover(a, b) == supercover(b, a)`; the
   result is defined up to set equality (caller order and any internal iteration order are
   irrelevant — see the `legal_move` / oracle consumption below).
5. **Integer-only.** The whole computation uses integer arithmetic; no floating-point
   values, casts, or intermediates appear at any step (design doc §3a core invariant). The
   half-integer geometry above is conceptual — implement it with exact integers (e.g. work
   in a doubled coordinate system, cross-products, gcd), never with fractional numbers.

The predicate is used **verbatim** as both the runtime move-legality rule (`legal_move`,
already wired in `crates/core/src/sim.rs`: `supercover(s.pos(), p1).iter().all(|&c| d.contains(c))`)
and the passability-oracle graph edge — one implementation, two call sites.

## Out of scope

- `legal_move`, `legal_mask` — already implemented in `sim.rs`; they consume `supercover`
  unchanged. No edits needed beyond what the return type (if changed) forces.
- `Corridor::contains` / the `D` membership machinery — already present in `geom.rs`.
- The oracle BFS, `step`, lap counter, crash, collision resolution — later Block-3a tasks;
  this task delivers only the predicate they will call.
- Chord ↔ S/F crossing test (§3 C2) — separate predicate, separate task.

## Deferred

- (none) — the predicate is self-contained; nothing spun off to a separate issue.

## Key decisions

| Question | Decision |
|---|---|
| Coordinate model | `Point` = integer cell center (`Coord = i32`); cell `c`'s closed square is `[c.x±½] × [c.y±½]`. Dual vertices sit at `(i+½, j+½)`. Fixed by design doc §1/§3 and existing `geom.rs` types. |
| Closed vs half-open square | **Closed** (cell + boundary), per §3 C4. Boundary/corner touches count — this is what makes the predicate strict. |
| Float ban | Integer-only, no floats anywhere (design doc §3a; AGENTS.md § Code Style). Applied silently; also an explicit AC. |
| Return container | Starting point is the existing stub signature `pub fn supercover(a: Point, b: Point) -> Vec<Point>`; the caller treats it as a set. The concrete container and any dedup/no-duplicate guarantee are a design/impl choice (§ Open questions) — the observable contract is "exactly the cell set", i.e. no duplicates and no spurious cells. |
| Internal integer width | Cross-products / doubled coords for large chords can exceed `i32`; widen internally (e.g. `i64`) as needed to stay overflow-free at plausible track scales. Still integer-only. Exact type left to design. |
| Test placement | Full C4 case table as `#[cfg(test)] mod tests` in `geom.rs` (unit tests, same file — AGENTS.md Rust Test Conventions). Assert exact sets, not sizes. |

## Technical constraints

- Rust, crate `gp-core`, edit `crates/core/src/geom.rs` only (plus its in-file test module).
- Strict clippy (`-D warnings`); `cargo fmt`; every public item keeps its `///` doc.
- Deterministic: identical output for identical (unordered) endpoints, every run.
- Result compared as a set in tests — normalize (sort / collect into a set) before asserting,
  since iteration order is not part of the contract.
- No new dependencies required for the core algorithm.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | For any integer endpoints, `supercover` returns exactly the corner-aware closed-square cell set — every cell whose closed unit square the segment touches, and no others. Order-independent up to set equality. |
| AC2 | `supercover((0,0),(1,1))` returns all four of `{(0,0),(1,0),(0,1),(1,1)}` (dual-vertex `(½,½)` tie case), and symmetrically for `(1,1)→(0,0)`. |
| AC3 | Axial segments (horizontal and vertical) return only the straight run of cells between the endpoints — no spurious diagonal neighbours. |
| AC4 | The implementation performs only integer arithmetic — no floating-point operations, casts, or intermediates anywhere. |
| AC5 | The result includes both endpoint cells; `a == b` returns exactly `{a}`. |
| AC6 | The result contains no duplicate cells and no cell outside the closed-square set (the returned collection is exactly the set, matching the caller's set semantics). |
| AC7 | The full §3 C4 case table ships as in-file unit tests asserting exact expected cell sets (see Test notes); `cargo test` passes and `cargo clippy --workspace --all-targets -- -D warnings` is clean. |

## Test notes

Ship the complete C4 case table as unit tests, each asserting the exact expected cell set
(not just its size); compare as sets. Required cases:

- **Axial horizontal** — e.g. `(0,0)→(3,0)` ⇒ `{(0,0),(1,0),(2,0),(3,0)}`, no diagonal cells.
- **Axial vertical** — e.g. `(0,0)→(0,3)` ⇒ `{(0,0),(0,1),(0,2),(0,3)}`.
- **`(1,1)` dual-vertex 4-cell case** — `(0,0)→(1,1)` ⇒ all four shared cells (AC2), plus the
  `(1,1)→(0,0)` symmetry check.
- **`(2,1)` with `gcd=1`** — a primitive slope that crosses cell interiors/edges without
  hitting a dual vertex; assert the exact touched set.
- **`(2,2)` with `gcd>1`** — passes through collinear dual vertices `(½,½)` and `(1½,1½)`;
  each contributes its full 4-cell tie set.
- **Chord grazing exactly one corner** — a segment that touches a single dual vertex; the
  4 cells sharing that corner are all included.
- **Chord through two collinear dual vertices** — the extended collinear case; both vertices'
  4-cell sets appear.
- **Degenerate `a == b`** — returns exactly `{a}` (AC5).

The `gp-core` core is deterministic; assert exact states/sets.

## Open questions

- **Return container & duplicate guarantee.** Keep `Vec<Point>` (current stub) versus a
  dedup'd set type, and whether to *guarantee* duplicate-freeness in the type vs the
  algorithm. Defensible default: keep `Vec<Point>` and produce each cell once. The `design`
  Subagent picks the container/guarantee; the observable contract (exact set, no dups, no
  spurious cells) holds either way. Not design-blocking.
- **Internal integer type for overflow safety.** Whether cross-products / doubled
  coordinates need `i64` (or a checked/saturating scheme) at the largest plausible track
  dimensions. Empirical-ish; resolved by the design/impl with the chosen algorithm.
