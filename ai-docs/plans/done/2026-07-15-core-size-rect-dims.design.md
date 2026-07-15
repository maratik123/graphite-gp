# Design: gp-core Size/Rect geometry types + unsigned Corridor dims

**Issue:** none — folded into PR #47 (branch `feat/2026-07-15-core-geom-corridor`)
**Date:** 2026-07-15

## Approach

Extract the corridor's box/index arithmetic into two small value types in
`crates/core/src/geom/mod.rs` and reshape `Corridor` to hold one of them, so the
negative-dimension state becomes unrepresentable and all three geom
`#[allow(clippy::cast_sign_loss)]` carve-outs disappear.

- **`Size { width, height }`** — unsigned grid extent. Methods (exactly, per
  AC1): `new`, `area() -> usize`, `is_empty()`. Unsigned fields make "negative
  dimension" unrepresentable — the whole reason the `Corridor::new` `assert!` can
  be *deleted* (not converted to `Result`).
- **`Rect { origin: Point, size: Size }`** — axis-aligned integer-grid box that
  owns the index/bounds math moved off `Corridor`. Public methods (exactly, per
  AC2): `index(p) -> Option<usize>`, `contains(p) -> bool`, `points()`
  (row-major, `y`-outer), `on_border(p) -> bool`. **Pub fields** (mirroring
  `Point`) so `Corridor::new` can build a `Rect` by struct literal (AC2 forbids a
  speculative `Rect::new`) and future `gen`/`render` consumers can read extents.
- **`Corridor { rect: Rect, cells: Vec<bool> }`** — every box query delegates the
  *index computation* to `rect`; `cells` still carries drivability. `new` takes
  unsigned dims, drops the `assert!` and its `# Panics` doc.

### Field-type decision (the OWNED YAGNI tiebreak): **`usize`**

The spec delegates `u32` vs `usize` to this design, tiebroken by "fewest `as`
casts + `#[allow]` sites" (Open Questions). I read the actual code both ways and
counted every conversion site. **Both readings reach zero `#[allow]`s** — the
deciding axis is `as`-cast count:

| Site | `usize` fields | `u32` fields |
|---|---|---|
| `Size::area() -> usize` | `width * height` → **0 casts** | `width as usize * height as usize` → **2 casts** |
| `Size::is_empty()` | `width == 0 \|\| height == 0` → 0 | 0 |
| `Rect::index` flat index | `usize::try_from(dx).ok()?` … all-`usize` → **0 casts** | `u32::try_from(dx).ok()?` then `dy as usize * width as usize + dx as usize` → **3 casts** |
| `Rect::on_border` | `usize::try_from` deltas, all-`usize` → 0 | `u32::try_from` deltas, all-`u32` → 0 |
| `Rect::points()` coord math | `i32::try_from(width).map_or(i32::MAX, \|w\| x0.saturating_add(w))` → **0 casts** | `x0.saturating_add_unsigned(width)` → **0 casts** |
| **Total** | **0 `as`, 0 `#[allow]`** | **5 `as`, 0 `#[allow]`** |

Both are `#[allow]`-free (the deleted `cast_sign_loss` allows do not return in
either reading), because every conversion is either a **checked** `try_from`
(folds the sign guard, no `as`) or a widening `u32 as usize` (clippy-clean:
`cast_possible_truncation`/`cast_lossless`/`cast_sign_loss` do not fire on
widening to `usize` with no `From<u32> for usize` impl). Under the spec tiebreak
(minimize `as` **and** allows), **`usize` wins: 0 casts vs 5.** It is also the
spec author's first-listed reading ("the entire index path stays `usize` with no
widening step"). `width()`/`height()` therefore return **`usize`**.

Rejected `u32`: its only edge is a marginally tidier `points()`
(`saturating_add_unsigned` takes `u32` directly, vs `usize`'s
`try_from(...).map_or(i32::MAX, saturating_add)`) and cosmetic alignment with
`CorridorScratch`'s existing `u32` stamp/generation fields — not worth 5 casts
when `usize` needs none and `points()`'s `map_or` is a clean two-liner.

