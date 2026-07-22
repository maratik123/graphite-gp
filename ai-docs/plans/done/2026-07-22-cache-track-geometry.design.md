# Design: Cache track geometry — rect-free baked geometry (REVISED, supersedes rect-keyed design)

**Issue:** #104
**Date:** 2026-07-22

> **This design supersedes the prior rect-keyed design after a product-owner-directed pivot** (see the spec's top-of-file AMENDMENT). The prior design's subtasks 1–3 are already committed on this branch (`27e1db0`, `8608c74`, `284561c`); this document plans the **delta** from that HEAD to the rect-free, track-identity-only baked geometry. `[measured: git log --oneline d77833e..HEAD → 284561c/8608c74/27e1db0 subtasks 3/2/1]`

## Approach

### The shape of the pivot

The triangle **topology** (`Vec<[u32; 3]>`) is a pure function of a track's lattice-space silhouette and is **invariant under the render rect**. `TrackTransform::map` is a plain affine map — uniform scale, translate, and a y-flip (`sy = (y - min).mul_add(-cell_size, offset)`, a negative-determinant term) — that only repositions vertices, never re-tiles the polygon. `[measured: read transform.rs:82-86 → sx/sy are single mul_add each; y term is -cell_size]` So indices computed by triangulating the **lattice** loop are valid for the affinely-mapped **screen** polygon. The committed cache (`27e1db0`) triangulated in *screen* space (`regions::triangulated_loop` maps → then ear-clips) and therefore falsely coupled the `O(n³)` ear-clip to the rect, forcing a rebuild on every resize. `[measured: read cache.rs:51-73 + regions.rs:377-391]`

The rect-free model inverts that: triangulate **once per track** in lattice space, store lattice verts + indices, and per frame run only the cheap `O(n)` lattice→screen vertex map inside the draw path (where the rect is already known).

### The type: `BakedTrackGeometry` (rewrites the committed `cache.rs`)

Rename the committed `TrackGeometryCache` → **`BakedTrackGeometry`**, and its module/file `track/cache.rs` → **`track/geometry.rs`**. Rationale: the type is no longer a memoized, rect-keyed *cache* (no staleness key, no `get_or_build`); "cache" became a misnomer under the AMENDMENT's own reframing to "baked geometry", and AGENTS.md § API Stability mandates clean renames with no compat shims. The spec's AC7/AC8 references to "`cache.rs`" resolve to the renamed `geometry.rs`. `[measured: rg TrackGeometryCache → only in-crate refs: lib.rs:34 pub use, mod.rs:31 pub use, cache.rs; no external consumer]`

Fields (all `pub(crate)` — only the type name is public API; `gp-game` constructs via `new` and never reads fields):

- `smoothed_loops: Vec<Vec<(f32, f32)>>` — chained, Chaikin-smoothed wall loops in **lattice** space (also the wall stroke's own input; doubles as the per-frame map's source verts).
- `loop_roles: regions::LoopRoles` — the outer/hole split.
- `triangulated_indices: Vec<Vec<[u32; 3]>>` — per-loop triangulation **topology**, parallel to `smoothed_loops` by index. **No `rect`, no `TrackTransform`, no screen-space `Pos2` stored.**

Builder `pub fn new(track: &TrackArtifact) -> Self` (no `Rect`, no `TrackTransform`): `chain_walls` → `chaikin_smooth` (per loop) → `classify_loops` → **`triangulate_lattice`** (per loop). It reuses the committed `build` pipeline verbatim except the final triangulation step, which moves from screen space to lattice space. Not const-eligible (allocates, calls non-const `chain_walls`/`triangulate`), so `missing_const_for_fn` (nursery, deny) does not fire. `[measured: Cargo.toml:63 nursery=deny]` `[derived → clippy gate, subtask 9]`

### Triangulating in lattice space: `triangulate_lattice`

Add `pub(crate) fn triangulate_lattice(loop_points: &[(f32, f32)]) -> Vec<[u32; 3]>` to `regions.rs`: maps each lattice `(x, y)` to `Pos2(x, y)` **directly** (identity, *not* through `TrackTransform`) and runs the existing `triangulate`, returning only the indices. This is exactly the pattern the existing `l_shape_loop` test already exercises (`dual_loop_to_lattice(...).map(|(x,y)| pos2(x,y))` → `triangulate`), so lattice-space triangulation is already proven correct on a concave polygon. `[measured: read regions.rs:573-582,680-704 — l_shape_loop triangulates lattice-as-Pos2 and asserts no triangle covers the notch]` Remove `triangulated_loop` (its screen-vert output is no longer stored). `[measured: rg triangulated_loop → 4 regions.rs + 1 mod.rs + 2 heatmap.rs + 2 cache.rs refs, all in-crate]`

### Per-frame draw path: map-on-the-fly (resolves Open Question 1)

**Recommendation: map on the fly every frame; do NOT memoize a rect-keyed screen-vertex buffer.** `draw_frame` builds `TrackTransform::new(&track.corridor, rect)` (O(1)), then maps each `geometry.smoothed_loops[i]` → a fresh `Vec<Pos2>` (O(n) per loop), producing `mapped: Vec<Vec<Pos2>>` once per frame. Both `regions::fill` and `heatmap::paint` receive the **same** `&mapped` (mapped once, shared across layers) plus the borrowed `&geometry.triangulated_indices`.

Justification: the primary win (eliminating the per-frame `O(n³)` ear-clip) holds regardless; the residual `O(n)` map is negligible against it. Map-on-the-fly keeps the API **rect-free everywhere** — no rect key in any type, no staleness path, no `&mut` handle. It does **not** reintroduce the immediate-mode-rect wrinkle the prior design fought: the map runs *inside* the draw path (`draw_frame`), where the rect is already an explicit parameter — nothing upstream (`gp-game`, `AppShell`, `Scene`) ever needs the rect. A memoized screen-vertex buffer would re-introduce a rect key and a staleness comparison for a constant-factor saving on unchanged-rect frames — net complexity for negligible gain (YAGNI). The index buffer (the expensive artifact) is already never recomputed per frame.

### fill/heatmap signature: parallel `(verts, indices)` slices (resolves the committed-work reconciliation)

The committed `fill`/`paint_infield_holes`/`heatmap::paint` consume a bundled `triangulated: &[(Vec<Pos2>, Vec<[u32; 3]>)]`. In the rect-free model the verts are **per-frame** (freshly mapped) and the indices are **borrowed from the baked geometry** — bundling them would force a per-frame `indices.clone()`, an allocation the whole task exists to remove. **Reshape** `fill`/`paint_infield_holes`/`heatmap::paint` to take two parallel borrowed slices: `verts: &[Vec<Pos2>]` (per-frame mapped) and `indices: &[Vec<[u32; 3]>]` (baked, borrowed). Internally each `roles` index reads `verts.get(idx)` + `indices.get(idx)` and calls the unchanged `paint_mesh(painter, &[Pos2], &[[u32;3]], color)` (which already takes verts and indices as separate slices — no signature change to `paint_mesh`). `heatmap::paint` retains its `transform` parameter (still needed for the per-cell `cell_rect` clip, independent of the mapped silhouette verts).

### Placement in `Scene` (resolves Open Question 2)

**Recommendation: add `geometry: &'a BakedTrackGeometry` as a field of `Scene`** (not a separate `render_frame` parameter). A shared `&T` reference is `Copy`, and `BakedTrackGeometry` derives `Debug`, so `Scene`'s `#[derive(Clone, Copy, Debug)]` survives unchanged. `[measured: read lib.rs:53 Scene #[derive(Clone,Copy,Debug)]; read app.rs:339, race.rs:133, lab.rs:151 — all Copy+Debug]` This is strictly less churn than a separate parameter:

- `render_frame(painter, rect, scene)` signature is **unchanged** — the geometry rides inside `scene`; AC7's "pure function of `(rect, scene)`" contract holds verbatim, with `geometry` now one of `scene`'s inputs.
- `RaceScreen::draw_canvas` reconstructs `Scene { overlays, ..self.input.scene }`; the spread **carries `geometry` through automatically** — race.rs production code needs **no** edit. `[measured: read race.rs:355-365]`
- `RaceInput` holds `scene: Scene`, so it auto-carries geometry — no `RaceInput` field change.

The one soft invariant: `scene.geometry` must have been built from `scene.track` (the same track↔geometry coupling the committed `triangulated`/`roles` "same call" doc-contract already carries). Documented on the `Scene::geometry` field. The prior design's `&mut Option<Cache>` could not live in `Copy` `Scene`; the rect-free shared ref can — exactly the reversal the spec's Open Question 2 anticipated.

### What stays unchanged

`grid::paint`, `walls::paint` (takes `&geometry.smoothed_loops` now, same `&[Vec<(f32,f32)>]` type), `fastest_lap::paint`, `sf::paint`, `car::paint`, `TrackTransform`, the physics core (`gp-core` untouched — no float, no render data in `TrackArtifact`), and `gp-render`'s dependency graph (no new normal dep). `[derived → cargo tree -p gp-render --edges no-dev unchanged, AC4, subtask 9]`

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | `regions.rs`: add `triangulate_lattice(&[(f32,f32)]) -> Vec<[u32;3]>`; drop `triangulated_loop`; reshape `fill` + `paint_infield_holes` to `(verts: &[Vec<Pos2>], indices: &[Vec<[u32;3]>], roles)`; update the regions `#[cfg(test)]` suite (add `triangulate_lattice` tests, adapt `fill_emits_asphalt_mesh_then_infield_mesh` to build mapped verts + lattice indices) | `crates/render/src/track/regions.rs` | — |
| 2 | `heatmap.rs`: reshape `paint` to `(painter, transform, verts: &[Vec<Pos2>], indices: &[Vec<[u32;3]>], roles, heatmap)`; adapt the 3 painter-driven tests (`paint_emits_…`, `paint_is_noop_…`, `heatmap_reuses_cached_outer_mesh` → assert baked `indices[outer_idx].as_ptr()` stable) | `crates/render/src/track/heatmap.rs` | 1 |
| 3 | `git mv crates/render/src/track/cache.rs crates/render/src/track/geometry.rs` (preserves history — confirmed by orchestrator); rename `TrackGeometryCache` → `BakedTrackGeometry`; fields `{ smoothed_loops, loop_roles, triangulated_indices }`; `pub fn new(&TrackArtifact)` (no rect/transform); remove `rect`/`transform` fields, `build(track,rect)`, `get_or_build`; rewrite module tests rect-free (build once; assert 2 loops, 1 outer + 1 hole, per-loop indices non-empty and `len == loop.len()-2`; determinism); update the `pub use`/`mod` references in `mod.rs` (`mod cache;` → `mod geometry;`, `pub use cache::TrackGeometryCache` → `pub use geometry::BakedTrackGeometry`) + `lib.rs` re-export | `crates/render/src/track/geometry.rs` (was `cache.rs`), `track/mod.rs`, `lib.rs` | 1 |
| 4 | `Scene` gains `geometry: &'a BakedTrackGeometry`; `render_frame` passes `scene.geometry` to `draw_frame`; `draw_frame` takes `geometry`, builds `TrackTransform`, maps `smoothed_loops`→`mapped: Vec<Vec<Pos2>>` once, calls `regions::fill`/`heatmap::paint` with `&mapped` + `&geometry.triangulated_indices` + `&geometry.loop_roles`, feeds `walls::paint` from `geometry.smoothed_loops`; **remove** per-frame `chain_walls`/`chaikin_smooth`/`classify_loops`/`triangulate`; update lib.rs/mod.rs "pure function" doc (AC7); adapt `render_shapes` test helper (build `BakedTrackGeometry::new`, put in `Scene`) and add the AC2/AC5 no-rebuild-across-rects test | `crates/render/src/lib.rs`, `track/mod.rs` | 1, 2, 3 |
| 5 | `LabScreen`: `LabInput` gains `geometry: &'a BakedTrackGeometry`; `draw_canvas` takes it and builds its internal `Scene` with it. (race.rs production code needs no change — Scene spread carries geometry) | `crates/render/src/screens/lab.rs` | 4 |
| 6 | `AppShell`: `ShellSession` gains `geometry: &'a BakedTrackGeometry`; Race branch's `Scene` gets `geometry: session.geometry`; Lab branch's `LabInput` gets `geometry: session.geometry` | `crates/render/src/app.rs` | 4, 5 |
| 7 | `gp-game`: `GraphiteGpApp` owns a `BakedTrackGeometry` field, built once in `new` from the fixture track (document the "rebuild on `TrackArtifact` swap only" contract — the fixture never swaps, so build-once suffices); thread `&self.geometry` into `ShellSession` | `crates/game/src/main.rs` | 3, 6 |
| 8 | Update all remaining test/gallery construction sites to build + thread a `BakedTrackGeometry`: `golden.rs` (draw_scene_with Scene), `app_gallery.rs` (5 `ShellSession`), `lab_gallery.rs` (2 `LabInput`), `race_gallery.rs` (2 `RaceInput.scene` Scene) | `track/golden.rs`, `app_gallery.rs`, `screens/lab_gallery.rs`, `screens/race_gallery.rs` | 3, 4, 5, 6 |
| 9 | Run the full gate suite (AC9) + verify **all committed goldens byte-identical** (AC6): the 8 canvas-bearing goldens (`track`, `track_grid`, `track_heatmap`, `track_fastest_lap`, `race_screen`, `lab_screen`, `app_shell_race`, `app_shell_lab`) plus `app_shell.png` **must PASS against the existing PNGs — do NOT re-mint.** A golden mismatch is a real triangulation bug (degenerate/inverted ear under the flipped winding), STOP and diagnose | (verification only) | 1–8 |

Scope: 9 atomic subtasks (< 15 — no split needed). All change-type **code** (`*.rs`).

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping (a)–(h), required for every M ≥ 1.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned in frontmatter)**, 1M-token window, via the `code-writer` subagent — subtasks **1–9**. Change-type **code** (`crates/render/**/*.rs`, `crates/game/src/main.rs`) — homogeneous. Terminal group (9 subtasks; within `1..=10`). This is the **only** group: all 9 subtasks share one change-type, and the dependency chain (1 → {2,3} → 4 → 5 → 6 → 7; 8 after 3–6; 9 last) fits under the size cap 10, so minimization yields exactly one group (well within the default max of 4). `[measured: 9 subtasks, all *.rs]`
- **Handoff into Group A:** at the start of Group A, spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry), then route the group to `subagent_type="code-writer"` (its `model: sonnet` + `effort: medium` are frontmatter-pinned; no inline `model=`/effort override). The `design`/`design-review`/`self-review` gates stay on Opus.
- No inter-group handoff (single group). Group A completes Step 8 in its own `/context-reset` subagent.

