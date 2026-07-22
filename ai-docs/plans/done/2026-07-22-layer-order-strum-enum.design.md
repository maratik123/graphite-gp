# Design: Replace `LAYER_ORDER` `[&str; 9]` with strum `Layer`/`RegionLayer` enums + enum-driven draw dispatch

**Issue:** #121
**Date:** 2026-07-22

## Approach

The spec pins the shape (approach A, two enums, enum-driven dispatch at both
levels). This design fills in the concrete contract and resolves the one
delegated fork.

### Chosen solution

Two fieldless `pub(crate)` marker enums, each deriving `strum::EnumIter +
strum::IntoStaticStr` with `#[strum(serialize_all = "kebab-case")]`, mirroring
the in-repo `Icon` enum `[measured: Read crates/render/src/icons.rs:28-54 →
#[derive(Clone, Copy, Debug, …, strum::EnumIter, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]]`:

- **`Layer`** in `track/mod.rs` — 7 variants, back-to-front:
  `Regions, Heatmap, Grid, Walls, FastestLap, Sf, Cars`. The region trio
  collapses into the single `Regions` variant (no `Asphalt`/`Infield` no-op
  arms at this level).
- **`RegionLayer`** in `track/regions.rs` — 3 variants, back-to-front:
  `Outfield, Asphalt, Infield`.

`draw_frame` iterates `Layer::iter()` and `match`-dispatches each variant to its
existing draw action; `regions::fill` iterates `RegionLayer::iter()` and
`match`-dispatches its three region draws. Draw order becomes enum-iter order
**by construction** — the strongest structural guard, allocation-free (no
`Vec<Layer>` in the per-frame hot path).

`LAYER_ORDER` (and its `#[cfg_attr(not(test), allow(dead_code, …))]`) is deleted
with no compat alias (AGENTS.md § API Stability — clean break; game app never
published). Its order-documenting doc block is re-homed as the `///` on the new
`Layer` enum.

### The delegated fork — resolved: NO second painter-driven `draw_frame` guard

**Decision: do NOT add a second, direct painter-driven ordering guard on
`draw_frame`.** The three-layer defence is sufficient:

1. **Construction invariant** — `draw_frame` contains exactly one
   `Layer::iter()` dispatch loop; there is no second hardcoded order to drift
   from. Reordering a `Layer` variant *is* reordering the real draw sequence
   (AC3). This is reviewable by reading the single loop.
2. **Flattened parity test** (AC4, un-gated) — pins the 9 kebab names in
   documented order; a reorder of any variant in either enum fails it.
3. **Existing goldens + overlay suite** (AC5) — the `track/mod.rs`
   difference/no-op/pure-visual tests and the wgpu golden catch any *visible*
   reorder byte-for-byte.

A painter guard would `Context::default()` + drive a painter, so it carries the
Miri gate (AGENTS.md § gp-render Context/painter rule), and its only added
assertion — the order of *captured shapes* — is already covered visually by the
goldens and structurally by the single-loop construction. It buys little
marginal safety for a new Miri-gated, interpreted-cost test. `[derived → the
residual gap "the test pins enum order, not that draw_frame still iterates the
enum" is closed by the reviewable single-loop invariant + goldens; discharged at
review + cargo test]`

### Two settled sub-decisions

**strum derive set / `Display`.** Each enum derives
`Clone, Copy, Debug, strum::EnumIter, strum::IntoStaticStr` +
`#[strum(serialize_all = "kebab-case")]`. `EnumIter` powers the dispatch loops
and the parity test; `IntoStaticStr` yields the kebab names via
`<&'static str>::from` (AC7). `Copy` makes by-value iteration/match ergonomic;
`Debug` aids test-failure diagnostics. **`Display` is NOT added** — nothing
needs `to_string()`/`{}`; the `Icon` precedent omits it too `[measured: Read
icons.rs:28-54 → no Display in derive list]`. `PartialEq`/`Eq`/`EnumCount`/
`enum_map::Enum` are omitted (YAGNI — the parity test compares `&str`s, not
enums; no `EnumMap` keying here).

