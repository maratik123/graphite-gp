# gp-render: app shell — top bar + screen router

**Source:** issue #23
**Date:** 2026-07-22
**Tracked in:** #23

## Scope

1. A top-level **app shell** in `gp-render` that composes the existing four screens (`SetupScreen`, `LabScreen`, `RaceScreen`, `ResultsScreen`) into one navigable flow, ported from `docs/design-system/ui_kits/game/App.jsx`.
2. A **screen router** — a small state machine that owns the current screen and dispatches transitions when a screen reports a navigation intent through its existing `*Response` value:
   - `SetupScreen` → `SetupResponse.generated` ⇒ **Lab**
   - `LabScreen` → `LabResponse.test_lap` ⇒ **Race**; `LabResponse.menu` ⇒ **Menu**; `LabResponse.regenerate` ⇒ stay on **Lab** (track regeneration is the caller's job — draw-only crate)
   - `RaceScreen` → `RaceResponse.finish` ⇒ **Results**
   - `ResultsScreen` → `ResultsResponse.again` ⇒ **Race**; `ResultsResponse.menu` ⇒ **Menu**
3. **Shared config state** the router owns and carries across transitions: the `RaceConfig` (produced/updated by `SetupResponse.config`) and the Race-screen `Overlays` (updated by `RaceResponse.overlays`). These persist as the current screen changes.
4. A **top bar**: the `GRAPHITE GP` wordmark (accent-dot + display face, accent on `GP`) and the **interactive** navigation row (`New race` / `Race` / `Track lab`, per the mock's three `NavItem`s), with the current screen visually indicated. Each nav item is **clickable to jump directly** to its screen (mock `onClick={() => setScreen(id)}` behavior) — the router state machine accepts these arbitrary transitions, subject to the guard below.
5. The router consumes the **consolidated per-screen input structs** landed in #111 (`LabInput` / `RaceInput { scene, … }` / `ResultsInput`, and `SetupScreen::new(RaceConfig)`). Externally-sourced session data (the generated `TrackArtifact`, `CarRender`s, `StandingEntry`s, `RaceSummary`) is **borrowed per frame** from the caller — the router does not (and cannot) own it, because `gp-render` is draw-only and has no dependency on `gp-gen`/`gp-ai`/`gp-core::sim`.
6. **Wire the `gp-game` binary** (`crates/game/src/main.rs`) to instantiate and drive the shell, **replacing `draw_placeholder`**, so `cargo run -p gp-game` shows the navigable click-through. The binary feeds the shell **placeholder/fixture session data** (the JS-mock approach — physics/AI faked): a **hand-built** fixture track constructed inline in `gp-game` from `gp_core::geom::{Corridor, walls_from_boundary}` plus a hand-populated `TrackMetrics` (speed_heatmap / fastest_lap over the ring cells) — mirroring the render-safe pattern that `crates/render/src/track/mod.rs`'s own `fixture_track` test uses — **not** a real generator call (`gp_gen`'s generator is a `todo!()` stub — `crates/gen/src/lib.rs:52-54` — that would panic at startup, breaking AC8 and the zero-production-panics constraint). Alongside the track, inline fixture `CarRender`s / `StandingEntry`s / `RaceSummary` / `PhaseStatus`es are constructed in `gp-game` (the crate's own non-`#[cfg(test)]` fixtures — `gp-render`'s gallery fixtures are `#[cfg(test)]` and not linkable from the binary).
7. **Remove the vestigial `draw_placeholder` scaffold.** Once the shell is wired (item 6), the scaffold `draw_placeholder` production function (`crates/render/src/placeholder.rs`) has **zero production callers** — it is exercised only by its own `#[cfg(test)]` tests. Remove `draw_placeholder`, the `placeholder`/`golden_guard` wgpu golden test, and its `crates/render/tests/snapshots/placeholder.png` snapshot (the placeholder art is never shown to a user now). **PRESERVE the `tessellation_smoke` canary** — it verifies the full text/glyph tessellation path yields non-empty, non-zero-geometry meshes (one of the two `gp-render` Miri gates AGENTS.md documents) and currently uses `draw_placeholder` as its draw payload; **repoint it onto a real draw path** (e.g. `AppShell::show` with a minimal fixture `ShellSession`, or a screen's draw) so the canary survives without depending on the removed placeholder art. The canary keeps its existing `#[cfg_attr(miri, ignore)]` gate (drawing text still aborts Miri). If `placeholder.rs` has nothing meaningful left after removal, the module (and its `pub mod placeholder;` in `lib.rs`) may be deleted and the canary relocated; otherwise trim the module and any now-unused helpers (`pixel_at`, `geometry`, `CANVAS_RECT`, etc.) to what remains used. The exact mechanics (delete-vs-trim, canary relocation, real-draw fixture choice) are the design phase's call.

## Out of scope

- Track generation, physics stepping, AI moves, ranking, timing, crash counting — all produced outside `gp-render`. The router only routes and draws.
- Persisting UI state that already lives inside a single screen and is re-derived each frame (e.g. an in-progress slider drag) beyond the `RaceConfig`/`Overlays` the screens already surface through their `*Response`.
- The `Settings` icon-button and the `{cars} cars · {laps} laps` status readout on the right of the mock's top bar are **display-only chrome**; wire them in only if trivial — no settings screen/behavior is in scope.

## Deferred

- Full block-3b orchestration (real generation → sim → AI loop feeding live session data into the shell) | that is block 3b's own work; this task's `gp-game` wiring uses faked placeholder/fixture session data, not the real loop | tracked by the block-3b issues, not a new issue here.
- Input handling, timing/clock, and real player/AI control in `gp-game` | block 3b (the binary here only routes screens + draws fixtures) | block-3b issues.

## Key decisions

| Question | Decision |
|---|---|
| Is there a distinct "Menu" screen? | **No.** "Menu" is the **Setup** screen (`New race`), matching `App.jsx` (`onMenu → setCfg…setScreen('setup')`). `Lab.menu` / `Results.menu` both route to Setup. The four issue-dependency screens are the only screens. |
| What state does the draw-only router own vs borrow? | **Owns** (small, `gp-render`-local, `Copy`): the current `Screen` enum + `RaceConfig` + `Overlays`. **Borrows per frame** (`&'a`): `TrackArtifact`, `CarRender`s, `StandingEntry`s, `RaceSummary`, `PhaseStatus`es, seed, lap counters — everything sourced from `gp-gen`/`gp-ai`/`gp-core`. The router's per-frame draw call receives the borrowed session data alongside its owned state. |
| Which screens appear in the top-bar nav? | The mock's three: `New race` (Setup), `Race`, `Track lab`. `Results` is **not** a nav item (reached only via `Race.finish`). |
| Are nav items interactive? | **Yes — clickable jumps** to any of the three screens (mock behavior). The router accepts arbitrary nav transitions, not just the linear flow. |
| Guarding jumps before a track exists | The `New race` (Setup) nav item is **always enabled**. The `Race` and `Track lab` nav items are **disabled** (non-clickable, styled inactive) until a track has been generated at least once — i.e. until the first `SetupResponse.generated`. Clicking a disabled item is a no-op; this is the "safe fallback" (never route to Lab/Race with no track/session). The router tracks a `has_generated` latch flipped `true` on the first `generated` intent. (Staleness of an already-generated track after a config change without regeneration is the caller's concern — draw-only crate. The exact mechanism — owned latch vs a caller-supplied per-frame availability flag — is a design-phase choice; the *policy* here is fixed.) |
| `gp-game` binary wiring | In scope. `main.rs` replaces `draw_placeholder` with the shell, driven by placeholder/fixture session data (a **hand-built** fixture track — `gp_core::geom::{Corridor, walls_from_boundary}` + hand-populated `TrackMetrics`, per `crates/render/src/track/mod.rs`'s `fixture_track` test — **not** the real generator, whose `todo!()` stub at `crates/gen/src/lib.rs:52-54` would panic at startup; plus inline faked cars/standings/summary/phases). `cargo run -p gp-game` shows the click-through. |
| `Screen` enum variants | `Setup`, `Lab`, `Race`, `Results` (the mock's `setup | race | lab | results`). |
| Config seed at startup | The mock's defaults: `cars: 4, laps: 5, difficulty: Pro, v_target: 7` — the shell opens on **Setup** with this `RaceConfig`. |
| `Race again` (`Results.again`) target | Returns to **Race** (per mock `onAgain → race`), reusing the current `RaceConfig`; the caller re-supplies session data. |

## Technical constraints

- **Draw-only `gp-render`.** No new dependency; no change to `gp-core`/`gp-gen`/`gp-ai`. The shell/router lives in `gp-render`; the binary wiring lives in `crates/game/**`. `gp-game` already depends on `gp-gen`/`gp-core`/`gp-ai`/`gp-render`, so no `Cargo.toml` dependency change is expected. Zero production panics maintained (in both crates).
- The router's owned state must be the minimal set the screens actually mutate through their `*Response` (`RaceConfig`, `Overlays`) plus the `Screen` cursor — do not shadow-store borrowed session data.
- Follow the established screen module conventions: builder `::new` FORCED `const fn` where the nursery lint demands it; `///` on every public item; broken intra-doc links denied.
- Any unit/interaction test that constructs an `egui::Context` or drives a painter (incl. an `egui_kittest` click-through) **must** carry `#[cfg_attr(miri, ignore = "<why>")]` per AGENTS.md § *Rust Test Conventions* (gp-render Context/painter Miri gate); any wgpu golden must be `#[cfg_attr(miri, ignore)]`d and `image-check`-verified at mint.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The router transitions **Setup → Lab → Race → Results** on the corresponding navigation intents (`generated` / `test_lap` / `finish`), and **Results/Lab → Menu (Setup)** on `menu`. Asserted as a state-machine unit test that feeds each intent and checks the resulting `Screen`. |
| AC2 | Shared config state persists across transitions: a `RaceConfig` updated on Setup is unchanged when the router is later on Lab/Race/Results; `Overlays` updated on Race persists while the router stays in the race sub-flow. Asserted directly on the router's owned state. |
| AC3 | The top bar renders the `GRAPHITE GP` wordmark and the three-item navigation row, with the active screen indicated. Asserted via a wgpu golden of the shell (`#[cfg_attr(miri, ignore)]`, `image-check`-verified). |
| AC4 | An `egui_kittest` **click-through smoke test** drives the full loop Setup→Lab→Race→Results→Menu by activating each screen's navigation control and asserts the router lands on the expected screen at each step (`#[cfg_attr(miri, ignore)]`). |
| AC5 | `regenerate` on Lab does **not** change the current screen (stays on Lab). |
| AC6 | Nav items are interactive: clicking `New race` / `Race` / `Track lab` jumps the router directly to that screen. Asserted as a state-machine unit test (feed each nav-jump intent from an arbitrary current screen, check the resulting `Screen`). |
| AC7 | Nav guard: before the first `generated` intent, the `Race` and `Track lab` nav items are disabled and clicking them is a no-op (router stays put); `New race` is always enabled. After the first `generated`, all three are enabled. Asserted on the router's `has_generated` state + the disabled no-op behavior. |
| AC8 | `cargo run -p gp-game` launches the shell (not the placeholder): `main.rs` instantiates the router with placeholder/fixture session data and drives the click-through. Asserted by the binary compiling with the shell wired (the `draw_placeholder` call is gone) and `cargo build -p gp-game` green; the flow itself is covered by AC4's `gp-render` click-through smoke test. |
| AC9 | The `draw_placeholder` scaffold is removed: the `draw_placeholder` production fn, the `placeholder`/`golden_guard` wgpu golden test, and `crates/render/tests/snapshots/placeholder.png` no longer exist, and `main.rs` references none of them. The `tessellation_smoke` canary still exists and passes, now driving a **real** draw path (no dependency on the removed placeholder art); it retains its `#[cfg_attr(miri, ignore)]` gate (un-Miri behavior unchanged — drawing text still aborts Miri). Asserted by `cargo test -p gp-render tessellation_smoke` green and `rg 'draw_placeholder' crates/` returning no hits — i.e. the fn, its module, its golden test, and every call site are gone from **live production code** (`crates/**`). The verification is scoped to `crates/` only: frozen historical records under `ai-docs/plans/done/**` and the APPEND-ONLY `ai-docs/learnings.md` legitimately reference `draw_placeholder` as history and MUST NOT be rewritten. Narrative docs (README, `ai-docs/context.md`) may be updated to past-tense but are not part of this AC's pass/fail grep. |

## Open questions

None — both round-1 questions resolved (nav = clickable jumps with a generate-gated guard on Race/Lab; binary wiring in scope with faked placeholder session data).
