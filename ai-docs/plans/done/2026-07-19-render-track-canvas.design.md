# Design: gp-render track canvas — regions, walls, S/F, cars + move animation

**Issue:** #17
**Date:** 2026-07-19

## Approach

Turn `render_frame` (`crates/render/src/lib.rs`, currently `todo!()`
`[measured: rg -n render_frame --type rust → only lib.rs decl + placeholder.rs
doc-comments; no callers]`) into the real block-2 entry point that draws design
doc §4 layers **1 (regions), 2 (walls), 3 (S/F), 6 (cars)** back-to-front, with
layers 4 (grid) and 5 (analytics) deferred (Q2) and the existing `Overlays`
flags threaded inert. Everything derives from the duality (design doc §0/§1/§3a):
asphalt is *derived* from the corridor `D`, walls are the fill boundary on the
half-grid, and all sub-cell arithmetic (transform, trail fade, animation lerp,
Chaikin) lives in gp-render — the gp-core physics core is never touched
(`[measured: read crates/core/src/{geom/mod.rs,track.rs,sim/mod.rs} → integer-only,
this task only reads TrackArtifact]`).

**House pattern (adopted).** Every sibling widget splits into a pure
`resolve`/geometry layer (Miri-clean, no `egui::Ui`, no allocation), a
`pub(crate) paint(painter, rect, …)` layer, and a `pub show`/entry layer
(`[measured: read crates/render/src/widgets/{mod.rs,movepad.rs,car_chip.rs} →
resolve→paint→show, style only from crate::tokens]`). This task keeps that
split: each layer is a `track/` submodule with a **pure geometry fn** producing
lattice-space vertices/cell-sets (unit-testable, Miri-clean) and a thin `paint`
fn that transforms those to screen `Pos2` and strokes/fills them. All colors
come from `gp_render::tokens::color::*`; the token names the spec cites all
exist (`SURFACE_PAGE`/`PAPER_1`, `SURFACE_INFIELD`/`PAPER_2`,
`SURFACE_ASPHALT`/`ASPHALT_1`, `WALL`, `GRAPHITE_900`, `PAPER_0`, `CAR_COLORS`,
`ACCENT`, `car_color(i)`) `[measured: read crates/render/src/tokens/color.rs]`.

**Module layout.** A new `crates/render/src/track/` module tree:
`transform.rs` (coordinate map), `regions.rs`, `walls.rs`, `sf.rs`, `car.rs`,
plus `mod.rs` (orchestration) and a `#[cfg(test)] golden.rs` gallery. `mod track;`
is registered in `lib.rs`; `render_frame` stays a crate-root `pub fn`. The
per-car render struct `CarRender` is defined in `track::car` and re-exported at
crate root. Keeping layers in small files respects the soft-500 file rule
(`AGENTS.md`).

**Signature (clean break, no compat).** `render_frame` has no callers, so its
shape is free (`AGENTS.md` API-stability AXIOM; `[measured: rg -n render_frame
--type rust]`). Recommended:

```
pub fn render_frame(
    painter: &egui::Painter,
    rect:    egui::Rect,          // explicit target — see below
    track:   &TrackArtifact,
    cars:    &[CarRender<'_>],
    reduced_motion: bool,
    overlays: Overlays,           // kept, threaded inert (Q2)
)
```

- **Explicit `rect` over `painter.clip_rect()`.** The spec's coordinate-mapping
  note names `painter.clip_rect()`, but `draw_placeholder` deliberately takes an
  explicit `rect` because `egui_kittest` insets `Ui::painter()` by 8px and
  `clip_rect` depends on painter provenance, making the drawn output a pure
  function of `(rect)` for the golden `[measured: read
  crates/render/src/placeholder.rs:158-179 + widgets/game_gallery.rs:174-182,
  198-228 → both draw via a background-layer painter + explicit CANVAS_RECT]`.
  We follow that precedent; gp-game passes `ui.max_rect()` exactly as it does
  for `draw_placeholder` today `[measured: read crates/game/src/main.rs:22]`.
