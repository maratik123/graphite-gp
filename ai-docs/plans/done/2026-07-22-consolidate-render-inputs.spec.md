# Consolidate gp-render frame-immutable inputs into cohesive input struct(s)

**Source:** issue #111
**Date:** 2026-07-22
**Tracked in:** #111

## Scope

A **pure plumbing refactor** of `gp-render` with **no behavior change**: group the
caller-supplied, **frame-immutable** (read-only per-frame) values currently passed
as loose positional parameters to `render_frame` and to the screen builders'
`new()` constructors into cohesive **input struct(s)**, so the future app
shell / screen router (#23) has a clean, stable interface to construct and pass.

Current reality (verified in-tree, 2026-07-22 — the issue's future-tense framing
is stale; all three data screens have landed):

- `render_frame(painter, rect, track, cars, reduced_motion, overlays)`
  (`crates/render/src/lib.rs:68`) — `track` / `cars` / `reduced_motion` /
  `overlays` are the frame-immutable scene inputs; `painter` / `rect` are the draw
  target. Called internally by `RaceScreen`/`LabScreen` (`screens/race.rs:378`,
  `screens/lab.rs:329`) and by the golden/unit harnesses
  (`track/golden.rs`, `track/mod.rs`).
- `LabScreen::new(track, phases, valid, seed)` + optional icon setters
  (`screens/lab.rs:170`).
- `RaceScreen::new(track, cars, active, overlays, laps_done, total_laps)` +
  optional `reduced_motion` setter (`screens/race.rs:151`).
- `ResultsScreen::new(standings, summary)` + optional `again_icon` setter
  (`screens/results.rs:190`).

In scope:
1. Introduce cohesive input struct(s) bundling the frame-immutable inputs above.
2. Rewrite `render_frame` and the `LabScreen`/`RaceScreen`/`ResultsScreen`
   builders to accept the new struct(s); update every in-tree call site
   (the screens' internal `render_frame` calls, plus the golden/unit-test
   harnesses).
3. Preserve the builder pattern for **optional** inputs (icon handles,
   `reduced_motion`) unless design decides otherwise — the consolidation target
   is the required positional argument lists.

## Out of scope

- Any rendering-behavior change — this is a pure refactor; wgpu goldens must stay
  **byte-identical**.
- `gp-core` / `gp-gen` / `gp-ai` changes — the bundled types' internals
  (`TrackArtifact`, `CarRender`, `StandingEntry`, `RaceSummary`, `Overlays`,
  `PhaseStatus`) are untouched; only the *grouping* around them changes.
- Adding any new crate dependency — `gp-render` keeps its draw-only,
  no-`gp-gen`/`gp-ai` contract.
- The #23 app shell / screen router itself (this refactor is its pre-req).
- `SetupScreen` — it already takes a single cohesive `RaceConfig` struct, not a
  loose positional list (see Key decisions / Open questions).

## Deferred

- App shell / screen router wiring the consolidated inputs together | separate
  issue already exists | #23.

## Key decisions

| Question | Decision |
|---|---|
| Bundling shape: one shared "scene/frame input" struct vs. per-screen input structs? | **Deferred to design time** — the issue author explicitly punts this to `/task` design (AC4). Both are defensible; design picks and justifies. |
| Do optional inputs (icon handles, `reduced_motion`) move into the input struct? | Default: **keep as builder setters**; only the required positional args are the consolidation target. Design may fold them in if it produces a cleaner router interface. |
| Is `SetupScreen` in scope? | Default: **no** — it already takes one `RaceConfig` struct, so it is not a "many loose params" surface. Design may include it for symmetry via a Design Amendment. |
| New struct naming / module placement | Design detail — left to the `design` Subagent. |

## Technical constraints

- `gp-render` is **draw-only** and integer-deterministic-core-agnostic; it must
  add no new crate dependency and make no `gp-core`/`gp-gen`/`gp-ai` change.
- `render_frame` must stay a **pure function** of `(rect, track, cars, overlays,
  reduced_motion)` — the drawn output may not depend on hidden state (the
  precedent documented at `lib.rs:52`).
- Every `gp-render` builder is `Copy` today (except `SetupResponse`); any new
  input struct that holds only borrows/scalars should preserve `Copy` where the
  existing builder relied on it, unless design justifies otherwise.
- The two Miri-gate mechanical triggers in AGENTS.md § *Rust Test Conventions*
  (Context/painter tests; token/helper parity tests) still apply to any test the
  refactor touches or adds.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `render_frame` and each of `LabScreen`/`RaceScreen`/`ResultsScreen` accept a small number of cohesive input struct(s) in place of their current multi-argument required-positional lists. |
| AC2 | All existing `gp-render` unit tests and wgpu goldens pass **byte-identical** — no visual change (`cargo test -p gp-render`, golden compare with threshold 0 / failed-pixel-count 0). |
| AC3 | `gp-render` stays draw-only: no new crate dependency (`git diff` on `Cargo.toml`/`Cargo.lock` shows no added edge) and no `gp-core`/`gp-gen`/`gp-ai` change. |
| AC4 | Every in-tree call site of the changed surfaces (screens' internal `render_frame` calls, golden/unit-test harnesses) is updated and the workspace builds clean (`cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`). |
| AC5 | Public docs updated: each new input struct and changed signature carries a `///` doc line; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` passes. |

## Open questions

- **`SetupScreen` inclusion** — default is out of scope (already `RaceConfig`-shaped).
  The product owner may want it folded into the same consolidation pattern for
  router symmetry; design can revisit without blocking this spec.
- **Shared vs per-screen struct shape** — the central design decision, explicitly
  deferred to `/task` design (AC4 of the issue). Not blocking spec readiness.
