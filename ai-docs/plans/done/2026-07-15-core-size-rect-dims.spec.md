# gp-core Size/Rect geometry types + unsigned Corridor dims

**Source:** free-text — follow-up to a PR #47 review comment on `ai-docs/panic-index.md:7` ("why not to make width and height unsigned?")
**Date:** 2026-07-15
**Tracked in:** none — folded into PR #47

## Scope

1. **`Size { width, height }`** — an unsigned non-negative grid extent in `crates/core/src/geom/mod.rs`. Methods (minimal, per YAGNI): `new`, `area() -> usize`, `is_empty()`. Unsigned fields make the negative-dimension state unrepresentable, which is the whole point: it eliminates the `Corridor::new` non-negative-dimensions `assert!`.
2. **`Rect { origin: Point, size: Size }`** — an axis-aligned integer-grid box in `crates/core/src/geom/mod.rs` that owns the corridor's index/bounds math (moved off `Corridor`):
   - `index(p) -> Option<usize>` via a `try_from`-style delta conversion that folds away **both** the explicit `dx < 0 || dy < 0` guard **and** the `#[allow(clippy::cast_sign_loss)]` carve-out. Widen to `usize` only at the final flat-index step.
   - `contains(p) -> bool` (`index(p).is_some()`).
   - `points()` — a row-major (`y`-outer) iterator over every cell point in the box (replaces the current private `Corridor::box_points`).
   - an on-border predicate using the `dx + 1 == width` form to avoid `width - 1` underflow at `width == 0`.
   - No speculative methods (no `translate` / `intersection` / `union` / etc.). Reusable by block-1 `gen` and block-2 `render` later.
3. **`Corridor` → `{ rect: Rect, cells: Vec<bool> }`** — delegating `origin()` / `width()` / `height()` / `contains()` / `index()` to `rect`. `Corridor::new` takes unsigned `width`/`height`, **drops the `assert!`**, and loses its `# Panics` doc section. `area()` / `box_points()` become thin delegations to `rect` (or are replaced by `rect.size.area()` / `rect.points()`).
4. **Update the corridor-graph helpers + `CorridorScratch`** in `crates/core/src/geom/graph.rs` to the new Rect/Size-based `Corridor` API: `flood_component`, `flood_fill`, `component_count`, `bounded_complement_components`, `geodesic_bfs`, `geodesic_layers`, `walls_from_boundary`, and `CorridorScratch` (its cached `width`/`height` fields + the `debug_assert!` match). Route boundary/border checks through `Rect` (e.g. `rect.on_border`) rather than hand-rolling `d.width() - 1` comparisons. Keep all **32** existing geom tests green (behaviour unchanged); add unit tests for `Size` and `Rect`.
5. **Delete the `Corridor::new` row from `ai-docs/panic-index.md`** — gp-core then reaches its zero-production-panic target and the panic table is header-only. Reconcile `ai-docs/code-style.md`'s "Shipped example" sentence (Propagation Rule — see Key decisions): it cites the geom `cast_sign_loss` carve-outs, all of which this change deletes.

## Out of scope

- Any speculative `Rect`/`Size` API beyond §2 (`translate`, `intersection`, `union`, scaling, resizing, set operations).
- Adopting `Rect`/`Size` inside `gen` (block 1) or `render` (block 2) — those are future consumers; this task only makes the types available.
- Changing `Point` / `Wall` / `Side` / `supercover` or any `sim.rs` logic beyond the mechanical recompile against the new `Corridor` API.
- A validated non-negative-dimension newtype (the old panic-index "preferred future fix") — superseded: unsigned `Size` fields achieve the same guarantee directly.
- New branch or new PR — this lands on the existing branch and PR (see Technical constraints).

## Deferred

- (none) — the task is fully specified; no follow-on issues are needed.

## Key decisions

| Question | Decision |
|---|---|
| `Size` field type — `u32` vs `usize`? | **Design decision, YAGNI tiebreak** (pick whichever removes the most cast/allow machinery). NOT asked of the owner. See Open questions for the two readings. |
| Index arithmetic | Integer-only; widen to `usize` only at the final flat-index computation. The `< 0` guard is folded into the `try_from` delta conversion, eliminating the explicit guard and the `#[allow(clippy::cast_sign_loss)]`. |
| `Rect::index` const-ness | **NOT `const`** — `u32`/`usize::try_from` is not const-stable at MSRV 1.97, and `index` is never called in a const context, so this is a non-issue. (Const-ness verifiable against the std source at `/home/syt/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/`, `core/src/num/mod.rs`.) |
| `Corridor::new` fallibility | Non-panicking and **infallible** — unsigned dims make the negative state unrepresentable, so the `assert!` is *deleted*, not converted to a `Result`. No validation is pushed onto call sites. |
| Border check for `width == 0` | Use `dx + 1 == width` (never `width - 1`) so the predicate is correct for `width`/`height` of 0 as well as ≥ 1. |
| Derives on `Rect` / `Size` | Must preserve `Corridor`'s existing `#[derive(Clone, Debug, Default)]`; `Rect` and `Size` therefore derive at least `Clone, Debug, Default` (add `Copy, PartialEq, Eq, Hash` where sensible, mirroring `Point`). |
| `code-style.md` shipped example | `ai-docs/code-style.md:15` cites the geom `cast_sign_loss` carve-outs (`Corridor::new` / `Corridor::index`, plus a third on `area`) as a shipped allow-list-discipline example. Removing all three falsifies it → reconcile in the **same PR** (Propagation Rule). Exact reconciliation (drop the sentence vs. re-point to another still-existing carve-out — there may be none left in the workspace) is a design call. |
| Tracking | None — folded into PR #47. No tracking issue, no cross-link comment. |

