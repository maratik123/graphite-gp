# Design: Render overlays — speed heatmap, fastest-lap spline, notebook grid

**Issue:** #18 (gp-render: analytics overlays — speed heatmap, fastest-lap line, graph-paper grid+dots)
**Date:** 2026-07-19
**Spec:** `ai-docs/plans/2026-07-19-render-overlays.spec.md`
**Amendment 2026-07-20 (owner-directed, no spec change):** the **speed-heatmap
integration** changed from per-cell blocky `rect_filled` squares to recoloring
the **shared smoothed asphalt mesh** per-cell via `with_clip_rect` — the shipped
squares regressed #100's smoothed boundary (owner: "the fills should use the same
smoothed boundary the walls trace"). Sections touched: § Approach → Draw order +
Key decisions 1, § Risks (heatmap bleed RESOLVED), § Decomposition subtask 1 + 5,
§ Test Design subtask 1. `fastest_lap` / `grid` unaffected.

## Approach

#17 (PR #100) drew design-doc §4 layers 1–3 + 6 and threaded the three
`gp_render::Overlays` flags **inert** (`draw_frame`'s param was `_overlays`; an
`overlays_are_inert` test pinned byte-identical output when they toggled). This
task made each flag drive a real, individually-toggleable, semi-transparent,
pure-visual layer and **retired** that inert scaffolding — **all five subtasks
below are already committed**: `draw_frame` reads `overlays` and wires the three
conditional overlay draws, and `overlays_are_inert` is deleted
`[measured: read track/mod.rs:64-106 → draw_frame(.., overlays); speed_heatmap
wired :86-88, grid :90-92, fastest_lap :96-98]`. This amendment is a **delta
against that committed baseline**, reworking only the speed-heatmap integration
(§ Key decisions 1); the design below documents the whole feature for the record,
with the heatmap subtask stated as its committed-state delta (§ Decomposition
subtask 1).

The crate's **house pattern** is one submodule per §4 layer, each a pure
lattice-space geometry fn (Miri-clean) plus a thin `pub(crate) paint` that maps
to screen via `TrackTransform` and strokes/fills
`[measured: read crates/render/src/track/{car,sf,walls,regions,transform}.rs]`.
I follow it exactly: **three new sibling modules** `track/heatmap.rs`,
`track/fastest_lap.rs`, `track/grid.rs`, each with a pure geometry/color core +
a `paint`. `draw_frame` gains three conditional calls. **No `gp-core` change,
no new dependency, no new token** — every color + effect const already exists
(`color.rs`: `HEAT_0..HEAT_3`/`HEAT_RAMP`, `GRID_LINE`/`GRID_LINE_MAJOR`/
`GRID_DOT`, `ASPHALT_1`, `ACCENT`; `effects.rs`: `BG_GRID_RULING_WIDTH`/
`BG_GRID_COLOR`/`BG_DOTS_RADIUS`/`BG_DOTS_COLOR`), and `egui`/`egui_kittest`/
`egui-wgpu`/`image` are already deps
`[measured: read color.rs:41-45,50,59,84-91,155-156; effects.rs:109-120; render/Cargo.toml:15-25]`.

### Draw order / `LAYER_ORDER` (AC5, AC4-baseline invariant)

The spec's relational placements — heatmap "recolor of the asphalt fill (1b)",
grid "over regions, behind analytics", fastest-lap "over walls, under S-F/cars"
— do **not** follow strict §4 numeric order (fastest-lap is layer 5 yet drawn
*under* the layer-3 S/F). Resolving the relational constraints into a coherent,
Track.jsx-consistent order (grid is a background sheet, drawn under the ink):

```
outfield → asphalt → infield → [heatmap] → [grid] → walls → [fastest_lap] → sf → cars
```

