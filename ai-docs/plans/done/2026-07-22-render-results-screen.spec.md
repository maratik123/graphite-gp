# gp-render: Results screen — final standings, fastest lap, crashes

**Source:** issue #22
**Date:** 2026-07-22
**Tracked in:** #22

## Scope

Add a `ResultsScreen` to `gp-render` — a port of `Screens.jsx`'s `ResultsScreen`
(design-system reference at `docs/design-system/ui_kits/game/Screens.jsx:205-235`).
It is the post-race summary shown when a race finishes.

Draw-only and caller-supplies-data, exactly like the existing
`SetupScreen`/`RaceScreen`/`LabScreen` (`crates/render/src/screens/`): a builder
struct constructed from caller-supplied outcome data, with a `show(ui) ->
ResultsResponse` method that draws the layout and returns the player's chosen
navigation intent. It performs **no** race simulation or ranking — the caller
(future `gp-game` orchestration) supplies already-ranked standings and the
summary metrics.

In scope:

1. A new `screens::results` module holding a `ResultsScreen` builder and its
   `ResultsResponse`, wired into `screens/mod.rs` (`pub mod results;` +
   re-exports) mirroring `race`/`setup`/`lab`.
2. **Header** — a mono `RACE COMPLETE` eyebrow and a display-face
   "You finished P<n>" title, where the player's finishing position is
   data-driven from the caller-supplied outcome (JSX hardcodes `P1`; the port
   binds to real data).
3. **Final standings `Card`** (`Card` title `"Final standings"`, grid variant)
   listing **one `CarChip` per car in rank order**, each paired with the car's
   finish time on a mono right-aligned label. `CarChip` carries the car's
   `rank`, `color`, `name`, and `You`/`Ai` `kind` (`CarKind`), matching how
   `race.rs::draw_standings` builds chips.
4. **Summary Telemetry row** inside the standings `Card`, below a hairline
   divider: three `Telemetry` tiles bound to the outcome — `Fastest lap`
   (accent tone, `s` unit), `Tempo` (default tone, unitless), `Crashes`
   (danger tone). Values come from the caller-supplied outcome data.
5. **Action row** — a primary "Race again" `Button` (with the `rotate-ccw`
   leading icon) and a secondary "Menu" `Button`. Each click is surfaced as a
   distinct navigation intent on `ResultsResponse`.
6. A caller-facing input type carrying the race outcome (per-car standings
   entries in rank order + the summary metrics + the player's finishing
   position). Following `RaceConfig`'s precedent (`screens/mod.rs`), this type
   lives in `gp-render` (draw-only, no `gp-gen`/`gp-ai` dependency) — its exact
   name/shape/field split is a design-phase decision.
7. Unit tests + a golden gallery test (`results_gallery.rs`, `#[cfg(test)]`),
   mirroring `race_gallery.rs`.

## Out of scope

- Any race simulation, ranking, timing, or lap/crash counting — the caller
  supplies finished outcome data (`gp-core`/`gp-game` orchestration owns this,
  consistent with `race.rs` never calling `sim::step`).
- Screen-to-screen navigation wiring / an app-level screen state machine — this
  screen only *emits* the intent; `gp-game` routes it (a later block).