## Risks

- **Lattice-vs-screen triangulation picks different diagonals → a golden could shift (AC6, load-bearing).** The y-flip in `TrackTransform::map` is a negative-determinant affine map, so `triangulate`'s internal CCW-normalization (`signed_area_pos2 < 0 → reverse`) sees the opposite sign for a lattice loop vs its screen image, and `find_ear` may clip **different** ears. Mitigation / why the raster is still identical: (a) a valid triangulation of polygon `P` maps under a non-degenerate affine `T` to a valid triangulation of `T(P)` — `T` preserves straight lines and interior-disjointness; the orientation flip only flips all triangle windings uniformly, and egui's raw `Mesh` fill does **not** backface-cull. (b) egui raw-mesh fills are **not edge-feathered**, so internal diagonal edges are invisible (both adjacent triangles share one flat color); the outer silhouette is identical (same vertices, same order → same boundary edges). Therefore the rasterized fill is pixel-identical for *any* valid triangulation of the same silhouette. `[measured: read regions.rs:400-412 — paint_mesh builds a Mesh and calls painter.add(Shape::mesh(mesh)); raw meshes bypass egui's Tessellator feathering, so internal triangulation diagonals produce no pixels]` (c) lattice-space triangulation of a concave loop is **already** proven by the existing `triangulate_concave_l_shape_covers_area_without_exiting` test (triangulates lattice-as-`Pos2`, asserts no triangle exits the notch). `[measured: read regions.rs:680-704]` The empirical discharge of the byte-identity property is the subtask-9 golden re-run: the 8 canvas-bearing goldens **must PASS UNCHANGED against the existing PNGs — they are NOT re-minted**; a failure is a real degenerate/inverted-ear bug, not a benign diagonal swap. `[derived → subtask 9 golden suite]`
- **`Scene`/`ShellSession`/`LabInput` losing `Copy` or `Debug`.** Adding `&'a BakedTrackGeometry`: a shared ref is always `Copy`; `BakedTrackGeometry` derives `Debug` so `&BakedTrackGeometry: Debug`. All three derives survive. `[measured: read lib.rs:53, app.rs:339, lab.rs:151 — all #[derive(Clone,Copy,Debug)]]` `[derived → cargo build, subtask 9]`
- **Per-frame index `clone` sneaking back in.** Bundling verts+indices into a tuple would force `indices.clone()` per frame. Mitigated by the parallel-slice signature (subtask 1/2): indices are borrowed from the baked geometry, never cloned; only the `Vec<Pos2>` map allocates (inherent to map-on-the-fly). `[derived → code review + AC2 pointer-stability test, subtask 9]`
- **`-D warnings` masks later sites after the enumerated ones clear.** The signature changes ripple through many call sites; clippy/build aborts on the first error, hiding the rest. Mitigation: subtask 9 re-runs the full gate after subtasks 1–8 land; any newly-surfaced out-of-contract site is surfaced to the orchestrator, not silently absorbed. `[derived → subtask 9 gate re-run]`
- **A gp-render Context/painter test missing the Miri gate reds the workspace Miri job (blocks merge).** All adapted tests that build an `egui::Context`/drive a painter (regions `fill_*`, all heatmap painter tests, all mod.rs `render_shapes` tests incl. the new no-rebuild test, golden.rs wgpu tests) **keep/carry** `#[cfg_attr(miri, ignore = "…")]`. The pure `geometry.rs` tests (no Context) and the pure `triangulate_lattice` tests stay un-gated — matching the current cache.rs (pure, un-gated). `[measured: read cache.rs:6-10 — pure module, no Miri gate; heatmap.rs/regions.rs/mod.rs painter tests all carry the gate]` `[derived → workspace Miri gate, subtask 9]`
- **File-rename churn (`cache.rs` → `geometry.rs`).** The rename is **confirmed by the orchestrator** (both the file `git mv cache.rs geometry.rs` and the type `TrackGeometryCache` → `BakedTrackGeometry`); the spec's `cache.rs` references are being reconciled to `geometry.rs` in parallel. Adds a `git mv` (history-preserving) and updates the two `mod`/`pub use` sites (`mod.rs:17,31`, `lib.rs:34`); low risk. `[measured: rg cache → mod cache; (mod.rs:17), pub use cache::TrackGeometryCache (mod.rs:31), pub use track::{…, TrackGeometryCache} (lib.rs:34)]`

