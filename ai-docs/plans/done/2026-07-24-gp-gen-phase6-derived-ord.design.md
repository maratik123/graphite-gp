# Design: gp-gen Ф6 — derived `Ord` for phase6 dispatch + wall ordering

**Issue:** #153 (follow-up to #31 / PR #152; owner-amended to widen scope)
**Date:** 2026-07-24

## Approach

Replace **five** hand-rolled ordering surrogates with derived `Ord`, in two
parallel collapses that share one mechanism (make declaration/field order equal
the target rank, then `#[derive(PartialOrd, Ord)]`):

1. **Ф6 dispatch ordering** — delete `axis_rank`, `issue_rank`, `issue_sort_key`
   (`phase6_repair.rs`); derive `Ord` on `DispatchLabel`, backed by a new order
   on `gp_core::geom::Orient` and a severity order on `crate::Issue`.
2. **Wall ordering** — delete `phase5b::wall_sort_key`; derive `Ord` on
   `gp_core::geom::Side` and `gp_core::geom::Wall`; collapse every call site onto
   the derived order (`.sort()` / `.min()` / `w < best_w`).

Both collapses are **behavior-preserving by construction** — the derived tuple
reproduces the old sort key term-for-term (AC5/AC6 for dispatch; AC12 for walls),
discharged by the retained ordering/determinism tests under `cargo test`.

`gp-core`'s surface delta is exactly three added orderings: `Orient`, `Side`,
`Wall` gain `Ord` (+`PartialOrd`). Nothing else in `gp-core` changes (AC8). No
new dependency, no new function — only derives, deletions, and call-site edits,
so no `missing_const_for_fn` trigger is introduced
`[measured: the change adds zero fns; it removes four const fns — wall_sort_key, axis_rank, issue_rank, issue_sort_key]`.

### Mechanism decisions (the reorder-vs-impl calls the spec delegated)

**1. `Orient` order — plain `#[derive(PartialOrd, Ord)]` (not a strum-discriminant impl).**
`Orient` is a fieldless 2-variant enum declared `Horizontal` then `Vertical`
`[measured: sed -n '64,72p' crates/core/src/geom/mod.rs → enum Orient { Horizontal, Vertical }]`,
so a plain derive yields exactly `Horizontal < Vertical` — identical to today's
`axis_rank` (`Horizontal → 0`, `Vertical → 1`). A strum-discriminant impl adds
machinery for zero behavioural gain; nothing needs an integer repr of `Orient`.
YAGNI selects the plain derive; no lint forces the alternative
`[measured: rg -n 'lints' Cargo.toml → [workspace.lints.clippy] pedantic/nursery deny; none mandates an int repr or strum discriminant]`.
Additive to the existing set (`Clone, Copy, PartialEq, Eq, Hash, Debug`);
`PartialOrd` is added alongside `Ord` (clippy `derive_ord_xor_partial_ord`
requires both). Adds no arithmetic — `gp-core` stays integer-only/deterministic
(`docs/design.md` §3a).

**2. `Issue` order — option (a): reorder the variant declarations to severity order, then `#[derive(PartialOrd, Ord)]`.**
Chosen over option (b) (keep declaration order + hand-written `Ord`/explicit rank):

- **Option (b) reintroduces the exact surrogate #153 removes.** A hand-written
  `impl Ord for Issue` mapping each variant to a severity rank *is* `issue_rank`
  relocated into a trait impl — AC4 requires deleting that logic, not moving it.
  Option (a) makes the discriminant order == severity order, so the derive
  carries the rank with no hand-written table.
