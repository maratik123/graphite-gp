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
| `gp-render` (`crates/render`) | **2** | Rendering + UX — asphalt/walls derived from the corridor `D`. Visual language = the imported [design system](../docs/design-system/IMPORT.md); backend = **eframe/egui 0.35** (#11). `gp-render` is **draw-only** — it takes `egui` (plus the `resvg`/`tiny-skia` CPU raster stack for #88's icon bake) and **never** `eframe`/`winit`/`wgpu` on a normal edge (`cargo tree -p gp-render --edges no-dev`), so it ships GUI-free by construction; `gp-game` owns the window + event loop. |
| `gp-ai` (`crates/ai`) | **4** | AI training — feedforward policy over honest local features, 5-action masked softmax. |
| `gp-game` (`crates/game`) | **3b** | Game loop / orchestration — the runnable `graphite-gp` binary. |

Dependency edges: `gen · render · ai → core`; `game → all`; `core` depends on nothing (pure).
**Build order** (design doc §6): `3a → (1 ∥ 2) → 4`.

## Track artifact — the block-1 → block-{3a,4} contract

`{ D, walls, sf (+ `TimingGate` segment), race_dir, s_field, start_grid, centerline, metrics }` (see `crates/core/src/track.rs` + `docs/design.md` §2). The **contract types + read accessors are finalized** (issue #6): `SField` (`scalar_at`/`gradient_at`/`tangent_at`), `StartGrid`, the half-grid `TimingGate` on `StartFinish`, and `Centerline::at` arc-length sampling — exercised on hand-filled fixtures; the block-1 generator that *populates* them stays `todo!`. The AI frame is derived from the `s`-field gradient (`t̂ = normalize(∇s)`); the racing-line `centerline` curve is render-only.

## Status (2026-07-18)

- **Design:** finalized (`docs/design.md`), reviewed across 4 rounds.
- **Block 3a (`gp-core`) — COMPLETE:** `supercover` (#4), corridor-graph helpers (#5), the `TrackArtifact` contract (#6), `sim::step` + `legal_move`/`legal_mask` (#7), `LapCounter::register_move` (#8), `resolve_crash` (#9), `resolve_collisions` (#10) all landed; zero open `block:core` issues; **zero production panics** by construction (integer-only, overflow-/signedness-safe — #48).
- **Block 1 (`gp-gen`) — in progress:** the grouped seeded-RNG config (`gp_core::rng::Seeds`, #49/#50) plus Ф1 coarse-block ring (#24), Ф2 rasterize-to-`D` (#25), and Ф3 start/finish + accel zone + start grid + timing gate (#26) have landed; next unit is **Ф4 static validation (#27)**.
- **Block 2 (`gp-render`) — in progress:** GUI backend eframe/egui 0.35 (#11), design tokens + fonts (#12, Onest swap #73), SVG icon bake (#88), all component/forms/HUD widgets + MovePad (#13–#16), the hero track canvas (#17) + analytics overlays/grid (#18), all four screens (Setup #19 / Lab #20 / Race #21 / Results #22), the render-input consolidation (#111), the app shell + screen router (#23), single-galley text refactor (#96), and track-geometry caching (#104) have landed. Remaining surface is the live game-screen assembly / turn loop (**block 3b**), deferred until block 1 lands.
- **CI / tooling:** `changes`-gated fmt/build/test/clippy/docs + a **required** Miri gate (#76), Dependabot, MSRV **1.97.1**, strict workspace lints (`clippy::pedantic`/`nursery`/`arithmetic_side_effects` = `deny`). See [`code-style.md`](code-style.md) § Linter posture.

> **The full per-issue implementation log — every merged issue's design decisions, traps, and invariants worth not rediscovering — lives in [`context-status.md`](context-status.md). Read it on demand when touching a specific block or issue.**


## Load-bearing details worth knowing before touching a block

- **supercover (§3, C4):** exact integer predicate, no floats. A segment through a dual vertex `(i+½,j+½)` includes **all 4** shared cells. First thing to unit-test in 3a.
- **Reward:** pure potential-based shaping (`γΦ(s')−Φ(s)`, `Φ=s`) — any `s`-field preserves the optimum; the field must be **fold-free** (BFS distance on the annulus *cut at the gate*), not perfect.
- **Lap counter:** signed S/F crossing; the timing gate is a half-grid dual edge one edge ahead of the start grid; half-open crossing test.
- **Crash:** normal→0, tangential→⌊t/2⌋, + one scrub tick; the real anti-abuse deterrent is `P_crash` / lost position, not the kinematics.
- **Collisions:** seeded nearest-free BFS in `D` for **same-final-cell conflicts only** (product-owner amendment 2026-07-16 — the swap/pass-through detector was dropped; swaps / mid-segment / orthogonal crossings ending on distinct cells are allowed). The RNG is a **caller-owned `&mut ChaCha8Rng` handle** (PR #68 amendment) — one long-lived stream per domain (physics / gen / AI-learn / AI-infer).

Key design decisions with rationale live in [`key-decisions.md`](key-decisions.md).
