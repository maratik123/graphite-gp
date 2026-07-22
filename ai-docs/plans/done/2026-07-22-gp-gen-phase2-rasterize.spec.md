# gp-gen Ф2 — rasterize coarse ring to points `D` with width taper

**Source:** issue #25
**Date:** 2026-07-22
**Tracked in:** #25

## Scope

Implement phase **Ф2** of the generation pipeline (design doc §2, `phase2_rasterize`):
turn the coarse-block skeleton from Ф1 into the fine lattice corridor `D`.

1. **Expand each coarse ring cell into a `k×k` patch of drivable lattice points.**
   Input is Ф1's [`CoarseSkeleton`](../../crates/gen/src/phase1.rs) (`ring`/`hole` as
   `BTreeSet<Point>` of coarse-block coordinates, plus `dir`). Each `ring` cell
   `(cx, cy)` maps to the `k×k` block of fine points; the union of all patches is
   the baseline `D`. Output is a `gp_core::geom::Corridor` (dense grid, the same
   type carried by `TrackArtifact::corridor`).
2. **Wide zones reach `2k`.** Where Ф1 made the ring ≥2 coarse cells thick (its
   outward `widen` step), uniform `k×k` expansion yields a corridor ≥`2k` points
   wide by construction — no extra work beyond the expansion itself.
3. **Taper every abrupt wall step.** A block-thickness change (e.g. `k`→`2k` where a
   widened side meets a nominal side) expanded naively produces a `k`-point concave
   jog — exactly the "concave niche a supercover would cut" the design forbids.
   Ф2 must reshape such transitions so the concave wall advances by **≤ ~1 point per
   several columns** (no single column moves the concave boundary by more than 1
   point), spreading a Δ-point width change over ≥ several columns.
4. **Carve narrow zones to exactly `n`, never below `n`** (`n = ⌈m/2⌉`), with the same
   taper invariant on entry/exit of each narrow span. **Narrow zones are
   geometry-forced, not chosen** (round-1 decision): Ф2 carves an arm to `n` **only**
   where the skeleton geometry forces a thin arm — the S-hairpin case, where two
   corridor arms flank an infield finger and the local cross-gabarit is squeezed
   (`≥ 2n+1` across = two arms `≥ n` each + isthmus `≥ 1`, design §1). Everywhere
   else the corridor keeps nominal width `k` (`k ≥ n`, so no carve is needed on a
   plain 1-block-thick arm). **No RNG** — narrowing is a deterministic function of the
   coarse skeleton; Ф2 never draws which sections narrow.