`[…]` = conditional (drawn only when its flag is on). **Critically, deleting
the three conditionals yields exactly the #17 order** `outfield → asphalt →
infield → walls → sf → cars` `[measured: track/mod.rs:35]`, so all-off output is
byte-identical to #17 — the existing `track.png` golden (rendered with
`Overlays::default()`) stays valid with **no re-mint**, and AC4's "all-off
equals the #17 baseline" holds *by construction* (the overlay `paint` fns are
not called when their flag is off, so they add zero shapes).

`regions::fill` draws outfield+asphalt+infield in one monolithic call
`[measured: read regions.rs:400-428]`, and its 2-mesh output is pinned by
`fill_emits_asphalt_mesh_then_infield_mesh` (`regions.rs:697`). Rather than
split that tested #100 fn to slot heatmap between asphalt and infield, I draw
heatmap **after** `regions::fill`, at layer 1b. The heatmap recolors the shared
smoothed asphalt mesh per-cell and then re-cuts the infield holes as
`SURFACE_INFIELD` (§ Key decisions 1), mirroring `fill`'s own
asphalt-then-infield-on-top structure — so the infield reads correctly whether
or not the overlay is on, at zero risk to #100's `fill` (which is recomposed from
the shared `pub(crate)` primitives `triangulated_loop`/`paint_mesh`/
`paint_infield_holes`, preserving its exact 2-mesh output — § Key decisions 1).
`LAYER_ORDER`
(currently `[&str; 6]`) becomes the 9-entry list
`["outfield","asphalt","infield","heatmap","grid","walls","fastest_lap","sf","cars"]`,
re-pinned by `layer_order_is_documented`; each overlay subtask extends the const
as it wires its draw so the const equals the actual draw order at every gate.

### Key decisions (resolving the spec's deferred choices)

1. **Heatmap integration — recolor the SHARED smoothed asphalt mesh per-cell
   (owner-directed amendment, 2026-07-20).** *Supersedes the original
   "per-cell ±0.5 squares over the asphalt mesh" approach, which regressed
   #100's smoothed boundary: raw `painter.rect_filled` squares give a blocky
   staircase silhouette that pokes past the Chaikin-smoothed boundary the walls
   trace — exactly the #17 corner-fill class this design's own § Risks flagged.*
   Owner directive verbatim: "the fills should use the same smoothed boundary the
   walls trace." The heatmap now draws the **same smoothed asphalt mesh
   `regions::fill` draws**, colored per-cell via a rectangular clip, so its outer
   silhouette IS the smoothed boundary by construction.

   `draw_frame` already computes `smoothed_loops: Vec<Vec<(f32,f32)>>` (Chaikin
   loops) and `loop_roles = regions::classify_loops(&smoothed_loops)`, and passes
   them to `regions::fill` `[measured: read track/mod.rs:72-84]`. Pass the **same**
   `&smoothed_loops` + `&loop_roles` to `heatmap::paint` (signature change). For
   each `(cell, speed)` in `speed_heatmap`: `color =
   ramp_color(normalize(…)).gamma_multiply(HEATMAP_ALPHA=0.9)`, then draw the
   **outer** asphalt mesh in `color` through
   `painter.with_clip_rect(cell_rect(transform, cell))`. `with_clip_rect` returns
   a sub-`Painter` whose clip is the INTERSECTION of the given rect and the
   parent clip `[measured: read egui-0.35.0/src/painter.rs:67-75 → clip_rect =
   rect.intersect(parent.clip_rect)]`, so the clip scissors the mesh to that one
   cell; the union over all cells is the full smoothed asphalt, colored per-cell,
   with the outer edge tracing the smoothed boundary exactly. **Infield re-cut:**
   the outer mesh fills the hole region too, so per-cell coloring near the hole
   would bleed heatmap into the infield — after the per-cell pass, redraw each
   `roles.holes` loop as `SURFACE_INFIELD` mesh on top (mirrors `regions::fill`'s
   own asphalt-then-infield-on-top structure,
   `[measured: read regions.rs:400-428 → outer SURFACE_ASPHALT (:414) then holes
   SURFACE_INFIELD (:424)]`). Net: heatmap = smoothed outer boundary **minus
   holes**, per-cell colored. Cell rect: `Rect::from_two_pos(map((x-0.5,y-0.5)),
   map((x+0.5,y+0.5)))`, y-flip handled by `from_two_pos` as `sf::bar_rect`.

   **Triangulate ONCE, reuse the shared index buffer per cell** (perf — the naive
   "one `paint_loop_mesh` call per cell" re-ear-clips the outer loop every call,
   `O(K·V²)` for `K` heatmap cells and `V` smoothed-loop vertices, a material
   per-frame regression over the original `O(K)` squares). Instead: ear-clip the
   outer asphalt loop **once** into a screen-space `(Vec<Pos2> verts, Vec<[u32;3]>
   indices)`, then for each cell build a per-cell `Mesh` **reusing that shared
   index buffer** — only the per-cell vertex `color` and the `with_clip_rect(cell)`
   scissor vary. Cost drops to `O(V²)` (one ear-clip) `+ O(K·V)` (K mesh-builds).

   **Reuse, not duplication** (per the ≥3-site / coordinator steer): `regions.rs`
   already has `paint_loop_mesh` (private, `:367` — one smoothed loop → solid-color
   ear-clipped `Mesh`), `classify_loops` (`:212`), `triangulate` (`:334`)
   `[measured: grep regions.rs]`. Split `paint_loop_mesh` into two `pub(crate)`
   primitives the heatmap needs to triangulate-once: `regions::triangulated_loop(
   transform, loop_points) -> (Vec<Pos2>, Vec<[u32;3]>)` (map to screen + ear-clip,
   **once**) and `regions::paint_mesh(painter, verts, indices, color)` (build a
   colored `Mesh` from *shared* verts+indices and add it) — `paint_loop_mesh` then
   composes the two. Also add `regions::paint_infield_holes(painter, transform,
   loops, roles, color)` (the re-cut, used by both `fill` and heatmap). `fill`
   recomposes as `rect_filled(SURFACE_PAGE)` + per-outer-loop
   `triangulated_loop`+`paint_mesh(.., SURFACE_ASPHALT)` +
   `paint_infield_holes(.., SURFACE_INFIELD)` — its 2-mesh output + colors + order
   are preserved, so `fill_emits_asphalt_mesh_then_infield_mesh` (`regions.rs:697`)
   still passes. `heatmap::paint` calls `triangulated_loop` **once** on the outer
   loop, then `paint_mesh(&clip, &verts, &indices, color)` per cell, then
   `paint_infield_holes(painter, .., SURFACE_INFIELD)` once. No triangulation is
   re-implemented — or re-run per cell — in `heatmap.rs`.

   Drawn at layer 1b, still after `regions::fill` and under the walls (draw order
   unchanged). AC1 per-cell granularity is preserved (crisp per-cell color blocks;
   only the *silhouette* becomes the smoothed boundary). The pure geometry/color
   core (`speed_bounds`/`normalize`/`ramp_color`, Miri-clean) is **unchanged** —
   only `paint` (already non-Miri-tested, touches `Painter`) changes.
   Rejected: recolor the single asphalt `Mesh` per-triangle — the ear-clipped
   triangles span the whole corridor, not one cell, so they cannot express
   per-cell speed (fails AC1 granularity); the clip approach reuses that same mesh
   but scissors it per cell, keeping both the smoothed silhouette and per-cell
   granularity.

2. **Heatmap ramp — piecewise-linear blend across `HEAT_0..HEAT_3` at uniform
   stops (0, ⅓, ⅔, 1).** Chosen over discrete 4-buckets for per-cell speed
   *resolution* (the heatmap's whole purpose); both satisfy AC1. Normalization
   (spec's pinned default): over the frame's observed `[min,max]`, `range =
   max.saturating_sub(min)` (never raw `-` — `arithmetic_side_effects` is deny),
   `t = if range == 0 { 0.0 } else { speed.saturating_sub(min) as f32 / range
   as f32 }`; `range == 0` (single distinct value) → `t = 0` → all `HEAT_0`;
   empty `speed_heatmap` → no-op.
   Endpoints are **exact** (blend at `t=0`→`HEAT_0`, `t=1`→`HEAT_3`, since the
   lerp reduces to the endpoint channel byte with no rounding) and `t` is linear
   in speed → monotone — satisfying AC1's stop + monotonicity assertions
   `[derived → subtask-1 AC1 ramp_color tests]`.

3. **Fastest-lap — uniform Catmull-Rom, dashed `ACCENT`.** An interpolating
   spline (passes *through* each `metrics.fastest_lap` cell center — matching the
   issue's "spline through the path", and giving an exact, parameterization-free
   unit assertion: the sampled curve contains every control point at its knot).
   **Uniform**, not centripetal: centripetal adds a `sqrt`/`powf` per span **and**
   a divide-by-zero on coincident control points — a NaN hazard against the
   crate's no-panic/no-NaN posture — whereas uniform has fixed parameter spacing
   (no div-by-zero) and no `sqrt` (no Miri last-bit sensitivity). Uniform's only
   cost is mild overshoot on sharp lattice turns, which design-doc §4 rules
   cosmetically free ("валидна by construction, сглаживать свободно, на коллизии
   не влияет" §310) and which the AC6 golden + `image-check` gate. Rejected:
   Chaikin — approximating (does not pass through the points), and the in-crate
   `chaikin_smooth` is M6-*guarded* + `DualCorner`-closed-loop specific, so
   reusing it for an open `Vec<Point>` lap would mean a new unguarded variant
   anyway (no real reuse). Style: dashed `2 6` scaled by `CELL_SM` (mirrors
   `car.rs`'s `YOU_RING_*` dash idiom; `car.rs:31` already reserves "2 6" for
   "the deferred fastest-lap ideal-line overlay"), width `BW_2`, round-ish caps,
   `gamma_multiply(FASTEST_LAP_ALPHA=0.9)` `[measured: Track.jsx:76 → strokeWidth
   2, strokeDasharray "2 6", opacity 0.9; car.rs:28-35]`. Splined in **lattice
   space** then transformed (resolution-independent, testable).

4. **Grid — single faint whole-canvas notebook sheet (spec default extent).**
   Ruling + dots aligned to the corridor's actual lattice via `TrackTransform`,
   pitch = one lattice cell = `transform.cell_size()` (AC3), **not** the fixed
   24 px `spacing::CELL` decorative token. Ruling lines run at cell-corner
   (half-integer) lattice lines anchored to the bbox min corner; **major** line
   every 5th (`k.rem_euclid(5) == 0`) in `GRID_LINE_MAJOR` (the darker token is
   the "heavier" distinction; width stays `BG_GRID_RULING_WIDTH`); minor in
   `GRID_LINE`; a `GRID_DOT` dot (`BG_DOTS_RADIUS`) at every cell corner
   (ruling-line intersection). Grid uses the faint `GRID_*` tokens at **full**
   alpha — they already encode the intended faintness, and the design system's
   own `BG_GRID`/`BG_DOTS` render them solid `[measured: effects.rs:107-120]`;
   an extra alpha would double-faint them to near-invisibility. Whole-canvas
   (not corridor-clipped) per the spec default; drawn over the filled regions,
   under the walls.

### Constants (magic-number rule)

New module-level consts: `HEATMAP_ALPHA = 0.9`, `FASTEST_LAP_ALPHA = 0.9`,
`FASTEST_LAP_WIDTH = BW_2`, `FASTEST_LAP_DASH_FACTOR = 2.0/CELL_SM`,
`FASTEST_LAP_GAP_FACTOR = 6.0/CELL_SM` (each with a comment citing Track.jsx:58/76
or car.rs), and a defensive `MIN_GRID_PITCH_PX` guard in `grid::paint`. Grid
reuses `BG_GRID_RULING_WIDTH`/`BG_DOTS_RADIUS`/`GRID_LINE`/`GRID_LINE_MAJOR`/
`GRID_DOT` directly.

### Lint / const posture (binding constraint)

Workspace `pedantic` + `nursery` are `deny`, `arithmetic_side_effects` is `deny`
`[measured: Cargo.toml:46-58]`. Consequences the implementer must honor:

- Integer speed math uses `saturating_sub` (never raw `-`); `i32 as f32` /
  `f32 as u8` / `f32 as usize` casts carry the crate's standard
  `#[allow(clippy::cast_precision_loss | cast_possible_truncation | cast_sign_loss,
  reason = …)]` (precedent throughout `walls.rs`/`car.rs`/`sf.rs`). f32
  arithmetic does **not** trip `arithmetic_side_effects` (precedent:
  `regions::signed_area` does unguarded f32 `*`/`-`/`/`).
- `missing_const_for_fn` (nursery, deny) is the **authority** on `const`:
  `normalize` (saturating_sub + cast + f32 div + branch, no fused-multiply
  pattern) is const-eligible → **must be `const fn`** — precedent `sf.rs::
  bar_rect_lattice` is a `const fn` doing f32 arithmetic
  `[measured: read sf.rs:57; probe: rustc 1.97.1 compiled `const fn { (af+(bf-af)*t) as u8 }`]`.
  `ramp_color`'s channel lerp uses `mul_add` (house style / `suboptimal_flops`);
  `f32::mul_add` is not const, so `missing_const_for_fn` correctly will **not**
  fire and it stays non-const. `paint`/`line_coords`/`catmull_rom` are non-const
  (touch `Painter` / allocate a `Vec`). **Implement, run `cargo clippy
  --workspace --all-targets -- -D warnings`, and add/drop `const` exactly as the
  lint directs — never preemptively.** `ecolor 0.35`'s `Color32::from_rgb`,
  `r()`/`g()`/`b()`, `to_array()` are `const fn`; `gamma_multiply` is not
  `[measured: grep ecolor-0.35.0/src/color32.rs:108,213-256,294]`.
- **No production panics** (crate invariant): every overlay degrades on
  empty/degenerate input to a no-op (heatmap/fastest-lap empty → nothing drawn;
  `cell_size <= 0` → grid no-op). Ramp/segment indices into the fixed
  `[Color32; 4]` are clamped in-range before indexing — bounded, never OOB. (The
  zero-panic *index* invariant is `gp-core`-scoped, `ai-docs/panic-index.md`;
  `gp-render` already indexes with bounded indices, e.g. `regions::find_ear`.)

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Heatmap → smoothed-mesh recolor (DELTA against committed baseline).** `track/heatmap.rs`, `mod heatmap;`, the `_overlays`→`overlays` rename, `LAYER_ORDER`/`layer_order_is_documented`, and the `draw_frame`/`render_frame` doc corrections are **already committed** (§ Approach); the pure core `normalize`/`ramp_color`/`speed_bounds` is **unchanged**. This amendment's open delta: (a) **`heatmap::paint`** — change approach + signature to `paint(painter, transform, &smoothed_loops, &loop_roles, heatmap)`: recolor the shared smoothed asphalt mesh per-cell via `with_clip_rect` (triangulate outer loop **once**, reuse the index buffer per cell — § Key decisions 1) + infield re-cut, **replacing** the shipped blocky `rect_filled` squares; update the `draw_frame` call site (`track/mod.rs:86-88`) to pass `&smoothed_loops` + `&loop_roles` (both already in scope). (b) **`track/regions.rs`** — split `paint_loop_mesh` into `pub(crate) triangulated_loop` (map+ear-clip once → verts+`[u32;3]`) + `pub(crate) paint_mesh` (colored `Mesh` from shared verts+indices), add `pub(crate) paint_infield_holes`; recompose `fill` from them (its 2-mesh output preserved → `fill_emits_asphalt_mesh_then_infield_mesh` still passes). (c) **heatmap paint unit test** → the Mesh assertions (§ Test Design subtask 1). (d) **re-mint `track_heatmap.png`** (subtask 5). | `track/heatmap.rs`, `track/regions.rs`, `track/mod.rs` (call site) | — |
| 2 | **Fastest-lap module + wire.** New `track/fastest_lap.rs`: `catmull_rom` (open polyline, clamped-endpoint, fixed segments/span, `<2` pts → no-op) + `paint` (lattice→spline→screen, dashed `ACCENT`@0.9) + unit tests. `track/mod.rs`: `mod fastest_lap;`, wire `if overlays.fastest_lap { … }` **between `walls::paint` and `sf::paint`**, extend `LAYER_ORDER`. | `track/fastest_lap.rs` (new), `track/mod.rs` | 1 |
| 3 | **Grid module + wire.** New `track/grid.rs`: `line_coords` (pure: line screen-positions + `is_major` over a range, given anchor+pitch) + `paint` (whole-canvas ruling minor/major + `GRID_DOT` lattice, `cell_size<=0`/`<MIN_GRID_PITCH_PX` guards) + unit tests. `track/mod.rs`: `mod grid;`, wire `if overlays.grid { grid::paint(painter, rect, &transform) }` **between the heatmap block and `walls::paint`**, extend `LAYER_ORDER` to the final 9-entry list. | `track/grid.rs` (new), `track/mod.rs` | 1 |
| 4 | **AC4/AC5/AC7 behavioral tests.** In `track/mod.rs` `#[cfg(test)]`: delete `overlays_are_inert`; add a metric-populated unit fixture; add per-overlay difference tests (each single flag on ≠ all-off), all-8-combinations-render-without-panic, all-off == metrics-independent baseline, heatmap/fastest-lap no-op on empty metrics (AC7), and fastest-lap pure-visual (D/metrics unchanged across a full render, AC2). | `track/mod.rs` | 1, 2, 3 |
| 5 | **AC6 per-overlay wgpu goldens.** In `track/golden.rs`: a `scene_metrics`-populated fixture (spatially-graded `speed_heatmap` over the corridor cells + a `fastest_lap` loop path); three `#[cfg_attr(miri, ignore)]` goldens `track_heatmap` / `track_fastest_lap` / `track_grid`, each single-flag-on, CPU-adapter-asserted, exact-compare (`threshold(0.0)`+`failed_pixel_count_threshold(0)`), `image-check`-verified at mint. **Amendment (2026-07-20):** `track_heatmap.png` MUST be **re-minted** under the smoothed-mesh recolor approach (the shipped blocky-square golden is the regression the owner flagged) and re-verified by `image-check` **and** owner-visual before merge. `track_fastest_lap.png` / `track_grid.png` unaffected. | `track/golden.rs` | 1, 2, 3 |

