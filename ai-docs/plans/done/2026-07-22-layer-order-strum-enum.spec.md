# Replace LAYER_ORDER `[&str; 9]` with strum `Layer`/`RegionLayer` enums + enum-driven draw dispatch

**Source:** issue #121
**Date:** 2026-07-22
**Tracked in:** #121

## Scope

1. **Remove** the `pub(crate) const LAYER_ORDER: [&str; 9]` at `crates/render/src/track/mod.rs:51`
   (and its `#[cfg_attr(not(test), allow(dead_code, …))]` attribute).
2. **Add a `Layer` enum** (`pub(crate)`, `crates/render/src/track/mod.rs`) with **7** variants in
   back-to-front order — `Regions, Heatmap, Grid, Walls, FastestLap, Sf, Cars`. The three region
   layers collapse into the single `Regions` variant (there are **no** `Asphalt`/`Infield` no-op arms
   at this level). Derive `strum::EnumIter` + `strum::IntoStaticStr` with
   `#[strum(serialize_all = "kebab-case")]` (`Regions` → `"regions"`, `FastestLap` → `"fastest-lap"`),
   mirroring the `Icon` enum at `crates/render/src/icons.rs:28`.
3. **Rewrite `draw_frame`** (`mod.rs:81`) to iterate `Layer::iter()` and `match`-dispatch each variant
   to its existing draw action: `Regions` → `regions::fill`, `Heatmap` → `heatmap::paint`,
   `Grid` → `grid::paint`, `Walls` → `walls::paint`, `FastestLap` → `fastest_lap::paint`,
   `Sf` → the `sf::checker_cells` + `sf::paint` pair, `Cars` → the per-car `car::paint` loop. The
   overlay-gated variants (`Heatmap`, `Grid`, `FastestLap`) are skipped when their `Overlays` flag is
   off — same gating as today, now inside the match. Draw order becomes enum-iter order by construction.
4. **Add a `RegionLayer` enum** (`pub(crate)`, `crates/render/src/track/regions.rs`) with **3**
   variants in back-to-front order — `Outfield, Asphalt, Infield`. Same derives / kebab-case
   (`Outfield` → `"outfield"`, `Asphalt` → `"asphalt"`, `Infield` → `"infield"`).
5. **Rewrite `regions::fill`** (`regions.rs:472`) to iterate `RegionLayer::iter()` and `match`-dispatch:
   `Outfield` → `painter.rect_filled(rect, 0, SURFACE_PAGE)`, `Asphalt` → the `roles.outer`
   `paint_mesh(.., SURFACE_ASPHALT)` loop, `Infield` → `paint_infield_holes(.., SURFACE_INFIELD)`.
   Today these are three bare imperative statements guarded by no test; this makes the region sub-order
   enum-driven and guardable.
6. **Retarget the two `` [`LAYER_ORDER`] `` intra-doc references** — the removed const's own doc
   comment at `mod.rs:39`–50 and the `draw_frame` doc at `mod.rs:64` — to point at `[`Layer`]` (and
   `[`RegionLayer`]` where the region sub-order is described), keeping
   `RUSTDOCFLAGS="-D warnings" cargo doc` green.
7. **Rework the guard test.** Replace `layer_order_is_documented` with a flattened
   documentation-parity test (shape below, AC4) and let the `/task` design phase decide whether to add
   a second, more direct painter-driven guard on `draw_frame` (see Key decisions).

## Out of scope

- Reordering, adding, or removing any actual layer/draw action. The refactor makes the existing order
  enum-driven; it does not change what is drawn or in what order.
- Any change to `Overlays`, `TrackTransform`, `BakedTrackGeometry`, or a layer submodule's paint fn
  internals (only the two dispatch call sites `draw_frame` / `fill` change).
- Changing the rendered output. All-overlays-off stays byte-for-byte the #17 baseline; existing
  goldens are not regenerated.

## Deferred

- Second, direct painter-driven `draw_frame` ordering guard | genuine test-design fork, leaning "not
  needed" | **no separate issue** — decided within this task's `/task` design phase (see Key decisions).

## Key decisions