- **`CarRender<'a>`** carries the caller-supplied per-car render input the AC9
  contract requires (gp-render is draw-only, buffers no history): recommended
  `{ state: CarState, color_index: usize, trail: &'a [Point], you: bool,
  progress: f32 }`. `color_index` resolves through the existing `car_color(i)`,
  which returns `None` out-of-range; fall back via `.unwrap_or(CAR_COLORS[0])`
  (== `CAR_1`), never a panic `[measured: read tokens/color.rs:165 → car_color
  returns Option]`. `progress ∈ [0,1]` is the
  per-car animation clock; `trail` is prior lattice cells (older → fainter). Exact
  field set is the implementer's call within AC9 — this is the recommended shape.

**Coordinate transform.** A `TrackTransform` maps the corridor bounding box
(`Corridor::{origin,width,height}`, all `pub` `[measured: read geom/mod.rs:254-268]`)
into `rect`, aspect-preserving: `cell = min(rect.w / bbox_w, rect.h / bbox_h)`,
then center. Lattice `y` increases northward but egui screen `y` increases
downward, so the map flips `y`. It accepts `(f32, f32)` lattice coords (for
animation lerp and Chaikin vertices), mapping a lattice point/corner to a screen
`Pos2`. `i32`→`f32` coordinate conversion needs a documented
`#[allow(clippy::cast_precision_loss, reason = …)]` — the in-tree precedent is
`normalize` `[measured: read crates/core/src/track.rs:207-216 → `let (gx, gy) =
(g.0 as f32, g.1 as f32)` under a `#[allow(clippy::cast_precision_loss)]`]`.

**Rejected alternatives.**
- *Author asphalt/walls directly instead of deriving from `D`.* Rejected — the
  whole design-doc §0/§1 duality is "asphalt derived from `D`, walls = fill
  boundary, never through a point by construction"; authoring them re-introduces
  the desync the duality exists to prevent (design doc §4 line 306
  `[measured: read docs/design.md:295-308]`).
- *Buffer trail history inside gp-render.* Rejected — violates the draw-only
  split (`ai-docs/key-decisions.md` 2026-07-16); the caller (gp-game) owns
  history and the clock `[measured: read ai-docs/key-decisions.md:33-38]`.
- *Add a gp-core primitive returning infield cells.* The spec allows it "unless
  `design` finds it cleaner". Rejected — `Corridor::{origin,width,height,contains}`
  already let gp-render flood the complement itself with **no gp-core change**
  (see subtask 2); adding a gp-core fn widens the physics-core surface for a
  pure-render need.
- *Native `Painter::arrow` vs hand-rolled arrow.* **Adopted** `Painter::arrow`:
  egui 0.35 ships it (`origin: Pos2, vec: Vec2, stroke`, tip = `origin + vec`,
  arrowhead at `vec.length()/4`) `[measured: sed -n 417,470p egui-0.35.0/src/
  painter.rs → pub fn arrow(&self, origin: Pos2, vec: Vec2, stroke)]`. Direction
  and length come straight from the transformed `(vx, vy)` vector.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | `track/` module scaffold + `TrackTransform`: aspect-preserving corridor-bbox→`rect` fit, `y`-flip, derived cell size, `(f32,f32)`-lattice→screen `Pos2` map. Pure + `#[cfg(test)]`. Register `mod track;` in `lib.rs`. | `crates/render/src/track/mod.rs` (new), `crates/render/src/track/transform.rs` (new), `crates/render/src/lib.rs` | — |