**Kebab spelling.** `FastestLap` → `"fastest-lap"` (per issue); the array's old
`snake_case` `"fastest_lap"` is superseded outright, no retention (clean break).
`Regions`→`"regions"`, `Sf`→`"sf"`, `Cars`→`"cars"`, `Outfield`→`"outfield"`,
`Asphalt`→`"asphalt"`, `Infield`→`"infield"` all fall out of `kebab-case`.

### Load-bearing implementation constraints

- **`painter.rect_filled` returns `ShapeIdx`, not `()`** `[measured: rg -Un 'pub
  fn rect_filled' egui-0.*/src/painter.rs → line 397, `) -> ShapeIdx`]`. In the
  `RegionLayer::Outfield` arm it MUST be a **statement-in-block** that discards
  the value — `RegionLayer::Outfield => { painter.rect_filled(rect, 0,
  SURFACE_PAGE); }` — so the arm's type is `()` and matches the sibling arms. A
  bare expression arm `=> painter.rect_filled(...)` is a hard `E0308`
  type-mismatch against the `()`-typed `Asphalt`/`Infield` arms. `[derived →
  cargo build]`
- **`strum::IntoEnumIterator` must be in scope** to call `Layer::iter()` /
  `RegionLayer::iter()` `[measured: rg -Un 'IntoEnumIterator' crates/render/src
  → icons.rs:232 `use strum::{EnumCount, IntoEnumIterator};` guards
  `Icon::iter()`]`. Add `use strum::IntoEnumIterator;` at module scope in
  `mod.rs` (production `draw_frame` use) and in `regions.rs` (production `fill`
  use); the `mod.rs` tests module already needs it for the parity test.
- **Byte-for-byte order preservation.** `Layer` order
  `Regions, Heatmap, Grid, Walls, FastestLap, Sf, Cars` reproduces the current
  statement sequence in `draw_frame` exactly; `RegionLayer` order
  `Outfield, Asphalt, Infield` reproduces `fill`'s three statements exactly.
  `transform` and `mapped` are still computed **once before** the `Layer::iter()`
  loop (unchanged from today). `[derived → AC5 existing overlay/golden tests +
  cargo test]`

### Stale plain-code-span references the doc gate cannot see (clean-break class)

The task prompt names "two `[`LAYER_ORDER`]` intra-doc references". Removing the
const and renaming the test leaves several **plain-code-span** name references
that are **not `[...]` intra-doc links**, so `broken_intra_doc_links = deny`
does **NOT** catch them — each would silently become a dangling reference to a
removed/renamed symbol. Full audit `[measured: rg -Un
'layer_order_is_documented|fastest_lap|LAYER_ORDER'
crates/render/src/track/mod.rs + rg -Un 'LAYER_ORDER' crates/render/src]`:

- **`lib.rs:83`** — `` `track::LAYER_ORDER` `` code span → retargeted to
  `` `track::Layer` `` (subtask 4).
- **mod.rs:64** — the actual `[`LAYER_ORDER`]` intra-doc *link* in `draw_frame`'s
  doc → retargeted to `[`Layer`]` / `[`regions::RegionLayer`]` (subtask 2).
- **mod.rs:39-50** — the removed const's own order-doc block (re-homed onto
  `Layer`, subtask 2). Its prose at **mod.rs:40** uses the old snake-case
  documented name `fastest_lap` and at **mod.rs:41** names the old test
  `layer_order_is_documented`; the re-homed `Layer` doc MUST use the kebab
  documented name `fastest-lap` (clean-break decision, § strum derives) and
  MUST drop/update the old-test mention (subtask 2).
- **mod.rs:14** — the module-level `//!` doc sentence "…`layer_order_is_documented`
  builds no `Context` and stays un-gated" → the test name is updated to
  `layer_order_matches_documented_names` (subtask 3).