| Question | Decision |
|---|---|
| Guard approach (issue A/B/C) | **(A) enum-driven dispatch**, applied at two levels. Order is enum-iter order by construction — the strongest structural guard. (C) ordering-helper was rejected: it allocates a `Vec<Layer>` in the per-frame hot path; (A) is allocation-free and unrollable in release/release-lto. (B) instrumented-trace was rejected: the issue disfavors captured-render assertions and it would pull in the Miri painter gate. |
| One 9-variant enum vs two | **Two enums** — a 7-variant top-level `Layer` (region trio merged into one `Regions` variant, no no-op arms) plus a 3-variant `RegionLayer` in `regions.rs`. This **intentionally amends the issue's original AC1/AC3** (which asked for one 9-variant enum). Product-owner-directed. Merging the trio removes the no-op-arm wart, and the `RegionLayer` split closes a second ordering gap that is **completely unguarded today** (three imperative statements in `fill`). |
| Where the 9 documented names live | Across the two enums: `Layer` yields `regions, heatmap, grid, walls, fastest-lap, sf, cars`; `RegionLayer` yields `outfield, asphalt, infield`. Flattening `Regions` → `RegionLayer::iter()` reproduces the documented back-to-front 9-name sequence. |
| strum derives | `strum::EnumIter` (required for the dispatch loops + the parity test) + `strum::IntoStaticStr` with `#[strum(serialize_all = "kebab-case")]`, mirroring `Icon`. `Display` optional if the design finds it useful. |
| Kebab-case name spelling | Per the issue: `FastestLap` → `fastest-lap`. The prior array's `snake_case` `"fastest_lap"` spelling is superseded outright — the old spelling is not retained (AGENTS.md § API Stability — clean break, game app never published). |
| Enum visibility | `pub(crate)` for both, mirroring the `pub(crate)` const `LAYER_ORDER` replaced and the `pub(crate) fn fill` scope. |
| Flattened parity test — role | AC4's test flattens both enums and asserts the 9 kebab names in documented order. Because draw order == enum-iter order by construction, reordering any variant **both** fails this test **and** changes the real draw order — the non-self-referential property the old `layer_order_is_documented` lacked (the old const and `draw_frame` were disconnected). Residual gap acknowledged honestly: the test pins the enum order, not the fact that `draw_frame` still *iterates* the enum; the backstops are the goldens (AC5, catch visible reorders) plus the reviewable single-`Layer::iter()`-dispatch-loop construction invariant. |
| A second, direct painter-driven guard on `draw_frame`? | **Delegated to the `/task` design phase.** Leaning: "construction + goldens suffice; a second painter-driven guard would only add the Miri gate for little marginal safety." Design decides and records the final call. If added, it constructs a `Context`/painter and therefore carries the Miri gate (AC8). |
| Region sub-order & byte-for-byte | `Outfield` must stay first (it is the full-`rect` background fill — output-meaningful). `Asphalt` ↔ `Infield` relative order is visually inert (disjoint by the annulus invariant, per `fill`'s own doc), but the enum still pins the documented order. AC5's byte-for-byte constraint now **also** covers the `fill` refactor. |

## Technical constraints

- strum 0.28 is already a direct `gp-render` dependency (`crates/render/Cargo.toml:19`,
  `strum = { workspace = true }`) — no `Cargo.toml` change.
- `gp-render` runs strict clippy (`-D warnings`) across `--all-targets`; both new enums need a
  one-line `///` on the type and on each variant (AGENTS.md § Code Style / doc-convention).
- The doc gate `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` must stay green — every
  former `[`LAYER_ORDER`]` intra-doc link is retargeted to a valid `[`Layer`]` / `[`RegionLayer`]`
  link; no broken links.
- Existing track goldens and the overlay-difference/no-op suite in `track/mod.rs` must remain green
  with no golden regeneration (all-off and every overlay combination unchanged).
- Miri gate (AGENTS.md § gp-render Context/painter rule): the flattened parity test builds **no**
  `egui::Context`/painter, so it stays **un-gated** (like `layer_order_is_documented` today). Any test
  that constructs a `Context`/drives a painter (e.g. a design-added second guard, or the existing
  `render_shapes`-driven tests) carries `#[cfg_attr(miri, ignore = "…")]`.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `LAYER_ORDER` const (and its `allow(dead_code)` attr) is removed. A `pub(crate) Layer` enum in `track/mod.rs` has exactly the 7 variants `Regions, Heatmap, Grid, Walls, FastestLap, Sf, Cars` in that back-to-front order, deriving `strum::EnumIter` + `strum::IntoStaticStr` (kebab-case). |
| AC2 | A `pub(crate) RegionLayer` enum in `track/regions.rs` has exactly the 3 variants `Outfield, Asphalt, Infield` in that order, same derives/kebab-case. |
| AC3 | `draw_frame` draws by iterating `Layer::iter()` + `match` dispatch, and `regions::fill` draws by iterating `RegionLayer::iter()` + `match` dispatch. Reordering a variant in either enum changes the real draw order (order is enum-driven, not a second hardcoded copy). |
| AC4 | A test flattens both enums (`Layer::iter()`, expanding `Regions` to `RegionLayer::iter()`) and asserts the 9 kebab names equal `["outfield","asphalt","infield","heatmap","grid","walls","fastest-lap","sf","cars"]`. Reordering any variant fails this test. |
| AC5 | All-overlays-off draw output is unchanged and every overlay combination is unchanged — existing goldens and the `track/mod.rs` overlay tests pass with no golden regeneration, covering **both** the `draw_frame` and the `fill` refactors. |
| AC6 | Every former `` [`LAYER_ORDER`] `` intra-doc reference is retargeted to `Layer` / `RegionLayer`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` is clean. |
| AC7 | Every new enum type and variant carries a `///` doc; the documented kebab-case names are reachable via `<&'static str>::from` (`strum::IntoStaticStr`). |
| AC8 | Any guard/parity test that constructs an `egui::Context`/painter carries `#[cfg_attr(miri, ignore = "…")]`; the flattened parity test (no `Context`) stays un-gated. The workspace Miri job (`cargo miri test --workspace`) is green. |
| AC9 | `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, and Miri are all green. |

## Open questions

- (none) — the one residual fork (whether to add a second, direct painter-driven `draw_frame` guard)
  is a test-design decision delegated to the `/task` design phase, not a user question.