- **Option (a) is what makes the `DispatchLabel` derive reproduce the full key.**
  A derived enum `Ord` compares the discriminant first, then fields in
  declaration order. With `Issue` in severity order the discriminant *is* the
  rank; and each payload-bearing `Issue` variant already declares its `Point`
  payload **first** (`Narrow`/`NarrowSf` are `{ center, axis, width }`)
  `[measured: sed -n '31,92p' crates/gen/src/phase4.rs → Narrow{center:Point,axis:Orient,width:u32}, NarrowSf{center,axis,width}, LostHairpin{tip:Point}, ConcaveChordCut{tooth}, ArmsMerging{bridge}, NoBraking{at} — Point payload is the first field of every payload-bearing variant]`,
  so the derived tuple compares `(discriminant=rank, Point, axis, width)` —
  the old `issue_sort_key` key
  `[measured: sed -n '212,235p' crates/gen/src/phase6_repair.rs → issue_sort_key returns (rank, center, axis_rank(axis), width) for Narrow/NarrowSf; (rank, payload-Point, 0, 0) otherwise; (8,(0,0),0,0) for DynamicStall]`,
  with `Orient`'s new `Ord` supplying the `axis` position (== old `axis_rank`)
  and `u32` supplying `width`.
- **Reordering is behaviorally safe for every consumer other than the new `Ord`.**
  **Nothing across the whole workspace observes `Issue`'s declaration order:** no
  strum discriminant, no `as` cast, no `EnumIter`, no manual `Ord`/`PartialOrd`
  `[measured: grep -rn 'Issue as|as u8|as usize|EnumIter|strum::Enum' crates/ | grep -i issue → (empty); grep -rn 'impl.*Ord.*Issue|impl.*PartialOrd.*Issue' crates/ → (empty)]`.
  **`Issue` is referenced only inside `crates/gen/`** (`phase1/2/4/4_defects/5_runout/6_arms/6_repair`);
  it is **not** re-exported from gen's lib root, and **gp-game never names `Issue`**
  `[measured: grep -rln Issue crates/ → only crates/gen/src/*.rs (plus a prose "Issue-#1" comment in core/geom/distance.rs:201, not a type ref); grep -rn Issue crates/game/ → (empty); grep -n 'pub use.*Issue' crates/gen/src/lib.rs → (empty)]`.
  `Eq`/`Hash` are field-based and order-independent, so the `HashSet<Issue>`
  assertions in `phase4.rs` are unaffected. `phase4_static_checks`' emission
  order is a `Vec` push order in *check* order, independent of declaration order
  (#31 Decision 4 keeps them decoupled).

  Target declaration (severity ranks 0–7):
  `Disconnected, BadTopology, LostHairpin, ArmsMerging, ConcaveChordCut, Narrow, NarrowSf, NoBraking`.
  Today's order is `Disconnected, BadTopology, Narrow, NarrowSf, LostHairpin, ConcaveChordCut, ArmsMerging, NoBraking`
  `[measured: sed -n '31,92p' crates/gen/src/phase4.rs]` — the middle five move; the two payload-less top-rank variants stay put.

**3. `DispatchLabel` order — plain `#[derive(PartialOrd, Ord)]`, `DynamicStall` stays the last variant.**
Declared `Issue(Issue)` then `DynamicStall`
`[measured: sed -n '169,176p' crates/gen/src/phase6_repair.rs → enum DispatchLabel { Issue(Issue), DynamicStall } with derive(Clone,Copy,PartialEq,Eq,Debug)]`,
so the derive makes every `Issue(_)` sort strictly before `DynamicStall` — the
old rank-8 "runs last" behaviour, no sentinel key. `DynamicStall` is payload-less,
so cross-variant comparison never reaches a field. `Eq`+`PartialEq` are already
present (the `Ord`/`PartialOrd` prerequisites); `Hash` is absent and not needed.
`labels.sort_by_key(|&l| issue_sort_key(l))` becomes `labels.sort()` — a stable
sort over `Ord`, matching the old stable `sort_by_key` (equal keys keep input
order → determinism preserved).

**4/5. `Side` + `Wall` order — plain `#[derive(PartialOrd, Ord)]`, no reorder (trivial, no delegated choice).**
- `Side` is declared `East, West, North, South`
  `[measured: sed -n '20,31p' crates/core/src/geom/graph.rs → enum Side { East, West, North, South }, derive(Clone,Copy,PartialEq,Eq,Hash,Debug,strum::EnumIter)]`
  — **exactly** `wall_sort_key`'s side rank (`East=0, West=1, North=2, South=3`)
  `[measured: sed -n '79,88p' crates/gen/src/phase5b.rs → match side: East→0, West→1, North→2, South→3]`.
  No variant reorder. Additive to the derive set; `strum::EnumIter` is unaffected.
- `Wall` is a struct declared `cell: Point` then `side: Side`
  `[measured: sed -n '76,84p' crates/core/src/geom/mod.rs → struct Wall { cell: Point, side: Side }, derive(Clone,Copy,PartialEq,Eq,Hash,Debug)]`,
  so a derived lexicographic `Ord` compares `cell` (`Point` already derives `Ord`,
  `x` then `y`) then `side` — **term-for-term equal** to
  `wall_sort_key(w) = (w.cell, side_rank(w.side))`. No field reorder. `Wall: Ord`
  requires `Side: Ord`, hence subtask 3 depends on subtask 2.

### Wall call-site collapse (mechanical, term-for-term)

- `Vec<Wall>::sort_by_key(|&w| wall_sort_key(w))` → `.sort()` — `Vec::sort`
  requires `Wall: Ord` (stable, matching the old stable `sort_by_key`).
- `iter.min_by_key(|&w| wall_sort_key(w))` → `.min()` — `Iterator::min` requires
  `Wall: Ord`, returns `Option<Wall>`. Both `min` and `min_by_key` return the
  **last** of equal-minimum elements; walls in each iterator are distinct
  (distinct cell/side), so there are no ties → identical result.
- `wall_sort_key(w) < wall_sort_key(best_w)` → `w < best_w` — `Wall: PartialOrd`.

All three are byte-identical outcomes because the derived `Wall: Ord` **is** the
`wall_sort_key` order (mechanism 5). This is a derivation, discharged by the
retained wall-ordering tests under `cargo test` (AC12).

### Grep-clean inventory (AC11 — binding contract, verified)

`wall_sort_key` appears **27 times across 4 files**
`[measured: rg -Uc wall_sort_key crates/ → phase6_arms.rs:11, phase5b.rs:9, phase6.rs:6, phase6_repair.rs:1; rg -Un wall_sort_key crates/ | wc -l → 27]`.
All 27 must be removed (AC11 grep-clean over `crates/`). By category:

| Category | Sites | Discharge |
|---|---|---|
| Definition (`pub(crate) const fn`) | phase5b.rs:79 | delete |
| Code — `sort_by_key`→`.sort()` | phase5b.rs 144, 401, 596, 920, 944 | edit + `cargo build`/`cargo test` |
| Code — `min_by_key`→`.min()` | phase6_arms.rs 52, 71 | edit + build/test |
| Code — `<` cmp → `w < best_w` | phase6.rs 170, phase6_arms.rs 160, 305 | edit + build/test |
| Imports | phase6.rs 76, phase6_arms.rs 16 | drop from `use` (else unused-import `-D warnings`) |
| **Intra-doc links** `` [`wall_sort_key`] `` | phase6_arms.rs 40, 61, 117, 239; phase5b.rs 127 | **doc gate** `[derived → RUSTDOCFLAGS=-D warnings … : broken intra-doc link after deletion]` |
| Prose comments (code-spans, **doc-gate-blind**) | phase6.rs 62, 127, 358, 391; phase5b.rs 58, 594; phase6_repair.rs 180 | **grep only** — `[measured: rg -Un wall_sort_key crates/ → (empty) after subtask 6]` |
| Test names carrying the substring | phase6_arms.rs 353, 380 | **rename** (see § Test Design) |

The prose code-span comments are **not** intra-doc links, so the doc gate never
catches them — only the explicit AC11 grep does. phase6_repair.rs:180 rides the
`axis_rank` doc comment, so it is removed **with** the `axis_rank` deletion in
subtask 5 (no file overlap between subtasks 5 and 6).

### Rejected alternatives

- **Option (b) for `Issue`** (keep declaration order + hand-written `Ord`):
  relocates `issue_rank` instead of deleting it (AC4); blocks the `DispatchLabel`
  derive from reproducing the key without a second hand-written layer.
- **strum-discriminant order for `Orient`/`DispatchLabel`/`Side`**: machinery
  with no consumer; YAGNI, no lint forces it.
- **Reordering `Side` or `Wall`'s fields**: unnecessary — declaration/field
  order already equals the target rank, so the plain derive is behavior-preserving
  by construction (unlike `Issue`, which needs the reorder).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `PartialOrd, Ord` to `Orient`'s derive (additive to `Clone, Copy, PartialEq, Eq, Hash, Debug`). Add unit test `orient_ord_orders_horizontal_before_vertical` (mirrors `point_ord_orders_by_x_then_y`, mod.rs:598). No other gp-core change (AC1, AC8). | `crates/core/src/geom/mod.rs` | — |
| 2 | Add `PartialOrd, Ord` to `Side`'s derive (additive; `strum::EnumIter` untouched). **No variant reorder.** Add unit test `side_ord_orders_east_west_north_south` pinning `East < West < North < South` (AC9). | `crates/core/src/geom/graph.rs` | — |
| 3 | Add `PartialOrd, Ord` to `Wall`'s derive. **No field reorder.** Add unit test `wall_ord_matches_wall_sort_key_order` pinning `(cell, side_rank)` lexicographic order via concrete ordered pairs (AC10). | `crates/core/src/geom/mod.rs` | 2 |
| 4 | Reorder `Issue`'s variant declarations to severity order (`Disconnected, BadTopology, LostHairpin, ArmsMerging, ConcaveChordCut, Narrow, NarrowSf, NoBraking`); add `PartialOrd, Ord` to its derive. Leave every payload field order untouched (Point-first). No logic change to `phase4_static_checks`/detectors (AC2 support). | `crates/gen/src/phase4.rs` | 1 |
| 5 | Add `PartialOrd, Ord` to `DispatchLabel`'s derive; delete `axis_rank`, `issue_rank`, `issue_sort_key` (incl. their docs — this removes the phase6_repair.rs:180 `wall_sort_key` ref); replace `labels.sort_by_key(|&l| issue_sort_key(l))` (line 318) with `labels.sort()`; rewrite the module doc head (drop `issue_rank`/`issue_sort_key` names); **retarget the `phase6_local_repair` doc-comment at line 307** (an `issue_sort_key` code-span — doc-gate-blind, so it must be edited directly, not left to the AC4 grep) to name `.sort()` / the derived order. Re-target/add dispatch ordering tests (AC2, AC3 required, AC4, AC5, AC6). `recheck_scope` stays. | `crates/gen/src/phase6_repair.rs` | 1, 4 |
| 6 | Delete `phase5b::wall_sort_key`; collapse all 26 remaining call/import/doc/comment/test-name sites onto derived `Wall: Ord` (`.sort()`/`.min()`/`w < best_w`; drop the two `use` imports; fix the 5 intra-doc links; strip the 6 prose comments; rename the 2 tests). Re-target `add_edit_wall_picks_the_canonical_min_*` / `remove_edit_wall_picks_the_canonical_min_*` and the phase5b wall-sort tests onto the derived order. Grep-clean: `rg -Un wall_sort_key crates/` empty (AC11, AC12). | `crates/gen/src/phase5b.rs`, `crates/gen/src/phase6.rs`, `crates/gen/src/phase6_arms.rs` | 2, 3 |

M = 6. All six subtasks are Rust `*.rs` (code change-type). Dependency edges:
3→2 (`Wall: Ord` needs `Side: Ord`), 4→1 (`Issue: Ord` compares `axis: Orient`),
5→{1,4} (`DispatchLabel` wraps `Issue`; dispatch uses `Orient` order),
6→{2,3} (wall collapse needs `Side`+`Wall: Ord`). All gp-core derives (1,2,3)
are upstream of every gp-gen change (4,5,6). Ordering is forced but all six fit
one homogeneous group (≤ 10 cap).

## Handoff plan

Per `.claude/skills/task/SKILL.md` Step 8, a `/context-reset` handoff binds at the
**start of every** design-defined group (every M ≥ 1), including the first and
including this single-group design.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, 1M-token
  window, via the `code-writer` subagent — subtasks **1, 2, 3, 4, 5, 6** (code
  change-type: `*.rs` only; homogeneous). Group size 6 (≤ 10 cap; within the
  terminal `1..=10` range). Subtasks are listed in dependency-valid order, so the
  `code-writer` executes them sequentially as written (gp-core derives 1–3 land
  before the gp-gen deletions 4–6). This is the **terminal** group; no inter-group
  handoff follows it. Entry into Group A is itself a `/context-reset` re-entry per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry); the
  group completes /task Step 8 in its own `/context-reset` subagent.

