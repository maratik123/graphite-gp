# Design: gp-render track canvas — regions, walls, S/F, cars + move animation

**Issue:** #17
**Date:** 2026-07-19 (amended 2026-07-19 — see § Amendment — Rounded track (PR #100))

> **Amendment 2026-07-19 (PR #100 review).** The regions **fill** approach in the
> original design below (subtask 2 / AC1: "fill one unit square per drivable
> cell") is **superseded**. Asphalt and infield are now filled to the *same*
> Chaikin-smoothed boundary the walls trace — not to raw axis-aligned unit
> squares — so fill and outline agree at every corner (the "rounded track" fix).
> The measured fill-primitive decision, loop reuse, decomposition, handoff, and
> tests are in the **§ Amendment — Rounded track (PR #100)** section at the end.
> Everything else in the original design (walls stroke, S/F, cars, move
> animation, `Overlays` inert) is unchanged.

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
| 2 | Regions layer: outfield (`PAPER_1`) + asphalt (union of unit cells over `D`, `ASPHALT_1`) + infield (bounded ¬D hole, `SURFACE_INFIELD`). Infield derived in-crate via a complement flood over the corridor bbox from border seeds (`Corridor::{origin,width,height,contains}`), no gp-core change. Pure cell-classification fns (AC1/AC2) + `paint`. **[Fill primitive superseded by § Amendment — Rounded track (PR #100): fill to the smoothed boundary, not unit squares. The `classify`/`RegionCells` cell-sets are retained as the AC1/AC2 test oracle.]** | `crates/render/src/track/regions.rs` (new), `crates/render/src/track/mod.rs` | 1 |
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
  **[These `classify` cell-set tests are retained verbatim as the AC1/AC2 oracle
  by § Amendment — Rounded track (PR #100); the amendment adds fill-primitive
  tests, it does not weaken these.]**
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

---

# Amendment — Rounded track (PR #100)

**Amends:** the § Approach *Regions* / subtask 2 fill approach and § Test Design
subtask 2. **Source:** PR #100 product-owner review comment; spec amendment
already applied (AC1 reworded, AC7 extended, new *Rounded track (PR #100)*
Key-decision row) `[measured: read ai-docs/plans/done/2026-07-19-render-track-canvas.spec.md
→ AC1 (line 113), AC7 (line 119), Rounded-track row (line 85)]`. **Date:**
2026-07-19.

## Context — the bug

The shipped render fills asphalt + infield as full axis-aligned unit **squares**
(`regions::paint` → per-cell `painter.rect_filled(cell_rect(...), 0, …)`, sharp
corners) but strokes walls as a Chaikin-**smoothed** closed line
(`walls::paint` → `Shape::closed_line` over `chaikin_smooth`'d loops, beveled
corners) `[measured: read crates/render/src/track/regions.rs:106-146 (per-cell
rect_filled) + walls.rs:213-230 (Shape::closed_line over smoothed loops)]`. Fill
and outline therefore disagree at every corner — cream notches on the outer
ring, hatched slivers at the infield-hole corners (product owner, PR #100). The
chosen fix is **"Rounded track": one boundary shared by the wall stroke and the
region fills**, so fill and outline agree at every corner (spec AC7).

## Decision — fill each region to the smoothed boundary (measured fill primitive)

**Measured constraint — epaint 0.35 has no concave/annulus fill.** Its only
path-fill routines (`Shape::convex_polygon` → `PathShape::convex_polygon`, and
the internal tessellator `fill_closed_path`) triangulate as a **fan from vertex
0** — correct for **convex** polygons only; there is no even-odd, no concave,
and no ear-clip fill anywhere in the crate `[measured: sed -n 760,822p
epaint-0.35.0/src/tessellator.rs → doc "Tessellate the given convex area into a
polygon" + both branches emit `for i in 2..n { add_triangle(idx, idx+i-1, idx+i)
}`; read shapes/shape.rs:251-257 + shapes/path_shape.rs:52-63 → convex_polygon
just sets `closed:true, fill`; `rg -rn 'even.odd|concave|ear.clip' epaint-0.35.0/src
→ (none)`]`. The drivable region is an **annulus** — design doc §1: `D` is a
ring with exactly **one** bounded hole `[measured: read docs/design.md:13 →
"связное полимино с ровно одной дыркой"]` — which is exactly what grounds the
max-|area|=outer / rest=holes loop classification and the layered (no-even-odd)
fill below. **Separately** (topology-independent — a plain oval ring is an
annulus with a *convex* outer boundary, so §1 does **not** establish this),
generated racetracks generically have **concave** boundary stretches (S-curves,
hairpins) `[derived → the concave-polygon triangulator unit test in A2; a
convex-only fill would silently mis-fill the first non-trivial generated track —
wrong pixels, no panic]`, so `Shape::convex_polygon` is **insufficient in
general**.

**Chosen approach — layered fills, each simple loop triangulated by gp-render
into an `epaint::Mesh`.** Two decisions:

1. **Layered (sidesteps even-odd/holes).** Fill the whole **outer** smoothed
   loop(s) with `SURFACE_ASPHALT`, then fill each **infield-hole** smoothed
   loop with `SURFACE_INFIELD` **on top** — exactly the existing
   outfield→asphalt→infield draw order, and what Track.jsx itself does
   (asphalt even-odd path, then the infield rect painted over it) `[measured:
   read docs/design-system/ui_kits/game/Track.jsx:56-67]`. No single path ever
   needs a hole, so no even-odd is required.
2. **Concave-capable single-loop fill via a gp-render triangulator → `Mesh`.**
   Each simple loop (outer, or a hole) is filled by an **ear-clipping**
   triangulation (simple polygon → triangles; input winding normalized to CCW by
   the loop's shoelace-area sign; collinear/degenerate ears skipped), assembled
   into an `epaint::Mesh` via `Mesh::colored_vertex` + `Mesh::add_triangle` and
   drawn with `Shape::mesh` — a solid colored mesh needs no texture (default
   white texture, `WHITE_UV`) `[measured: read epaint-0.35.0/src/mesh.rs:169
   colored_vertex → `Vertex::untextured(pos, color)`, :179 add_triangle, :32
   `uv: WHITE_UV`; shapes/shape.rs:361 `Shape::mesh(impl Into<Arc<Mesh>>)`]`.
   The mesh fill carries **no edge feathering** (unlike epaint's own convex
   fill), but the feathered `WALL` stroke drawn on the *same* shared boundary
   sits exactly over the fill edge and masks it — which is precisely the
   "fill and outline agree at every corner" AC7 goal.

**Loop classification (outer vs hole).** `walls::chain_walls` already returns
ordered closed loops (outer + inner) `[measured: read crates/render/src/track/walls.rs:154-192
+ test `ring_chains_into_outer_and_inner_loops` walls.rs:320-342 → 3×3 ring →
exactly 2 loops]`. Classify each loop by **signed area** (shoelace on its lattice
points): the max-|area| loop is the outer asphalt boundary; the rest are infield
holes. The annulus invariant (design §1: exactly one bounded hole) makes
max-|area|=outer / rest=holes exact; **containment** (a loop-vertex point-in-
polygon test) is the robust refinement if a future fixture nests deeper.

**Boundary reuse (fill and stroke cannot drift).** `draw_frame` already computes
`smoothed_loops` once `[measured: read crates/render/src/track/mod.rs:59-64]`.
The amendment passes that **same** `smoothed_loops` slice to both the new region
fill and the unchanged `walls::paint`, so fill and stroke are literally the same
polygon — they cannot disagree by construction.

**AC1 stays meaningful.** The covered cell set is still exactly `D`: M6 keeps
every smoothed vertex within ±0.5 cell of the boundary (already enforced by
`chaikin_smooth`'s `guarded` clamp `[measured: read walls.rs:98-140]`), so each
drivable cell's center stays inside the outer loop and outside every hole loop —
the polygon covers exactly `D`, sub-cell rounding aside. The AC1 unit test stays
the **`classify` cell-set assertion** (`set(&classify(d).asphalt) == {p :
d.contains(p)}`) — `classify`/`RegionCells` are retained unchanged as the
AC1/AC2 oracle (test-only in production if `draw_frame` no longer calls them, via
the crate's existing `#[cfg_attr(not(test), allow(dead_code, reason = …))]`
idiom `[measured: read regions.rs:26-30 (outfield field) + walls.rs:201-208
(dual_loop_to_lattice)]`).

**Rejected alternative — `Shape::convex_polygon` per loop.** Simpler, and it
fills every *current* fixture correctly (the 3×3-ring outer + hole loops are
convex after Chaikin), but it silently mis-fills the first concave generated
track — re-introducing the corner-disagreement class this amendment exists to
kill, and failing without a panic. Rejected on the measured convex-only fact
above; the ear-clip mesh route is the general-correct one AC7 ("agree at every
corner") demands.

## Decomposition (amendment delta)

| # | Task | Files | Depends on |
|---|------|-------|------------|
| A1 | Loop classification: `classify_loops` (or `LoopRole`) — signed-area (shoelace) split of the smoothed loops into the outer asphalt boundary vs infield holes; containment refinement noted. Pure + `#[cfg(test)]`. | `crates/render/src/track/regions.rs` | — |
| A2 | Ear-clipping triangulator + layered `Mesh` fill: `triangulate(&[Pos2]) -> Vec<[u32;3]>` (winding-normalized, concave-capable); rewrite `regions::paint`/add `regions::fill` to draw the outfield `PAPER_1` rect, then the outer loop(s) as an `ASPHALT` `Mesh`, then the hole loop(s) as a `SURFACE_INFIELD` `Mesh` on top; remove the per-cell `cell_rect`/square-fill path; retain `classify`/`RegionCells` as the test-only AC1/AC2 oracle. Pure geometry + `#[cfg(test)]`. | `crates/render/src/track/regions.rs` | A1 |
| A3 | `draw_frame` wiring: compute `smoothed_loops` once, classify via A1, pass the **same** loops to `regions::fill` **and** `walls::paint`; preserve `LAYER_ORDER` (outfield→asphalt→infield→walls→S/F→cars) and `Overlays` inert. The existing `mod.rs` shape tests (`render_frame_draws_without_panicking`, `overlays_are_inert`) stay valid (behavior-agnostic). | `crates/render/src/track/mod.rs` | A2 |
| A4 | Re-mint the golden `track.png` (fill is now rounded) and run `image-check` at mint — the golden **test code** is unchanged; only the PNG re-mints. | `crates/render/tests/snapshots/track.png` (re-mint), `crates/render/src/track/golden.rs` (only if a probe/threshold needs adjusting) | A3 |

`M = 4` amendment subtasks. Walls stroke, S/F, cars, move animation, and
`Overlays`-inert are **unchanged** — no subtask touches them.

## Handoff plan (amendment)

Per the every-group handoff contract (`.claude/skills/task/SKILL.md` Step 8 +
`.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry)) —
**required for every M ≥ 1**. All four amendment subtasks change **code only**
(Rust `*.rs` under `crates/render/`, plus the re-minted `track.png` artifact) — a
single homogeneous change-type. With 4 ≤ the size cap of 10 and no change-type
switch, group-minimization packs them into the **fewest possible: one group**.
Max-groups default is 4; one group is within it.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent (`model: sonnet` + `effort: medium` frontmatter-pinned;
  no inline `model=`/effort override), 1M-token window — subtasks **A1–A4** (code
  change-type: `*.rs` + the `track.png` re-mint). Terminal group (4 subtasks;
  within the `1..=10` range). **Entry handoff:** at the start of Group A, spawn
  `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction
  recovery (re-entry); the group completes its Step 8 in its own `/context-reset`
  subagent. No inter-group handoff (single group). The `design` /
  `design-review` / `self-review` / `image-check` subagents stay on their pinned
  models regardless of this marker.

## Risks (amendment)

- **Ear-clipping correctness/robustness.** O(n²) simple-polygon ear clipping has
  edge cases — collinear runs (a smoothed straight edge), near-degenerate ears,
  and winding. Mitigation: normalize the loop to CCW by its shoelace-area sign
  before clipping; treat a zero-area ear as clippable; the loops are guaranteed
  simple by `chain_walls` (each corner degree ≤ 2 `[measured: read
  walls.rs:142-153 doc + test walls.rs:334-341 "no repeated corner within a
  loop"]`). Tests assert `triangle_count == n − 2` and that the triangles'
  areas sum to the polygon's |area|. `[derived → A2 unit tests + cargo clippy /
  cargo test (AC10)]`
- **Concave fill not exercisable by a real track yet.** The generator (`gp-gen`)
  is still `todo!()`, so every reachable input this task can render (hand-built
  fixtures + golden) is convex; the concave path is proven only by a hand-built
  concave-polygon unit test on the triangulator, not by a rendered concave
  track. This is accepted — the triangulator is general and unit-tested; a real
  concave track arrives with the generator task, which carries its own render
  review. `[measured: read crates/render/src/track/golden.rs + mod.rs fixture →
  3×3-ring (convex loops)]`
- **Golden re-mint drift.** A4 re-mints `track.png`; `code-writer` MUST spawn
  `image-check` before committing the PNG and must not commit until it PASSes
  (`ai-docs/key-decisions.md` step 4 + `code-writer.md` § Invariants) `[measured:
  read ai-docs/key-decisions.md:44-51]`. The rounded fill has more AA edges than
  the square fill; if the exact `threshold(0.0)` compare reddens on cross-renderer
  AA noise, fall back to `threshold(1.0)` as `game_gallery` does — do not
  pre-emptively loosen `[measured: read crates/render/src/widgets/game_gallery.rs:234-243]`.
- **Mesh fill lacks feathering (hard edge).** Accepted and load-bearing: the
  feathered `WALL` stroke on the shared boundary covers the fill's aliased edge,
  which is the AC7 "agree at every corner" property, not a defect. `[derived →
  A4 golden + image-check]`
- **Lint / file-size.** `regions.rs` gains the triangulator + classifier + mesh
  fill; watch the soft-500 line and split (e.g. `track/fill.rs`) if crossed.
  `i32→f32` and any `f32 as u32` casts keep documented `#[allow(clippy::
  cast_precision_loss | cast_possible_truncation | cast_sign_loss, reason = …)]`
  (precedent already in `regions.rs::cell_rect` / `walls.rs`) `[measured: read
  regions.rs:106-117 + walls.rs:56-91]`. `[derived → cargo clippy --workspace
  --all-targets -D warnings + file-size check at self-review]`

## Test Design (amendment)

Pure geometry tests stay Miri-clean and unignored; only A4's golden is
wgpu/`#[cfg_attr(miri, ignore)]`.

- **A1 — loop classification** (`regions.rs` `#[cfg(test)]`).
  Entry: `classify_loops(&smoothed_loops)`.
  Scenarios: (happy) the 3×3-ring's 2 smoothed loops split into exactly one
  outer (asphalt) and one hole (infield), outer having the larger |area|;
  (sign-agnostic) reversing a loop's winding does not change its role;
  (edge) a single solid-cell corridor yields one outer loop and zero holes.
- **A2 — triangulator + fill** (`regions.rs` `#[cfg(test)]`).
  Entry: `triangulate(&[Pos2])`; the region `fill`/`paint` fn (via
  `render_shapes`-style bare-`Context` shape capture, mirroring `mod.rs`'s
  existing fontless render helper `[measured: read crates/render/src/track/mod.rs:131-151]`).
  Scenarios: (convex) a rounded-square loop triangulates to `n − 2` triangles
  covering the polygon (area sum == |polygon area|); (**concave**) a hand-built
  concave (L-/U-shaped) polygon triangulates correctly — triangle count `n − 2`,
  area sum equals the polygon's |area|, and no triangle exits the polygon (the
  case `convex_polygon` would fail); (fill) the region fill emits an `ASPHALT`
  mesh then a `SURFACE_INFIELD` mesh in that order; (AC1 retained) the existing
  `classify` cell-set tests still pass unchanged.
- **A3 — wiring** (`mod.rs` `#[cfg(test)]`, existing tests).
  `layer_order_is_documented`, `render_frame_draws_without_panicking`, and
  `overlays_are_inert` continue to pass; the last two are behavior-agnostic
  (non-empty shapes / default==all-on) and need no change.
- **A4 — golden** (`golden.rs`, `#[cfg_attr(miri, ignore)]`). Re-mint
  `track.png` with the rounded fill; `image-check` confirms the minted image
  matches the drawing code before commit. Same CPU/software-adapter assertion and
  flat-region probe guard as the original subtask 9.

---

# Second amendment — S/F thin bar + widened golden fixture (PR #100)

**Amends:** § Approach *S/F* (subtask 5) rendering proportions and the § Amendment
A4 golden fixture. **Source:** PR #100 product-owner review (same review as the
first amendment; rounded-fill A1–A4 already done + owner-approved). **No spec/AC
change** — AC4 ("S/F checkered along `sf.chord`") still holds; this is a
rendering-proportion + fixture change. **Date:** 2026-07-19.

## Context — two coupled bugs

1. **S/F is full-cell-thick, wrong proportion.** `sf.rs` renders each `sf.chord`
   cell as a **full unit square** (`cell_rect` at ±0.5) filled + hairline-stroked
   `[measured: read crates/render/src/track/sf.rs:39-64 → cell_rect maps
   (fx±0.5, fy±0.5); paint does rect_filled + rect_stroke per cell]`. The design's
   S/F is a **thin checkered bar across the track, perpendicular to the racing
   direction**: 5 rects `width 16 × height CELL(24)`, alternating
   `graphite-900`/`paper-0` (start graphite-900), hairline graphite-900 stroke —
   so it is `16/24 = 2/3` cell **thin in the racing direction** and full-cell
   along the bar `[measured: read docs/design-system/ui_kits/game/Track.jsx:79-86
   (rects width 16, height 24, alt fill, stroke graphite-900) + :13 (CELL = 24)]`.
2. **Golden fixture reads the checker on the wrong axis.** The golden's chord
   `[(1,1),(2,1),(3,1)]` runs **along** the bottom straight (parallel to motion),
   over a thin 1-cell-wide 3×3 ring, so the checker reads along the racing axis
   instead of across the track `[measured: read crates/render/src/track/golden.rs:30-64
   → 3×3 ring, chord along the top row, Orient::Horizontal]`.

## Decision — thin cross-track S/F bar + widened rounded-rect fixture

### (1) S/F rendering (`sf.rs`)

Render each chord cell as a rect **full-cell along the chord axis** and
**`2/3`-cell thin in the perpendicular (racing) axis**, centered on the cell.
The thin axis is picked from `StartFinish.orient` `[measured: read
crates/core/src/geom/mod.rs:56-60 (Orient::Horizontal = "spanning east–west",
Vertical = "spanning north–south") + crates/core/src/track.rs:85 (pub orient:
Orient)]`:

- **`Orient::Horizontal`** (chord spans east–west, cells laid along **x**) →
  racing direction is **y** → **thin in y**: half-extents `(0.5, half)` cells.
- **`Orient::Vertical`** (chord spans north–south, cells laid along **y**) →
  racing direction is **x** → **thin in x**: half-extents `(half, 0.5)` cells.

where `half = SF_BAR_THICKNESS_CELLS / 2`. **The thickness is a token-derived
ratio, not a magic literal:** `spacing::CELL = 24.0` and `spacing::CELL_SM = 16.0`
are exactly Track.jsx's grid cell (24) and S/F rect width (16) `[measured: read
crates/render/src/tokens/spacing.rs:40 (CELL = 24.0), :42 (CELL_SM = 16.0)]`, so
the design ratio is

```
const SF_BAR_THICKNESS_CELLS: f32 = spacing::CELL_SM / spacing::CELL; // = 16/24 = 2/3 cell
```

— a new `sf.rs` module const **derived from existing spacing tokens** (answers
the owner's "new const or spacing token": both — a named const whose value is a
token ratio, so no magic number and exact Track.jsx provenance).

`paint` gains an `orient: Orient` parameter and builds the thin rect via the
already-shipped `rect_filled` + `rect_stroke` calls (no new egui/epaint API —
only the rect **extents** change) `[measured: read sf.rs:49,61,62 → already uses
Rect::from_two_pos / rect_filled / rect_stroke]`. `checker_cells` (color
alternation, AC4) and the fill/stroke colors are **unchanged**. `draw_frame`
passes `track.sf.orient` (one-line change in `mod.rs`, at the `sf::paint` call
`[measured: read crates/render/src/track/mod.rs:69-70 → sf::checker_cells +
sf::paint call in the post-A1–A3 working tree; the first amendment's already-
applied `regions::fill(…, &loop_roles)` sits just above at :65-66]`).

### (2) Widened golden fixture (`golden.rs`)

Replace the 1-cell-wide 3×3 ring with a **chunky rounded-rect corridor**: a large
outer cell-rectangle minus a smaller centered inner-hole rectangle → a thick loop
with multi-cell-wide arms, over a **square** bbox (keeps the current
"square bbox → no aspect-fit letterboxing" rationale `[measured: read
golden.rs:18-24]`). Recommended clean dimensions (implementer may adjust while
holding the constraints):

- **bbox** `Corridor::new((0,0), 16, 16)` on the existing `320×320` canvas →
  `cell = 20 px`, with a 2-cell outfield margin.
- **drivable** = outer block `x∈[2,13] × y∈[2,13]` **minus** hole
  `x∈[6,9] × y∈[6,9]` → a loop with **4-cell-wide arms**.
- **S/F chord** = a **column across the bottom straight**, spanning its width:
  `[(7,2),(7,3),(7,4),(7,5)]` with **`Orient::Vertical`** (cells along y, thin
  in x = racing direction) — a proper checkered bar across the 4-cell arm width,
  matching Track.jsx's cross-track S/F.
- **cars** repositioned onto the wider track: **you** mid-move near the S/F on the
  bottom straight (recommended `CarState{x:4,y:3,vx:2,vy:0}`, `color_index 0`,
  `you:true`, `progress 0.5` → drawn at `(5,3)`, arrow east toward the S/F,
  a short bottom-arm trail e.g. `[(2,3),(3,3)]`, dashed ring); **rival** parked
  on another arm (recommended `CarState{x:11,y:7,vx:0,vy:0}`, `color_index 1`,
  no trail, no arrow per `Track.jsx:95`'s `v≠0` guard, `you:false`).
- **`OUTFIELD_PROBE`**: re-verify it lands on `PAPER_1` for the new bbox — the
  probe assertion runs before the golden compare and **self-catches** a bad probe
  at mint `[measured: read golden.rs:108-169 → probe asserted == paper-1 before
  the snapshot compare]`. The current `(4.5,4.5)` remains a deep-outfield corner
  for the 16×16 / 2-cell-margin bbox (screen top-left ≈ lattice `(0,15.7)`, `x<2`
  and `y>13` → outfield), but the widened fixture makes `(4,4)` drivable, so the
  implementer MUST confirm the probe (move it if the final dimensions differ).

Re-mint `track.png` and run `image-check` at mint. **The A1–A3 rounded-fill code
(`classify_loops`, ear-clip `triangulate`, layered `Mesh` fill) needs no change**
— it already handles an arbitrary corridor; a wider track just exercises it at
scale. Note the widened loop's outer boundary (a rectangle → rounded rect after
Chaikin) and its hole (a rectangle) are both **convex**, so the concave
triangulation path stays **unit-tested only** (no concave generated track exists
until `gp-gen` lands) — consistent with the first amendment's accepted risk.

## Decomposition (second-amendment delta)

| # | Task | Files | Depends on |
|---|------|-------|------------|
| B1 | S/F thin-bar rework: add `SF_BAR_THICKNESS_CELLS = spacing::CELL_SM / spacing::CELL` (= 2/3) + a pure **`const fn`** `bar_rect_lattice(point, orient) -> (min,max)` returning lattice `(f32,f32)` corners (full-cell along the chord axis, `2/3`-thin in the racing axis picked from `Orient`, centered on the cell). **Write it `const fn` from the start:** its body is only `i32 as f32` casts, f32 div/add/sub, and an `Orient` match — all const-stable, and it constructs only tuples (no non-const constructor) — so `clippy::missing_const_for_fn` (nursery, deny) forces it, and the `#[allow(clippy::cast_precision_loss, reason=…)]` it needs composes cleanly with `const fn`. `paint` gains an `orient: Orient` param and draws the thin rect (fill per `checker_cells` + hairline stroke — colors unchanged); `draw_frame` passes `track.sf.orient`. Pure geometry + `#[cfg(test)]`. | `crates/render/src/track/sf.rs`, `crates/render/src/track/mod.rs` | — |
| B2 | Widen the golden fixture: replace the 3×3 ring with the chunky rounded-rect corridor (outer block minus centered hole, 4-cell arms) over the square 16×16 bbox; S/F chord = the `Orient::Vertical` column across the bottom straight; reposition both cars (you mid-move near S/F with trail+arrow+ring, rival parked on another arm); re-verify/`move` `OUTFIELD_PROBE`. Re-mint `track.png` + `image-check`. Golden test harness code otherwise unchanged. | `crates/render/src/track/golden.rs`, `crates/render/tests/snapshots/track.png` (re-mint) | B1 |

`M = 2` second-amendment subtasks. Walls stroke, cars rendering, move animation,
`Overlays`-inert, the rounded-fill decision (A1–A3), and AC1–AC3/AC5–AC10 are
**unchanged**.

## Handoff plan (second amendment)

Per the every-group handoff contract (`.claude/skills/task/SKILL.md` Step 8 +
`.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry)) —
**required for every M ≥ 1**. Both subtasks change **code only** (Rust `*.rs`
under `crates/render/` + the re-minted `track.png` artifact) — a single
homogeneous change-type. With 2 ≤ the size cap of 10 and no change-type switch,
group-minimization packs them into the **fewest possible: one group**.
Max-groups default is 4; one group is within it.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent (`model: sonnet` + `effort: medium` frontmatter-pinned;
  no inline `model=`/effort override), 1M-token window — subtasks **B1–B2** (code
  change-type: `*.rs` + the `track.png` re-mint). Terminal group (2 subtasks;
  within the `1..=10` range). **Entry handoff:** at the start of Group A, spawn
  `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction
  recovery (re-entry); the group completes its Step 8 in its own `/context-reset`
  subagent. No inter-group handoff (single group). `design` / `design-review` /
  `self-review` / `image-check` stay on their pinned models.

## Risks (second amendment)

- **Orient→thin-axis inversion.** Getting the thin axis backwards (thin along the
  chord instead of across the racing direction) would draw the bar the wrong way
  and is not caught by the color-alternation test. Mitigation: the pure
  `bar_rect_lattice` orientation test (below) pins Horizontal→thin-y /
  Vertical→thin-x + centering, and the widened golden visually confirms the
  cross-track bar. `[derived → B1 orientation unit test + B2 golden/image-check]`
- **Stale `OUTFIELD_PROBE` after the fixture widen.** `(4,4)` becomes drivable in
  the new fixture, so a probe left on asphalt would fail the pre-compare guard.
  Mitigation: the guard self-catches at mint (asserts `== paper-1` before the
  snapshot compare); B2 re-verifies/moves the probe. `[measured: read
  golden.rs:108-169]`
- **Golden re-mint drift.** B2 re-mints `track.png`; `code-writer` MUST spawn
  `image-check` before committing the PNG (`ai-docs/key-decisions.md` step 4 +
  `code-writer.md` § Invariants `[measured: read ai-docs/key-decisions.md:44-51]`).
  Threshold guidance is inherited from the first amendment (exact `threshold(0.0)`,
  fall back to `threshold(1.0)` only on measured cross-renderer AA noise).
- **No new egui/epaint API.** The thin bar reuses the shipped `rect_filled` /
  `rect_stroke` / `Rect::from_two_pos` calls — only the rect extents change, so
  there is nothing new to verify against raw bytes `[measured: read sf.rs:49,61,62]`.

## Test Design (second amendment)

Pure geometry tests stay Miri-clean and unignored; only B2's golden is
wgpu/`#[cfg_attr(miri, ignore)]`.

- **B1 — S/F thin-bar orientation** (`sf.rs` `#[cfg(test)]`).
  Entry: `bar_rect_lattice(point, orient)` (returns the two lattice corners
  centered on `point`).
  Scenarios: (Horizontal) `bar_rect_lattice((2,1), Horizontal)` → min `(1.5,
  1 − 1/3)`, max `(2.5, 1 + 1/3)`: **full-cell in x** (width `1.0`), **thin in y**
  (height `2/3`), centered on `(2,1)`; (Vertical) `bar_rect_lattice((1,2),
  Vertical)` → **thin in x** (width `2/3`), **full-cell in y** (height `1.0`),
  centered on `(1,2)`; (thickness) the thin extent equals `spacing::CELL_SM /
  spacing::CELL` via `crate::tokens::css::assert_f32`. The existing
  `checker_cells` alternation test (AC4) and empty-chord test stay verbatim.
- **B2 — widened golden** (`golden.rs`, `#[cfg_attr(miri, ignore)]`). Re-mint
  `track.png` on the chunky rounded-rect fixture with the cross-track `Vertical`
  S/F bar and repositioned cars; `image-check` confirms the minted image matches
  the drawing code before commit. Same CPU/software-adapter assertion; the
  `OUTFIELD_PROBE` guard is re-verified for the new bbox.
