# Render overlays — speed heatmap, fastest-lap spline, notebook grid

**Source:** issue #18
**Date:** 2026-07-19
**Tracked in:** #18

Make the three `gp_render::Overlays` flags **real**. #17 (PR #100) drew design-doc
§4 layers 1–3 + 6 (regions / walls / S-F / cars) and threaded
`Overlays { speed_heatmap, fastest_lap, grid }` **inert** — an `overlays_are_inert`
test asserts byte-identical output when they toggle. This task implements the
deferred §4 layers 4 (notebook grid) + 5 (analytics: heatmap, fastest-lap) so each
flag drives a real, individually-toggleable, semi-transparent, pure-visual overlay.

## Scope

1. **`speed_heatmap` overlay** — color each asphalt cell by its per-cell max speed
   from `TrackMetrics::speed_heatmap: Vec<(Point, i32)>`, mapped onto the 4-stop
   slow→fast ramp `HEAT_0` (blue) → `HEAT_1` (teal) → `HEAT_2` (amber) → `HEAT_3`
   (red). Semi-transparent so the asphalt/paper reads through (Track.jsx tints the
   asphalt fill at ~0.9 opacity).
2. **`fastest_lap` overlay** — draw `TrackMetrics::fastest_lap: Vec<Point>` as a
   thin, smooth (splined) line over the asphalt in `ACCENT`. Pure-visual: does not
   touch physics, the corridor `D`, or any metric (design §4: "валидна by
   construction, сглаживать свободно, на коллизии не влияет").
3. **`grid` overlay ("notebook sheet", §4 layer 4)** — faint engineering-blue
   ruling (`GRID_LINE`) at the lattice **cell pitch**, a heavier major line
   (`GRID_LINE_MAJOR`) every 5th, plus the dotted lattice of points (`GRID_DOT`)
   at cell corners. Semi-transparent.
4. **Independent toggles** — each flag drives its overlay independently; every
   combination (including all-off, which must equal the current #17 output) renders
   correctly and without panic.
5. **Retire the inert contract** — replace the `overlays_are_inert` test
   (`track/mod.rs`) with per-overlay difference tests; correct the "threaded but
   inert" / "deferred (Q2)" doc comments in `lib.rs::render_frame` and
   `track/mod.rs::draw_frame`; rename the now-used `_overlays` param to `overlays`;
   update `LAYER_ORDER` / draw order to include the new layers.
6. **Tests** — per-overlay unit assertions (ramp mapping, grid pitch = one lattice
   cell, pure-visual invariant, empty-metrics no-op) plus a wgpu **snapshot golden
   per overlay** on a metric-populated fixture track (Miri-ignored,
   `image-check`-verified, per the crate's house rule).

## Out of scope

- **Generating** the metrics (`speed_heatmap`, `fastest_lap`, `s_field`,
  `centerline`) — that is block 1 (`gp-gen`), not built yet. This task **consumes**
  the already-shipped `gp_core::track::TrackMetrics` contract (#6) and hand-populates
  test fixtures.
- The **default on/off state** of overlays and any toggle UI control — `gp-render`
  merely honors the flags it is handed; who constructs `Overlays` (the `gp-game`
  HUD) is a separate concern.
- The **`Centerline`** render-only ideal curve — a distinct §2 product, not part of
  #18's `fastest_lap` overlay.
- Any change to `gp-core`, the physics core, the corridor `D`, or the oracle.

## Deferred

- Heatmap ramp interpolation shape (discrete 4 buckets vs blended) | a defensible
  default is pinned below and either is testable at the stops | no separate issue.
- Grid extent (whole-canvas vs corridor-clipped) | low visual risk, design's call
  below | no separate issue.

## Key decisions

| Question | Decision |
|---|---|
| Which field drives the heatmap? | `TrackMetrics::speed_heatmap` (per-point max speed across live states), colored per asphalt cell. Not raw car speed. |
| Ramp normalization | Normalize each cell's speed across the frame's observed `[min, max]` of the present `speed_heatmap` values → position in `[0, 1]` → the 4-stop ramp. Rationale: full slow→fast contrast on every track without depending on `vmax_attain`. Degenerate: a single distinct value → all `HEAT_0`; an **empty** `speed_heatmap` → heatmap is a no-op (solid `ASPHALT_1`, as today). (Alternative absolute `[0, vmax_attain]` reference is an Open Question.) |
| Ramp interpolation | 4 stops `HEAT_0..HEAT_3`. Discrete-bucket vs piecewise-linear blend (Track.jsx gradient offsets `0 / 0.4 / 0.7 / 1.0`) is **design's choice** — the AC test asserts the endpoints (min speed → `HEAT_0`, max → `HEAT_3`) and monotonicity, which both satisfy. |
| Heatmap vs smoothed-mesh fill | PR #100 draws asphalt as a **smoothed rounded-boundary `Mesh`** (`regions::fill`, ear-clipped), **not** per-cell. Per-cell heatmap must layer on / replace that fill (e.g. per-cell colored squares clipped to `D`, over or instead of the solid mesh). Integration approach is **design's** (`regions.rs`). |
| fastest-lap style | A **thin smooth spline** through `metrics.fastest_lap`, `ACCENT`, semi-transparent, drawn over the asphalt (issue wording "thin smooth spline"). Smoothing method (Chaikin — already in-crate for walls — or Catmull-Rom) is design's choice. **Empty path → no-op.** |
| Grid pitch & alignment | Ruling at **one lattice cell** (transform-derived screen pitch via `TrackTransform`, so it aligns to the actual cells — not the fixed 24px `spacing::CELL` decorative-background token), major line every **5th** lattice line (`GRID_LINE_MAJOR`), dotted lattice (`GRID_DOT`) at cell corners. |
| Grid extent | Whole-canvas vs corridor-clipped-through-asphalt (Track.jsx does both a full faint grid and a lighter grid clipped to `D`) is **design's choice**; default to a single faint whole-canvas ruling + dots as the §4 "notebook sheet". |
| Semi-transparency | All overlays semi-transparent (issue). Exact alphas from the existing effect tokens / Track.jsx (asphalt tint ~0.9, fastest-lap ~0.9) are design's to finalize. |
| Layer placement (§4) | grid = layer 4 (over regions, behind analytics); heatmap = recolor of the asphalt fill (layer 1b); fastest-lap = layer 5 (over walls, under S-F/cars). `LAYER_ORDER` and `draw_frame`'s order updated and re-pinned by test. |
| Tokens / deps | All colors + metrics already exist — **no new dependency, no new token.** `color.rs`: `HEAT_0..HEAT_3`, `GRID_LINE`, `GRID_LINE_MAJOR`, `GRID_DOT`, `ASPHALT_1`, `ACCENT`. `effects.rs`: `BG_GRID_RULING_WIDTH`, `BG_GRID_COLOR`, `BG_DOTS_COLOR`, `BG_DOTS_RADIUS`. |

## Technical constraints

- **Draw-only.** Overlays are a pure function of `(rect, track, cars, overlays)`;
  no history, no clock, no `gp-core` mutation (the crate's established contract).
- **Retire inert artifacts.** `overlays_are_inert` (`track/mod.rs`) is now false and
  must be replaced. The doc comments claiming overlays are "threaded but inert" /
  "deferred (Q2)" in `lib.rs::render_frame` and `track/mod.rs::draw_frame` must be
  corrected. `_overlays` → `overlays`.
- **Fixtures carry empty metrics today.** `track/mod.rs::fixture_track()` and
  `track/golden.rs::scene_track()` set `metrics`/`s_field` to `default()` (empty).
  Overlay tests + goldens must hand-populate `metrics.speed_heatmap` and
  `metrics.fastest_lap` on a fixture (block 1 generator not yet built).
- **Miri.** Any wgpu snapshot golden must be `#[cfg_attr(miri, ignore = "<why>")]` —
  a red Miri blocks merge (workspace Miri gate). Pure-geometry overlay unit tests
  stay Miri-clean.
- **No production panics** (crate invariant). Empty/degenerate metrics → overlays
  degrade to no-ops, never index off the end.
- **Sub-cell math is `f32` in `gp-render`** (established; `TrackTransform`, wall
  smoothing already do this). The heatmap and spline use `f32` screen coords via the
  transform. `gp-core` remains integer-only and untouched.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | With `speed_heatmap` on, each asphalt cell is colored by its `metrics.speed_heatmap` max-speed on the `HEAT_0`→`HEAT_3` ramp (slowest→fastest); a unit test asserts the mapping — the min-speed cell maps to `HEAT_0`, the max-speed cell to `HEAT_3`, monotone between. |
| AC2 | With `fastest_lap` on, `metrics.fastest_lap` renders as a thin smooth spline in `ACCENT`; a test asserts it is pure-visual — toggling it changes only drawn shapes, and the corridor `D` / metrics are unchanged. |
| AC3 | With `grid` on, faint `GRID_LINE` ruling renders at the lattice cell pitch with heavier `GRID_LINE_MAJOR` lines every 5th and a `GRID_DOT` dotted lattice; a test asserts the grid line pitch equals one lattice cell. |
| AC4 | Each of the three overlays toggles independently — turning exactly one on changes output vs all-off, all-off equals the #17 baseline, and every combination renders without panic. |
| AC5 | The inert contract is gone: `overlays_are_inert` is replaced by difference tests, and no `render_frame` / `draw_frame` doc comment still claims overlays are inert or deferred; `_overlays` is renamed `overlays`. |
| AC6 | A wgpu snapshot golden per overlay (heatmap / fastest-lap / grid) on a metric-populated fixture track — each `#[cfg_attr(miri, ignore)]` and `image-check`-verified at mint. |
| AC7 | Empty metrics (default fixture) → `speed_heatmap` and `fastest_lap` overlays are no-ops (no panic; output for those layers equals the no-overlay frame). |
| AC8 | `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, the doc gate, and workspace Miri are all green. |

## Open questions

- **Heatmap normalization reference** — the pinned default normalizes across the
  frame's observed `[min, max]` (per-track contrast). The alternative is an absolute
  `[0, vmax_attain]` reference (`metrics.vmax_attain`), which makes the heatmap
  comparable across tracks but leaves a slow track mostly blue. Design may pick
  either; revisit if the product owner wants cross-track comparability.
- **Ramp interpolation shape** — discrete 4 buckets vs blended gradient (offsets
  `0 / 0.4 / 0.7 / 1.0`). Design's choice; either passes the AC1 stop assertions.
- **Grid extent** — single whole-canvas ruling vs additionally clipping a lighter
  grid through the asphalt (Track.jsx does both). Design's choice; low visual risk.
