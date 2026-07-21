# gp-render: Race screen

**Source:** issue #21
**Date:** 2026-07-21
**Tracked in:** #21

## Scope

Build the hero `RaceScreen` in `gp-render` — a `Screens.jsx::RaceScreen` port that composes the already-built track canvas, HUD widgets, MovePad, overlay switches, and CarChips into the two-column race view. Following the crate's established screen pattern (`screens::setup::SetupScreen`, `screens::lab::LabScreen`): **draw-only, caller-supplies-data, emits per-frame responses** — `gp-render` buffers no game state and owns no clock (`ai-docs/key-decisions.md`, 2026-07-16). The screen calls no orchestration logic; it draws a frame and reports what the player did.

In scope:

1. **New module** `crates/render/src/screens/race.rs` with a `RaceScreen<'a>` builder + `show(self, ui) -> RaceResponse`, exported through `screens/mod.rs` and re-exported from `lib.rs` (mirrors `LabScreen`/`SetupScreen` wiring).
2. **Two-column layout** (`Screens.jsx:98` `gridTemplateColumns: '1fr 300px'`, `gap: 20`, `padding: 20`): left = HUD strip + overlay toolbar + bordered track canvas; right = "Your move" `Card` (MovePad + helper caption + Coast shortcut `Button`) and "Standings" `Card` (one `CarChip` per car).
3. **HUD strip** (dark `GRAPHITE_900` band, `radius-2`): `Telemetry` tiles bound to the **active car's** live `CarState` — `SPEED` (accent, `lg`) = `|v|`, `v` = `(vx, vy)`, `POS` = `(x, y)` — plus a right-aligned `LapMeter` (laps-done / total laps). On-ink rendering via `Telemetry.on_ink`.
4. **Overlay toolbar**: three `Switch`es (Grid / Heatmap / Fastest lap) that toggle the corresponding `crate::Overlays` flags, plus a right-aligned ghost "Finish →" `Button`. The screen takes the current `Overlays` and emits the toggled `Overlays`.
5. **Track canvas**: bordered (`1.5px GRAPHITE_900`, `radius-2`) region rendered via `crate::render_frame(painter, inner, track, cars, reduced_motion, overlays)` with the caller-supplied car slice and the live overlay flags.
6. **"Your move" `Card`**: a `MovePad` sized `52` showing the **active car's legal mask** (computed from the corridor + active state via `gp_core::sim::legal_mask`), the mono helper caption (`±1 per axis · no diagonal accel` / `supercover ⊆ D`), and a secondary full-width "Coast (·)" `Button` shortcut.
7. **"Standings" `Card`**: one `CarChip` per car (`m` cars), driver name + color + rank + `You`/`Ai` kind pill + active flag.
8. **Move emission**: `show` returns the `Action` selected this frame (MovePad cell click **or** the Coast shortcut) so the caller (`gp-game`) can drive `gp_core::sim::step`. The screen itself does not call `step`, `resolve_crash`, `resolve_collisions`, or `LapCounter` — those are `gp-game` orchestration (block 3b).
9. **Tests**: pure-core unit tests for the HUD/MovePad/overlay binding helpers; a `MovePad` mask-equals-`legal_mask` test; an overlay-toggle test; an `egui_kittest` interaction smoke test driving a scripted move selection; and a wgpu golden (`race_screen.png`) — all mirroring `lab_gallery.rs`/`setup_gallery.rs`.

## Out of scope

- Any window/event-loop or `gp-game` wiring (`main.rs` still draws the placeholder — screen selection/routing is separate future work).
- Turn orchestration: crash resolution, collision resolution, lap-counter bookkeeping, scrub-tick enforcement, AI opponent moves, turn-order sequencing. The screen renders one frame and emits one player choice; the caller owns the step/collision/lap pipeline.
- Computing car trails, move-animation `progress`, or the per-frame clock — these arrive pre-computed on the caller-supplied `CarRender` slice (`gp-render` is draw-only).
- Real race-position ranking for Standings (see Deferred).
- New shared crate or config type beyond what already exists (`RaceConfig` supplies `laps`/`cars`).

## Deferred

- Race-position-based Standings ordering + rank | The design doc defines no finishing-position/ranking metric yet; JSX renders cars in fixed index order with `rank = k+1`. Ship the index-order placeholder (matching JSX); real ranking is a later gameplay concern. | separate issue: **not yet** — revisit when `gp-game` turn orchestration lands.
- End-of-race / results transition wiring (`onFinish` → `ResultsScreen`) | Cross-screen routing is `gp-game`'s job and `ResultsScreen` is a separate build-order item. The screen only emits a `finish` click signal. | separate issue: no (covered by the routing/results items).