### Key mechanics

- **Shared delta helper `Rect::offset` (private, owner-pinned)** — one
  overflow-safe delta site, reused by `index` and `on_border` (DRY):
  `fn offset(&self, p: Point) -> Option<(usize, usize)>` =
  `Some((usize::try_from(p.x.checked_sub(self.origin.x)?).ok()?,
  usize::try_from(p.y.checked_sub(self.origin.y)?).ok()?))`. `checked_sub`
  returns `None` on i32 overflow (adversarial coords, e.g.
  `i32::MAX.checked_sub(-10)`); `usize::try_from` returns `Err` → `None` for a
  negative delta — folding **both** the overflow case **and** the
  `dx < 0 || dy < 0` sign guard, with **no `as` and no `#[allow]`**. Private
  helper — not part of AC2's "exactly index/contains/points/on_border" public
  surface, and not speculative (it is the single delta site both methods need).
  Non-`const` (`try_from`); `checked_sub` is const-stable but does not lift the fn.
- **`Rect::index`** — `let (dx, dy) = self.offset(p)?;
  (dx < w && dy < h).then(|| dy * w + dx)`. **Total and panic-free for every
  `Point`** (AC3): out-of-box / negative / overflowing inputs all yield `None`, no
  `cast_sign_loss` allow, no explicit sign guard.
  *AC3-letter note:* with `usize` fields the i32→usize conversion **is** the
  `try_from` AC3 mandates; there is no separate `as` widening at all, so "widen to
  `usize` only at the final step" is vacuously satisfied (nothing to widen). This
  is the `usize` reading the spec pre-blesses in Open Questions.
- **`Rect::on_border`** — `let Some((dx, dy)) = self.offset(p) else { return false };`
  then `dx < w && dy < h && (dx == 0 || dy == 0 || dx + 1 == w || dy + 1 == h)`.
  Same overflow-safe deltas as `index` via the shared `offset` helper. The
  `dx + 1 == width` form (never `width - 1`) plus the in-box `dx < w` guard make it
  correct for `width == 0` (empty box → no border cells) and immune to unsigned
  underflow. For any in-box point this is **identical** to the old
  `flood_component` predicate `dx == 0 || dy == 0 || dx == width-1 || dy == height-1`
  (AC4), so the 32 behaviour tests stay green.
- **`Size::area` / `Corridor::area`** — `width * height` raw `usize` multiply;
  both stay `const`. The `///` intent must state the **domain bound**: real grid
  dims are ≪ `usize::MAX`, so a product that large is unreachable (the backing
  `Vec<bool>` cannot allocate it — see Risks), the same bounded-domain treatment as
  `supercover`'s overflow precondition (`docs/design.md` §3 C4); not a panic-index
  entry. Signature unchanged (`area(self) -> usize`).
- **`Rect::points()`** — `y`-outer / `x`-inner over i32 endpoint ranges
  (`x0..x1`, `y0..y1` where `x1 = i32::try_from(width).map_or(i32::MAX, |w| x0.saturating_add(w))`).
  Byte-identical order and contents to today's `Corridor::box_points`; `saturating`
  is strictly non-panicking where the old code could debug-overflow, but yields the
  same points for every in-domain grid.
- **`Corridor` delegation (minimal-churn):** keep `index` / `area` / `box_points`
  as **private** delegating methods plus a new private `on_border`, so
  `graph.rs`'s existing `d.index(..)`, `d.area()`, `d.box_points()`, `d.contains()`
  calls stay untouched (`graph` is a child module of `geom`, so it reaches these
  private methods unchanged). Only three `graph.rs` sites move:
  1. `flood_component`'s hand-rolled boundary check → `d.on_border(p)`;
  2. `CorridorScratch`'s cached `width`/`height` (`i32` → `usize`) + its
     `debug_assert!` compare (both `usize`, no cast);
  3. the test helper `corridor(origin, w, h, …)` params (`Coord` → `usize`).
  **`Corridor::contains` must NOT delegate to `Rect::contains`** — they differ:
  `Rect::contains` = "in box", `Corridor::contains` = "drivable". Corridor keeps
  `self.rect.index(p).is_some_and(|i| self.cells[i])` (delegates only the *index*
  to `rect`). Getting this wrong silently breaks every flood/geodesic test.
