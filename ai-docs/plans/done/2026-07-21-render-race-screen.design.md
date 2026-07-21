# Design: gp-render Race screen

**Issue:** #21
**Date:** 2026-07-21

## Approach

Port `Screens.jsx::RaceScreen` (`docs/design-system/ui_kits/game/Screens.jsx:73-148`
`[measured: Read Screens.jsx → RaceScreen lines 73-148]`) into a new
`crates/render/src/screens/race.rs`, following the exact draw-only /
caller-supplies-data / emits-per-frame-response contract the two sibling
screens already establish (`screens::setup::SetupScreen`,
`screens::lab::LabScreen`) `[measured: Read setup.rs, lab.rs → both hold data by
ref, install an Order::Middle layer, return a *Response struct]`. The screen
buffers no game state, owns no clock, and calls no `gp-core` mutation
(`step`/`resolve_crash`/`resolve_collisions`/`LapCounter`) — it draws one frame
and reports the player's choice (spec § Scope, AC6).

Structure mirrors `lab.rs` precisely: a `RaceScreen<'a>` builder holding
borrowed per-frame data + `Copy` scalars, a `show(self, ui) -> RaceResponse`
entry point that installs an `Order::Middle` child layer (so the two `Card`
fills — painted on `LayerId::background()` — render *behind* their contents,
`[measured: Read card.rs:250-256 → layer_painter(LayerId::background())]`), lays
out the two columns as explicit `Rect`s, and delegates to private
`draw_*` helpers. Pure, `egui`-free helpers carry the AC-testable logic
(HUD string formatting, legal-mask derivation, overlay assembly), matching the
`oracle_tile_strings` / `phase_badge` pure-helper idiom in `lab.rs`.

### Internal data-shape decisions (spec § Open questions delegates these)

**Per-car input = `&[CarRender<'a>]` slice + `active: usize`** (rejecting a
bespoke per-car struct). Rationale: `render_frame` already consumes exactly
`&[CarRender<'_>]` `[measured: Read lib.rs:66-73 → pub fn render_frame(...,
cars: &[CarRender<'_>], ...)]`, so the slice flows into the canvas with **zero
reconstruction**. Everything else the screen needs is derivable from the slice
+ index: HUD/mask read `cars[active].state` (a `CarState`); the standings
color reuses `CarRender::color()` `[measured: Read track/car.rs:80-84 → pub fn
color(&self)]`; name/kind/rank/active derive from the car's index. A bespoke
struct would duplicate `CarState`+color and force the caller to build a second
parallel array — rejected as redundant (YAGNI). `CarRender` carries no
name/kind, which is correct: those are screen-role concerns, not render-frame
data.

**`CAR_NAMES` = module-level `const [&str; 6]`** (rejecting caller-supplied).
Rationale: the JSX table is fixed (`Screens.jsx:8`) and the `CarRender` slice
carries no names, so there is nothing caller-specific to supply. A const
mirrors the established `DIFFICULTY_LABELS`/`PHASE_NAMES` precedent
`[measured: Read mod.rs:52, lab.rs:68-76 → both fixed module consts]`. Access is
**total** (`CAR_NAMES.get(i).copied().unwrap_or("Car")`) so a slice longer than
6 never panics — consistent with the crate's no-panic-on-bad-index posture
(`CarRender::color` falls back rather than panicking, track/car.rs:80-84).

**Active-car access is total.** `cars.get(active)` → `Option<&CarRender>`; a
`None` (empty slice or out-of-range index) falls back to
`CarState::default()` (all-zero) `[measured: Read sim/mod.rs:18 → derive(...,
Default)]`, giving a defined HUD (`0.00`, `(0, 0)`, `(0, 0)`) and an
`legal_mask` over the origin cell — never a panic. This keeps gp-render's
draw layer panic-free without an `.expect`.

**Laps in = discrete `laps_done: i32` + `total_laps: i32`** (rejecting passing
the whole `RaceConfig`). Rationale: the screen consumes only `LapMeter::new(lap,
total)` (both `i32`) `[measured: Read lap_meter.rs:49 → pub const fn new(lap:
i32, total: i32)]`; car count comes from `cars.len()`, and `RaceConfig`'s
`v_target`/`difficulty` are unused here. Passing the two scalars the screen
actually reads (YAGNI) avoids coupling the Race view to the full config type.

