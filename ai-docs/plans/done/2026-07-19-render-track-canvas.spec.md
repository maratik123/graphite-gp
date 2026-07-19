# gp-render: track canvas — regions, walls, S/F, cars + move animation

**Source:** issue #17
**Date:** 2026-07-19
**Tracked in:** #17

## Scope

Implement the hero track canvas in `gp-render` (crate `crates/render`, block 2),
drawing the corridor `D` and cars per design doc §4. `render_frame`
(`crates/render/src/lib.rs`, currently `todo!()`) becomes the real entry point.
Everything derives from the duality (design doc §0/§1/§3a): a point is a unit
cell center; a wall is a dual edge on the half-grid — so asphalt is *derived*
from `D`, not authored, and walls never cross an integer point by construction.

Layers to draw (back to front, design doc §4):

1. **Regions.** Outfield = paper background (`SURFACE_PAGE`/`PAPER_1`); infield
   (the bounded ¬D hole, so the loop reads) in a distinct tint
   (`SURFACE_INFIELD`/`PAPER_2`); asphalt = union of unit cells over the points
   of `D` (`TrackArtifact.corridor`), filled `SURFACE_ASPHALT`/`ASPHALT_1`.
2. **Walls.** The fill boundary of `D` on the half-grid (`TrackArtifact.walls`,
   from `gp_core::geom::walls_from_boundary`), stroked `WALL` — never through a
   point.
3. **S/F line.** The start/finish chord (`TrackArtifact.sf.chord`) in its
   distinct dashed/checkered style (per Track.jsx: alternating `GRAPHITE_900` /
   `PAPER_0` cells across the corridor).
4. **Cars (layer 6).** Each car a point (`GRAPHITE_900`-outlined colored dot),
   with a velocity-vector arrow (drawn direction/length ∝ `(vx, vy)`) and a
   fading trail of prior positions. Per-car color from the `CAR_COLORS` palette;
   optional "you" ring (Track.jsx / CarChip visual language).
5. **Move animation.** A car slides linearly (linear easing) along the chord
   `(x,y)→(x+vx,y+vy)`; `supercover` has already certified the chord ⊆ `D`
   (design doc §4). Reduced-motion respected (snap to final, no slide).

**Cosmetic Chaikin wall smoothing is in scope (Q1).** Walls stay the fill
boundary of `D` on the half-grid; the smoothing is purely cosmetic and, per M6
(design doc §4), must stay within the half-cell gap — never cross an integer
point, never enter a grazeable cell, never change the drivable set `D`. The
smoothing geometry ships with an M6 guard and tests (see Acceptance Criteria).

## Out of scope

- The block-1 generator (`gp-gen`) that produces a `TrackArtifact` at runtime is
  itself `todo!()`; tests build a `TrackArtifact` (or the sub-structures a layer
  needs) by hand. No runtime track generation here.
- Window / event loop / frame timing / OS reduced-motion detection — owned by
  `gp-game` (draw-only split, `ai-docs/key-decisions.md`). This task consumes a
  borrowed `Painter` and caller-supplied animation progress + reduced-motion.
- Graph-paper grid (layer 4) and analytics overlays (layer 5: `speed_heatmap`,
  `fastest_lap` line, ideal-line spline) — **deferred to a follow-up task (Q2)**,
  matching the issue's "§4 layers 1-3,6". The existing
  `Overlays { speed_heatmap, fastest_lap, grid }` flags are **not** wired to real
  rendering in this task — they stay inert / threaded through as no-ops. The
  Track.jsx grid-through-asphalt fix (light ruling clipped to the corridor) is
  captured here as a forward-reference so the follow-up task doesn't lose it.
- Physics/logic changes in `gp-core` — this task reads the artifact, never
  mutates `D`, the oracle, or supercover.

## Deferred

- Graph-paper grid (layer 4) + analytics overlays (layer 5: speed heatmap,
  fastest-lap line, ideal-line spline) | out of scope per "§4 layers 1-3,6"
  (Q2 — deferred) | separate follow-up issue needed; carry the Track.jsx
  grid-through-asphalt clip fix into it. The `Overlays` flags stay inert here.

## Key decisions