- New widgets — `Button`, `Card`, `CarChip`, `Telemetry` all already exist
  (deps #13 and #15 are both merged/closed).
- Animations / transitions on the results screen.

## Deferred

- (none) — the screen is self-contained given existing widgets.

## Key decisions

| Question | Decision |
|---|---|
| Screen contract | Draw-only builder + `show(ui) -> ResultsResponse`, matching `RaceScreen`/`SetupScreen`/`LabScreen`. Caller supplies all data; screen holds it by value/reference like the siblings. |
| Where the outcome type lives | In `gp-render` (`screens` module), following `RaceConfig`'s precedent in `screens/mod.rs` — single definition, single consumer, no shared crate. `gp-render` stays free of `gp-gen`/`gp-ai` deps. |
| Standings ordering | Caller supplies entries **already sorted in rank order**; the screen renders them in slice order and does not sort. Consistent with the draw-only, caller-supplies-data invariant. |
| Player finishing position | Data-driven from the outcome (the `You`-kind entry's rank, or an explicit field — design's call), not hardcoded `P1` as in the JSX. |
| Navigation intents | `ResultsResponse` exposes the "Race again" and "Menu" clicks as two distinct signals (bool flags and/or the button `Response`s), mirroring `RaceResponse::finish`. |
| Car names/colors | Reuse the existing `CAR_NAMES` table (`screens::race::CAR_NAMES`) and the car color ramp already used by `CarChip`/`race.rs`, rather than duplicating a new table. Actual reuse-vs-parameter split is design's call. |
| Summary metric shape (`Fastest lap`/`Tempo`/`Crashes`) | Values are caller-supplied and fed to `Telemetry` with the tones/units the JSX specifies (accent+`s`, default, danger). Numeric-vs-preformatted-string representation of each value is a design-phase decision (see how `Telemetry` consumes its `value`). |

## Technical constraints

- **`gp-render` is draw-only**: no dependency on `gp-gen`/`gp-ai`; the outcome
  input type is defined locally (`ai-docs/key-decisions.md`, and the
  `screens/mod.rs` module doc).
- **Font precondition**: `show`/paint must document (and uphold) the same
  "install `crate::fonts::definitions` first" precondition as every other
  screen/widget, and panic at layout time otherwise (consistent with siblings).
- **Miri gate (mandatory)**: any unit/gallery test that constructs an
  `egui::Context` or drives a painter (directly or via a shared capture helper)
  MUST carry `#[cfg_attr(miri, ignore = "<why>")]`, per CLAUDE.md § *Rust Test
  Conventions* (gp-render Context/painter Miri gate). Golden/kittest tests carry
  it too.
- **Const-fn / clippy posture**: follow the workspace's strict clippy
  (`-D warnings`, nursery `missing_const_for_fn`) — builder setters are `const
  fn` where the siblings' are.
- **Magic numbers**: layout literals ported from the JSX become module-level
  `const`s with a source-line comment, exactly as `race.rs` does
  (`PAD_OUTER`, `COL_GAP`, …).
- **File-size budget**: keep the new module within the soft 500 / hard 1000
  (excl. tests) budget; split the gallery test into its own `#[cfg(test)]`
  module file (`results_gallery.rs`) like `race_gallery.rs`.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `screens::results::ResultsScreen` exists as a draw-only, caller-supplies-data builder with a `show(ui) -> ResultsResponse` method, wired into `screens/mod.rs` with the appropriate `pub mod` + re-exports. |
| AC2 | The header renders a `RACE COMPLETE` eyebrow and a "You finished P<n>" title whose position `<n>` is bound to the caller-supplied outcome (not a hardcoded constant). |
| AC3 | The Final-standings `Card` renders exactly **one `CarChip` per car, in the caller-supplied rank order**, each carrying that car's rank, color, name, and `You`/`Ai` kind, paired with its finish time. A unit test asserts the chip count equals the car count and that ranks appear in ascending order. |
| AC4 | The summary row renders three `Telemetry` tiles — `Fastest lap` (accent, `s`), `Tempo` (default), `Crashes` (danger) — with values bound to the outcome data. A unit test asserts the tiles bind to the outcome (labels present and values reflect the supplied data). |
| AC5 | "Race again" and "Menu" each emit a **distinct, correct** navigation signal on `ResultsResponse` when clicked, and no signal when not clicked. A unit/interaction test asserts each button drives only its own intent. |
| AC6 | A golden gallery test (`results_gallery.rs`) renders `ResultsScreen` against the design-system `ResultsScreen` reference and passes exact-compare (per the project's golden conventions), and is Miri-gated. |
| AC7 | `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass; every public item has a `///` doc; the workspace Miri job stays green. |

## Open questions

- **Exact shape of the outcome input type** (one flat struct vs. a
  per-car-entry struct + a summary struct; numeric vs. preformatted values;
  reuse of `CAR_NAMES`/color ramp vs. per-entry parameters). A defensible
  default exists (a per-car `StandingEntry` slice + a summary struct, numeric
  values formatted at draw time), so this is left to the `design` Subagent,
  which may choose otherwise via the normal design flow.
