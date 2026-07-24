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
| `gp-core` (`crates/core`) | **3a** | Pure, deterministic, integer-only physics core — `geom` (dual grid + `Size`/`Rect` value types, supercover, corridor-graph helpers, distance-transform + medial-axis), `track` (the `TrackArtifact` contract), `sim` (`step`, `legal_move`, lap counter, crash, collisions). The shared dependency of render **and** AI. |
| `gp-gen` (`crates/gen`) | **1** | Track generation — coarse-block ring (infield-first) + local repair, phases Ф1–Ф7. |
| `gp-render` (`crates/render`) | **2** | Rendering + UX — asphalt/walls derived from the corridor `D`. Visual language = the imported [design system](../docs/design-system/IMPORT.md); backend = **eframe/egui 0.35** (#11). `gp-render` is **draw-only** — it takes `egui` (plus the `resvg`/`tiny-skia` CPU raster stack for #88's icon bake) and **never** `eframe`/`winit`/`wgpu` on a normal edge (`cargo tree -p gp-render --edges no-dev`), so it ships GUI-free by construction; `gp-game` owns the window + event loop. |
| `gp-ai` (`crates/ai`) | **4** | AI training — feedforward policy over honest local features, 5-action masked softmax. |
| `gp-game` (`crates/game`) | **3b** | Game loop / orchestration — the runnable `graphite-gp` binary. |

Dependency edges: `gen · render · ai → core`; `game → all`; `core` depends on nothing (pure).
**Build order** (design doc §6): `3a → (1 ∥ 2) → 4`.

## Track artifact — the block-1 → block-{3a,4} contract

`{ D, walls, sf (+ `TimingGate` segment), race_dir, s_field, start_grid, centerline, metrics }` (see `crates/core/src/track.rs` + `docs/design.md` §2). The **contract types + read accessors are finalized** (issue #6): `SField` (`scalar_at`/`gradient_at`/`tangent_at`), `StartGrid`, the half-grid `TimingGate` on `StartFinish`, and `Centerline::at` arc-length sampling — exercised on hand-filled fixtures; the block-1 generator that *populates* them stays `todo!`. The AI frame is derived from the `s`-field gradient (`t̂ = normalize(∇s)`); the racing-line `centerline` curve is render-only.

## Status (2026-07-23)

- **Design:** finalized (`docs/design.md`), reviewed across 4 rounds.
- **Block 3a (`gp-core`) — COMPLETE:** `supercover` (#4), corridor-graph helpers (#5), the `TrackArtifact` contract (#6), `sim::step` + `legal_move`/`legal_mask` (#7), `LapCounter::register_move` (#8), `resolve_crash` (#9), `resolve_collisions` (#10) all landed; zero open `block:core` issues; **zero production panics** by construction (integer-only, overflow-/signedness-safe — #48).
- **Block 1 (`gp-gen`) — in progress:** the grouped seeded-RNG config (`gp_core::rng::Seeds`, #49/#50) plus Ф1 coarse-block ring (#24), Ф2 rasterize-to-`D` (#25), Ф3 start/finish + accel zone + start grid + timing gate (#26), Ф4 static validation (#27 — the four static checks emitting typed `Issue`s [`Disconnected`/`BadTopology`/`Narrow`/`NarrowSf`/`LostHairpin`] for the future Ф6 repair phase, plus reusable integer distance-transform + medial-axis primitives in `gp-core` `geom`), Ф5a passability reachability substrate + V=1 liveness oracle (#28 — `forward_reachable`/`backward_reachable`/`oracle_liveness_v1` in `gp-gen`, reusing core `legal_move`; also fixed a confirmed `gp-core` `LapCounter::register_move` bug where the S/F crossing tested the gate's infinite line instead of design §3's bounded chord, so a closed-ring lap could never reach `raw() >= 1`), and Ф5b full `Vmax` oracle (#29 — `phase5_full_oracle` in `gp-gen`: iterative-deepening `R ∩ B` live-set with the true `Vmax_attain` halt, provable-crash exclusion, `tempo`/`fastest_lap`/`speed_heatmap` metrics into `TrackMetrics`, and a goal-aware frontier diagnostic on no-lap — all composing the Ф5a floods, no reimplementation), and Ф6 `map_frontier_gap_to_edge` (#30 — the design's riskiest step `[N3]`: a **verified-growth greedy** returning `RepairCandidate::Edge(Wall)` only for an edit it has *proved* strictly grows `|P0|` at `V_ceil = 1`, else `NoCandidate`; it also **amended the Ф5b diagnostic** `break_points: Vec<Point>` → `stall_walls: Vec<Wall>`, because the old payload was provably a subset of `D` and so could never name the non-drivable cell a geometric sever needs added. Its AC7 proof gate — run *before* the outcome enum's arity was locked — established that the dynamic-only stall class is **empty**, structurally: `live` is monotone in `V_ceil`, so `NotLappable` can only be returned at `V_ceil = 1`), and the **full Ф6 local-repair phase** (#31 — `phase6_local_repair`: a **single pass** over the issue list dispatching six defect labels to five repair arms, each a one-wall/one-cell edit committed only when that arm's own metric strictly improves on a scratch copy, else `RepairOutcome::Failed`; the `[C3]` recheck scope is chosen by **edit type** — add→local, remove→global flood-fill, run-out→**speed-sink→speed-sink**. It also **built the three missing detectors** so no arm ships as dead vocabulary: `ConcaveChordCut` + `ArmsMerging` (static, Ф4 family) and `NoBraking` (dynamic, a `v_target`-referenced run-out budget check — it **cannot** be a lappability verdict, since #30's AC7 proof makes the dynamic-only stall class empty). Spec-amending discovery: the originally specified path-indexed `runout_room` made every run-out add-arm **provably vacuous**, so it is geometric [`wall_run − 1`] plus a measured `v_corner` term), plus a behaviour-preserving **derived-`Ord` cleanup** (#153 — the `phase6_repair`/`phase5b` ordering surrogates `issue_rank`/`axis_rank`/`issue_sort_key`/`wall_sort_key` replaced by `Ord` derives on `gp-core`'s `Orient`/`Side`/`Wall` and gen's `Issue`/`DispatchLabel`; `Issue` reordered to severity order), and the **Ф7 s-field producer** (#32 — `SField::from_gate_bfs` in `gp-core` `track.rs` + a generic multi-seed, barrier-aware `barrier_distance_field` 4-conn BFS in `gp-core` `geom/graph.rs`, seeded from the gate's forward face with `TimingGate::separates` edges as barriers: the fold-free single-valued `0→L` progress coordinate `s`, source of the AI frame `∇s` and reward `Δs` — delivered as an independently-tested unit since `generate()` is still a stub, so it lives in `gp-core` not `gp-gen`), have landed; next unit is **the Ф6 repair budget loop + Ф7 pipeline assembly** (iterate `phase6_local_repair` under a repair budget in `generate()`, then reseed / output assembly / wiring — including calling `from_gate_bfs` to populate `s_field`).
- **Block 2 (`gp-render`) — COMPLETE:** every `block:render` issue closed — GUI backend eframe/egui 0.35 (#11), design tokens + fonts (#12, Onest swap #73), SVG icon bake (#88), all component/forms/HUD widgets + MovePad (#13–#16), the hero track canvas (#17) + analytics overlays/grid (#18), all four screens (Setup #19 / Lab #20 / Race #21 / Results #22), the render-input consolidation (#111), the app shell + screen router (#23), single-galley text refactor (#96), and track-geometry caching (#104) all landed; **zero production panics**. The live game-screen assembly / turn loop is **block 3b** (separate).
- **CI / tooling:** `changes`-gated fmt/build/test/clippy/docs + a **required** Miri gate (#76), Dependabot, MSRV **1.97.1**, strict workspace lints (`clippy::pedantic`/`nursery`/`arithmetic_side_effects` = `deny`). See [`code-style.md`](code-style.md) § Linter posture.

> **The full per-issue implementation log — every merged issue's design decisions, traps, and invariants worth not rediscovering — lives in [`context-status.md`](context-status.md). Read it on demand when touching a specific block or issue.**


## Load-bearing details worth knowing before touching a block

- **supercover (§3, C4):** exact integer predicate, no floats. A segment through a dual vertex `(i+½,j+½)` includes **all 4** shared cells. First thing to unit-test in 3a.
- **Reward:** pure potential-based shaping (`γΦ(s')−Φ(s)`, `Φ=s`) — any `s`-field preserves the optimum; the field must be **fold-free** (BFS distance on the annulus *cut at the gate*), not perfect.
- **Lap counter:** signed S/F crossing; the timing gate is a half-grid dual edge one edge ahead of the start grid; half-open crossing test.
- **Crash:** normal→0, tangential→⌊t/2⌋, + one scrub tick; the real anti-abuse deterrent is `P_crash` / lost position, not the kinematics.
- **Collisions:** seeded nearest-free BFS in `D` for **same-final-cell conflicts only** (product-owner amendment 2026-07-16 — the swap/pass-through detector was dropped; swaps / mid-segment / orthogonal crossings ending on distinct cells are allowed). The RNG is a **caller-owned `&mut Xoshiro256PlusPlus` handle** (PR #68 amendment; engine set by issue #139) — one long-lived stream per domain (physics / gen / AI-learn / AI-infer). Each domain materializes a purpose-fit engine (#139): `Xoshiro256PlusPlus` for generation / collision / AI-inference, `ChaCha8Rng` for AI-learning.

Key design decisions with rationale live in [`key-decisions.md`](key-decisions.md).