| 2 | Regions layer: outfield (`PAPER_1`) + asphalt (union of unit cells over `D`, `ASPHALT_1`) + infield (bounded ¬D hole, `SURFACE_INFIELD`). Infield derived in-crate via a complement flood over the corridor bbox from border seeds (`Corridor::{origin,width,height,contains}`), no gp-core change. Pure cell-classification fns (AC1/AC2) + `paint`. | `crates/render/src/track/regions.rs` (new), `crates/render/src/track/mod.rs` | 1 |
| 3 | Wall geometry: `Wall{cell,side}` → half-grid edge segment (endpoints at `(±0.5,±0.5)` corners); chain the dual-edge set into ordered boundary polylines. Pure geometry + tests that no vertex/segment coincides with an integer lattice point (AC3) + `paint` (stroke `WALL`). | `crates/render/src/track/walls.rs` (new) | 1 |
| 4 | Cosmetic Chaikin smoothing of the wall polylines + **M6 guard**: clamp each generated vertex to stay within the half-cell gap (±0.5 cell of the block boundary) and never enter a grazeable/drivable cell; assert the drivable set `D` is unchanged (AC7). **Raw-stroke fallback invariant:** smoothing is an opt-in transform layered over the subtask-3 raw half-grid stroke — when a generated vertex would violate the guard the raw stroke stays the drawn geometry, so AC3 (walls miss every integer point) never depends on AC7 (smoothing bounds). | `crates/render/src/track/walls.rs` | 3 |
| 5 | S/F layer: checkered chord across the corridor along `sf.chord` — alternating `GRAPHITE_900`/`PAPER_0` unit cells, each `GRAPHITE_900`-hairline-stroked (per Track.jsx). Pure geometry + tests (AC4) + `paint`. | `crates/render/src/track/sf.rs` (new) | 1 |
| 6 | `CarRender<'_>` struct (caller-supplied color/identity, trail, `you`, progress) + move-animation interpolation: pure `lerp((x,y),(x+vx,y+vy),t)` (linear), reduced-motion snaps to final. Re-export `CarRender` at crate root. Tests (AC6/AC9). | `crates/render/src/track/car.rs` (new), `crates/render/src/lib.rs` | 1 |
| 7 | Car paint layer: `GRAPHITE_900`-outlined colored dot, velocity-vector arrow via `Painter::arrow` (dir/len ∝ transformed `(vx,vy)`), fading trail (older→fainter), optional dashed "you" ring (`ACCENT`). Pure arrow-vector fn tested to match `(vx,vy)` (AC5). | `crates/render/src/track/car.rs` | 1, 6 |
| 8 | `render_frame` rewrite: drop `todo!()`, evolve the signature (explicit `rect`, `&[CarRender]`, `reduced_motion`), draw layers 1→2(+4 smoothing)→3→6 back-to-front, thread `Overlays` inert (layers 4/5 no-op, documented deferral). Pure back-to-front layer-order unit check + doc update (AC9). | `crates/render/src/lib.rs`, `crates/render/src/track/mod.rs` | 2, 4, 5, 7 |
| 9 | AC8 golden: `#[cfg(test)]` gallery module drives a wgpu `egui_kittest` snapshot of a hand-built `TrackArtifact` scene, capturing back-to-front order (outfield→asphalt→infield→walls→S/F→cars). `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD")]`. Mint PNG; `image-check` at mint. | `crates/render/src/track/golden.rs` (new), `crates/render/tests/snapshots/track.png` (new) | 8 |

`M = 9` (≤ 15, no split needed).

## Handoff plan