Single group ⇒ group count (1) is already minimized and well under the default
max of 4 — no user gating needed. Routing (applied by `/context-reset` / /task
Step 8): a code group → `subagent_type="code-writer"`, whose `model: sonnet` +
`effort: medium` are frontmatter-pinned (no inline `model=`/effort override). The
`design`, `design-review`, and `self-review` gates stay on Opus.

## Risks

- **Adding `Ord` to `Wall`/`Side` newly orders some existing collection, changing behaviour**:
  ruled out — there is **no** `BTreeSet<Wall>`, `BTreeSet<Side>`, `BTreeMap`
  keyed by either, or `BinaryHeap` of either anywhere; every ordered collection
  in gen/core is over `Point`/`usize`/`i32`/tuple keys (all already `Ord`), and
  the one `Side` in a BTree signature is a plain `fn` parameter beside a
  `BTreeSet<Point>`. No code relied on `Wall`/`Side` **not** being `Ord` except
  `wall_sort_key` itself (deleted). — `[measured: grep -rn 'BTreeSet|BTreeMap|BinaryHeap' crates/core/src crates/gen/src | grep -i 'wall\|side' → only phase2.rs:186 taper_pass(…, h: &BTreeSet<Point>, side: Side); grep -rn 'impl.*Ord.*Wall|impl.*Ord.*Side|Wall as|Side as' crates/ → (empty)]`