## Test Design

**Subtask 1 — `regions.rs` (`#[cfg(test)]`):**
- `triangulate_lattice_convex_square` / `triangulate_lattice_concave_l_shape` — entry `triangulate_lattice`; scenarios: a lattice square → `n-2` triangles, area-sum equals `|area|`; the L-shape lattice loop → `n-2` triangles, no triangle covers the notch. Pure (no Context) → **no Miri gate**. Reuse the existing `l_shape_loop`/`triangle_area_sum` helpers.
- `fill_emits_asphalt_mesh_then_infield_mesh` (adapt) — entry `fill`; build `mapped: Vec<Vec<Pos2>>` (map each ring loop via `TrackTransform`) + `indices: Vec<Vec<[u32;3]>>` (`triangulate_lattice` per loop); assert 2 meshes, `[0]` asphalt, `[1]` infield. Context-driven → **keeps** `#[cfg_attr(miri, ignore = "…run_ui + layer_painter…")]`.

**Subtask 2 — `heatmap.rs` (`#[cfg(test)]`):**
- `paint_emits_per_cell_meshes_plus_infield_recut`, `paint_is_noop_on_empty_heatmap` (adapt to `(verts, indices)` signature). Context-driven → keep Miri gate.
- `heatmap_reuses_cached_outer_mesh` (adapt, AC3) — capture `indices[outer_idx].as_ptr()` before, run `paint`, assert unchanged → baked indices reused, no second triangulation. Context-driven → keep Miri gate.
- Pure `speed_bounds_*`/`normalize_*`/`ramp_color_*` tests unchanged, un-gated.