- **Const-ness:** `Size::{new, area, is_empty}` stay `const` (satisfies the
  `missing_const_for_fn` nursery deny); `Corridor::{origin, width, height, area}`
  stay `const` (field access / const `Size::area`). `Rect::{index, contains,
  on_border, points}` and `Corridor::{index, contains, on_border, box_points}` are
  **non-`const`** (`try_from` is not const-stable at MSRV 1.97, verified against
  the std source; `missing_const_for_fn` cannot fire on a non-const-able fn).
- **Derives:** `Size` and `Rect` each derive `Clone, Copy, PartialEq, Eq, Hash,
  Debug, Default` (mirroring `Point`; both are `Copy` value types). `Corridor`
  keeps its existing `#[derive(Clone, Debug, Default)]` (has a `Vec`, so no `Copy`;
  `Default` = empty box, unchanged behaviour). `TrackArtifact` derives only
  `Clone, Debug` (verified) — no trait on `Corridor` is dropped, so `track.rs`
  compiles unchanged; `sim.rs` only calls `contains` (signature unchanged).

### Docs reconciliation (§5 / AC7 / AC8)

- **`panic-index.md`** — delete the single `Corridor::new` row; the table becomes
  header-only (gp-core hits its zero-production-panic target). Header/intro text
  stays intact (AC7).
- **`code-style.md:15`** — the "Shipped example" sentence cites the geom
  `cast_sign_loss` carve-outs, and it is **doubly stale**: it names the file as
  `crates/core/src/geom.rs` (now `geom/mod.rs`) and counts "two" carve-outs
  (there are three: `new`, `index`, `area`). This change deletes **all three**,
  and a workspace grep confirms **no in-source `#[allow(clippy::…)]` survives
  anywhere** (`crates/` has zero left) — so "re-point to another still-existing
  carve-out" is **not available**. Reconciliation = **drop** the two
  example-specific sentences ("Shipped example: …" and the trailing "The attribute
  sits on the enclosing `let` statement …", which only exists to explain that
  example). The preceding general principle ("Where a clean fix isn't possible, a
  justified carve-out is preferred …") stays — still true. Optionally note "no
  in-source carve-outs currently exist"; recommended **against** (YAGNI — a new
  staleness magnet). This is a *content* edit to a doc, not a rule change; a
  Propagation grep for the shipped example / these carve-outs finds no other
  **live** instruction file (only the immutable `done/` plans and this spec), so
  no sync-group sibling edit is required. Both doc edits are net deletions →
  no AGENTS-char-cap risk.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `Size { width: usize, height: usize }` — `new`/`area`/`is_empty` (all `const`), derives, `///` docs; add `Size` unit tests | `crates/core/src/geom/mod.rs` | — |
| 2 | Add `Rect { origin: Point, size: Size }` — pub fields; private `offset` (overflow-safe `checked_sub` delta helper) reused by `index`/`on_border`; public `index`/`contains`/`points`/`on_border`; derives, `///` docs; add `Rect` unit tests | `crates/core/src/geom/mod.rs` | 1 |
| 3 | Reshape `Corridor` → `{ rect, cells }`: `new(origin, width: usize, height: usize)` (no `assert!`, no `# Panics`), delegators (`origin`/`width`/`height`/`contains`/`index`/`set`/`len`/`is_empty`/`area`/`box_points`/`on_border`), `width()`/`height()` → `usize`; update the three `graph.rs` sites (boundary check → `on_border`, `CorridorScratch` `width`/`height` → `usize` + `debug_assert!`, test helper `corridor` params → `usize`). All 32 existing geom tests green | `crates/core/src/geom/mod.rs`, `crates/core/src/geom/graph.rs` | 2 |
| 4 | Docs reconciliation: delete the `Corridor::new` row from `panic-index.md` (header-only); drop the stale "Shipped example" sentences in `code-style.md:15` | `ai-docs/panic-index.md`, `ai-docs/code-style.md` | 3 |

