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
| `gp-core` (`crates/core`) | **3a** | Pure, deterministic, integer-only physics core — `geom` (dual grid, supercover), `track` (the `TrackArtifact` contract), `sim` (`step`, `legal_move`, lap counter, crash, collisions). The shared dependency of render **and** AI. |
| `gp-gen` (`crates/gen`) | **1** | Track generation — coarse-block ring (infield-first) + local repair, phases Ф1–Ф7. |
| `gp-render` (`crates/render`) | **2** | Rendering + UX — asphalt/walls derived from the corridor `D`. Visual language = the imported [design system](../docs/design-system/IMPORT.md); backend = **native Rust GUI** (tokens→consts, JSX components→widgets). |
| `gp-ai` (`crates/ai`) | **4** | AI training — feedforward policy over honest local features, 5-action masked softmax. |
| `gp-game` (`crates/game`) | **3b** | Game loop / orchestration — the runnable `graphite-gp` binary. |

Dependency edges: `gen · render · ai → core`; `game → all`; `core` depends on nothing (pure).
**Build order** (design doc §6): `3a → (1 ∥ 2) → 4`.

## Track artifact — the block-1 → block-{3a,4} contract

`{ D, walls, sf, race_dir, s_field, centerline, Vmax, tempo, fastest_lap, speed_heatmap }` (see `crates/core/src/track.rs` + `docs/design.md` §2). The AI frame is derived from the `s`-field gradient (`t̂ = normalize(∇s)`); the racing-line `centerline` curve is render-only.

## Status (2026-07-13)

- Design: **finalized** (`docs/design.md`), reviewed across 4 rounds.
- Code: **scaffold + first physics predicate** — module structure, `TrackArtifact` type, and stub APIs in place; the exact integer `supercover` predicate (block 3a, `crates/core/src/geom.rs`) is now implemented with its full §3 C4 test table (12 unit tests); the remaining algorithms are still `todo!()` (marked `TODO(<block>)`). Whole workspace builds clean.
- **Visual base:** the Claude Design "Graphite GP Design System" is imported to [`docs/design-system/`](../docs/design-system/IMPORT.md) and adopted as the canonical visual language; render target is a **native Rust GUI** (design tokens/components are a spec to port, not runnable web code).
- **Next implementation step:** continue block 3a — `step` (state advance), the signed lap counter, crash resolution, and car-collision resolution in `crates/core/src/sim.rs`, then the passability oracle. (`supercover` — the first 3a predicate and the shared foundation of `legal_move` + the oracle graph edge — is done.)

## Load-bearing details worth knowing before touching a block

- **supercover (§3, C4):** exact integer predicate, no floats. A segment through a dual vertex `(i+½,j+½)` includes **all 4** shared cells. First thing to unit-test in 3a.
- **Reward:** pure potential-based shaping (`γΦ(s')−Φ(s)`, `Φ=s`) — any `s`-field preserves the optimum; the field must be **fold-free** (BFS distance on the annulus *cut at the gate*), not perfect.
- **Lap counter:** signed S/F crossing; the timing gate is a half-grid dual edge one edge ahead of the start grid; half-open crossing test.
- **Crash:** normal→0, tangential→⌊t/2⌋, + one scrub tick; the real anti-abuse deterrent is `P_crash` / lost position, not the kinematics.
- **Collisions:** seeded nearest-free BFS in `D`; detected swaps route through the same path.

Key design decisions with rationale live in [`key-decisions.md`](key-decisions.md).