### The `LapMeter`-on-dark-band port gap (settled — owner-approved amendment)

The JSX HUD strip is a `GRAPHITE_900` band; all four readouts (3×`Telemetry` +
`LapMeter`) sit on it, and the JSX makes them legible by overriding
`--text-ink`/`--text-muted` to light values *per element* (`Screens.jsx:104-108`).
The Rust port added an `on_ink` mode to `Telemetry` only `[measured: Read
telemetry.rs:77,143-168 → on_ink field + resolve()]`; **`LapMeter` has no such
mode** `[measured: Read lap_meter.rs full → no on_ink field; paint() hard-codes
color::TEXT_MUTED (label, :95/:125), TEXT_INK (done, :106/:132), TEXT_FAINT
(total, :110/:139)]`. Because `TEXT_INK == GRAPHITE_900` `[measured: grep
color.rs:132 → 'pub const TEXT_INK: Color32 = GRAPHITE_900']`, the `LapMeter`
`done` readout would be band-color-on-band — **invisible** — so AC1's "LapMeter
shows laps-done / total" is not visually met by a pure reuse. This is a genuine
widget-capability gap, not a composition choice.

**Settled decision (was an Open Question in round 1; now owner-approved).** The
amended spec explicitly permits *and* requires the fix: § Key decisions row
*"LapMeter on-ink legibility (amendment, owner-approved during Step 7
GO-with-notes resolution)"* and § Technical constraints *"`LapMeter::on_ink`
extension (permitted API extension)"* both mandate a **minimal in-crate
`LapMeter::on_ink` builder** mirroring `Telemetry::on_ink`. AC1 now references
"an on-ink `LapMeter`" / "uses its `on_ink` mode". This is the *one* permitted
API extension — no new widget crate, no new widget type. Decomposed as subtask 1.

**Per design-review (GO-with-notes note incorporated): the on-ink color
selection is factored into a pure `const fn`**, exactly as `Telemetry::resolve`
does `[measured: Read telemetry.rs:143 → pub const fn resolve(tone, size, on_ink)
-> TelemetryStyle, selecting between const Color32 values]`, so it stays
Miri-clean and its unit test is **un-gated** — matching the crate's
pure-`resolve`-layer idiom. Selecting between the module-level `const Color32`
tokens in an `if on_ink { … } else { … }` is const-eligible (Telemetry's is
already `pub const fn`), so `missing_const_for_fn` (nursery = deny) is satisfied
by declaring it `const`. On-ink color trio (mirrors `Telemetry::resolve`'s
`value_color`/`muted_color` split): **done = `PAPER_0`** (light, legible on the
band), **label = `TEXT_FAINT`**, **total = `TEXT_FAINT`**; off-ink is the current
trio unchanged (done = `TEXT_INK`, label = `TEXT_MUTED`, total = `TEXT_FAINT`).
`[measured: grep color.rs → PAPER_0 :18, TEXT_FAINT :138, TEXT_INK :132]`

### Rejected alternatives

- **Persisting `MovePad.selected` across frames** — rejected: the screen is
  stateless per frame and emits the click for the caller to drive `step`; a
  persisted highlight would imply owned state (contradicts draw-only).
- **A gp-render-local `RaceCar` struct** — rejected as above (redundant with
  `CarRender`).
- **Testing AC2/AC3 only through the interaction harness** — rejected in favor
  of pure `egui`-free helpers (`overlays_from_switches`, `active_legal_mask`,
  `hud_readouts`) that give deterministic, Miri-clean unit tests, matching the
  crate's pure-`resolve`-layer idiom.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **(owner-approved amendment)** Add `on_ink` mode to `LapMeter` mirroring `Telemetry::on_ink`: add `on_ink: bool` field + `pub const fn on_ink(bool)` builder; factor the on-ink color selection into a **pure `const fn`** (mirroring `Telemetry::resolve`) returning the color trio — on-ink `done=PAPER_0`, `label=TEXT_FAINT`, `total=TEXT_FAINT`; off-ink `done=TEXT_INK`, `label=TEXT_MUTED`, `total=TEXT_FAINT` (current); thread the trio through `paint`; un-gated (context-free) unit tests for both modes | `crates/render/src/widgets/lap_meter.rs` | — |