## Handoff plan

Per `.claude/skills/task/SKILL.md` Step 8, a `/context-reset` handoff binds at the
start of **every** group (M = 4 ≥ 1, so this section is mandatory). Max
non-terminal group size = 3 consecutive subtasks; the terminal group may be
1..=3.

- **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) before
  starting subtask 1.
- **Group A:** subtasks 1–3 — the full code change (Size, Rect, Corridor+graph
  migration). Non-terminal group of exactly 3.
- **Handoff after Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Parent
  `/task` resumes in Group B with fresh context.
- **Group B:** subtask 4 — docs reconciliation. Terminal group (1 subtask; within
  the 1..=3 range). It completes Step 8's handoff in its own `/context-reset`
  subagent.

## Risks

- **`Corridor::contains` mis-delegated to `Rect::contains`** (in-box vs drivable):
  silently breaks all flood/geodesic/wall tests. *Mitigation:* explicit note above
  + the 20 `graph.rs` tests catch it immediately.
- **`on_border` diverging from the old boundary predicate:** *Mitigation:* proven
  identical for in-box points (all `flood_component` callers are in-box); the
  `ring_flush_to_box_edges` / `annulus` / `two_hole_shape` complement tests are the
  guard. Add a direct `Rect::on_border` unit test for zero-dim/single-cell/edge/
  corner (AC4/AC10).
- **`points()` order/content drift** breaking `component_count` /
  `bounded_complement_components` / `walls_from_boundary` iteration: *Mitigation:*
  keep `y`-outer/`x`-inner and identical endpoints; the determinism tests
  (`all_helpers_are_deterministic`, `geodesic_layers_*`) guard order.
- **`u32 as usize` widening had been *assumed* clippy-clean** — mooted by choosing
  `usize` (zero `as` casts). Residual std surface used: `i32::checked_sub`
  (const-stable, since 1.0), `usize::try_from(i32)`, `i32::try_from(usize)`,
  `i32::saturating_add` — all verified present well under MSRV 1.97. AC11's
  `cargo clippy -- -D warnings` is the final gate; because the change removes 3
  allows and adds 0 casts/allows, a clippy regression is highly unlikely.
- **Intermediate red build if Task 3 is split** (Corridor reshape alone breaks
  `graph.rs`): *Mitigation:* Task 3 is deliberately one atomic compile unit —
  `mod.rs` Corridor reshape **and** the three `graph.rs` edits land together.
- **AC7 honesty (index overflow) — now a strict improvement:** the owner-pinned
  `checked_sub` deltas make `Rect::index`/`on_border` **provably total and
  panic-free for every `Point`**, including adversarial full-range i32 coords that
  today's raw `p.x - origin.x` would debug-panic / release-wrap on. This
  *reinforces* the zero-production-panic target rather than merely preserving it —
  the header-only panic table is more honest than before, not just as honest.
  `supercover` is untouched and keeps its own bounded-chord precondition
  (`docs/design.md` §3 C4); only the Rect index path becomes total. *Mitigation:*
  a dedicated overflow-inducing `None` test (Task 2) locks it in.
