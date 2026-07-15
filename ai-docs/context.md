# Project context — graphite-gp

## Purpose

A grid-based **vector-racing game** (the classic "Racetrack" pencil game: integer position + velocity, accelerate ±1 per axis per turn) with procedurally generated closed tracks and self-taught AI opponents, in Rust.

The canonical, finalized specification is **[`docs/design.md`](../docs/design.md)** — read it for anything non-obvious about the model, physics, generation, rendering, or AI. The multi-round review that hardened it is **[`docs/design-review.md`](../docs/design-review.md)**. This file is the short orientation; the design doc is the source of truth.

## Core invariant

> **A point is the center of a unit cell; a wall is a dual edge on the half-grid.**

From this duality, "a wall never crosses a point", "a car never touches a wall", wall derivation from the corridor, and the correctness of legality masks all hold *by construction*. Every module reads geometry through it.

## Architecture — 4 blocks → 5 crates

| Crate | Block | Role |
|-------|-------|------|
| `gp-core` (`crates/core`) | **3a** | Pure, deterministic, integer-only physics core — `geom` (dual grid + `Size`/`Rect` value types, supercover, corridor-graph helpers), `track` (the `TrackArtifact` contract), `sim` (`step`, `legal_move`, lap counter, crash, collisions). The shared dependency of render **and** AI. |
| `gp-gen` (`crates/gen`) | **1** | Track generation — coarse-block ring (infield-first) + local repair, phases Ф1–Ф7. |
| `gp-render` (`crates/render`) | **2** | Rendering + UX — asphalt/walls derived from the corridor `D`. Visual language = the imported [design system](../docs/design-system/IMPORT.md); backend = **native Rust GUI** (tokens→consts, JSX components→widgets). |
| `gp-ai` (`crates/ai`) | **4** | AI training — feedforward policy over honest local features, 5-action masked softmax. |
| `gp-game` (`crates/game`) | **3b** | Game loop / orchestration — the runnable `graphite-gp` binary. |

Dependency edges: `gen · render · ai → core`; `game → all`; `core` depends on nothing (pure).
**Build order** (design doc §6): `3a → (1 ∥ 2) → 4`.

## Track artifact — the block-1 → block-{3a,4} contract

`{ D, walls, sf, race_dir, s_field, centerline, Vmax, tempo, fastest_lap, speed_heatmap }` (see `crates/core/src/track.rs` + `docs/design.md` §2). The AI frame is derived from the `s`-field gradient (`t̂ = normalize(∇s)`); the racing-line `centerline` curve is render-only.

## Status (2026-07-15)

- Design: **finalized** (`docs/design.md`), reviewed across 4 rounds.
- Code: **scaffold + geom physics primitives** — module structure, `TrackArtifact` type, and stub APIs in place. `crates/core/src/geom/` (split into `mod.rs` + a private `graph.rs`) implements the exact integer `supercover` predicate (full §3 C4 test table) plus the corridor-graph helpers: 4-conn `flood_fill` / `component_count`, `bounded_complement_components` (the §2 Ф4 infield-hole test), in-`D` geodesic BFS (`CorridorScratch::geodesic_bfs` reusable-scratch visitor + eager `geodesic_layers`), and `walls_from_boundary` (Ф7 dual edges). The box/index math is factored into `Size { width, height }` (unsigned) + `Rect { origin, size }` value types; `Corridor` is `{ rect, cells }` with **unsigned** dimensions — negative dims are unrepresentable, so `Corridor::new` needs no `assert!` and **gp-core has zero production panics** (`Rect::index` is total via `checked_sub` + `usize::try_from`). `Wall` is `{ cell, side: Side }` (4-way outward `Side`). **45 gp-core unit tests.** The remaining `sim` / `gen` / `render` / `ai` algorithms are still `todo!()` (marked `TODO(<block>)`). Whole workspace builds clean.
- **Visual base:** the Claude Design "Graphite GP Design System" is imported to [`docs/design-system/`](../docs/design-system/IMPORT.md) and adopted as the canonical visual language; render target is a **native Rust GUI** (design tokens/components are a spec to port, not runnable web code).
- **CI / tooling:** GitHub Actions CI (`ubuntu-latest`) is in place — `changes`-gated format/build/test/clippy/docs + advisory Miri + `-pass` branch-protection gates, sccache, and a mandatory Linux software-Vulkan env-init in the `test` job (ready for the block-2 wgpu/Vulkan renderer). Plus Dependabot (cargo + github-actions), MSRV bumped to **1.97.0** (`resolver = "3"` retained — virtual workspace), `CARGO_BUILD_WARNINGS=deny`, and a strict workspace lint policy (`clippy::pedantic`/`nursery` = `deny`, `missing_docs`/`broken_intra_doc_links` = `deny`) in the root `Cargo.toml` + `clippy.toml`, each crate opting in via `[lints] workspace = true`. See [`code-style.md`](code-style.md) § Linter posture.
- **Next implementation step:** continue block 3a — `step` (state advance), the signed lap counter, crash resolution, and car-collision resolution in `crates/core/src/sim.rs`, then the passability oracle. (`supercover` and the corridor-graph helpers — `flood_fill` / `component_count` / `bounded_complement_components`, geodesic BFS, `walls_from_boundary` — are done; `resolve_collisions` can now build nearest-free placement on `CorridorScratch::geodesic_bfs`, and `gen`'s Ф4 static-validation on the component/complement helpers.)

## Load-bearing details worth knowing before touching a block

- **supercover (§3, C4):** exact integer predicate, no floats. A segment through a dual vertex `(i+½,j+½)` includes **all 4** shared cells. First thing to unit-test in 3a.
- **Reward:** pure potential-based shaping (`γΦ(s')−Φ(s)`, `Φ=s`) — any `s`-field preserves the optimum; the field must be **fold-free** (BFS distance on the annulus *cut at the gate*), not perfect.
- **Lap counter:** signed S/F crossing; the timing gate is a half-grid dual edge one edge ahead of the start grid; half-open crossing test.
- **Crash:** normal→0, tangential→⌊t/2⌋, + one scrub tick; the real anti-abuse deterrent is `P_crash` / lost position, not the kinematics.
- **Collisions:** seeded nearest-free BFS in `D`; detected swaps route through the same path.

Key design decisions with rationale live in [`key-decisions.md`](key-decisions.md).
