# gp-gen Ф7: racing centerline (render-only)

**Source:** issue #33
**Date:** 2026-07-24
**Tracked in:** #33

## Scope

Implement Ф7's render-only racing centerline in `gp-gen`: the `racing_line`
producer named at `docs/design.md` §2 line 191 (`centerline =
racing_line(medial_axis(D))`). Given the final repaired corridor `D`, the
start/finish `TimingGate`, and the global `race_dir`, produce a single
non-branching closed curve parameterised by arc length and write it into the
existing `Centerline` (`crates/core/src/track.rs`).

1. **Consume the medial axis.** Build `DistanceTransform::compute(&D)` and
   `gp_core::geom::medial_axis(&dt)` (both already exist). The medial-axis
   primitive deliberately does **not** thin an even-width 2-cell ridge band to a
   single strand, bridge a residual 1-cell diagonal gap at a rectilinear corner,
   or trim branches — its rustdoc states these are `racing_line`'s job. Ф7 owns
   all three.
2. **Trim to a single closed loop.** Reduce the (potentially branching, on
   infield-finger / hairpin tracks) ridge cell set to one non-branching cycle:
   thin 2-cell bands, bridge 1-cell diagonal corner gaps, and prune spur
   branches so the remainder is a single closed 4-/8-connected loop.
3. **Order into a cycle and resample by arc length.** Walk the loop into an
   ordered ring, then resample at an even arc-length step so successive samples
   are approximately equidistant.
4. **Assign monotone closed `s` and race_dir-oriented tangents.** `s` grows
   monotonically `0 → length` around the loop and closes on itself
   (`at(length) ≡ at(0)`). Each sample carries a unit tangent whose sign follows
   `race_dir`. `samples[0].s` MUST equal `0` (the `Centerline::at` precondition).
5. **Populate `core::track::Centerline`.** Fill `samples` (ordered
   `CenterlineSample { s, pos, tangent }`, sub-cell fractional positions) and
   `length` (total loop arc length). This is the artifact the renderer's ideal
   line (P2) reads; it is wired into `TrackArtifact::centerline`.
6. **Keep it render-only (P2 / D2).** The block1→block4 contract stays
   `{ D, walls, sf, race_dir, s_field }` — the centerline is NOT part of the AI
   contract. No `gp-ai` symbol may read the `Centerline` type or the
   `TrackArtifact::centerline` field.

## Out of scope

