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
| [`crates/render`](crates/render) | **2**  | Rendering + UX — asphalt and walls derived from `D`. |
| [`crates/ai`](crates/ai)       | **4**  | AI training — feedforward policy over honest local features, 5-action masked softmax. |
| [`crates/game`](crates/game)   | **3b** | Game loop / orchestration — the runnable `graphite-gp` binary. |

Dependency edges: `gen → core`, `render → core`, `ai → core`, `game → {core, gen, render, ai}`.
`core` depends on nothing (pure).

**Build order** (design doc §6): `3a → (1 ∥ 2) → 4`. AI features/reward are
designed up front so their requirements propagate backward (`centerline(s)` onto
block 1, state + legal mask onto 3a).

## Status

Block 3a in progress — the `TrackArtifact` contract is **finalized** (`SField`
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
#49): a seeded `rand_chacha` ChaCha8 RNG (cross-arch-reproducible) picks the winner
and displacement order for cars sharing a final cell; losers teleport to the
nearest free cell via geodesic BFS, velocity retained. Per a product-owner
amendment (2026-07-16) the predicate is same-final-cell only — swap/pass-through
detection was dropped, so crossings ending on distinct cells are allowed. The
`rand` + `rand_chacha` (`0.10`, `default-features = false` → no `getrandom`) stack
is adopted in both `gp-core` and `gp-gen`, with a seeded `GenParams::rng()`; the
core still carries **zero production panics**. The remaining algorithms (generation
pipeline, oracle, feature extraction, policy) are still `todo!()`. See the
`TODO(<block>)` markers.

## Build

```sh
cargo build            # whole workspace
cargo run -p gp-game   # run the graphite-gp binary (scaffold banner)
cargo test             # 97 gp-core + 2 gp-gen tests green (supercover + corridor-graph + Size/Rect + overflow-safety + typed legal_mask + track-artifact contract + sim step + lap counter + crash rule + collisions + seeded RNG)
```

MSRV: **Rust 1.97.0**. CI (GitHub Actions, `ubuntu-latest`) runs format, build,
test, clippy (`-D warnings`), and docs on every push/PR to `main`, plus an
advisory Miri lane; the workspace lint policy (`clippy::pedantic`/`nursery` =
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