Scope: 5 atomic subtasks (< 15). All change-type **code** (`*.rs`). **All five
are already committed** (§ Approach); rows 2–5 stand as the feature-of-record.
The **only open work** is subtask 1's committed-state delta above (heatmap → the
smoothed-mesh recolor) and the subtask-5 `track_heatmap.png` re-mint it forces.

## Handoff plan

Per `.claude/skills/task/SKILL.md` Step 8 + `reference.md` § *Every-group
handoff*, boundaries are pre-computed here so Step 8 reads them. `M = 5`; all
five subtasks are **code** change-type (`*.rs` only — no `*.md`/`.claude/**`/
`AGENTS.md`/`ai-docs/**` edits), so change-type homogeneity groups them into
**one** group, which is also the minimum (§ Rules (e)–(h)).

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent, 1M-token window — subtasks 1, 2, 3, 4, 5 (code
  change-type: `*.rs`). Terminal group (5 subtasks; within the `1..=10` range,
  ≤ the size cap 10). Sequential order 1 → 2 → 3 → 4 → 5 respects dependencies
  (2, 3 depend on 1; 4, 5 depend on 1–3).
- **Handoff into Group A:** at the start of the group, spawn `/context-reset`
  per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
  Being the single, terminal group, Group A completes Step 8 in its own
  `/context-reset` subagent; there is no inter-group handoff.