| 2 | Module scaffolding: layout `const`s (JSX-cited), `CAR_NAMES` const + length guard, `RaceResponse` struct, `RaceScreen<'a>` builder (`new` + fields), module doc comment | `crates/render/src/screens/race.rs` | — |
| 3 | Pure helpers + unit tests: `hud_readouts(CarState) -> (String,String,String)` (AC1), `active_legal_mask(track, cars, active) -> BitFlags<Action>` (AC3), `overlays_from_switches(bool,bool,bool) -> Overlays` (AC2), standings name/kind derivation (AC5) | `crates/render/src/screens/race.rs` | 2 |
| 4 | `show()`: install `Order::Middle` layer, compute two-column + left-stack `Rect`s, call `draw_*`, assemble `RaceResponse` (action precedence: Coast button → MovePad change → none) | `crates/render/src/screens/race.rs` | 2, 3 |
| 5 | `draw_hud` (dark `GRAPHITE_900` band, radius-2; 3 on-ink `Telemetry` + right-aligned on-ink `LapMeter`), `draw_toolbar` (3 `Switch` + ghost "Finish →"), `draw_canvas` (1.5px border + `render_frame`) | `crates/render/src/screens/race.rs` | 4 |
| 6 | `draw_your_move` (`Card`: `MovePad` size 52 + mono caption + full-width secondary "Coast (·)" `Button`) + `draw_standings` (`Card`: one `CarChip` per car) | `crates/render/src/screens/race.rs` | 4 |
| 7 | Wire module: `pub mod race;` + `#[cfg(test)] mod race_gallery;` in `screens/mod.rs`; re-export `RaceScreen`/`RaceResponse` from `screens/mod.rs` and `lib.rs` | `crates/render/src/screens/mod.rs`, `crates/render/src/lib.rs` | 4 |
| 8 | `race_gallery.rs`: fixture track + `CarRender` cars, wgpu golden (Miri-gated) minting `race_screen.png` (AC7), `egui_kittest` interaction test (Miri-gated) driving Coast-shortcut + MovePad-center clicks and a rest frame (AC8) | `crates/render/src/screens/race_gallery.rs` | 5, 6, 7 |

M = 8. All subtasks are Rust `*.rs` (code change-type). `[measured: every Files
cell is a `.rs` path]`

## Handoff plan

Per `.claude/agents/design.md` § Rules (a)–(h) and `.claude/skills/task/SKILL.md`
Step 8: a `/context-reset` handoff binds at the start of **every** design-defined
group, including the first.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent, 1M-token window — subtasks **1–8**. All eight
  subtasks are the **code** change-type (Rust `*.rs` only), so they form one
  homogeneous group; the count (8) is `≤ 10`, so no size-cap split is needed
  and clustering into a single group is the minimization-optimal result
  (§ Rules (e)/(f)). **Terminal group** (8 subtasks; within `1..=10`). Entry
  into Group A spawns `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry); the
  single group completes Step 8 in its own `/context-reset` subagent. No
  inter-group handoff (only one group). Group count = 1 (≤ 4, no user gate).

## Risks

- **`LapMeter` invisible on the dark band (AC1)**: resolved by subtask 1 (add an
  `on_ink` mode — owner-approved spec amendment, no longer an open question).
  Root cause verified — `[measured: grep color.rs:132 → TEXT_INK = GRAPHITE_900;
  Read lap_meter.rs:106,132 → paint() draws the done readout in TEXT_INK]`. The
  on-ink `done` readout uses `PAPER_0` (light), legible on the `GRAPHITE_900`
  band; the color selection is factored into a pure `const fn` (mirrors
  `Telemetry::resolve`), so its unit test is Miri-clean and un-gated.
  `[derived → clippy -D warnings + un-gated resolve test + golden race_screen.png
  (AC1/AC7/AC9)]`
- **Golden non-determinism / Miri abort**: the wgpu golden + interaction tests
  drive a `Context`/painter and (golden) `dlopen` the Vulkan ICD, aborting
  under Miri; both carry `#[cfg_attr(miri, ignore = "...")]` with honest
  per-cause reasons (golden: "drives wgpu; dlopens the Vulkan ICD"; interaction:
  "Harness::builder() calls getcwd ... under Miri isolation"), copied verbatim
  from `lab_gallery.rs`'s two proven reasons. `[measured: Read lab_gallery.rs:112-115,
  165-170 → the two gate reasons]` `[derived → cargo miri test --workspace stays
  green (AC9)]`
- **`arithmetic_side_effects` / `cast_precision_loss` denies (nursery+pedantic =
  deny)** `[measured: rg Cargo.toml → pedantic/nursery deny, arithmetic_side_effects
  deny]`: SPEED = `f32::hypot(vx as f32, vy as f32)` needs a
  `#[allow(clippy::cast_precision_loss, reason=...)]` for the two small-int
  casts, precedent `track/car.rs:92-96`; layout arithmetic uses `mul_add`/field-wise
  `Pos2` construction as the sibling screens do to avoid the `Pos2 - Pos2`
  overload trap `[measured: Read track/car.rs:129-132 → field-wise Vec2 to dodge
  arithmetic_side_effects]`. `[derived → cargo clippy --workspace --all-targets
  -- -D warnings (AC9)]`
