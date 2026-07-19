# graphite-gp

[![CI](https://github.com/maratik123/graphite-gp/actions/workflows/ci.yml/badge.svg)](https://github.com/maratik123/graphite-gp/actions/workflows/ci.yml)

A grid-based **vector-racing game** (the classic "Racetrack" pencil game:
integer position + velocity, accelerate ±1 per axis per turn) with procedurally
generated closed tracks and self-taught AI opponents.

Full design: [`docs/design.md`](docs/design.md).

## Core invariant

> **A point is the center of a unit cell; a wall is a dual edge on the
> half-grid.**

From this duality, "a wall never crosses a point", "a car never touches a wall",
wall derivation from the corridor, and the correctness of legality masks all hold
*by construction*. Every block reads geometry through it.

## Workspace layout (the 4-block architecture, design doc §6)

| Crate | Block | Role |
|-------|-------|------|
| [`crates/core`](crates/core)   | **3a** | Pure, deterministic physics core — geometry, track artifact, `step` / `legal_move` / lap counter / crash / collisions. The shared dependency of render **and** AI. |
| [`crates/gen`](crates/gen)     | **1**  | Track generation — coarse-block ring (infield-first) + local repair, phases Ф1–Ф7. |
| [`crates/render`](crates/render) | **2**  | Rendering + UX — asphalt and walls derived from `D`. Draw-only: takes `egui` 0.35, never `eframe`/`winit`/`wgpu` on a normal edge. |
| [`crates/ai`](crates/ai)       | **4**  | AI training — feedforward policy over honest local features, 5-action masked softmax. |
| [`crates/game`](crates/game)   | **3b** | Game loop / orchestration — the runnable `graphite-gp` binary. |

Dependency edges: `gen → core`, `render → core`, `ai → core`, `game → {core, gen, render, ai}`.
`core` depends on nothing (pure).

**Build order** (design doc §6): `3a → (1 ∥ 2) → 4`. AI features/reward are
designed up front so their requirements propagate backward (`centerline(s)` onto
block 1, state + legal mask onto 3a).

## Status

**Block 3a (the `gp-core` physics core) is complete** — every `block:core` issue is
closed; next up per the §6 build order (`3a → (1 ∥ 2) → 4`) are block 1 (`gp-gen`,
the Ф1–Ф7 generator pipeline) and, in parallel, block 2 (`gp-render`).

**Block 2 has started**: the foundational GUI-backend decision (issue #11) is landed
— the backend is **eframe/egui 0.35**, `gp-game` owns the window + event loop, and
`gp-render` stays a **draw-only** library (`render_frame` takes a borrowed
`&egui::Painter`; `cargo tree -p gp-render --edges no-dev` carries no
`eframe`/`winit`/`wgpu`). `cargo run -p gp-game` opens a window drawing a
placeholder frame — paper background, graph-paper motif, one card, one hairline —
covered by an offscreen `egui_kittest` wgpu/Vulkan golden test that needs no display
server. The **design tokens are landed** (issue #12): all 127 CSS tokens from
[`docs/design-system/tokens/`](docs/design-system/tokens/) are ported to module-level
`SCREAMING_SNAKE_CASE` consts in `gp_render::tokens` (117 value-checked against the
CSS at test time, 10 in a documented exclusion table), and the two design-system
faces — Onest + JetBrains Mono, vendored as OFL-1.1 variable `[wght]` files —
are registered through `gp_render::fonts::definitions()`, which `gp-game` installs via
`Context::set_fonts` (`gp-render` still holds no `egui::Context`). Onest replaced the
Space Grotesk placeholder at #73, which also turned egui's `default_fonts` off: the
display face now carries the Cyrillic the design system's own `Ф1–Ф7` labels need, so
egui's bundled fallbacks stopped being load-bearing and the release binary shrank
**1,443,320 B (1.376 MiB)** — measured, not estimated. Block 2 continues at
the component units (#13–#16), which style egui widgets from these consts —
the **SVG icon pipeline (#88)** landed first: `gp_render::icons` bakes vendored
Lucide SVGs to egui textures via `resvg` + `tiny-skia` (ported from the sibling
`marshrutka` project). Building on it, the **core widgets (#13)** are now landed —
`gp_render::widgets` ports the five design-system core components (Button,
IconButton, Badge, Tag, Card) to native egui widgets styled entirely from
`crate::tokens`, each split into a Miri-clean pure `const fn resolve` style layer,
a private `paint` layer, and a public `show(ui) -> Response` interaction shell; a
`#[cfg(test)]` wgpu gallery golden covers the full variant/size/state matrix. The
**forms widgets (#14)** followed on the identical three-layer pattern —
`gp_render::widgets` adds Slider, Switch, SegmentedControl, and Stepper (their
value logic — snap/clamp, toggle, single-selection, bound-clamp — in the pure
`resolve`/value-logic layer and unit-tested; a second `forms_gallery` wgpu golden
covers their state matrix). The **game HUD widgets (#15)** are now landed on the identical three-layer pattern — `gp_render::widgets` adds Telemetry (a mono metric readout with tone/size and an `on_ink` render mode for the graphite HUD strip), LapMeter (lap/total progress cells), and CarChip (a car-ramp standings token with rank/kind/active), plus a third `game_gallery` wgpu golden matching the design-system HUD specimen. Finally the **MovePad control (#16)** landed on the same three-layer pattern — `gp_render::widgets` adds the signature plus-shaped 5-action von-Neumann keypad (Coast + `↑ ↓ ← →`, diagonals impossible) that masks illegal moves from `gp_core::sim::legal_mask` and emits the chosen `Action`, plus a fourth `movepad_gallery` wgpu golden over its state matrix — **completing all component units (#13–#16)**. The **hero track canvas (#17)** then makes `render_frame` real: `gp_render::track` draws design-doc §4 layers 1–3 + 6 back-to-front over the corridor `D` — regions (outfield / asphalt = union of unit cells over `D` / bounded infield hole), the half-grid walls (chained into loops and cosmetically Chaikin-smoothed within the half-cell gap under an M6 guard that never changes `D`), the checkered S/F chord, and cars (a `GRAPHITE_900`-outlined dot + a `Painter::arrow` velocity vector ∝ `(vx,vy)` + a fading trail + an optional dashed `ACCENT` "you" ring) with a linear move animation; per-car render input is **caller-supplied** (draw-only). The **analytics overlays + notebook grid (#18)** then make the `Overlays { speed_heatmap, fastest_lap, grid }` flags real (design-doc §4 layers 4–5): a per-cell **speed heatmap** that recolors PR #100's smoothed asphalt mesh (each cell drawn through `Painter::with_clip_rect` over a triangulate-once shared mesh, infield re-cut — so the fill traces the **same** Chaikin-smoothed boundary the walls stroke), a dashed `ACCENT` **fastest-lap** uniform Catmull-Rom spline, and a whole-canvas **notebook grid** (ruling at the lattice cell pitch, heavier line every 5th, `GRID_DOT` lattice) — each flag independently toggleable, all-off byte-identical to #17, with three new miri-ignored wgpu goldens and no new dependency or token. The
`TrackArtifact` contract is **finalized** (`SField`
distance/gradient/tangent accessors, `StartGrid`, the `TimingGate` half-grid
segment on `StartFinish`, and `Centerline::at` arc-length sampling — issue #6;
contract types + read accessors on hand-filled fixtures, the block-1 generator
that populates them stays `todo!`). `crates/core/src/geom/` implements the exact
integer `supercover` predicate (full §3 C4 test table) plus the corridor-graph
helpers — 4-conn flood-fill / connected-component counting,
`bounded_complement_components` (the §2 Ф4 infield-hole test), in-`D` geodesic
BFS, and `walls_from_boundary` (dual edges). The corridor's box/index math is factored into unsigned `Size`/`Rect`
value types, so `Corridor` dimensions are unsigned and **gp-core carries zero
production panics** (`Rect::index` is total via `checked_sub` + `try_from`). All
integer arithmetic is **overflow- and signedness-safe**, machine-enforced by a
workspace `clippy::arithmetic_side_effects = "deny"` lint (issue #48). The `sim`
`step` (accelerate-then-advance state advance) is implemented alongside the
already-live `legal_move` / `legal_mask` legality path (issue #7), and the signed
`LapCounter::register_move` — the half-open S/F crossing test over the half-grid
timing gate, with the `legal_move`-first valid-finish conjunction (issue #8).
`sim::resolve_crash` — the quench-with-scrub crash rule (respawn at the last swept
cell in `D`, normal→0 / tangential→`⌊t/2⌋`, one forced-`Coast` scrub tick, and an
`L∈D`-guarded whole-vector-halving fail-safe that never yields a penalty-free
`v=0`) — returns a `CrashOutcome` with `action_mask` / `consume_scrub` (issue #9).
`sim::resolve_collisions` — **same-final-cell** collision resolution (issues #10 +
#49): a caller-owned `rand_chacha` ChaCha8 RNG handle (`&mut ChaCha8Rng`,
cross-arch-reproducible) picks the winner and displacement order for cars sharing
a final cell; losers teleport to the nearest free cell via geodesic BFS, velocity
retained. Per product-owner amendments (2026-07-16) the predicate is
same-final-cell only (swap/pass-through detection dropped, so crossings ending on
distinct cells are allowed), and the RNG is a shared per-domain stream handle
(physics / track-gen / AI) rather than a per-call seed. The
`rand` + `rand_chacha` (`0.10`, `default-features = false` → no `getrandom`) stack
is adopted in both `gp-core` and `gp-gen`, with a seeded `GenParams::rng()`; the
core still carries **zero production panics**. The remaining algorithms (generation
pipeline, oracle, feature extraction, policy) are still `todo!()`. See the
`TODO(<block>)` markers.

## Build

```sh
cargo build            # whole workspace
cargo run -p gp-game   # run the graphite-gp binary (scaffold banner)
cargo test             # 285 workspace tests green (98 gp-core; 183 gp-render: design tokens, fonts, tessellation smoke + golden guard, icon pipeline, core widgets + forms widgets + game HUD widgets + MovePad + track canvas + analytics overlays/notebook grid + gallery/track/overlay goldens; 2 gp-gen; 2 doc-tests)
```

MSRV: **Rust 1.97.1**. CI (GitHub Actions, `ubuntu-latest`) runs format, build,
test, clippy (`-D warnings`), and docs on every push/PR to `main`, plus a
required Miri lane (Tree Borrows, gated via the `miri-pass` aggregator, #76);
the workspace lint policy (`clippy::pedantic`/`nursery` =
`deny`) lives in the root `Cargo.toml` + `clippy.toml` (see
[`ai-docs/code-style.md`](ai-docs/code-style.md) § Linter posture).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