## Technical constraints

- **Same branch, same PR.** Land on the existing `feat/2026-07-15-core-geom-corridor` branch and PR #47. No new branch, no new PR, no cross-link comment.
- **std-only; no new dependencies.** Integer-only and deterministic throughout (`docs/design.md` §3a).
- **Pre-publish clean break** (AGENTS.md § API Stability): `Corridor::new`'s signature and the `width()` / `height()` return types change freely.
- **MSRV 1.97.0.**
- **Strict clippy** (`pedantic` + `nursery` = deny). Net effect of this change is *fewer* in-source `#[allow]`s — do not introduce new blanket allows.
- **External `Corridor` consumers to verify still compile:** `crates/core/src/track.rs` (holds a `pub corridor: Corridor` field in `TrackArtifact` and reads `walls: Vec<Wall>`) and `crates/core/src/sim.rs` (takes `&Corridor` parameters). Neither calls `Corridor::new`, so the signature change is internal to `geom` + its tests.
- Files touched: `crates/core/src/geom/mod.rs`, `crates/core/src/geom/graph.rs`, `ai-docs/panic-index.md`, `ai-docs/code-style.md` (Propagation Rule).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `Size { width, height }` exists in `geom/mod.rs` with unsigned fields and exactly the methods `new`, `area() -> usize`, `is_empty()`; negative dimensions are unrepresentable. |
| AC2 | `Rect { origin: Point, size: Size }` exists in `geom/mod.rs` exposing exactly `index(p) -> Option<usize>`, `contains(p) -> bool`, `points()` (row-major), and an on-border predicate — no speculative methods. |
| AC3 | `Rect::index` returns `None` for out-of-box points via a `try_from` delta conversion; the index path contains **no** `#[allow(clippy::cast_sign_loss)]` and **no** explicit `dx < 0 || dy < 0` guard. Widening to `usize` happens only at the final flat-index step. |
| AC4 | The on-border predicate uses the `dx + 1 == width` form (no `width - 1`) and is correct for zero-sized, single-cell, edge, and corner cases. |
| AC5 | `Corridor` is `{ rect: Rect, cells: Vec<bool> }`; `origin()` / `width()` / `height()` / `contains()` / `index()` delegate to `rect`, and `width()`/`height()` return the unsigned dimension type. |
| AC6 | `Corridor::new` accepts unsigned `width`/`height`, contains no `assert!`, and carries no `# Panics` doc section. |
| AC7 | The `Corridor::new` row is removed from `ai-docs/panic-index.md`; the panic table is header-only (gp-core has zero production panics) with its header/intro text intact. |
| AC8 | All three geom `#[allow(clippy::cast_sign_loss)]` carve-outs (`Corridor::new` cell-count, `Corridor::index`, `Corridor::area`) are gone; `ai-docs/code-style.md`'s shipped-example sentence is reconciled to match reality. |
| AC9 | The graph helpers (`flood_component`, `flood_fill`, `component_count`, `bounded_complement_components`, `geodesic_bfs`, `geodesic_layers`, `walls_from_boundary`) and `CorridorScratch` are updated to the new API, and **all 32** existing geom tests pass unchanged. |
| AC10 | New unit tests cover `Size` (`area`, `is_empty`, zero dimensions) and `Rect` (in-box / out-of-box `index`, `contains`, `points()` row-major order, on-border for zero-dim / single-cell / edge / corner). |
| AC11 | `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass; `track.rs` and `sim.rs` compile unchanged. |
| AC12 | No new dependency is introduced; the geom core stays integer-only and deterministic. |

## Open questions

- **`Size` field type: `u32` vs `usize`** (design-resolved via YAGNI — minimize cast/allow machinery; do not surface to the owner). Two defensible readings:
  - **`usize`** — `area()` and `is_empty()` need zero casts, and `usize::try_from(delta)` handles the index guard directly; the entire index path stays `usize` with no widening step.
  - **`u32`** — matches the "widen to `usize` only at the final flat-index step" phrasing in the task notes: deltas convert via `u32::try_from`, compare against `u32` dims, and widen to `usize` only when computing the flat index.
  Design should pick whichever yields the fewest `as`/`#[allow]` sites across `Size::area`, `Rect::index`, the on-border predicate, and the `CorridorScratch`/`graph.rs` comparisons, then set `width()`/`height()`'s return type to match.
