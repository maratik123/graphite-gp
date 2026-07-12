# graphite-gp

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

Scaffold only — module structure, the `TrackArtifact` contract, and stub APIs are
in place; the algorithms (supercover, `step`, generation pipeline, oracle,
feature extraction, policy) are `todo!()`. See the `TODO(<block>)` markers.

## Build

```sh
cargo build            # whole workspace
cargo run -p gp-game   # run the graphite-gp binary (scaffold banner)
cargo test             # once tests exist
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