- **Reordering `Issue` breaks a declaration-order-dependent consumer**: ruled out
  workspace-wide — no `as`-cast / strum discriminant / `EnumIter` / manual `Ord`
  observes the order; `Issue` is gen-internal (not re-exported, gp-game never
  names it); `Eq`/`Hash`/`HashSet` are order-independent; `phase4_static_checks`
  emission order is decoupled. — `[measured: grep -rn 'Issue as|as u8|as usize|EnumIter|strum::Enum' crates/ | grep -i issue → (empty); grep -rln Issue crates/ → gen only; grep -rn Issue crates/game/ → (empty)]`
- **Derived `DispatchLabel::Ord` / `Wall: Ord` silently diverges from the old key**
  (would break AC5/AC6 / AC12): mitigated structurally — the derived tuples equal
  the old keys term-for-term, contingent on (i) `Issue` in severity order,
  (ii) `Point`-first payloads, (iii) `Orient` `Horizontal<Vertical`,
  (iv) `DynamicStall` last, (v) `Side` `East<West<North<South`, (vi) `Wall`
  `cell` then `side`. All six are pinned by unit tests. — `[derived → cargo test: the retained/​re-targeted severity_order_processes_removes_before_adds…, ac9_repeated_and_shuffled_calls_yield_identical_outcome, tie-break, DynamicStall-last, canonical-min-wall, and phase5b wall-sort tests exercise both derived sorts]`