Per the every-group handoff contract (`.claude/skills/task/SKILL.md` Step 8 +
`.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry)), this
is **required for every M ≥ 1**. All nine subtasks change **code only** (Rust
`*.rs` under `crates/render/`, plus one golden `.png` artifact minted by the
code implementor) — a single homogeneous change-type. With 9 ≤ the size cap of
10 and no change-type switch, group-minimization packs them into the **fewest
possible: one group** (splitting into two would be an avoidable non-minimized
group-count). Max-groups default is 4; one group is within it.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent (its `model: sonnet` + `effort: medium` are
  frontmatter-pinned; no inline `model=`/effort override), 1M-token window —
  subtasks **1–9** (code change-type: `*.rs` + the `track.png` golden). Terminal
  group (9 subtasks; within the `1..=10` range). **Entry handoff:** at the start
  of Group A, spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md`
  § Compaction recovery (re-entry); the group completes Step 8 in its own
  `/context-reset` subagent. No inter-group handoff (single group).

The `design`, `design-review`, `self-review`, and `spec-writer` subagents stay
on Opus regardless of this marker — only the per-group implementor model/effort
is `sonnet`/`medium`.

## Risks

- **Chaikin polyline chaining + M6 guard is the highest-complexity piece
  (subtask 4).** Chaikin needs *ordered* boundary loops, but `walls_from_boundary`
  returns an unordered `Vec<Wall>` `[measured: read crates/core/src/geom/graph.rs:308-331
  → pushes edges cell-by-cell, no ordering]`. Mitigation: chain edges into loops
  by shared half-grid corners in subtask 3 (its own tested step) before subtask 4
  smooths them; keep the raw (unsmoothed) stroke as the always-correct fallback so
  AC3 never depends on AC7. If `walls.rs` approaches the soft-500 line, split the
  chaining helper into `track/wall_path.rs`. `[derived → subtask-3/4 unit tests +
  file-size check at self-review]`
- **M6 render↔physics desync (AC7).** A smoothed vertex bulging into a grazeable
  cell visually clips a car that legally grazes a concave corner (design doc §4
  M6) `[measured: read docs/design.md:307-308]`. Mitigation: the guard clamps
  every generated vertex to (a) within 0.5·cell of the original boundary
  (half-cell gap) and (b) the non-drivable side (outside every `D` unit square),
  asserted directly in lattice coords; `D` itself is never mutated (gp-render only
  reads the corridor). `[derived → subtask-4 AC7 tests]`
- **Lint posture (`pedantic`+`nursery`+`arithmetic_side_effects` all deny)
  `[measured: read Cargo.toml [workspace.lints.clippy] → pedantic/nursery deny,
  arithmetic_side_effects deny]`.** (1) Raw `f32` `+`/`-`/`*` does **not** fire
  `arithmetic_side_effects`, but `Pos2 + Vec2` / `Pos2 - Vec2` operator overloads
  **do** — build positions field-wise via `Pos2::new(...)` / `mul_add`, exactly
  as the placeholder does `[measured: read placeholder.rs:120-156 → "clippy::
  arithmetic_side_effects (deny) fires on the latter"]`. (2) `missing_const_for_fn`
  (nursery, deny) forces `const fn` on const-*eligible* pure fns, but the geometry
  here calls `f32::hypot`/`sqrt`/division and `i32 as f32`, none const-stable, so
  those fns are plain `fn` (same reason `movepad::cell_rect` is non-const:
  `f32::from(u8)` isn't const-stable) `[measured: read movepad.rs:283-288]`; any
  fn whose body is const-eligible (e.g. a `CarRender` accessor returning `Point`)
  MUST be `const fn`. (3) `i32`→`f32` casts + any `f32 as u32` (golden pixel
  probes) need documented `#[allow(clippy::cast_precision_loss|cast_possible_
  truncation|cast_sign_loss, reason = …)]`, precedent `normalize`/`pixel_at`
  `[measured: read placeholder.rs:340-348]`. `[derived → cargo clippy --workspace
  --all-targets -D warnings (AC10)]`
- **Golden bit-exactness (AC8).** The track canvas draws **no text** (Track.jsx
  has no text nodes `[measured: read docs/design-system/ui_kits/game/Track.jsx →
  only rect/path/circle/line]`), so the golden test needs no `set_fonts`
  frame-1-install dance and never hits the `vello_cpu` glyph-cast Miri abort —
  only the wgpu/`dlopen` abort applies, so `#[cfg_attr(miri, ignore)]` is the sole
  guard. Recommend the placeholder threshold (`threshold(0.0)` +
  `failed_pixel_count_threshold(0)`, bit-exact in flat regions, AA edges exempt by
  library property) plus a flat-region probe guard; the canvas is AA-heavy
  (circles, diagonal wall strokes) so if a cross-renderer AA-noise failure appears,
  fall back to `threshold(1.0)` as `game_gallery` does — do not pre-emptively
  loosen `[measured: read placeholder.rs:497-521 + game_gallery.rs:234-243]`.
  `[derived → AC8 golden green under CI lavapipe]`
- **Golden regen contract.** Subtask 9 mints a new PNG, so `code-writer` must
  spawn `image-check` before committing it and must not commit until it PASSes
  (`ai-docs/key-decisions.md` step 4 + `code-writer.md` § Invariants) `[measured:
  read ai-docs/key-decisions.md:44-51]`. A missing lavapipe ICD yields a loud
  failure, by design — no skip hatch.
- **`Painter::arrow` returns `()` (not a `ShapeIdx`).** Fine for fire-and-forget
  drawing; the AC5 vector assertion is on the pure fn that computes the arrow's
  screen-space `(origin, vec)`, not on the `Painter` call `[measured: sed -n
  417,470p egui-0.35.0/src/painter.rs]`.
- **Dashed "you" ring.** epaint 0.35 provides **`Shape::dashed_line(path: &[Pos2],
  stroke, dash_length: f32, gap_length: f32) -> Vec<Shape>`** (plus
  `dashed_line_with_offset` / `dashed_line_many` / `dashed_line_many_with_offset`)
  `[measured: rg -n 'pub fn dashed_line' epaint-0.35.0/src/shapes/shape.rs →
  L170/189/210/229; `rg -n 'pub fn n_line' → (none)`]`. This gives a genuinely
  dashed you-ring matching the `c.you &&` ring's `strokeDasharray="2 3"`
  (`dash_length` 2, `gap_length` 3, scaled by cell size) `[measured: read
  Track.jsx:101]` — **not** the `"2 6"` at Track.jsx:76, which is the deferred
  layer-5 fastest-lap / ideal-line overlay: build the ring as a `Pos2` polyline
  and pass it to `Shape::dashed_line`, then `painter.add` each returned shape. A solid thin `ACCENT` ring is the optional fallback (AC5
  makes the you-ring optional). `[derived → subtask-7 compile + golden]`

## Test Design

Pure geometry tests stay Miri-clean and unignored (design doc §3a integer/
sub-cell split lives in gp-render); only subtask 9 is wgpu/`#[cfg_attr(miri,
ignore)]`. All non-trivial modules (~50+ lines) get a `#[cfg(test)] mod tests`
(`AGENTS.md`). Fixtures build a `TrackArtifact` (or the sub-structure a layer
needs) by hand — no runtime generation (spec Out-of-scope) — reusing the
`corridor(origin,w,h,&cells)` helper shape from `geom/graph.rs` tests
`[measured: read crates/core/src/geom/graph.rs:341-352]`.

- **Subtask 1 — `TrackTransform`** (`track/transform.rs` `#[cfg(test)]`).
  Entry: `TrackTransform::new(bbox, rect)` + its `map((f32,f32)) -> Pos2`.
  Scenarios: (happy) a known lattice cell-center maps to the expected screen
  `Pos2`; (y-flip) increasing lattice `y` yields *decreasing* screen `y`;
  (aspect) a non-square bbox in a square rect preserves cell size on both axes +
  centers; (edge) a 1×1 bbox and a degenerate 0-area rect do not panic (total).
- **Subtask 2 — regions** (`track/regions.rs` `#[cfg(test)]`).
  Entry: the pure asphalt-cell-set fn and the infield-cell-set fn.
  Scenarios: (AC1) rendered asphalt cell set `== { p : corridor.contains(p) }`
  exactly, on a hand-built ring; (AC2) infield = the enclosed bounded ¬D hole
  (the 3×3-ring center), outfield = border-reachable ¬D, the two disjoint and
  neither overlapping asphalt; (edge) a solid block has empty infield; flush-to-
  edge ring still yields its bounded hole (mirrors `bounded_complement_components`
  fixtures `[measured: read geom/graph.rs:440-485]`).
- **Subtask 3 — wall geometry** (`track/walls.rs` `#[cfg(test)]`).
  Entry: `Wall{cell,side}` → segment endpoints; polyline chaining.
  Scenarios: (AC3) every segment endpoint has both coords at a half-integer
  (`coord*2` is odd) → never an integer lattice point, over the solid-2×2 and
  3×3-ring wall sets; (chaining) the 3×3-ring's 16 edges chain into closed
  loops (outer + inner) with each edge used once.
- **Subtask 4 — Chaikin + M6** (`track/walls.rs` `#[cfg(test)]`).
  Entry: the smoothing fn over a polyline.
  Scenarios: (AC7 bound) every generated vertex is within 0.5·cell of the
  original boundary; (AC7 M6) no generated vertex lies inside any drivable/
  grazeable unit square; (AC7 invariance) the corridor `D` passed in is
  bit-identical before/after (no mutation); (edge) a straight run is unchanged;
  a concave corner is clamped, not bulged.
- **Subtask 5 — S/F** (`track/sf.rs` `#[cfg(test)]`).
  Entry: the pure "checkered cell + color" fn along a chord.
  Scenarios: (AC4) an N-cell chord yields N cells alternating
  `GRAPHITE_900`/`PAPER_0` starting `GRAPHITE_900` (per Track.jsx
  `i % 2 == 0 → graphite-900` `[measured: read Track.jsx:79-86]`); (edge) an
  empty chord yields no cells, no panic.
- **Subtask 6 — car input + animation** (`track/car.rs` `#[cfg(test)]`).
  Entry: `lerp_pos(state, t)` (linear move animation) + `CarRender` construction.
  Scenarios: (AC6) `t=0` → `(x,y)`, `t=1` → `(x+vx, y+vy)`, `t=0.5` → midpoint,
  asserted at representative `t`; (AC6 reduced-motion) reduced-motion path returns
  the final position for any `t` (snap, no slide); (AC9) `CarRender` carries
  color/identity, trail slice, `you`, progress — a constructed instance round-trips
  its fields; `color_index` out of range falls back without panic.
- **Subtask 7 — car paint vector** (`track/car.rs` `#[cfg(test)]`).
  Entry: the pure arrow-vector fn `(car, transform) -> (origin: Pos2, vec: Vec2)`.
  Scenarios: (AC5) for representative `(vx,vy)` the arrow tip `origin + vec` equals
  `transform(x+vx, y+vy)` and `vec` direction matches the transformed `(vx,vy)`
  (length ∝ speed); (edge) `(vx,vy) == (0,0)` draws no arrow (matches Track.jsx's
  `vx!==0 || vy!==0` guard `[measured: read Track.jsx:95]`). The optional "you"
  ring is drawn genuinely dashed via `Shape::dashed_line` (dash/gap `2`/`3` scaled
  by cell, per the `c.you &&` ring's `strokeDasharray="2 3"` `[measured: read
  Track.jsx:101]` — not the `"2 6"` at Track.jsx:76, which is the deferred
  fastest-lap overlay), with a solid-`ACCENT` ring as the fallback — a paint-only
  embellishment covered by the AC8 golden rather than a dedicated unit test.
- **Subtask 8 — `render_frame`** (`lib.rs`/`track/mod.rs` `#[cfg(test)]`).
  Entry: a pure back-to-front layer-order list/fn the orchestration follows.
  Scenarios: (AC9) the drawn-layer order is exactly outfield→asphalt→infield→
  walls→S/F→cars; `Overlays{grid,speed_heatmap,fastest_lap}` set true changes
  nothing (inert); the function no longer contains `todo!()`.
- **Subtask 9 — golden** (`track/golden.rs` `#[cfg(test)]`, `#[cfg_attr(miri,
  ignore)]`). Entry: one wgpu frame of a hand-built scene through the crate-visible
  `paint` layers (mirrors `game_gallery.rs`'s FORCED-value gallery `[measured:
  read widgets/game_gallery.rs:184-245]`). Scenario: (AC8) the frame matches the
  minted `crates/render/tests/snapshots/track.png`; a flat-region probe guard runs
  before the compare so a degenerate frame fails on the drawing code, not the
  golden. Asserts adapter is CPU/software (lavapipe) like the sibling goldens.

## Open questions

- None. Both round-1 forks are resolved by the product owner (spec § Open
  questions): **Q1 → Chaikin smoothing in scope (M6-guarded)**; **Q2 → grid
  (layer 4) + analytics overlays (layer 5) deferred, `Overlays` flags inert**.
