# gp-gen Ф6 — derived `Ord` for phase6 dispatch ordering

**Source:** issue #153
**Date:** 2026-07-24
**Tracked in:** #153

Follow-up to #31 / PR #152. Product-owner review left three threads on
`crates/gen/src/phase6_repair.rs` (lines 181/192/212) asking why the Ф6
dispatch ordering uses hand-rolled surrogates (`issue_rank`, `axis_rank`,
`issue_sort_key`) instead of derived `Ord` / strum. It was deferred out of
#152 because the clean version breaches that PR's `AC11 = no gp-core change`
and revisits a documented design decision (declaration order kept separate
from severity order, #31 Decision 4), so it needs its own design round.

**Amendment (owner-directed, #153):** the product owner brought the parallel
wall-ordering surrogate `phase5b::wall_sort_key` — originally listed under
*Out of scope* / *Deferred* — **into scope**. `gp-core`'s `Side` and `Wall`
gain a derived `Ord` and `wall_sort_key` is deleted, exactly the same "why not
derive `Ord`" collapse as `Orient`/`Issue`. Unlike `Issue`, neither `Side` nor
`Wall` needs a variant/field reorder (declaration order already matches the
rank), so their mechanism is a trivial plain derive.

## Scope

1. **Relax #31's AC11 for these types only.** This spec supersedes #31's
   AC11 (`no gp-core change`) narrowly: `gp-core`'s `Orient`, `Side`, and
   `Wall` may each gain an ordering, and nothing else in `gp-core` changes.
2. **Add a total order to `Orient`** (`crates/core/src/geom/mod.rs`) —
   `Horizontal < Vertical`, matching today's `axis_rank` (`Horizontal → 0`,
   `Vertical → 1`). Mechanism (a plain `#[derive(PartialOrd, Ord)]` vs a
   strum-discriminant-backed impl) is a design-phase choice; a fieldless
   2-variant derive already yields exactly `Horizontal < Vertical`. Add a
   unit test pinning the order (mirrors `point_ord_orders_by_x_then_y`).
3. **Give `Issue` (`crates/gen/src/phase4.rs`) a total order equal to
   today's severity rank** (design #31 Decision 4 rank table, ranks 0–7:
   `Disconnected, BadTopology, LostHairpin, ArmsMerging, ConcaveChordCut,
   Narrow, NarrowSf, NoBraking`). The design round picks the mechanism and
   pins the rationale: either (a) reorder the enum's variant declarations to
   severity order and `#[derive(PartialOrd, Ord)]`, or (b) keep the current
   declaration order and add a hand-written `Ord` (or retain an explicit
   rank). The reorder in (a) changes `Issue`'s natural declaration order for
   every consumer, so it is not free — but note nothing today depends on that
   order (no strum discriminant, no `as` cast, no `EnumIter` on `Issue`), so
   (a) is behaviorally safe for every consumer *other than* the `Ord` this
   spec introduces.
4. **`#[derive(PartialOrd, Ord)]` on the crate-local `DispatchLabel`**
   (`phase6_repair.rs`), keeping `DynamicStall` the last variant so it sorts
   after every `Issue(_)` (today's rank 8, "runs last"). This composes with
   items 2–3: with `Orient` and `Issue` ordered, a derived `DispatchLabel`
   `Ord` reproduces the full `(rank, payload Point, axis, width)` key —
   `Narrow { center, axis, width }`'s field order already matches
   `(center, axis_rank, width)`.
5. **Delete `issue_rank`, `axis_rank`, `issue_sort_key`** once the derives
   cover them; replace the `labels.sort_by_key(|&l| issue_sort_key(l))` call
   with a derived-`Ord` sort. `recheck_scope` is unrelated (recheck routing,
   not ordering) and stays.
6. **Keep the ordering / determinism tests** that pin "same edits, same
   order" (`issue_rank_matches_the_pinned_severity_table`,
   `issue_sort_key_orders_removes_before_adds…`,
   `issue_sort_key_breaks_same_rank_ties…`,
   `severity_order_processes_removes_before_adds…`,
   `ac9_repeated_and_shuffled_calls_yield_identical_outcome`). Update them
   only where a surrogate-named symbol is deleted, and only in a way that
   demonstrably preserves the asserted behavior (re-target the deleted
   `issue_rank`/`issue_sort_key` calls onto the derived `Ord`/sort).
7. **Derive a total order on `Side`** (`crates/core/src/geom/graph.rs`) — a
   plain `#[derive(PartialOrd, Ord)]`. `Side`'s declaration order
   (`East, West, North, South`) already **exactly equals** `wall_sort_key`'s
   side rank (`East = 0, West = 1, North = 2, South = 3`), so **no variant
   reorder is needed** (contrast `Issue`) and the mechanism is trivial. Adding
   `Ord` is additive to its existing derive set and leaves `strum::EnumIter`
   unaffected. Add a unit test pinning `Side::East < West < North < South`.