- **`-D warnings` clippy surfaces a second-order lint** (`derive_ord_xor_partial_ord`
  if only `Ord` added; `derivable_impls`; unused-import after dropping
  `wall_sort_key` from two `use` lists): mitigated — `PartialOrd` added alongside
  `Ord` on all five types; no manual impl remains; both `use` imports pruned in
  subtask 6. Because a hard-error gate aborts on the first failure and can mask a
  later same-class site, **re-run the full clippy gate after all six subtasks
  land**. — `[derived → cargo clippy --workspace --all-targets -- -D warnings (AC7)]`
- **Broken intra-doc links after deleting `wall_sort_key`**: the 5
  `` [`wall_sort_key`] `` links (phase6_arms 40/61/117/239, phase5b 127) become
  broken references → the doc gate fails until fixed. The 6 prose code-span
  comments are doc-gate-blind and rely on the AC11 grep alone. — `[derived → RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace (AC7) for the links; measured: rg -Un wall_sort_key crates/ → (empty) for AC11]`
- **Stale surrogate names in `phase6_repair.rs` module doc** (lines 1–6 name
  `issue_rank`/`issue_sort_key`; the `axis_rank` doc at 177–180 names them +
  `wall_sort_key`; the `phase6_local_repair` doc at line 307 names `issue_sort_key`):
  code-spans, not links — doc gate would not catch them.
  Mitigated — subtask 5 rewrites the module doc, deletes the `axis_rank` doc
  with the fn, and retargets the line-307 doc-comment. — `[derived → grep for the three surrogate names in phase6_repair.rs returns empty after subtask 5 (AC4)]`