1 group total — within the default max of 4; no user gate needed. The `design`,
`design-review`, `self-review` Opus quality gates are unaffected by the code
group marker (only the per-group *implementor* is `sonnet`/`code-writer`).

## Risks

- **Heatmap convex-corner bleed — RESOLVED by construction** (owner-directed
  amendment, 2026-07-20). The original per-cell blocky `rect_filled` squares
  poked past the smoothed boundary (the #17 corner-fill class) — this shipped and
  the owner flagged it on the minted `track_heatmap.png`. The former "clip the
  outer silhouette" *fallback is now the primary approach*: the heatmap recolors
  the **shared smoothed asphalt mesh** per-cell via `with_clip_rect`, so its outer
  silhouette IS the Chaikin boundary the walls trace — there is no blocky
  staircase to poke past it (§ Key decisions 1). Residual: at *concave* outer
  bulges (exotic outer-notch tracks — **not** the convex-boundary scene golden) a
  hairline of the mesh may extend beyond the per-cell ±0.5 clip coverage and show
  base `ASPHALT_1` — coherent (underlying asphalt, inside the wall), never a
  wrong color outside the wall — `[derived → the RE-MINTED AC6 heatmap golden +
  image-check + owner-visual re-verify]`.
- **Heatmap per-cell scissored-draw cost**: `K` cells each emit one clipped
  `Mesh` sharing a **single** outer-loop triangulation (`O(V²)` once + `O(K·V)`
  builds — § Key decisions 1), so no per-cell re-ear-clipping. Bounded and
  acceptable for an optional, turn-based (non-animation-critical) overlay —
  `[derived → shared-index-buffer heatmap::paint + cargo build]`.
- **Uniform Catmull-Rom overshoot on sharp lattice turns** (cosmetic bulge,
  possibly outside the corridor): design-doc §4 rules lap smoothing physically
  irrelevant; bounded and gated — `[derived → AC6 fastest_lap golden +
  image-check]`. No NaN risk (uniform CR has no divide-by-zero; `<2` points →
  no-op) — `[derived → workspace Miri gate + the empty-path no-op unit test]`.
- **All-off must stay byte-identical to #17** (else the existing `track.png`
  golden breaks): overlay `paint` fns are only called inside `if overlays.flag`,
  so all-off adds zero shapes — `[derived → the all-off==metrics-independent
  baseline unit test (subtask 4) + the existing `track_canvas_matches_golden`
  staying green]`.