- **`area` overflow is domain-bounded (GO-note fold-in), not a panic-index entry:**
  `Size::area`/`Corridor::area` compute `width * height` as a raw `usize` multiply.
  For adversarial dims whose product exceeds `usize::MAX` this would debug-panic /
  release-wrap — but that is **unreachable in the real grid domain**: a corridor of
  that cell count cannot exist, because `Corridor::new` must first allocate a
  `Vec<bool>` of that length and the allocation fails first (out of our control,
  the same class as `supercover`'s bounded-chord precondition, `docs/design.md`
  §3 C4). It is pre-existing (was `i32`, so *more* reachable before) and never
  indexed — not a regression. Per the owner's integer-safety rule
  (`ai-docs/learnings.md`: raw `+`/`-`/`*`/`/` allowed only where operands are
  knowingly bounded so no overflow can occur, AND a test covers the bound), we
  **keep** `width * height` (signature unchanged) and carry the domain assumption
  forward explicitly — both in the `///` intent (Key mechanics) and here — so the
  header-only panic-table claim stays airtight for a future reviewer. *Mitigation:*
  the Task-1 in-domain `area` tests cover the bounded arithmetic (no overflow test
  is meaningful — the bound is a precondition, not a branch). *Rejected:*
  `saturating_mul` — it merely relocates the failure to the `Vec` allocation (which
  panics on a `usize::MAX` capacity anyway → no net zero-panic gain) and would
  silently mis-report `area` for out-of-domain input.
- **`code-style.md` reconciliation over- or under-reaching:** *Mitigation:* drop
  exactly the two example sentences; keep the general carve-out principle;
  Propagation grep confirms no live sibling references the example.

## Test Design

- **Task 1 — `Size`** (`crates/core/src/geom/mod.rs` `#[cfg(test)] mod tests`):
  - Entry points: `Size::new`, `area`, `is_empty`.
  - Scenarios: `area` = `w*h` for a normal box (e.g. `3×4 → 12`); `area == 0` and
    `is_empty()` true for `0×5`, `5×0`, `0×0`; `is_empty()` false for `1×1`;
    `Size::default()` is `{0,0}` empty with `area 0`. These in-domain cases satisfy
    the owner's "raw op must be covered by a test" rule for the `width * height`
    multiply; no overflow test is meaningful — the bound is a precondition, not a
    branch (a `usize::MAX`-product `Size` cannot be constructed without first
    failing to allocate its `Vec`).
  - Fixtures: none (literal dims).
- **Task 2 — `Rect`** (same test module):
  - Entry points: `Rect::index`, `contains`, `points`, `on_border`.
  - Scenarios: `index` in-box → correct row-major flat index (assert exact `usize`
    for a couple of cells in an off-origin box, e.g. origin `(2,3)`, size `4×5`);
    `index` out-of-box → `None` for negative `dx`/`dy` (point left/below origin),
    `dx >= width`, `dy >= height`; `index` returns `None` **without panicking** for
    an overflow-inducing point (e.g. a `Rect` with a negative-coord origin and `p`
    at `Point::new(i32::MAX, 0)`, exercising `checked_sub`); `contains` mirrors
    `index(..).is_some()`;
    `points()` returns the exact row-major `Vec<Point>` for a small box (e.g.
    `2×2`) and is **empty** for a zero-dim box; `on_border` → false for zero-dim,
    true for the sole cell of `1×1`, true on each edge/corner, false for a strict
    interior cell (e.g. center of `3×3`), false for an out-of-box point.
  - Fixtures: a small helper building a `Rect` by literal (`Rect { origin, size }`).
- **Task 3 — `Corridor` + `graph.rs`** (no new tests required):
  - Validation: the **32** existing geom tests (12 `supercover` in `mod.rs`, 20 in
    `graph.rs`) pass unchanged (AC9) via `cargo test -p gp-core`. `supercover` is
    untouched; `Corridor` behaviour (new/contains/set/width/height/flood/geodesic/
    walls) is exercised transitively by the 20 `graph.rs` tests.
- **Task 4 — docs:** no tests (markdown).

## Open questions

- None. The one delegated decision (`Size` field type) is resolved in-design to
  **`usize`** (fewest `as` casts: 0 vs `u32`'s 5; both `#[allow]`-free), with
  `width()`/`height()` returning `usize`. No product-owner input required (per the
  spec, this is a YAGNI tiebreak owned by design, not surfaced to the owner).