**Scope guard — Rust identifiers vs documented names.** Only the human-readable
*documented layer name* string (`fastest_lap` → `fastest-lap` in the mod.rs:40
order prose) is superseded. The Rust **identifiers** `mod fastest_lap`, the
`Overlays.fastest_lap` field, and `fastest_lap::paint` (mod.rs:18/68/129/130,
etc.) stay `snake_case` and are **out of scope** — the kebab rename applies to
the enum's serialized display names, not to module/field identifiers.

**Post-refactor audit command** (Recommendation B — the implementer runs this;
it must return **zero** stale hits): `rg -Un
'layer_order_is_documented|LAYER_ORDER' crates/render/src`.

### Doc-link resolvability (doc gate)

- On `Layer` (mod.rs) and in `draw_frame`'s doc: the region sub-order links to
  **`[`regions::RegionLayer`]`** — module-qualified, because `RegionLayer` is
  not in scope at `mod.rs` level `[derived → RUSTDOCFLAGS="-D warnings" cargo
  doc, discharges resolvability]`. `[`Layer`]` (same module) resolves bare.
- `lib.rs:83` stays a **plain code span** `` `track::Layer` `` (no `[...]`), the
  current style there — zero doc-gate surface, no need to reason about
  `private_intra_doc_links` for the private `track` module.

### Rejected alternatives