- **`const`/lint churn** (`missing_const_for_fn`, `arithmetic_side_effects`,
  `cast_*`, `suboptimal_flops` all deny/nursery): the design pins `normalize` as
  const-eligible and defers every other `const`/cast/`mul_add` decision to the
  gate — `[derived → cargo clippy --workspace --all-targets -- -D warnings]`.
- **New goldens must not red the workspace Miri job** (a red Miri blocks merge):
  each golden carries `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the
  Vulkan ICD")]` (mirrors `golden.rs:134`); all overlay unit tests are pure
  geometry/color (no wgpu) → Miri-clean — `[derived → workspace Miri gate
  (MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace)]`.
- **Grid runaway shape count if `cell_size` is tiny**: `grid::paint` guards
  `cell_size <= 0` and `< MIN_GRID_PITCH_PX`; `line_coords`' `k`-range is bounded
  by `rect_size / cell_size` (finite) — `[derived → degenerate-transform grid
  unit test (no panic, bounded output)]`.

## Test Design

All unit tests are Miri-clean (no wgpu, no `Painter` where avoidable — geometry/
color fns are pure); the three goldens are Miri-ignored. Follow the crate's
`assert_f32`/`css::assert_f32` tolerance idiom for f32; use exact `assert_eq!`
where values are bit-stable (integer, or transform-mapped/knot points).