Signature follows the Ф1 pattern — a standalone `pub fn phase2_rasterize(skel:
&CoarseSkeleton, k, n) -> Corridor` in a new `crates/gen/src/phase2.rs`, re-exported
from `lib.rs`. (Exact signature / helper decomposition is the design phase's call.)

## Out of scope

- Ф1 skeleton construction (#24, done) and Ф3+ (start/finish, validation, oracle,
  repair, export). Ф2 emits only the corridor `D`; walls, `sf`, `s_field`,
  `centerline`, metrics are later phases.
- Ф4 static validation (connectivity / one-hole / width-floor certification). Ф2
  should *produce* a `D` that passes Ф4 by construction, but the certification pass
  itself is #(later).
- The `generate()` pipeline wiring (`lib.rs` `todo!`) — Ф2 is a standalone phase fn;
  end-to-end wiring lands when all phases exist.
- Repairing a `D` that already violates an invariant — that is Ф6.

## Deferred

- Empirical tuning of the exact taper constant ("several" columns) | needs
  playtest/oracle-run feedback, not resolvable on paper (design doc explicitly) |
  tracked in `## Open questions`, no separate issue yet.

## Key decisions

| Question | Decision |
|---|---|
| Ф2 input | Ф1's `CoarseSkeleton { ring, hole, dir }` (coarse-block cells). |
| Ф2 output | `gp_core::geom::Corridor` = the fine lattice corridor `D`. |
| Coarse→fine map | each coarse ring cell → a `k×k` block of drivable fine points. |
| Wide-zone source | Ф1's `widen` (≥2 coarse cells thick) → ≥`2k` by construction; Ф2 adds no widening. |
| `n`, `k` source | `n = ⌈m/2⌉` = `GenParams::min_width()`; `k` = `GenParams::block_size` (`k ≥ n`). |
| Taper invariant | concave wall advances ≤1 point/column; a Δ-point change spans ≥ several columns; post-check no concave corner is cut by `supercover` at plausible entry speeds. |
| Narrow-zone designation | **Geometry-forced** (round-1): carve to `n` only where the skeleton forces a thin arm (S-hairpin arms flanking an infield finger, `≥ 2n+1` cross-gabarit); else keep `k`. No RNG. |
| Determinism | Fully deterministic in `(skeleton, k, n)` — Ф2 consumes no RNG. |

## Technical constraints

- **gp-core building blocks (all from #5, present today):** `Corridor` (dense grid;
  `new`/`set`/`contains`/`origin`/`width`/`height`), `flood_fill`,
  `component_count`, `bounded_complement_components`, `geodesic_bfs`/
  `geodesic_layers` (in-`D` BFS), `walls_from_boundary`, and `supercover(a, b)` (the
  exact integer predicate) for the concave-cut post-check. `Point`, `Rect`, `Side`,
  `Orient`, `Wall` in `gp_core::geom`.
- **No cross-section-width helper exists in gp-core** — Ф2's tests measure a
  cross-sectional width by counting contiguous drivable points along a scan line
  perpendicular to the local corridor direction. Whether Ф2 needs an internal
  width-scan helper is a design-phase call.
- **Integer-only, deterministic** (`gp-core` posture, design §3a): Ф2 geometry uses
  integer arithmetic only.
- File-size / test conventions per AGENTS.md; a `#[cfg(test)] mod tests` block is
  required (Ф2 is well over the ~50-line logic threshold).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Each coarse `ring` cell becomes a `k×k` block of drivable points in the output `Corridor` `D` (every fine point of the block is drivable; no coarse cell is dropped). |
| AC2 | Where Ф1 made the ring ≥2 coarse cells thick, the corresponding cross-sectional width of `D` is ≥`2k`. |
| AC3 | Every cross-sectional width of `D` is ≥`n` (`= ⌈m/2⌉`); narrow (technical) sections carve to **exactly** `n` and never below. |
| AC4 | At any width transition, the concave wall boundary advances by ≤1 point per column, and a Δ-point width change is spread over ≥ several columns (no abrupt concave step). Asserted on a fixed skeleton fixture. |
| AC5 | No concave corner of `D` is cut by `supercover` at the plausible entry speeds checked (post-taper invariant, design §"width profile"). |
| AC6 | Ф2 is deterministic: same `(CoarseSkeleton, k, n)` yields a byte-identical `D` (Ф2 consumes no RNG). A known small skeleton fixture pins an exact snapshot. |
| AC7 | Cross-sectional widths along the corridor match the intended profile — `n` in narrow, `k` nominal, `≥2k` wide — asserted on a deterministic fixed skeleton fixture (test note). |

## Open questions

- **Exact taper constant.** "≤ ~1 point per several columns": the concrete "several"
  (≥3? scaled by `k`?) is empirical (design doc). Design picks a defensible default
  constant; revisit under oracle/playtest data.
- **Forced-pinch detection.** The exact test for "geometry forces a thin arm here"
  (how Ф2 recognizes an S-hairpin cross-gabarit squeeze from the coarse skeleton /
  fine expansion, and how far the `n`-carve extends) is a design-phase call; a plain
  1-block-thick arm is `k ≥ n` and is never carved. No frequency/length parameter —
  narrowing occurs exactly where, and only where, the skeleton geometry forces it.