## Test Design

All test-only; no production panic surface added. `gp-core` stays panic-free
(`ai-docs/panic-index.md` empty) — the change adds derives + total `.sort()`/`.min()`,
no indexing/arithmetic. No golden images (pure integer/logic crates).

**Subtask 1 — `crates/core/src/geom/mod.rs` `#[cfg(test)]`** (AC1)
- Entry point: `Orient`'s derived `Ord`/`PartialOrd`.
- New `orient_ord_orders_horizontal_before_vertical` (mirrors `point_ord_orders_by_x_then_y`):
  assert `Orient::Horizontal < Orient::Vertical`; `Horizontal.cmp(&Horizontal) == Equal`;
  `vec![Vertical, Horizontal].sort()` → `[Horizontal, Vertical]`.
- Fixtures: none. Flat value comparison — no golden/threshold.

**Subtask 2 — `crates/core/src/geom/graph.rs` `#[cfg(test)]`** (AC9)
- Entry point: `Side`'s derived `Ord`/`PartialOrd`.
- New `side_ord_orders_east_west_north_south`: assert the chain
  `Side::East < Side::West < Side::North < Side::South`; sort a shuffled
  `vec![South, North, West, East]` and assert `[East, West, North, South]`.
- Fixtures: none.

**Subtask 3 — `crates/core/src/geom/mod.rs` `#[cfg(test)]`** (AC10)
- Entry point: `Wall`'s derived `Ord`/`PartialOrd`.
- New `wall_ord_matches_wall_sort_key_order` — assert lexicographic
  `(cell, side)` order directly (cannot reference `wall_sort_key`: it lives in
  gen and is being deleted). Concrete chain, e.g.
  `Wall{cell:(0,0),side:East} < Wall{(0,0),West} < Wall{(0,0),North} < Wall{(0,0),South} < Wall{(1,0),East} < Wall{(0,1),East}`
  — pins `cell` (`Point` `x`-then-`y`) dominating `side`, and `side` rank
  `East<West<North<South` within a fixed cell. This is the term-for-term AC10 pin.
- Fixtures: none.

**Subtask 4 — `crates/gen/src/phase4.rs` `#[cfg(test)]`** (AC2 support, AC6)
- The existing set/emission tests (`*` asserting `HashSet<Issue>` membership and
  `phase4_static_checks` output) must pass **unchanged** — the reorder does not
  affect set membership or emission order. No new test here; the `Issue`-order
  pin lives in `phase6_repair.rs` (subtask 5) next to where the order is consumed.

**Subtask 5 — `crates/gen/src/phase6_repair.rs` `#[cfg(test)]`** (AC2, AC3, AC4, AC5, AC6)
- Entry points: `DispatchLabel`'s derived `Ord`, `labels.sort()`, `phase6_local_repair`, `dispatch`.
- **Re-target** `issue_rank_matches_the_pinned_severity_table` → rename
  `issue_ord_matches_the_pinned_severity_table` and assert the derived order
  directly: a `<` chain over representative payloads
  `Issue::Disconnected < Issue::BadTopology < Issue::LostHairpin{..} < Issue::ArmsMerging{..} < Issue::ConcaveChordCut{..} < Issue::Narrow{..} < Issue::NarrowSf{..} < Issue::NoBraking{..}`.
  This is the AC2 pin; the asserted rank ordering is unchanged from the deleted
  `issue_rank` table.