**Subtask 1 — `track/heatmap.rs`** (`#[cfg(test)] mod tests`):
- `normalize`: `normalize(min, min, max) → 0.0` (min→HEAT_0 end), `normalize(max,
  min, max) → 1.0` (max→HEAT_3 end), monotone (`a<b ⇒ normalize(a,..) ≤
  normalize(b,..)`), degenerate `max==min → 0.0`, empty via `speed_bounds(&[]) →
  None` → paint no-op. (AC1, AC7)
- `ramp_color`: `ramp_color(0.0) == HEAT_0`, `ramp_color(1.0) == HEAT_3`,
  intermediate `t` differs from both endpoints (blend, not snap), `t` outside
  `[0,1]` clamped (no panic/OOB index). (AC1)
- `paint` (**amended 2026-07-20** — now emits `Shape::Mesh`, not `Shape::Rect`,
  and needs the smoothed `loops` + `roles`): over the ring fixture, build
  `smoothed_loops` + `roles` exactly as `draw_frame` does (`chain_walls` →
  `chaikin_smooth` → `classify_loops`), then `paint(painter, transform,
  &smoothed_loops, &roles, heatmap)`. With a K-cell hand-populated heatmap over a
  1-outer-loop + H-hole corridor, assert the emitted `Mesh` count `== K *
  roles.outer.len() + H` (K per-cell clipped outer-asphalt meshes + H infield
  re-cut meshes), the infield re-cut mesh(es) are `SURFACE_INFIELD`-colored, and
  a per-cell mesh's first-vertex color equals the expected
  `ramp_color(normalize(..)).gamma_multiply(HEATMAP_ALPHA)` — capturing the
  per-cell recolor and the smoothed-silhouette reuse. **Empty heatmap → no shapes
  at all** (the `speed_bounds`-`None` early return precedes the infield re-cut, so
  AC7 stays a true no-op — heatmap-on-empty adds zero shapes). Uses the
  `run_ui` shape-capture idiom from `regions.rs:697`
  (`fill_emits_asphalt_mesh_then_infield_mesh`).