- **`missing_const_for_fn` (nursery = deny) on pure helpers**: `overlays_from_switches`
  is a pure struct-literal over `bool`s → const-eligible → the lint FORCES
  `pub const fn`; `hud_readouts` allocates `String`/calls `f32::hypot` (not
  const-stable) → stays a plain `fn`; `active_legal_mask` calls `legal_mask`
  (non-const) → plain `fn`. Each declared per what its body *calls*, not its
  shape. `[measured: Read sim/mod.rs:113 → pub fn legal_mask (non-const)]`
  `[derived → clippy -D warnings (AC9)]`
- **File-size budget (soft 500/800)**: `lab.rs` is 536 lines incl. tests
  `[measured: Read lab.rs → 536 lines]`; race.rs has a comparable helper set
  (HUD/toolbar/canvas/your-move/standings) — split into ≥5 private `draw_*` fns
  as `lab.rs` does. Golden/interaction live in the separate `race_gallery.rs`.
  `[derived → wc -l at implementation < 800 incl tests]`
- **`gp-core` integer-only invariant**: the only float is the SPEED
  magnitude, computed in gp-render's `f32` draw layer — no float touches
  `sim`/`geom` (spec § Technical constraints, `docs/design.md` §3a). `[measured:
  Read sim/mod.rs → CarState is i32-only; hypot lives in race.rs]`

## Test Design

**Subtask 1 — `LapMeter::on_ink` (lap_meter.rs `#[cfg(test)]`)**
- Entry point: the new **pure `const fn`** resolving the on-ink color trio
  (mirroring `Telemetry::resolve`) + the `on_ink` builder. The color resolver is
  the AC-testable seam — it takes `on_ink: bool`, returns the `(label, done,
  total)` `Color32` trio, has no `egui::Ui` and no allocation (const-eligible,
  like `Telemetry::resolve`).
- Scenarios: `new(...).on_ink(true)` sets the field; the resolver **off-ink**
  returns `(label = TEXT_MUTED, done = TEXT_INK, total = TEXT_FAINT)` (current
  colors); **on-ink** returns `(label = TEXT_FAINT, done = PAPER_0, total =
  TEXT_FAINT)` — exactly mirroring
  `telemetry.rs::resolve_on_ink_overrides_default_and_muted`. Pure `const fn`,
  context-free → **un-gated** (no `egui::Context`), Miri-clean.

**Subtask 3 — pure helpers (race.rs `#[cfg(test)]`)**
- `hud_readouts(CarState)` (AC1): `CarState { x, y, vx, vy }` → `("|v| 2dp",
  "(vx, vy)", "(x, y)")`. Cases: `(3,4)` velocity → speed `"5.00"`; zero
  velocity → `"0.00"`, `"(0, 0)"`; negative coords/velocity format with signs.
  Context-free → un-gated.