- **One 9-variant enum** (issue's original AC1/AC3) — rejected per spec Key
  decision (product-owner-directed): it forces two no-op `Asphalt`/`Infield`
  arms at the top level and leaves the region sub-order still needing a home.
  Two enums remove the no-op wart and close the *currently-unguarded* region
  sub-order (three bare statements in `fill`).
- **(C) ordering-helper returning `Vec<Layer>`** — allocates in the per-frame
  hot path; enum-iter dispatch is allocation-free and release-unrollable.
- **(B) instrumented-trace guard** — pulls in the Miri painter gate and the
  issue disfavors captured-render assertions (this is the same reasoning that
  resolves the delegated fork above).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `pub(crate) RegionLayer` enum (`Outfield, Asphalt, Infield`; derives `Clone, Copy, Debug, strum::EnumIter, strum::IntoStaticStr` + kebab-case; `///` on type + each variant) and rewrite `regions::fill` to `for region in RegionLayer::iter()` + `match` dispatch (Outfield=block-discard `rect_filled`; Asphalt=`roles.outer` `paint_mesh` loop; Infield=`paint_infield_holes`). Add `use strum::IntoEnumIterator;`. Byte-for-byte order preserved; existing `fill_emits_asphalt_mesh_then_infield_mesh` stays green. | `crates/render/src/track/regions.rs` | — |
| 2 | Add `pub(crate) Layer` enum (`Regions, Heatmap, Grid, Walls, FastestLap, Sf, Cars`; same derives/kebab-case; `///` on type + each variant, re-homing the removed const's order-doc onto the type, linking `[`regions::RegionLayer`]` for the region sub-order). The re-homed prose MUST use the kebab documented name **`fastest-lap`** (not the old snake `fastest_lap` at mod.rs:40) and MUST drop the old-test mention `layer_order_is_documented` (mod.rs:41). **Remove** `LAYER_ORDER` const + its `#[cfg_attr(not(test), allow(dead_code,…))]`. Rewrite `draw_frame` to `for layer in Layer::iter()` + `match` dispatch (overlay-gated arms wrap their paint in `if overlays.X { … }`; `transform`/`mapped` computed once before the loop). Add `use strum::IntoEnumIterator;`. Retarget the `[`LAYER_ORDER`]` link in `draw_frame`'s doc (mod.rs:64) → `[`Layer`]` / `[`regions::RegionLayer`]`. | `crates/render/src/track/mod.rs` | — |
| 3 | Replace the `layer_order_is_documented` test with `layer_order_matches_documented_names` (AC4): flatten `Layer::iter()` (expanding `Regions` → `regions::RegionLayer::iter()`) to `Vec<&'static str>` via `<&'static str>::from`, assert equals `["outfield","asphalt","infield","heatmap","grid","walls","fastest-lap","sf","cars"]`. Update the test-module `use super::{…}` (drop `LAYER_ORDER`; the parity test reaches `Layer` via `super::Layer` and `RegionLayer` via `super::regions::RegionLayer`). **Also** update the module-level `//!` doc at **mod.rs:14** — the sentence naming `layer_order_is_documented` as building no `Context` — to the new test name `layer_order_matches_documented_names` (plain code span, doc-gate-invisible). Builds **no** `Context` → stays un-gated (AC8). | `crates/render/src/track/mod.rs` | 1, 2 |
| 4 | Retarget the stale code-span reference `` `track::LAYER_ORDER` `` at `lib.rs:83` → `` `track::Layer` `` (plain code span, no link). | `crates/render/src/lib.rs` | 2 |

M = 4 subtasks, all Rust `*.rs` (code change-type).

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping ((a) grouping required
for every M ≥ 1). All 4 subtasks are the **code** change-type (`*.rs` only), so
by (e) change-type homogeneity + (f) group-minimization they cluster into ONE
group; dependencies (3 after 1,2; 4 after 2) are respected by the natural 1→2→3→4
order and force no split. 4 ≤ 10 (b), so no size-cap split; single group ≤ 4
groups (h).

- **Group A** — model `sonnet` (sonnet-5), effort `medium` (pinned in
  `code-writer` frontmatter), 1M-token window, via the `code-writer` subagent —
  subtasks 1, 2, 3, 4 (code change-type: `crates/render/src/track/regions.rs`,
  `crates/render/src/track/mod.rs`, `crates/render/src/lib.rs`). Terminal group
  (4 subtasks; within the `1..=10` range). No inter-group handoff; this single
  group completes /task Step 8 in its own `/context-reset` subagent per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry),
  named at entry into Group A.

## Risks

- **`RegionLayer::Outfield` arm type-mismatch (`ShapeIdx` vs `()`):** the arm
  MUST discard `rect_filled`'s return in a block — `=> { painter.rect_filled(…);
  }`. A bare-expression arm fails to compile. — `[measured: rg -Un 'pub fn
  rect_filled' egui-0.*/src/painter.rs → `) -> ShapeIdx`]`; `[derived → cargo
  build]`
- **`Layer::iter()` / `RegionLayer::iter()` unresolved in production code:**
  needs `use strum::IntoEnumIterator;` at module scope in both `mod.rs` and
  `regions.rs` (not just in the test module). — `[measured: rg -Un
  'IntoEnumIterator' crates/render/src → icons.rs:232 imports it to call
  `Icon::iter()`]`; `[derived → cargo build]`
- **Silent stale plain-code-span references (doc-gate-invisible):** `lib.rs:83`
  (`LAYER_ORDER` code span, ST4), mod.rs:14 + the re-homed const doc's mod.rs:41
  (`layer_order_is_documented` test name, ST3/ST2), and mod.rs:40's snake
  `fastest_lap` documented-name spelling (ST2). None are `[...]` links, so
  `broken_intra_doc_links = deny` does not flag them — each subtask edit is the
  only thing that clears the dangling mention. Implementer's closing gate: `rg
  -Un 'layer_order_is_documented|LAYER_ORDER' crates/render/src` returns zero
  stale hits (Recommendation B). — `[measured: rg -Un
  'layer_order_is_documented|fastest_lap|LAYER_ORDER'
  crates/render/src/track/mod.rs → mod.rs:14/40/41 named; rg -Un 'LAYER_ORDER'
  crates/render/src → lib.rs:83 among 5 hits, no brackets]`
- **Broken intra-doc link on the re-homed order doc:** `[`regions::RegionLayer`]`
  (module-qualified) resolves from `mod.rs`; a bare `[`RegionLayer`]` would not.
  — `[derived → RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace]`
- **Accidental draw-order / output change (AC5):** enum orders reproduce the
  current statement sequences exactly; `transform`/`mapped` still computed once
  before the loop. — `[derived → existing track/mod.rs overlay suite +
  track/golden.rs wgpu golden, no regeneration; cargo test]`
- **clippy `-D warnings` (pedantic/nursery deny):** mixed
  block/expression match arms all return `()`; fieldless enums with unused-ish
  derives draw no clippy lint. — `[measured: grep -n -A15 'workspace.lints'
  Cargo.toml → pedantic/nursery = deny, priority -1]`; `[derived → cargo clippy
  --workspace --all-targets -- -D warnings]`
- **Miri (AC8/AC9):** the new parity test builds no `Context` → un-gated, like
  `layer_order_is_documented` today; no new `Context`/painter test is added
  (fork resolved NO), so no new Miri gate is introduced. — `[derived → cargo
  miri test --workspace]`

## Test Design

**TDD framing.** The dispatch rewrites (subtasks 1, 2) are regression-guarded by
the **existing** `track/mod.rs` overlay suite (`each_overlay_changes_output_when_on`,
`all_off_equals_metrics_independent_baseline`, the no-op/pure-visual tests) and
`track/regions.rs::fill_emits_asphalt_mesh_then_infield_mesh` — these already
pin the byte-for-byte output and pass unchanged (AC5), so they are the
already-red-if-broken backstop for the refactor. The one *new* test guards enum
order.

### Subtask 3 — flattened parity test (AC4)

- **Location:** `crates/render/src/track/mod.rs` `#[cfg(test)] mod tests`
  (replaces `layer_order_is_documented`).