**Subtask 2 — `track/fastest_lap.rs`**:
- `catmull_rom`: for hand-built control points, the sampled polyline **contains
  every control point** at its knot index (0, seg, 2·seg, …) — exact (uniform CR
  `q(0)=p_i`); `len < 2` → returns input unchanged / no-op; a 2-point path →
  a straight segment through both. (AC2 geometry)
- `paint`: empty `fastest_lap` → no shapes; a populated path → ≥1 dashed shape.

**Subtask 3 — `track/grid.rs`**:
- `line_coords`: consecutive positions differ by **exactly `cell_size`** (AC3
  pitch), a `major` flag every 5th line, coverage spans the requested range;
  `cell_size <= 0` → empty.
- `paint`: over a known transform (e.g. `cell_size == 10`) emits ≥1 ruling +
  dots and does not panic; degenerate transform (`cell_size == 0`) → no shapes,
  no panic.

**Subtask 4 — `track/mod.rs` behavioral** (replaces `overlays_are_inert`):
- Fixtures: `fixture_track()` (empty metrics, kept for AC7) + a
  `fixture_track_with_metrics()` (hand-populated `speed_heatmap` + `fastest_lap`).
- `each_overlay_changes_output_when_on` — on the populated fixture, each of the
  3 single-flag renders `!=` the all-off render. (AC4/AC5)