- The `s_field` scalar field (`SField::from_gate_bfs`) — a separate artifact
  (issue #32) and the source of the AI frame; unchanged here.
- Any AI feature / reward code. Per P2 the AI tangent/normal frame comes from
  `∇s` (`SField::tangent_at`), never from this render curve — do not route the
  centerline into `gp-ai`.
- Wiring a full end-to-end `phase7_output(...) → TrackArtifact` assembly if the
  surrounding Ф7 export orchestration does not yet exist; this task delivers the
  `racing_line` producer and its population of `Centerline`. (Design confirms
  the integration seam.)
- Renderer consumption of the centerline (block 2).
- Tuning the resample step for visual quality — a functional even-spacing
  default suffices; render polish is later.

## Deferred

- Best-effort robustness of loop-trimming on pathological branching medial axes
  beyond the test fixtures | render-only, no gameplay/AI impact, and an empty
  `Centerline` degrades gracefully (`at` returns `None`) | no separate issue —
  revisit if the renderer surfaces a bad ideal line.

## Key decisions

| Question | Decision |
|---|---|
| Where does `racing_line` live? | `gp-gen` (new module, e.g. `phase7.rs`); it is generation output, consumed by the renderer. Exact file layout / signature is design's call. |
| What is the `s = 0` anchor? | The start/finish gate. `s` increases along `race_dir` from the gate, matching the `s_field`'s gate-seeded orientation so the render curve and the AI field agree on direction. (Sensible default; the AC only requires monotone-closed + race_dir tangents, but anchoring at the gate keeps the ideal line coherent with the lap counter.) |
| Resample step (spacing vs fixed count)? | Even arc-length spacing at an implementation-chosen step (≈ one cell is a reasonable default); design picks. AC requires "approximately evenly spaced", not an exact count. |
| Loop-trim algorithm (thin / bridge / prune)? | Design's call — spur-pruning to a single cycle, band-thinning, and diagonal-corner bridging are all implementation detail. |
| Tangent computation | Geometric unit tangent of the resampled polyline (finite difference between neighbouring samples), sign-aligned to `race_dir`. This is the render curve's own tangent — distinct from the AI's `∇s` tangent (P2). |
| Fallback when no clean single loop can be extracted | Produce an empty / best-effort `Centerline`; it is render-only and `Centerline::at` already returns `None` gracefully. Never panic. |
| How is AC4 ("no AI symbol reads the centerline") enforced/tested? | Design chooses the mechanism (e.g. a source-scan test over `crates/ai/src`, or a documented architectural invariant). Currently `gp-ai` only has stubs and reads neither the type nor the field. |

## Technical constraints

- **Deterministic, integer-derived.** The medial-axis input and loop extraction
  are integer/BTree-based (design §3a). The `Centerline` output carries sub-cell
  fractional sample positions and unit tangents — the render curve is the
  established §3a exemption from the integer-only rule (the `Centerline` /
  `CenterlineSample` types already store fractional samples). No nondeterminism:
  same `D` + gate + `race_dir` ⇒ byte-identical `Centerline`.
- **Reuse the existing primitives.** `DistanceTransform::compute`,
  `gp_core::geom::medial_axis`, `Centerline` / `CenterlineSample`,
  `TimingGate::forward_face`, `RaceDir` all exist — do not reimplement.
- **`Centerline::at` precondition.** The resample MUST seed `samples[0].s == 0`;
  `length` MUST be the positive total loop arc length.
- **Determinism-friendly ordering.** `medial_axis` returns a `BTreeSet<Point>`
  for cross-platform-stable iteration; preserve deterministic ordering through
  trimming and resampling.
- **Code style / gates.** New logic ≥ ~50 lines needs a `#[cfg(test)]` module;
  strict clippy (`-D warnings`); every public item documented; module-level
  `SCREAMING_SNAKE_CASE` for any semantic numeric literal (e.g. the resample
  step). Miri-clean (pure integer/float compute, no FFI) — if any test is a cost
  outlier, gate per-test per the Miri rule, but `gp-gen` is currently `--exclude`d
  from the Miri gate (#134) so this is unlikely to bite.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | On a branching medial axis, `racing_line` trims it to a single non-branching closed loop (spurs pruned, 2-cell bands thinned, diagonal corner gaps bridged). |
| AC2 | The loop is resampled by arc length: `s` is monotone (strictly increasing) `0 → length`, samples are approximately evenly spaced, and the loop closes (`at(length) ≡ at(0)`, first/last samples adjacent around the ring). |
| AC3 | Each `CenterlineSample.tangent` is a unit vector whose sign follows `race_dir` (walking the samples advances along `race_dir`). |
| AC4 | Output populates `core::track::Centerline`: ordered `samples` with `samples[0].s == 0`, and `length` = total loop arc length (positive, finite). |
| AC5 | The centerline is not referenced by any AI feature code — no `gp-ai` symbol reads the `Centerline` type or `TrackArtifact::centerline` (render-only per P2/D2; block1→block4 contract stays `{ D, walls, sf, race_dir, s_field }`). |
| AC6 | Deterministic on a fixture: repeated runs on the same rectangular-ring `D` + gate + `race_dir` produce an identical `Centerline`. |
| AC7 | Fixture test — a rectangular ring: assert the centerline loop closes, `s` samples are monotone and approximately evenly spaced, and tangents follow `race_dir`. |
| AC8 | Gates green: `cargo build`, `cargo clippy --workspace --all-targets -D warnings`, `cargo fmt --check`, doc build clean. |

## Open questions

None blocking. The resample step value, the exact loop-trim algorithm, the
`racing_line` signature/module placement, and the AC5 enforcement mechanism are
all design-phase choices with defensible defaults recorded above.