8. **Derive a total order on `Wall`** (`crates/core/src/geom/mod.rs`) — a plain
   `#[derive(PartialOrd, Ord)]`. `Wall`'s fields are declared `cell: Point`
   then `side: Side`, so a derived lexicographic `Ord` compares `cell` (`Point`
   already derives `Ord`) then `side` — **term-for-term equal** to
   `wall_sort_key(w) = (w.cell, side_rank(w.side))`. No field reorder is
   needed. Add a unit test pinning `Wall` order == `wall_sort_key` order.
9. **Delete `phase5b::wall_sort_key`** (`crates/gen/src/phase5b.rs`, the
   `pub(crate) const fn` at line 79) once `Side` + `Wall: Ord` cover it, and
   replace every call site with the derived order:
   `.sort_by_key(|&w| wall_sort_key(w))` → `.sort()` (phase5b.rs 144, 401,
   596, 920, 944), `.min_by_key(|&w| wall_sort_key(w))` → `.min()`
   (phase6_arms.rs 52, 71), and `wall_sort_key(w) < wall_sort_key(best_w)`
   → `w < best_w` (phase6.rs 170, phase6_arms.rs 160, 305). Update the
   doc-comment references so no `wall_sort_key` symbol remains (phase6.rs
   62/76/127, phase5b.rs `OracleResult` doc line 58, phase6_repair.rs 180) —
   grep-clean. Keep the tests referencing it
   (`add_edit_wall_picks_the_canonical_min_wall_sort_key`,
   `remove_edit_wall_picks_the_canonical_min_wall_sort_key`, and the phase5b
   wall-sort tests), re-targeting the deleted-symbol calls onto the derived
   `Ord`/`.sort()`/`.min()` in a way that demonstrably preserves the asserted
   behavior.

## Out of scope

- Any `gp-core` change beyond adding an order to `Orient`, `Side`, and `Wall`.
- `phase4_static_checks`' emission order (an independent implementation
  detail; Ф6 dispatch order must stay decoupled from it — #31 Decision 4).
- Any behavioral change to `phase6_local_repair`'s output, or to any other
  consumer of `wall_sort_key`'s ordering (the collapse is behavior-preserving).

## Deferred

- None. `Side`/`Wall` `Ord` + the `wall_sort_key` collapse, previously
  deferred, were pulled into scope by the owner (see scope items 7–9).

## Key decisions