**Subtask 3 — `geometry.rs` (`#[cfg(test)]`, pure, NO Miri gate — mirrors the current cache.rs pure posture):**
- `new_produces_ring_loops_roles_and_indices` (AC1) — entry `BakedTrackGeometry::new`; over the `ring_3x3` fixture assert `smoothed_loops.len() == 2`, `loop_roles.outer.len() == 1`, `loop_roles.holes.len() == 1`, `triangulated_indices.len() == 2`, each index list non-empty with `len == smoothed_loops[i].len() - 2` (well-formed triangulation).
- `new_is_deterministic` (AC1/rect-independence) — `new(&track)` twice yields equal `triangulated_indices` (topology is a pure function of the track; no rect input exists to vary). Fixture: the existing `fixture_track` over `ring_3x3`.

**Subtask 4 — `mod.rs` (`#[cfg(test)]`):**
- `render_shapes` helper (adapt) — build `let geometry = BakedTrackGeometry::new(track);` and place `geometry: &geometry` in the constructed `Scene`. All existing render-path tests (`render_frame_draws_without_panicking`, `each_overlay_changes_output_when_on`, `all_overlay_combinations_…`, `all_off_equals_…`, the two `*_is_noop_on_empty_metrics`, `fastest_lap_paint_does_not_mutate`) route through it — keep their Miri gates.
- **NEW** `resize_does_not_rebuild_geometry` (AC2 + AC5) — build one `BakedTrackGeometry`, capture `geometry.triangulated_indices[0].as_ptr()`, drive `render_frame` at rect A (200×200) then rect B (240×200) through a fontless `Context::run_ui`, assert the pointer is unchanged after both → the baked topology is never recomputed and a resize triggers no rebuild (the static pipeline runs zero times in the draw path). Context-driven → **carries** `#[cfg_attr(miri, ignore = "render_frame run_ui pass at two rects…wall-clock cost, not an abort")]`.

**Subtask 8 — golden re-verification setup:** `golden.rs`/gallery tests build `BakedTrackGeometry::new(&track)` and thread `&geometry`; wgpu goldens keep `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD…")]`.

**Subtask 9 — gates (AC6/AC8/AC9):** `cargo test` (workspace green), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`, the workspace Miri gate (`MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace`, `+nightly` locally), and the 8 canvas goldens byte-identical against existing PNGs. `[derived → subtask 9]`

## Open questions

None. Both spec Open Questions are resolved in § Approach: (1) **map on the fly** every frame — no rect-keyed vertex buffer (keeps the API rect-free, the `O(n)` map is negligible vs the removed `O(n³)`, and it does not reintroduce the rect wrinkle since the map lives inside the draw path); (2) the baked geometry rides as a **`&'a BakedTrackGeometry` field in `Scene`** (viable now that it is a `Copy` shared ref, unlike the prior `&mut Option<Cache>`), which leaves `render_frame`'s signature and AC7's contract intact and lets the race.rs `Scene` spread carry it for free.
