# gp-render: Track lab screen (LabScreen)

**Source:** issue #20
**Date:** 2026-07-21
**Tracked in:** #20

## Scope

Port `Screens.jsx`'s `LabScreen` into `gp-render` as the fourth full screen
(after `SetupScreen`, #19), following the crate's established **draw-only,
caller-supplies-data** contract. A two-column layout:

1. **Left column**
   - **Header row:** "Track lab" display title, a `Badge` (VALID / tone from a
     caller-supplied validity flag), a selected `Tag` showing `seed <N>`, and a
     right-aligned ghost `Button` "← Menu".
   - **Track canvas:** a bordered region filling the remaining height, drawn via
     the existing [`crate::render_frame`] with `Overlays { speed_heatmap: true,
     fastest_lap: true, grid: true }` (heatmap + fastest-lap overlays ON — AC1).
   - **Action row:** primary `Button` "Regenerate" (shuffle icon) + secondary
     `Button` "Test lap" (play icon).
2. **Right column (fixed ~320px)**
   - **Oracle report `Card`** (eyebrow "Passability + metrics"): a 2×2 grid of
     `Telemetry` tiles — Vmax (`c/t`), Tempo (accent tone), Width min (`pts`),
     S/F width (`pts`) (AC2).
   - **Generation phases `Card`** (eyebrow "Ф1 – Ф7"): a 7-row list, each row =
     mono phase id (Ф1–Ф7) + UI-font phase name + a status `Badge`
     (`ok` → "✓" / `warn`|repair → "repair") (AC3).

The screen exposes a `show(ui) -> LabResponse` builder mirroring
[`crate::screens::setup::SetupScreen`]: it draws the caller-supplied data and
returns which action buttons (Regenerate / Test lap / Menu) were clicked this
frame. It does **not** call the generator, own an RNG, or buffer state.

All screen data is caller-supplied per-frame, exactly like `SetupScreen`'s
`RaceConfig`. The oracle report is **not** a gp-render-local struct: the screen
already takes a `gp_core::track::TrackArtifact` (for the canvas — AC1/AC6), and
it sources **all four** oracle-report values straight off that artifact:

| Tile | Source on `TrackArtifact` | Type → display |
|---|---|---|
| Vmax | `track.metrics.vmax_attain` | `Option<i32>` → "—" when `None` |
| Tempo | `track.metrics.tempo` | `Option<f32>` → "—" when `None` |
| Width min | `track.width_min` (new field, see Key decisions) | `u32`, always present → a real number, never "—" |
| S/F width | the new S/F-width getter (see Key decisions) | `usize`, always present → a real number, never "—" |

The phase list is a caller-supplied set of gp-render-local phase-status values;
the `valid` flag and `seed` for the header are caller-supplied; the canvas draws
the caller-supplied `TrackArtifact` fixture.

## Out of scope

- Implementing the Ф1–Ф7 generation pipeline. `gp_gen::generate` is a `todo!`
  stub (`crates/gen/src/lib.rs:52`, `_params` unused) — this task does **not**
  implement it. "Regenerate re-runs generation" is satisfied by emitting a
  regenerate-request signal for the caller to act on (see Key decisions),
  matching how `SetupScreen`'s "Generate track" emits a `RaceConfig` rather than
  calling generation.
- Implementing Ф4 static validation (width computation) or the `generate()`
  assembly pipeline. This task adds the `TrackArtifact.width_min` **field** and
  hand-populates it in fixtures; it does **not** compute it. Fixtures set a
  concrete `u32` width (just as they already hand-populate `metrics`) until #27
  computes the real value and #34 stores it at assembly.
- Any `gp-game` wiring of the window/event loop, real generation, or navigation
  between screens (the screen only emits click signals).
- The `RaceScreen` and `ResultsScreen` ports (separate issues).

## Deferred

- Wiring Regenerate to a live `gp_gen::generate` call | the generator is a
  `todo!` stub; the screen emits a request signal now, real generation lands
  when Block 1 is implemented | tracked by the existing Block-1 generation issue
  (no new issue needed).
- Real per-phase ok/repair status from the pipeline | requires the pipeline to
  report phase outcomes | same Block-1 dependency; the screen renders
  caller-supplied statuses until then.
- Populating `TrackArtifact.width_min` with a real Ф4 static-validation value |
  Ф4 width computation (DT + cross-sections) is not built yet; the field is
  hand-populated in fixtures until then | tracked by **#27** ("gp-gen Ф4: static
  validation — … width (DT + cross-sections) …") computing it and **#34**
  ("gp-gen: generate() top-level pipeline — … artifact assembly") storing it at
  assembly. Once those land, the screen shows the real value with **zero**
  screen-side change (the tile already reads `track.width_min`).

## Key decisions

| Question | Decision |
|---|---|
| Regenerate / Test lap behavior — draw-only signal vs. real generation | **Draw-only signal** (round-1 answer, unchanged). `show` returns a `LabResponse` with `regenerate: bool` / `test_lap: bool` / `menu: bool` click flags (mirrors `SetupResponse.generated`); the caller owns seed selection + the future `gp_gen::generate` call. The screen never calls the generator, owns no RNG, buffers no state. |
| Source of the four oracle-report values (Vmax / Tempo / Width min / S/F width) | **All four sourced off the caller-supplied `TrackArtifact`** (round-2 amendment — **reverses** the round-1 "gp-render-local report struct / zero gp-core change" decision). The screen already holds a `TrackArtifact` for the canvas, so: **Vmax** = `track.metrics.vmax_attain`, **Tempo** = `track.metrics.tempo` (both already on `TrackMetrics`); **Width min** = a new `TrackArtifact.width_min` field (see next row); **S/F width** = a new getter (see row below). There is **no** gp-render-local report struct. Rationale: 3 of the 4 values already live on the `TrackArtifact` the screen holds; adding `width_min` + the getter makes propagation **automatic** — once Block-1 generation fills `track.width_min`, the screen shows the real value with zero screen-side change and no call-site value copying. |
| Where **Width min** lives | **New `gp_core::track::TrackArtifact` field** `width_min: u32` — exactly one new field, **non-`Option`**. It is a Ф4 static-validation geometry output (`docs/design.md` §2; computed by #27 "width (DT + cross-sections)", stored at assembly by #34). **Not modeled as `Option`:** a `TrackArtifact` is a fully-validated, exported track (§2, Ф7) — by the time one exists, Ф4 has measured its width, so `width_min` is **always** defined and `≥ n = ⌈m/2⌉ ≥ 1`; there is no genuine "absent" state. **`u32` (unsigned):** `width_min` is a **count** of lattice points across a cross-section → inherently non-negative; a signed `i32` would admit impossible negatives. `u32` matches the width-floor domain — `GenParams::min_width()` and `GenParams::start_finish_width()` both return `u32` (`crates/gen/src/lib.rs:29,34`), and `width_min` is exactly the value Ф4 validates as `≥ n`. Hand-populated with a concrete `u32` in fixtures (exactly like `TrackArtifact.metrics` already is in `golden.rs::scene_track_with_metrics`) until Block-1 generation is real. Always renders a real number — **no** "—" case. `TrackArtifact` is `width_min`'s honest home: it is Ф4/§2 **geometry**, **not** a §3 `TrackMetrics` speed metric — do **not** add it to `TrackMetrics`. |
| Where **S/F width** lives | **A getter, not a data field.** S/F width is already derivable as `TrackArtifact.sf.chord.len()` (the chord is the `Vec<Point>` across the corridor), so do **not** add an `sf_width` field. Add an accessor so call sites use a named getter instead of reaching into `.sf.chord.len()`. Natural home: `impl StartFinish { pub fn width(&self) -> usize { self.chord.len() } }` (S/F width is a property of the `StartFinish`); a `TrackArtifact`-level convenience getter is also acceptable — the **design subagent** picks the exact placement/signature, but a named getter MUST exist, carry a one-line `///` rustdoc, and have a gp-core unit test, and the screen + tests MUST use it (never raw `.sf.chord.len()`). |
| Where the Ф1–Ф7 phase-status type lives | **gp-render-local**, mirroring `RaceConfig`/`Difficulty` in `crates/render/src/screens/mod.rs` — `gp-render` is draw-only with no `gp-gen` dependency (`screens/mod.rs` rationale). A local phase-status enum (`Ok`/`Repair`) + the 7 phases, caller-supplied. Design subagent picks the exact shape (enum vs `[PhaseStatus; 7]` slice vs a `Phase` id+name+status row). NOT placed in gp-core/gp-gen. |
| How oracle values + phase statuses + validity + seed reach the screen | Caller-supplied per-frame inputs (draw-only contract, `ai-docs/key-decisions.md`), exactly like `CarRender`/`RaceConfig`. Vmax/Tempo are `Option`s off `track.metrics` (→ "—" placeholder when `None`); Width min (`u32`) and S/F width (getter, `usize`) are always-present and always render a real number; the phase statuses / validity flag / seed are separate caller-supplied inputs. |
| Phase-status Badge tone mapping | `PhaseStatus::Ok → Badge tone Ok` ("✓"); `Repair → Badge tone Warn` ("repair"), reusing the existing `widgets::badge::Tone` (`Ok`/`Warn` already exist). |
| Integration smoke test artifact | Use a hand-built `TrackArtifact` fixture with hand-populated `TrackMetrics` **and** hand-populated `width_min`, the pattern `crates/render/src/track/golden.rs:scene_track_with_metrics` already uses — a "real generated artifact" is impossible while `generate` is a `todo!` stub. |

## Technical constraints

- `gp-render` is **draw-only** and has **no** dependency on `gp-gen`/`gp-ai`
  (`crates/render/src/screens/mod.rs`, `ai-docs/key-decisions.md`). The
  phase-status type (single consumer) lives in `gp-render` itself, not a shared
  crate. The oracle values are read off `gp_core::track::TrackArtifact` (already
  a `gp-render` dependency for the canvas), not a gp-render-local struct.
- **gp-core change (this task's only gp-core edit):** (a) add exactly one field
  `width_min: u32` to `gp_core::track::TrackArtifact`, with a one-line
  `///`; (b) add the S/F-width getter (`StartFinish::width` or a `TrackArtifact`
  convenience getter — design's choice) with a one-line `///` and a gp-core unit
  test. No `gp-gen`/`gp-ai` involvement; no change to `TrackMetrics`.
- Reuse existing building blocks — do not reinvent: [`crate::render_frame`] +
  [`crate::Overlays`] (canvas), `widgets::{Button, Badge, Tag, Card, Telemetry}`,
  `tokens::{color, spacing, typography}`, `icons` (shuffle/play/menu).
- The screen must install into an `Order::Middle` layer before drawing `Card`
  chrome, as `SetupScreen::show` documents (Card fill paints on
  `LayerId::background()` and would otherwise cover its own contents).
- Draw code that stands up an `egui::Context` / drives a painter (gallery/golden
  tests) carries `#[cfg_attr(miri, ignore = "…")]` per AGENTS.md § *Rust Test
  Conventions* (Context/painter Miri gate). The new gp-core getter unit test is
  pure integer/`Vec::len` logic — no Context, no Miri gate.
- Fonts must be installed by the caller before layout (`fonts::definitions`);
  document the layout-time panic as siblings do.
- Numeric literals with semantic meaning → module-level consts (spacing/sizes
  from `Screens.jsx`: `320` right column, `20`/`14`/`12`/`16`/`9`/`18` gaps).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The screen renders the track canvas via `render_frame` with `speed_heatmap` **and** `fastest_lap` overlays enabled; a test asserts both overlay flags are on in the `Overlays` passed. |
| AC2 | The oracle report renders four `Telemetry` tiles — Vmax, Tempo, Width min, S/F width — sourced from the caller-supplied `TrackArtifact`: **Vmax** from `track.metrics.vmax_attain`, **Tempo** from `track.metrics.tempo`, **Width min** from the new `track.width_min` field, **S/F width** from the new S/F-width getter. A test asserts each displayed tile string tracks its source on the artifact (Vmax/Tempo `Option` `None` → the "—" placeholder; Width min (`u32`) and S/F width always render a real number). There is **no** gp-render-local report struct. |
| AC3 | The generation-phases list renders exactly 7 rows (Ф1–Ф7), each with its phase id, name, and a status `Badge` sourced from a gp-render-local phase-status type; a test asserts 7 phase rows render and that an `Ok` status → `Ok`-tone badge and a `Repair` status → `Warn`-tone badge. |
| AC4 | Regenerate and Test lap are click **signals**, not generation calls: `show` returns a `LabResponse` whose `regenerate`/`test_lap` flags are `true` exactly on the frame each button is clicked (Menu likewise), and `false` otherwise. A test drives a click and asserts the flag flips through. The screen calls neither `gp_gen::generate` nor any RNG. |
| AC5 | The screen composes only existing crate building blocks (`render_frame`, `widgets::*`, `tokens::*`, `icons`) and adds **no** `gp-gen`/`gp-ai` dependency. The **only** gp-core change is (a) adding `width_min: u32` to `TrackArtifact` and (b) adding the S/F-width getter (with its unit test); `TrackMetrics` is unchanged. The screen keeps gp-render's zero-production-panics invariant, and any wgpu-golden / `egui::Context`-constructing / painter-driven test carries the `#[cfg_attr(miri, ignore = "…")]` gate (AGENTS.md § *Rust Test Conventions*). |
| AC6 | A gallery/golden test renders the whole `LabScreen` against a hand-built `TrackArtifact` fixture (the `track/golden.rs` pattern) as an integration smoke test; the fixture hand-populates `TrackMetrics` **and** `width_min` (alongside the existing hand-populated metrics), per the `golden.rs` pattern. |
| AC7 | The new S/F-width getter has a gp-core unit test asserting it returns the chord length (`chord.len()`) — including the empty-chord (`0`) case. |

## Open questions

Round-1 questions and the round-2 amendment are all resolved (see Key decisions):

- **Regenerate / Test lap semantics** → draw-only click signals; the screen never
  calls the generator. AC4 reframed accordingly, superseding the issue's "re-runs
  generation (new seed)" and "wire to a real generated artifact" wording.
- **Source of Width min & S/F width** → **round-2 amendment reverses** the
  round-1 "gp-render-local report struct / zero gp-core change" decision. All
  four oracle-report values are now sourced off the caller-supplied
  `TrackArtifact`: Vmax/Tempo from `track.metrics`, Width min from a new
  `TrackArtifact.width_min: u32` field, S/F width from a new getter over
  `sf.chord.len()`. This supersedes the issue's "from TrackMetrics" test-note
  wording and the round-1 local-struct decision.

None remaining — spec is ready for design.
</content>
</invoke>