| Question | Decision |
|---|---|
| Does `gp-core` change? | Yes, narrowly — `Orient`, `Side`, and `Wall` gain an order. Supersedes #31 AC11 for these three types only. |
| `Orient` order | `Horizontal < Vertical` (== today's `axis_rank`). Derive vs strum impl → design phase. |
| `Side` order | Plain `#[derive(PartialOrd, Ord)]`. Declaration order `East, West, North, South` already equals `wall_sort_key`'s side rank (`East = 0 … South = 3`) — **no reorder**, mechanism trivial. `EnumIter` unaffected. |
| `Wall` order | Plain `#[derive(PartialOrd, Ord)]`. Fields `cell` then `side` compare term-for-term equal to `wall_sort_key = (cell, side_rank)` — **no field reorder** needed. |
| Delete `wall_sort_key` | Yes: `pub(crate) wall_sort_key` in `phase5b.rs`. All call sites collapse to `.sort()` / `.min()` / `w < best_w` via the derived `Wall: Ord`. |
| `Issue` order = severity, not declaration/emission | Yes — must equal #31 Decision 4's rank table (0–7). Reorder-and-derive vs keep-order-and-impl → design phase; behavior preservation is the binding constraint either way. |
| Is reordering `Issue` safe for other consumers? | Yes — verified no strum discriminant / `as` cast / `EnumIter` on `Issue`; declaration order is unobserved except by the new `Ord`. |
| `DispatchLabel` order | Derive `Ord`; `DynamicStall` stays the last variant → sorts after every `Issue(_)`. |
| Delete the three surrogates | Yes: `issue_rank`, `axis_rank`, `issue_sort_key`. Keep `recheck_scope`. |
| `Point` already `Ord`? | Yes (derives `Ord`, `x` then `y`) — no change needed; it is already the tie-break key. |

## Technical constraints

- `Orient` order must be additive to its existing derive set
  (`Clone, Copy, PartialEq, Eq, Hash, Debug`); `PartialOrd` is required
  alongside `Ord`.
- `Side` order is additive to its existing derive set
  (`Clone, Copy, PartialEq, Eq, Hash, Debug, strum::EnumIter`) and `Wall`
  order is additive to its set (`Clone, Copy, PartialEq, Eq, Hash, Debug`);
  `PartialOrd` is required alongside `Ord` for both. Adding `Ord` does not
  affect `Side`'s `EnumIter`.
- `Side`/`Wall` need **no** variant or field reorder: `Side`'s declaration
  order already equals `wall_sort_key`'s side rank, and `Wall`'s field order
  (`cell` then `side`) already equals `wall_sort_key`'s `(cell, side_rank)`
  tuple order — so the plain derive is behavior-preserving by construction
  (no reorder-vs-impl decision, unlike `Issue`).
- The severity order is **pinned by #31 Decision 4**, not by `Issue`'s
  current declaration order and not by `phase4_static_checks`' emission order.
  The new derived/implemented order must equal ranks 0–7 exactly.
- Determinism: the replacement sort must be as deterministic as today's
  `sort_by_key` (a total order over all labels; `DynamicStall` last).
- No new dependency (`strum` is already a workspace dep, wired into
  `gp-core`, `gp-gen`, `gp-render`).
- Standard workspace gates: `cargo fmt --check`, `cargo clippy --workspace
  --all-targets -D warnings`, `cargo test`, doc gate. `gp-core` stays
  integer-only/deterministic (this change adds no arithmetic).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `Orient` has a total order with `Orient::Horizontal < Orient::Vertical`, covered by a unit test in `crates/core/src/geom/mod.rs`. |
| AC2 | `Issue` has a total order equal to #31 Decision 4's severity ranks 0–7 (`Disconnected < BadTopology < LostHairpin < ArmsMerging < ConcaveChordCut < Narrow < NarrowSf < NoBraking`), pinned by a test. |
| AC3 | `DispatchLabel` derives `Ord`; `DispatchLabel::DynamicStall` sorts strictly after every `DispatchLabel::Issue(_)`, pinned by a test. |
| AC4 | `issue_rank`, `axis_rank`, and `issue_sort_key` are deleted from `phase6_repair.rs`; the dispatch-list sort uses the derived `Ord`. No surrogate remains (grep-clean). |
| AC5 | Ф6 dispatch produces the **same** severity ordering and tie-breaks as today: same-rank ties break by ascending payload `Point`, then axis (`Horizontal < Vertical`), then width — identical to the old `(rank, Point, axis_rank, width)` key. |
| AC6 | No behavioral change to `phase6_local_repair`'s output on existing fixtures: the retained ordering/determinism tests (`severity_order_processes_removes_before_adds…`, `ac9_repeated_and_shuffled_calls_yield_identical_outcome`, and the tie-break test) pass — unchanged, or edited only to re-target a deleted symbol onto the derived order with no change to the asserted result. |
| AC7 | Full workspace gates pass: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, and the doc gate (`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`). |
| AC8 | No `gp-core` change other than the `Orient`, `Side`, and `Wall` `Ord` derives; no new dependency added. The gp-core surface delta is exactly: `Orient`, `Side`, `Wall` gain `Ord` (+`PartialOrd`); nothing else in gp-core changes. |
| AC9 | `Side` has a total order equal to its side rank (`Side::East < Side::West < Side::North < Side::South`, == `wall_sort_key`'s `East = 0, West = 1, North = 2, South = 3`), via a plain `#[derive(PartialOrd, Ord)]` with **no variant reorder**, covered by a unit test in `crates/core/src/geom/graph.rs`. |
| AC10 | `Wall` has a total order term-for-term equal to `wall_sort_key`: for any two walls `w1 < w2` iff `(w1.cell, side_rank(w1.side)) < (w2.cell, side_rank(w2.side))` — `cell` (`Point` `Ord`) first, then `side` — via a plain `#[derive(PartialOrd, Ord)]` with **no field reorder**, pinned by a unit test in `crates/core/src/geom/mod.rs`. |
| AC11 | `phase5b::wall_sort_key` is deleted; every call site uses the derived `Wall: Ord` (`.sort()` / `.min()` / `w < best_w`) and no `wall_sort_key` symbol or doc-comment reference remains anywhere in `crates/` (grep-clean). |
| AC12 | No behavioral change to any consumer of `wall_sort_key`'s ordering: the retained determinism / canonical-min tests (`add_edit_wall_picks_the_canonical_min_wall_sort_key`, `remove_edit_wall_picks_the_canonical_min_wall_sort_key`, and the phase5b wall-sort tests) pass — unchanged, or edited only to re-target a deleted symbol onto the derived order with no change to the asserted result. |

## Open questions

- None design-blocking. The reorder-`Issue`-and-derive vs keep-order-and-impl
  choice (item 3 / AC2) is delegated to the design round by the issue author,
  bounded by AC5/AC6 (behavior preservation). The design Subagent picks one
  and pins the rationale.
- The `Side`/`Wall` derives (items 7–8) introduce **no** new open question:
  declaration/field order already matches the target rank, so the mechanism is
  a fixed plain derive with no reorder-vs-impl trade-off.