- **Entry point:** `Layer::iter()` + `regions::RegionLayer::iter()` +
  `<&'static str>::from` (the `strum::IntoStaticStr` conversion).
- **Scenario (happy path — the only case):** flatten `Layer::iter()`, expanding
  `Layer::Regions` to `regions::RegionLayer::iter()`'s three names and mapping
  every other variant to its own name, into `Vec<&'static str>`; assert it
  equals `["outfield","asphalt","infield","heatmap","grid","walls","fastest-lap","sf","cars"]`.
  Reordering any variant in either enum fails the `assert_eq!`.
- **Shape:**
  ```
  #[test]
  fn layer_order_matches_documented_names() {
      use strum::IntoEnumIterator;
      let flat: Vec<&'static str> = Layer::iter()
          .flat_map(|layer| match layer {
              Layer::Regions => regions::RegionLayer::iter()
                  .map(<&'static str>::from)
                  .collect::<Vec<_>>(),
              other => vec![<&'static str>::from(other)],
          })
          .collect();
      assert_eq!(
          flat,
          [
              "outfield", "asphalt", "infield", "heatmap", "grid",
              "walls", "fastest-lap", "sf", "cars",
          ]
      );
  }
  ```
- **Fixtures/helpers:** none. **No `egui::Context`** → **un-gated** under Miri
  (AC8). `[derived → cargo miri test --workspace stays green with this test
  running]`
- **Import:** the test-module `use super::{BakedTrackGeometry, LAYER_ORDER};`
  drops `LAYER_ORDER` (removed). `Layer` is `super::Layer`; `RegionLayer` is
  `super::regions::RegionLayer` (both `pub(crate)`, crate-visible).

### Subtasks 1 & 2 — dispatch rewrites (no new tests)

Covered by the existing suites above. The `fill` region-order test and the
`draw_frame` overlay/golden suite assert the byte-for-byte output the refactor
must preserve (AC5). `[derived → cargo test; track/golden.rs wgpu golden not
regenerated]`

### Subtask 4 — doc-comment retarget (no test)

Pure prose; verified by the doc gate. `[derived → RUSTDOCFLAGS="-D warnings"
cargo doc --no-deps --workspace clean (AC6)]`

## Open questions

- (none) — the delegated fork (second painter-driven `draw_frame` guard) is
  resolved above (**NO**), with rationale recorded.