## Key decisions

| Question | Decision |
|---|---|
| State-ownership model | Draw-only, caller-supplies-data, emits per-frame responses — identical contract to `SetupScreen`/`LabScreen` (`ai-docs/key-decisions.md`). No internal game-state buffer. |
| Who advances the turn ("drives the core step") | The screen **emits** the selected `Action`; the caller calls `gp_core::sim::step`. The screen never calls `step`/`resolve_crash`/`resolve_collisions`/`LapCounter`. This keeps the single physics path in `gp-core` + `gp-game`, per the crate's draw-only charter. |
| MovePad legal mask source | The screen computes it internally as `gp_core::sim::legal_mask(&track.corridor, active_state)` for the active car (`gp-render` already depends on `gp-core::sim`). Directly satisfies the "mask matches core `legal_mask`" test. |
| Overlays state ownership | Caller supplies the current `crate::Overlays`; `show` returns the toggled `Overlays` (mirrors the `Switch`/`SetupScreen` value-in / value-out idiom). Initial default per JSX: `grid = true, heatmap = false, fastest_lap = false`. |
| SPEED value | The Euclidean magnitude of the car's `(vx, vy)`, formatted to 2 decimals. Computed as an `f32` in `gp-render`'s draw layer — this crate is `f32` throughout, and the integer-only rule is scoped to `gp-core` (`geom`/`sim`), which this display value does not touch (`Screens.jsx:80` `Math.hypot`). |
| POS value | `(x, y)` straight off `CarState` — cell coordinates already, unlike the JSX which divides pixel coords by `CELL`. No scaling. |
| `v` value | `(vx, vy)` off `CarState`, as `"(vx, vy)"`. |
| LapMeter binding | `LapMeter::new(laps_done, cfg.laps)` — `laps_done` is caller-supplied (the `LapCounter::laps()` value lives in `gp-game`). |
| LapMeter on-ink legibility (amendment, owner-approved during Step 7 GO-with-notes resolution) | `LapMeter::paint` draws the laps-done number in `TEXT_INK` (== `GRAPHITE_900`), invisible on the dark `GRAPHITE_900` HUD band. `LapMeter` gains a minimal `on_ink` builder mode mirroring `Telemetry::on_ink` (light on-ink readout color, e.g. `PAPER_0`) so it stays legible on the band. Minimal in-crate API extension only — no new widget crate/type. |
| Car render / HUD / standings input shape | Caller passes the per-frame car render data (`CarRender` slice or an equivalent per-car struct carrying `CarState` + color + name + kind) plus an active-car index; HUD, legal mask, and standings all derive from `cars[active]` / the full slice. Exact struct/API surface is the `design` Subagent's call (internal data shape). |
| Car names | Port the `CAR_NAMES` table `["You", "Rival Blue", "Rival Green", "Rival Amber", "Rival Plum", "Rival Teal"]` (`Screens.jsx:8`). Whether it is a module-level const (like `DIFFICULTY_LABELS`) or caller-supplied is the `design` Subagent's call. |
| CarChip kind/rank/active | Index `0` = player (`CarKind::You`, `active = true`); others `CarKind::Ai`. `rank = index + 1` (index-order placeholder, see Deferred). |
| Coast shortcut | The secondary full-width "Coast (·)" `Button` emits `Action::Coast` — the same emission channel as a MovePad Coast-cell click. |
| Finish button | Ghost "Finish →" `Button` emits a `finish` click signal in the response (no routing here). |
| Layer elevation | Install an `Order::Middle` child layer before drawing `Card` chrome, exactly as `SetupScreen::show`/`LabScreen::show` document (Card fill paints on `LayerId::background()`). |
| No RaceScreen reference PNG exists | "Render against RaceScreen reference" (issue) = the `Screens.jsx` layout + a newly-minted wgpu golden `race_screen.png` (mirrors `lab_screen.png`), not a pre-existing image. |

## Technical constraints

