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
closed. Per the §6 build order (`3a → (1 ∥ 2) → 4`), **block 1 (`gp-gen`) has now
started**: the first generator phases **Ф1 (coarse-block ring, infield-first) + the
grouped seeded-RNG config**, then **Ф2 (rasterize the coarse ring to the fine
corridor `D` with an additive width taper)**, and **Ф3 (start/finish, accel zone,
start grid, timing gate)** are landed (`gp_core::rng::Seeds` groups
four independently-seeded `ChaCha8Rng` sources — collision / generation / AI-learning /
AI-inference; `phase1_coarse_ring` builds the deterministic coarse annulus;
`phase2_rasterize` expands each coarse cell to a `k×k` fine block, tapers outfield
walls to a supercover-safe 45° ramp, and absorbs any pocket the taper seals;
`phase3_start_finish` thickens a straight coarse run and lays the start grid + the
half-grid timing gate — all deterministic, no-RNG, zero-panic), closing issues #24,
#25, #26, and #49 and discharging #50 (deterministic-order collections). **Ф4
(static validation)** has now landed too (#27): the four static checks — emitting
typed `Issue`s (`Disconnected` / `BadTopology` / `Narrow` / `NarrowSf` /
`LostHairpin`) for the future Ф6 repair phase — built on new reusable integer
**distance-transform + medial-axis** primitives in `gp-core` `geom` that Ф7's
centerline will consume. The remaining pipeline is Ф5–Ф7 (#28–#34). Block 2
(`gp-render`) is **complete** — see below.

**Block 2 (the `gp-render` draw-only renderer) is complete** — every
`block:render` issue is closed. The backend is **eframe/egui 0.35**: `gp-game`
owns the window and event loop while `gp-render` stays a draw-only library
(`render_frame` takes a borrowed `&egui::Painter`; `cargo tree -p gp-render
--edges no-dev` carries no `eframe`/`winit`/`wgpu`). It ships the full
design-system port — all 127 CSS tokens as `gp_render::tokens` consts and the
Onest / JetBrains Mono variable `[wght]` fonts via `gp_render::fonts` (#11–#12,
#73), an `resvg`/`tiny-skia` Lucide-SVG icon bake (`gp_render::icons`, #88), the
thirteen core / forms / HUD / MovePad widgets each a pure `const fn resolve` →
private `paint` → public `show` triple (#13–#16), the hero track canvas with
analytics overlays and notebook grid (#17–#18), all four screens — Setup /
Track lab / Race / Results (#19–#22) — and the `AppShell` screen router (#23),
with the `gp-game` binary wired to drive the shell from hand-built fixture data
(real generation → sim → AI wiring is block 3b). Every draw layer is covered by
an offscreen `egui_kittest` wgpu/Vulkan golden (Miri-gated) and `gp-render`
carries **zero production panics**.

The `TrackArtifact` contract is **finalized** (`SField`
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
cargo test             # 464 workspace tests green (116 gp-core; 248 gp-render: design tokens, fonts, tessellation smoke (canary), icon pipeline, core widgets + forms widgets + game HUD widgets + MovePad + track canvas + analytics overlays/notebook grid + setup screen + track lab screen + race screen + results screen + app shell/router + single-galley paint helper + gallery/track/overlay/setup/lab/race/results/app-shell goldens; 98 gp-gen; 2 doc-tests)
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