- `all_overlay_combinations_render_without_panic` — all 8 flag combinations
  render ≥1 shape, no panic. (AC4)
- `all_off_equals_metrics_independent_baseline` — `render(populated, all-off) ==
  render(empty-metrics, all-off)` (proves all-off is exactly the #17
  metrics-independent baseline). (AC4)
- `heatmap_is_noop_on_empty_metrics`, `fastest_lap_is_noop_on_empty_metrics` —
  empty fixture, single flag on `==` all-off. (AC7)
- `fastest_lap_paint_does_not_mutate` — corridor cell contents + metrics equal
  before/after a full `fastest_lap`-on render (pure-visual; mirrors
  `walls.rs::corridor_is_unchanged_by_smoothing`). (AC2)
- `layer_order_is_documented` re-asserts the already-final 9-entry list
  (subtask 3 extends `LAYER_ORDER` to its final 9 entries and updates this test;
  subtask 4 only re-confirms it alongside the behavioral tests). (AC5)

**Subtask 5 — `track/golden.rs`** (AC6, all `#[cfg_attr(miri, ignore)]`,
`image-check`-verified at mint):
- `scene_metrics()`: spatially-graded `speed_heatmap` across the rounded-rect
  corridor cells (so the ramp spans `HEAT_0`→`HEAT_3`, and a convex corner is
  covered to expose any bleed) + a `fastest_lap` loop of cell centers.
- `heatmap_overlay_matches_golden` (`Overlays{ speed_heatmap:true, .. }`, snapshot
  `track_heatmap`), `fastest_lap_overlay_matches_golden` (`track_fastest_lap`),
  `grid_overlay_matches_golden` (`track_grid`, metrics irrelevant). Each mirrors
  `track_canvas_matches_golden` (CPU/lavapipe adapter assert, outfield probe,
  `SnapshotOptions.threshold(0.0).failed_pixel_count_threshold(0)`). New PNGs
  land in `crates/render/tests/snapshots/` (git-tracked, no LFS)
  `[measured: find → snapshots/*.png tracked; .gitattributes has no png rule]`.

Every factual claim above is tagged `[measured: …]` or `[derived → …]`; the
spawn contract in § Handoff plan cites the routing rules by section.

## Open questions

- **Heatmap normalization reference** (spec Open question 1) — the pinned
  default normalizes across the frame's observed `[min,max]` (per-track
  contrast). The alternative absolute `[0, vmax_attain]` reference
  (`metrics.vmax_attain`) makes heatmaps comparable across tracks but leaves a
  slow track mostly blue. This design implements the pinned per-track default;
  revisit only if the product owner wants cross-track comparability. No blocker.