- New file `crates/render/src/screens/race.rs`; register + re-export in `screens/mod.rs` and `lib.rs`. Companion `screens/race_gallery.rs` (`#[cfg(test)] mod`) for the golden + interaction tests, mirroring `lab_gallery.rs`.
- Reuse existing widgets/APIs — no new widget crates and no new widget types; the one permitted API extension is a minimal in-crate `LapMeter::on_ink` builder method (see below). Widgets/APIs consumed: `Telemetry` (+ `on_ink`, `TelemetryTone::Accent`, `Size::Lg`), `LapMeter` (+ `on_ink`), `Switch`/`SwitchResponse`, `Button`/`ButtonVariant` (Secondary/Ghost), `Card`, `MovePad`/`MovePadResponse`, `CarChip`/`CarKind`, `crate::render_frame`, `crate::Overlays`, `crate::CarRender`, `gp_core::sim::{Action, CarState, legal_mask}`.
- **`LapMeter::on_ink` extension (permitted API extension).** `LapMeter::paint` renders the laps-done number in `color::TEXT_INK`, which is `GRAPHITE_900` (`crates/render/src/tokens/color.rs:132`; `crates/render/src/widgets/lap_meter.rs:106`) — identical to the dark HUD band color, so on the `GRAPHITE_900` HUD strip the laps-done readout renders band-color-on-band (invisible). `LapMeter` has no `on_ink` mode (unlike `Telemetry`). Add a minimal in-crate `LapMeter::on_ink(bool)` builder mirroring `Telemetry::on_ink` (`crates/render/src/widgets/telemetry.rs:126`) that renders the laps-done number in a light on-ink color (e.g. `PAPER_0` / `TEXT_ON_INK`) so it stays legible on the dark band. No new widget crate, no new widget type — a minimal API extension to an existing in-crate widget.
- Layout constants (paddings/gaps/column widths/heights) as module-level `const SCREAMING_SNAKE_CASE` with `Screens.jsx`-cited doc comments — no inline magic numbers (per `AGENTS.md` § Code Style; `lab.rs` precedent).
- Strict clippy clean (`-D warnings`); every public item documented (`///`); file within the 500/800 soft size budget (split helpers into private `draw_*` fns as `lab.rs` does).
- Miri gating: any test constructing an `egui::Context` / driving a painter / running the wgpu golden carries `#[cfg_attr(miri, ignore = "…")]` with an honest per-cause reason (`AGENTS.md` § Rust Test Conventions; `lab_gallery.rs` precedent).
- `f32` throughout the render layer is fine; **do not** introduce any non-integer arithmetic into `gp-core`.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The track canvas + dark HUD strip render live active-car state: `SPEED` = `|v|` (2 dp), `v` = `(vx, vy)`, `POS` = `(x, y)`, and an on-ink `LapMeter` shows laps-done / total — all bound to the active `CarState` / caller-supplied laps. The `LapMeter` uses its `on_ink` mode so the laps-done number renders in a light on-ink color (e.g. `PAPER_0`), visibly legible on the dark `GRAPHITE_900` HUD band. |
| AC2 | The Grid / Heatmap / Fastest-lap `Switch`es each flip exactly their own `Overlays` flag, and the emitted `Overlays` drives `render_frame`'s overlay layers (initial state `grid=true, heatmap=false, fastest_lap=false`). |
| AC3 | The MovePad shows the active car's legal mask (equal to `gp_core::sim::legal_mask(&track.corridor, active_state)`); selecting a legal cell emits that `Action`; illegal cells are inert. |
| AC4 | The "Coast (·)" shortcut `Button` emits `Action::Coast`. |
| AC5 | Standings lists exactly `m` `CarChip`s (one per car), index-0 = player (`You`, active), others `Ai`, `rank = index+1`, names from the ported `CAR_NAMES` table. |
| AC6 | `show` returns a response carrying: the selected `Action` (if any) this frame, the toggled `Overlays`, and the `finish` click signal — and calls no `gp-core` mutation/orchestration (`step`/`resolve_crash`/`resolve_collisions`/`LapCounter`). |
| AC7 | A wgpu golden test renders the full `RaceScreen` and matches a newly-minted `race_screen.png` exactly (flat-region exact compare; AA edges exempt via the text-bearing-screen `threshold`/`failed_pixel_count_threshold(0)` setting, per `lab_gallery.rs`). |
| AC8 | An `egui_kittest` interaction smoke test drives a scripted move selection (a MovePad cell and/or the Coast shortcut) and asserts the emitted `Action`; a rest frame emits no move. |
| AC9 | `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and the doc gate all pass; Miri stays green (abort-prone tests gated). |

## Open questions

- None design-blocking. The exact per-car input struct/API surface (a `CarRender` slice + active index vs a bespoke per-car struct carrying name/kind) and whether `CAR_NAMES` is a const vs caller-supplied are internal-data-shape choices left to the `design` Subagent; the Key decisions record the defaults to apply absent a reason to diverge.

```yaml
---
status: ready
round: 1
---
```