- **Re-target** `issue_sort_key_orders_removes_before_adds_regardless_of_input_order`
  (line 428): replace `labels.sort_by_key(|&l| issue_sort_key(l))` (line 439)
  with `labels.sort()`; keep the four positional `assert_eq!`s verbatim (they
  already assert `DynamicStall` at `labels[3]`, the last slot). (AC5)
- **Re-target** `issue_sort_key_breaks_same_rank_ties_by_ascending_payload_point`
  (line 457): build `[DispatchLabel::Issue(a), DispatchLabel::Issue(b)]` with two
  same-rank `ArmsMerging` payloads `(2,2)`/`(1,1)`; `labels.sort()`; assert
  `labels[0]` is `(1,1)`, `labels[1]` is `(2,2)` — same asserted result as the
  old key-tuple `<` comparison at lines 465–466. (AC5)
- **NEW (required) `dynamic_stall_sorts_after_every_issue`** (AC3, explicit pin):
  for each `Issue` variant `v`, assert
  `DispatchLabel::Issue(v) < DispatchLabel::DynamicStall`. Cheap, self-documenting;
  required (not optional) so AC3 has a dedicated pin rather than riding the
  removes-before-adds slot assertion.
- **Unchanged** (AC6, must pass as-is — no surrogate symbol referenced):
  `severity_order_processes_removes_before_adds_regardless_of_input_order`,
  `ac9_repeated_and_shuffled_calls_yield_identical_outcome`, the `dispatch_*` /
  `ac2_*` / `ac4_*` / `ac7_*` / `recheck_scope_*` tests.
- **Grep-clean (AC4)**: after deletion,
  `grep -n 'issue_rank\|axis_rank\|issue_sort_key' crates/gen/src/phase6_repair.rs`
  returns **empty** — the renamed test identifier `issue_ord_matches_…` contains
  none of the three surrogate substrings, so no surrogate symbol or doc reference
  (module head, `axis_rank`/`issue_rank`/`issue_sort_key` defs, or the line-307
  `phase6_local_repair` doc-comment) survives.
- Fixtures: reuse existing in-file fixtures; no new fixture.

**Subtask 6 — `crates/gen/src/{phase5b,phase6_arms}.rs` `#[cfg(test)]`** (AC11, AC12)
- **Rename + re-target** `add_edit_wall_picks_the_canonical_min_wall_sort_key`
  → `add_edit_wall_picks_the_canonical_min_wall` and
  `remove_edit_wall_picks_the_canonical_min_wall_sort_key`
  → `remove_edit_wall_picks_the_canonical_min_wall` (phase6_arms.rs 353/380).
  The rename is required for AC11 grep-clean (the `_sort_key` substring must go);
  the assertions are unchanged (they already assert the picked wall's `.cell`,
  which `add_edit_wall`/`remove_edit_wall` now derive via `.min()` over `Wall: Ord`
  — the same canonical-min wall). (AC12)
- **Re-target** the phase5b wall-sort tests (the `sort_by_key(|&w| wall_sort_key(w))`
  calls at lines 596/920/944, plus the line-594 `// Sorted by wall_sort_key.`
  comment): replace with `.sort()` and update the comment to name the derived
  order; asserted results (`assert_eq!(walls, sorted)` / `assert_eq!(stall_walls, expected)`)
  are unchanged because the derived order **is** the `wall_sort_key` order. (AC12)
- **Grep-clean (AC11)**: after all edits, `rg -Un wall_sort_key crates/` returns
  empty (27 → 0). — `[measured target: rg -Un wall_sort_key crates/ → (empty)]`
- Fixtures: reuse existing in-file fixtures; no new fixture.

## Open questions

- None design-blocking. Both delegated `Issue`-mechanism options are resolved
  above (`Orient`/`DispatchLabel`/`Side`/`Wall` → plain derive; `Issue` →
  option (a) reorder-and-derive). The AC3 dedicated test
  (`dynamic_stall_sorts_after_every_issue`) is **required**, not optional (prior
  design-review note). `Side`/`Wall` introduce no open question — declaration/field
  order already matches the target rank, so the mechanism is a fixed plain derive.