| Question | Decision |
|---|---|
| Entry point | `render_frame` stops being `todo!()` and draws layers 1,2,3,6 back-to-front. Its signature may evolve (AGENTS.md: clean breaks, no API-stability contract) — exact shape is the `design` Subagent's call. |
| Coordinate mapping | Map the corridor bounding box (`Corridor::origin`/`width`/`height`) into the painter's target rect (`painter.clip_rect()`), preserving aspect ratio. Lattice `y` increases northward (`Point`), egui screen `y` increases downward → the transform flips `y`. Cell size derived from the fit. |
| Asphalt = `D` | Draw one unit square (±0.5 cell) centered on each drivable point, or an equivalent traced fill, so the rendered asphalt cell set equals `D` exactly (testable). |
| Infield derivation | Outfield = ¬D cells reachable from the bbox border; infield = the remaining bounded ¬D component(s). Computed in `gp-render` from existing `gp-core` primitives (flood/complement), no `gp-core` change unless `design` finds it cleaner. |
| Car render input | `CarState` (`x,y,vx,vy`) carries no color/trail/identity. Extend the per-car render input (a render-facing struct: state + palette index/color + trail positions + "you" flag + animation progress). Caller (`gp-game`) supplies trail history and the animation clock; `gp-render` does **not** buffer history (draw-only). Exact struct shape → `design`. |
| Move-animation ownership | `gp-render` provides pure interpolation-at-progress drawing (`pos = lerp((x,y),(x+vx,y+vy),t)`, linear); the clock + reduced-motion decision come from the caller. Reduced-motion = progress snapped to final. |
| Wall smoothing (M6) | **Q1 → Chaikin now.** Implement cosmetic Chaikin smoothing this task, guarded so smoothed vertices stay within the half-cell gap and never enter a grazeable cell (M6), never altering `D`. Walls remain the fill boundary on the half-grid, never through an integer point. Ships with smoothing geometry + M6 guard tests. |
| Layers 4 & 5 | **Q2 → defer both.** Ship layers 1,2,3,6 only. Grid (layer 4) + analytics overlays (layer 5) are a later task. Do **not** wire the existing `Overlays { speed_heatmap, fastest_lap, grid }` flags to real rendering — keep them inert / no-op. Carry the Track.jsx grid-through-asphalt clip note into the follow-up. |

## Technical constraints

- **Draw-only.** Borrow the `Painter`; construct/own no window, event loop, or
  timer (`ai-docs/key-decisions.md`).
- **Colors from tokens.** Use `gp_render::tokens::color::*` (already ported from
  `docs/design-system/tokens/colors.css`) — `PAPER_1`, `PAPER_2`/`SURFACE_INFIELD`,
  `ASPHALT_1`/`SURFACE_ASPHALT`, `WALL`, `GRAPHITE_900`, `CAR_COLORS`, `ACCENT`
  (you-ring). No inline literals (AGENTS.md magic-numbers rule).
- **Wall geometry never through a point.** Wall edges live at half-integer
  offsets from cell centers; endpoints are `(±0.5, ±0.5)` corners. Assertable.
- **Sub-cell math stays in `gp-render`.** All fractional / interpolation
  arithmetic (the coordinate transform, trail fade, animation lerp) lives in
  `gp-render`. The physics core remains integer-only and deterministic (design
  doc §3a) — this task reads it, never adds fractional arithmetic to it.
- **Miri gate.** Any wgpu/`egui_kittest` golden test must be
  `#[cfg_attr(miri, ignore = "...")]` — a red Miri blocks merge (#76). Pure
  geometry unit tests stay Miri-clean and unignored.
- **File-size / test conventions.** Logic-bearing modules (~50+ lines) get a
  `#[cfg(test)] mod tests`; respect the soft 500 / hard 1000 file-size rule
  (`AGENTS.md`).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Asphalt renders as the union of unit cells over exactly `TrackArtifact.corridor`; a geometry unit test asserts the rendered asphalt cell set equals `D`. |
| AC2 | Infield (bounded ¬D hole) renders in `SURFACE_INFIELD`; outfield renders as `PAPER_1` paper background — visibly distinct from asphalt and each other. |
| AC3 | Walls render as the half-grid fill boundary of `D` (from `TrackArtifact.walls`); a geometry unit test asserts no wall segment/vertex coincides with any integer lattice point. |
| AC4 | S/F renders in its distinct dashed/checkered style along `TrackArtifact.sf.chord`. |
| AC5 | Cars render as `GRAPHITE_900`-outlined colored points, each with a velocity-vector arrow whose drawn direction and length match `(vx, vy)` (length ∝ speed), plus a fading trail; a unit test asserts the rendered vector matches `(vx, vy)`. |
| AC6 | Move animation places a car at `lerp((x,y),(x+vx,y+vy),t)` for progress `t∈[0,1]` (linear easing); a unit test asserts the interpolation at representative `t`. Reduced-motion snaps to the final position with no intermediate slide. |
| AC7 | Cosmetic Chaikin wall smoothing is applied to the wall polylines and, per M6, smoothed vertices stay within the half-cell gap (±0.5 cell of the block boundary) and never enter a grazeable cell; a geometry unit test asserts both bounds and that the smoothing does not change `D`. |
| AC8 | A wgpu golden snapshot test (`egui_kittest`, `#[cfg_attr(miri, ignore)]`) captures the back-to-front region/layer order (outfield → asphalt → infield → walls → S/F → cars). |
| AC9 | `render_frame` is no longer `todo!()` and draws layers 1,2,3,6 back-to-front (grid/analytics layers 4,5 deferred — the `Overlays` flags stay inert / no-op this task). `gp-render` stays draw-only; per-car render input (color/identity, trail, animation progress, you-flag) is caller-supplied. |
| AC10 | Gates green: `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc`, `cargo test`, and workspace Miri (FFI/wgpu tests gated). |

## Open questions

- None. Both round-1 forks are resolved by the product owner: **Q1 → Chaikin
  smoothing in scope (M6-guarded)**; **Q2 → grid (layer 4) + analytics overlays
  (layer 5) deferred, `Overlays` flags inert**. See Key decisions.