- `active_legal_mask(track, cars, active)` (AC3): assert it **equals**
  `gp_core::sim::legal_mask(&track.corridor, cars[active].state)` for a small
  hand-built corridor fixture; and that an out-of-range `active` (or empty
  slice) yields `legal_mask(&track.corridor, CarState::default())` — no panic.
  `BitFlags<Action>` is `PartialEq` `[measured: rg sim/mod.rs → BitFlags re-export
  + Action derives PartialEq]`. Context-free → un-gated.
- `overlays_from_switches(grid, heatmap, fastest)` (AC2): each `bool` maps to
  exactly its own `Overlays` field and no other (2×2×2 or a per-flag sweep);
  the initial `(true,false,false)` default matches JSX. Context-free → un-gated.
- Standings derivation (AC5): index 0 → `(CarKind::You, active=true, rank=1)`;
  index k>0 → `(CarKind::Ai, active=false, rank=k+1)`; `CAR_NAMES` length == 6
  and `CAR_NAMES[0] == "You"`; out-of-range name access returns the fallback,
  not a panic. Context-free → un-gated.

**Subtask 8 — golden + interaction (race_gallery.rs)** — mirrors
`lab_gallery.rs` exactly `[measured: Read lab_gallery.rs full → frame-1-install /
frame-2-draw dance, Rc<Cell> rect capture, SnapshotOptions threshold(1.0) +
failed_pixel_count_threshold(0)]`:
- Fixture: a hand-built `TrackArtifact` ring (reuse `lab_gallery::fixture_track`'s
  rounded-rect + metrics pattern) plus a 2–3-element `CarRender` array (player
  index 0 + rivals), a `CANVAS_SIZE` wide enough for `1fr + COL_GAP + 300px +
  padding`.
- Golden test (AC7): one wgpu frame renders the whole `RaceScreen`; compare to a
  newly-minted `race_screen.png` with `threshold(1.0)` +
  `failed_pixel_count_threshold(0)` (the text-bearing-screen setting). Assert the
  resolved adapter is a CPU device (copied guard). `#[cfg_attr(miri, ignore =
  "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)")]`. The mint is done
  by `code-writer`, which spawns `image-check` to verify the PNG against the
  drawing code (per `code-writer` charter) `[measured: Read design.md context →
  image-check spawned by code-writer at mint]`.
  **`image-check` expectation (on-ink `LapMeter` de-emphasis — NOT a defect):**
  the minted `race_screen.png` will show a *faint* `/total` (`TEXT_FAINT`) next
  to the *bright* laps-done number (`PAPER_0`). This is deliberate — the on-ink
  trio keeps `total = TEXT_FAINT` in both off-ink and on-ink modes (matching
  Telemetry's on-ink muted color); only the load-bearing `done` readout is
  promoted to the legible `PAPER_0`, which is what AC1 requires. The reviewer
  must **expect** the faint `/total` and not flag it as an illegible/missing
  readout.
- Interaction test (AC8): capture `coast_response.rect`, `finish_response.rect`,
  and `movepad_response.rect` via `Rc<Cell<Option<Rect>>>`; rest frame → `action
  == None`, `finish == false`; click the Coast button rect → `action ==
  Some(Action::Coast)`; click the MovePad rect center (the Coast cell of the
  plus) → `action == Some(Action::Coast)`. Uses the default (non-`render`)
  harness → the abort cause is `Harness::builder()`'s `getcwd`, so
  `#[cfg_attr(miri, ignore = "Harness::builder() calls getcwd ... under Miri
  isolation (no render() here ...)")]`. AC8's "MovePad cell and/or the Coast
  shortcut" is satisfied — the pad-center click exercises a real MovePad cell
  flowing through to `RaceResponse.action`.

## Open questions

- **None.** The round-1 open question — *"does adding `LapMeter::on_ink` exceed
  the spec's reuse-only constraint?"* — is **settled**: the spec was amended
  (owner-approved during the Step 7 design-review GO-with-notes resolution) to
  explicitly permit and require the minimal in-crate `LapMeter::on_ink`
  extension (§ Key decisions *"LapMeter on-ink legibility"*, § Technical
  constraints *"`LapMeter::on_ink` extension"*, AC1). It is now subtask 1, not
  an open question. See § Approach → *The `LapMeter`-on-dark-band port gap
  (settled — owner-approved amendment)*.
